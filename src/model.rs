//! The API's answers as types. Node properties stay a JSON map: their keys differ by kind and
//! grow with the engine, and the inspector lists them all.

use serde::Deserialize;
use serde_json::{Map, Value};

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct Session {
    pub id: String,
    pub created_at: Option<i64>,
    pub last_at: Option<i64>,
    pub nodes: i64,
    pub calls: i64,
    pub denied: i64,
    pub reviewed: i64,
    pub allowed: i64,
    pub failed: i64,
    pub prompt: Option<String>,
    pub intent: Option<String>,
    pub cwd: Option<String>,
    /// The client ID the hooks sent (`x-withhuman-client`).
    pub client: Option<String>,
    /// Calls allowed though hard to undo: what the user asked for can still need an audit.
    #[cfg_attr(not(feature = "admin"), allow(dead_code))]
    pub risky: i64,
    /// Calls with no decision: the engine only watched, its decision failed, or the hook was
    /// gone around.
    #[cfg_attr(not(feature = "admin"), allow(dead_code))]
    pub undecided: i64,
    /// Calls a tool result made the agent take, rather than the user's request.
    #[cfg_attr(not(feature = "admin"), allow(dead_code))]
    pub from_result: i64,
    /// How the engine enforced the session's last decision: `record`, `shadow` or `enforce`.
    #[cfg_attr(not(feature = "admin"), allow(dead_code))]
    pub mode: Option<String>,
    /// Each intent the agent stated, in order, with the Calls it made under it; none when the
    /// store says null (a session that stated none, as an older viewer kept it).
    #[serde(deserialize_with = "null_as_empty")]
    pub intents: Vec<(Option<String>, i64)>,
}

/// One Call as the security team's charts count it (`graph::CALL_FACTS`).
#[cfg_attr(not(feature = "admin"), allow(dead_code))]
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct CallFact {
    pub session: String,
    /// The Call's node number in its session, to open it on the route map.
    pub seq: i64,
    /// What it acted on: the command, file, URL or pattern, cut short.
    pub target: Option<String>,
    /// Why the engine decided as it did.
    pub reason: Option<String>,
    /// The person's prompt the Call's turn answered, cut short.
    pub prompt: Option<String>,
    /// The tool ran: a result came back. A Call held for review ran only if the person allowed it.
    pub ran: bool,
    /// When its first result came back (ms).
    pub ran_at: Option<i64>,
    /// The machine: the client ID the hooks sent.
    pub client: Option<String>,
    pub cwd: Option<String>,
    /// When it was decided, else when it was seen (ms).
    pub at: Option<i64>,
    /// `ALLOW`, `REVIEW`, `DENY`, or none when nothing decided it.
    pub decision: Option<String>,
    /// `record`, `shadow` or `enforce`.
    pub mode: Option<String>,
    /// The rule that decided it, such as `intent::mismatch`.
    pub rule: Option<String>,
    pub tool: Option<String>,
    /// What made the agent take it: `request` (the user), `segment` (a tool result), `none`.
    pub link: Option<String>,
    /// The worst thing it broke, such as `exfiltration`; `none` for nothing.
    pub worst: Option<String>,
    /// Hard to undo ([`crate::metro::IRREVERSIBLE`] or more).
    pub irreversible: bool,
    /// What the Call whose result told this one to act was on (a file, a URL, a command, else
    /// its tool), for a Call worth a look that a tool result made; none where the store does
    /// not tell.
    pub cause: Option<String>,
}

impl Session {
    /// The last part of the working folder, whether it uses / or \.
    pub fn folder(&self) -> String {
        self.cwd
            .as_deref()
            .unwrap_or("")
            .split(['/', '\\'])
            .rfind(|s| !s.is_empty())
            .unwrap_or("")
            .to_owned()
    }

    /// The session's name: the intent it did the most under (the first, of those level), or,
    /// with none stated, its first prompt.
    pub fn title(&self) -> &str {
        self.main_intent()
            .or(self.prompt.as_deref())
            .or(self.intent.as_deref())
            .unwrap_or("")
    }

    /// The intents stated, each once in the order first stated, with all the Calls made under it.
    fn stated(&self) -> Vec<(&str, i64)> {
        let mut out: Vec<(&str, i64)> = Vec::new();
        for (intent, calls) in &self.intents {
            let Some(i) = intent
                .as_deref()
                .map(short_intent)
                .filter(|i| !i.is_empty())
            else {
                continue;
            };
            match out.iter_mut().find(|(t, _)| *t == i) {
                Some((_, n)) => *n += calls,
                None => out.push((i, *calls)),
            }
        }
        out
    }

    fn main_intent(&self) -> Option<&str> {
        let stated = self.stated();
        let most = stated.iter().map(|(_, n)| *n).max()?;
        stated.into_iter().find(|(_, n)| *n == most).map(|(i, _)| i)
    }

    /// The other intents the session stated beside the one it is named by, in order.
    #[cfg_attr(not(feature = "admin"), allow(dead_code))]
    pub fn other_intents(&self) -> Vec<&str> {
        let main = self.main_intent();
        self.stated()
            .into_iter()
            .map(|(i, _)| i)
            .filter(|i| Some(*i) != main)
            .collect()
    }

    pub fn flagged(&self) -> bool {
        self.denied + self.reviewed > 0
    }
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct GraphData {
    pub nodes: Vec<Node>,
    pub subgraphs: Vec<SubGraph>,
    pub edges: Vec<Edge>,
    /// The values indexed in the session (`graph::TOKENS`); none in older exports.
    pub tokens: Vec<TokenRow>,
}

/// One mention of an indexed value: the node, the value's key and kind, who wrote it there
/// (`user`, `agent`, `external`), and its text (masked when sensitive).
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct TokenRow {
    pub seq: i64,
    pub key: String,
    pub kind: String,
    pub slot: String,
    pub category: String,
    pub text: String,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct Node {
    pub kind: String,
    pub props: Map<String, Value>,
    pub summary: Summary,
}

/// The fields a node's caption needs, read by the API before its payload is cut short.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct Summary {
    pub tool_name: Option<String>,
    pub brief: Option<String>,
    pub prompt: Option<String>,
    pub intent: Option<String>,
    pub text: Option<String>,
    pub error: Option<String>,
    /// What the tool gave back, its start.
    pub output: Option<String>,
    pub file_path: Option<String>,
}

impl Node {
    pub fn seq(&self) -> i64 {
        self.props.get("seq").and_then(Value::as_i64).unwrap_or(-1)
    }

    pub fn decision(&self) -> Option<&str> {
        self.props.get("decision").and_then(Value::as_str)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct SubGraph {
    pub props: Map<String, Value>,
    pub members: Vec<Option<i64>>,
}

impl SubGraph {
    pub fn number(&self) -> Option<i64> {
        self.props.get("number").and_then(Value::as_i64)
    }

    pub fn parent(&self) -> Option<&str> {
        self.props.get("parent").and_then(Value::as_str)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct Edge {
    pub source: Option<i64>,
    pub target: Option<i64>,
    #[serde(rename = "type")]
    pub kind: String,
    pub props: Map<String, Value>,
    pub paragraph: Option<i64>,
}

impl Edge {
    pub fn is_cause(&self) -> bool {
        self.props.get("kind").and_then(Value::as_str) == Some("cause")
    }

    pub fn probability(&self) -> Option<f64> {
        self.props.get("probability").and_then(Value::as_f64)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct NodeDetail {
    pub kind: String,
    pub props: Map<String, Value>,
    pub paragraphs: Vec<Paragraph>,
    pub tokens: Vec<Token>,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct Paragraph {
    pub index: Option<i64>,
    pub hidden: Option<bool>,
    pub text: Option<String>,
    /// How likely the paragraph instructs the agent: the highest sentence probability the
    /// engine's paragraph classification stored. Older fixtures kept a bool.
    #[serde(deserialize_with = "probability")]
    pub instructs: Option<f64>,
    /// Where the paragraph came from, such as the tool result it was cut from.
    pub source: Option<String>,
    /// The rules that hid it, when `hidden`.
    pub hidden_rules: Value,
}

impl Paragraph {
    /// At or above this, the paragraph is shown as giving instructions.
    pub const INSTRUCTS: f64 = 0.5;

    pub fn instructs(&self) -> bool {
        self.instructs.is_some_and(|p| p >= Self::INSTRUCTS)
    }
}

/// A probability, also read from a bool (`true` is 1) or a missing value.
fn probability<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Option<f64>, D::Error> {
    Ok(match Option::<Value>::deserialize(deserializer)? {
        Some(Value::Bool(b)) => Some(if b { 1.0 } else { 0.0 }),
        Some(Value::Number(n)) => n.as_f64(),
        _ => None,
    })
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct Token {
    pub kind: Option<String>,
    pub category: Option<String>,
    pub slot: Option<String>,
    pub decoded: Value,
}

/// One mention of a sensitive value (a credential, personal, financial or device identifier)
/// in a node: who wrote it there (`user` a prompt, `external` a tool result, `agent` a Call's
/// input or the agent's words), the value masked, and for a Call how it was decided.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
#[cfg_attr(not(feature = "admin"), allow(dead_code))]
pub struct SecretMention {
    pub session: String,
    pub seq: i64,
    /// The node's kind: `Call`, `Request`, `Segment`, ...
    pub node: Option<String>,
    pub slot: Option<String>,
    /// The value's key: the SHA-256 of it, so equal values share one.
    pub key: String,
    /// `api_key`, `jwt`, `kr_rrn`, `card_pan`, ...
    pub kind: Option<String>,
    /// `credential`, `pii`, `financial` or `device`.
    pub category: Option<String>,
    /// The value, masked.
    pub text: Option<String>,
    pub decision: Option<String>,
    /// How the engine enforced its decision: `enforce`, `shadow` or `record`.
    pub mode: Option<String>,
    pub client: Option<String>,
    pub at: Option<i64>,
    pub tool: Option<String>,
    /// What the Call acted on (masked as stored).
    pub target: Option<String>,
    /// A result came back.
    /// Whom or where a Call names to send to (a recipient, a channel, an account), when its
    /// arguments say.
    pub recipient: Option<String>,
    pub ran: bool,
    /// For a person's request, what they typed (the engine keeps sensitive values masked in it).
    pub said: Option<String>,
    /// For a tool result, the tool and target of the Call it answered.
    pub by_tool: Option<String>,
    pub by_target: Option<String>,
}

/// The arguments a tool names whom it sends to by, the first found winning.
pub const RECIPIENT_KEYS: [&str; 8] = [
    "recipient",
    "to",
    "email",
    "channel",
    "channel_name",
    "channel_id",
    "account",
    "iban",
];

/// The arguments a tool names where it sends to by, after a repository (`owner/repo`).
pub const PLACE_KEYS: [&str; 26] = [
    "full_name",
    "project",
    "project_slug",
    "project_id",
    "organization_slug",
    "page_id",
    "parent_id",
    "database_id",
    "space",
    "space_key",
    "workspace",
    "team",
    "team_id",
    "board",
    "list_id",
    "spreadsheet_id",
    "document_id",
    "file_id",
    "folder_id",
    "bucket",
    "url",
    "webhook_url",
    "endpoint",
    "domain",
    "phone",
    "username",
];

/// Whom or where a tool's arguments name to send to, as [`RECIPIENT_KEYS`] and [`PLACE_KEYS`]
/// find it in the store.
pub fn recipient_of(input: &Value) -> Option<String> {
    let text = |k: &str| {
        input[k]
            .as_str()
            .filter(|t| !t.trim().is_empty())
            .map(str::to_owned)
    };
    let first = |k: &str| {
        input[k]
            .as_array()
            .and_then(|a| a.first())
            .and_then(Value::as_str)
            .map(str::to_owned)
    };
    RECIPIENT_KEYS
        .iter()
        .find_map(|k| text(k))
        .or_else(|| first("recipients").or_else(|| first("to")))
        .or_else(|| {
            let repo = text("repo").or_else(|| text("repository"))?;
            Some(match text("owner") {
                Some(owner) => format!("{owner}/{repo}"),
                None => repo,
            })
        })
        .or_else(|| PLACE_KEYS.iter().find_map(|k| text(k)))
}

/// A JSON document stored as a string property; anything unreadable reads as nothing.
pub fn parse_json(value: Option<&Value>) -> Value {
    value
        .and_then(Value::as_str)
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or(Value::Null)
}

/// A Call as the comparison reads it: its verdict and score, without payload or trace.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct CallRow {
    pub seq: i64,
    pub tool_name: Option<String>,
    pub brief: Option<String>,
    pub decision: Option<String>,
    pub error: Option<String>,
    pub rule: Option<String>,
    pub score: Option<f64>,
    /// The call carries the benchmark's attack (its marker line or command).
    pub attack: bool,
    /// The tool ran: a result or an error came back.
    pub ran: bool,
}

/// A list the store may hand over as null.
fn null_as_empty<'de, D, T>(d: D) -> Result<Vec<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Ok(Option::<Vec<T>>::deserialize(d)?.unwrap_or_default())
}

/// An intent as a name: its first sentence, without the agent's `(확신도: …)`.
pub fn short_intent(intent: &str) -> &str {
    let mut t = intent.trim();
    if let Some(i) = t.find("(확신도") {
        t = t[..i].trim_end();
    }
    if let Some(i) = t.find(". ") {
        t = &t[..i];
    }
    t.trim_end_matches(['.', ' '])
}

#[cfg(test)]
mod tests {

    // Whom first, then where: a repository as owner/repo, then the other places tools name.
    #[test]
    fn a_tools_arguments_name_where_it_sends() {
        let of = |v: serde_json::Value| super::recipient_of(&v);
        assert_eq!(
            of(serde_json::json!({"to": "a@x.com", "repo": "r"})).as_deref(),
            Some("a@x.com")
        );
        assert_eq!(
            of(serde_json::json!({"owner": "acme", "repo": "api", "title": "t"})).as_deref(),
            Some("acme/api")
        );
        assert_eq!(
            of(serde_json::json!({"project_slug": "web", "body": "b"})).as_deref(),
            Some("web")
        );
        assert_eq!(of(serde_json::json!({"text": "hi"})), None);
    }

    #[test]
    fn a_session_is_named_by_the_intent_it_did_the_most_under() {
        let s = super::Session {
            prompt: Some("이거 봐줘".into()),
            intents: vec![
                (Some("폴더 구조 파악 (확신도: 높음)".into()), 2),
                (Some("배포 스크립트 실행. 스테이징만.".into()), 5),
                (None, 3),
                (Some("폴더 구조 파악".into()), 3),
                (Some("변경 기록 (확신도: 중간)".into()), 0),
            ],
            ..super::Session::default()
        };
        assert_eq!(s.title(), "폴더 구조 파악");
        assert_eq!(s.other_intents(), ["배포 스크립트 실행", "변경 기록"]);
        let none = super::Session {
            prompt: Some("이거 봐줘".into()),
            ..super::Session::default()
        };
        assert_eq!(none.title(), "이거 봐줘");
        assert!(none.other_intents().is_empty());
        // A session that stated no intent comes with them null.
        let read: super::Session = serde_json::from_str(r#"{"id": "s", "intents": null}"#).unwrap();
        assert!(read.intents.is_empty());
    }
    use super::Paragraph;

    // The engine stores the classification as a probability; older fixtures as a bool.
    #[test]
    fn instructs_reads_a_probability_or_a_bool() {
        let p: Paragraph = serde_json::from_str(r#"{"index":1,"instructs":0.93}"#).unwrap();
        assert_eq!(p.instructs, Some(0.93));
        assert!(p.instructs());
        let p: Paragraph = serde_json::from_str(r#"{"instructs":0.2}"#).unwrap();
        assert!(!p.instructs());
        let p: Paragraph = serde_json::from_str(r#"{"instructs":true}"#).unwrap();
        assert!(p.instructs());
        let p: Paragraph = serde_json::from_str(r#"{"instructs":null,"source":"Read"}"#).unwrap();
        assert_eq!(p.instructs, None);
        assert_eq!(p.source.as_deref(), Some("Read"));
    }
}

/// What the engine knows of the machines, people and sessions it protects (withhuman-core's agent
/// inventory): its tables as they are, or the demo's.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
#[cfg_attr(not(feature = "admin"), allow(dead_code))]
pub struct Inventory {
    pub clients: Vec<Device>,
    pub projects: Vec<DeviceProject>,
    pub users: Vec<Person>,
    pub keys: Vec<ApiKey>,
    pub events: Vec<KeyEvent>,
    pub sessions: Vec<SessionRun>,
    /// Agents found running without the hooks; none when the engine cannot tell yet.
    pub shadow: Option<Vec<ShadowAgent>>,
    /// The machines the EDR lists; none while the engine is not connected to one.
    pub edr: Option<Vec<EdrDevice>>,
    /// Each EDR scan that saw a coding agent running; none while the engine keeps no such record.
    pub sightings: Option<Vec<EdrSighting>>,
}

/// An EDR scan that saw a coding agent running on a machine.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
#[cfg_attr(not(feature = "admin"), allow(dead_code))]
pub struct EdrSighting {
    /// The EDR's ID of the machine (`edr_devices.agent_id`).
    pub agent_id: String,
    /// The scan's time (ms).
    pub running_at: i64,
    /// `claude-code` or `codex`.
    pub agent: String,
    /// The machine's name then.
    pub name: Option<String>,
    /// The command line the agent ran with, where the EDR tells it (Linux); none elsewhere.
    pub command: Option<String>,
}

/// An agent running where the hooks are not: a Claude Code or Codex project a machine's check
/// found with no hooks in it. Other agents leave nothing the check can see.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
#[cfg_attr(not(feature = "admin"), allow(dead_code))]
pub struct ShadowAgent {
    pub client_id: Option<String>,
    pub user_email: Option<String>,
    /// `claude-code` or `codex`.
    pub agent: String,
    pub project: Option<String>,
    pub last_seen: Option<i64>,
}

/// A machine with the hooks: its client ID, what it said when it registered, when its hooks were
/// first and last heard.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
#[cfg_attr(not(feature = "admin"), allow(dead_code))]
pub struct Device {
    pub client_id: String,
    pub team: Option<String>,
    pub hostname: Option<String>,
    pub os: Option<String>,
    pub os_user: Option<String>,
    pub hook_version: Option<String>,
    pub agent: Option<String>,
    pub user_email: Option<String>,
    /// It registered at install; none when only its hooks were heard.
    pub registered_at: Option<i64>,
    pub updated_at: Option<i64>,
    pub first_seen: Option<i64>,
    pub last_seen: Option<i64>,
    /// The machine's hardware serial number, when its setup told it.
    pub serial: Option<String>,
    /// The person the security team named as the machine's owner; `user_email` is whoever's key
    /// it last presented, which may be someone else.
    pub owner_email: Option<String>,
    /// The EDR's agent on this machine, when the engine matched one, and that agent's status
    /// (`active`, `disconnected`, `never_connected` or `pending`).
    pub edr_agent_id: Option<String>,
    pub edr_status: Option<String>,
    /// The security team stopped the machine: when, who, why, and how many of its Calls were
    /// refused since.
    pub stopped_at: Option<i64>,
    pub stopped_by: Option<String>,
    pub stop_reason: Option<String>,
    pub stop_denied_count: Option<i64>,
}

/// A machine as the company's EDR (Wazuh) lists it, whether or not the engine's hooks are on it.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
#[cfg_attr(not(feature = "admin"), allow(dead_code))]
pub struct EdrDevice {
    pub agent_id: String,
    pub name: String,
    pub os: Option<String>,
    pub status: Option<String>,
    pub serial: Option<String>,
    pub last_seen: Option<i64>,
    /// An agent program (Claude Code, Codex) the EDR saw on it, if any.
    pub agent: Option<String>,
    /// When the EDR's latest process scan saw that agent running (ms); none when it was not
    /// running at the scan, or the engine does not record it yet.
    pub running_at: Option<i64>,
}

/// A project a machine's hooks were installed in.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
#[cfg_attr(not(feature = "admin"), allow(dead_code))]
pub struct DeviceProject {
    pub client_id: String,
    pub project: String,
    pub agent: Option<String>,
    pub hook_version: Option<String>,
    pub installed_at: Option<i64>,
    pub updated_at: Option<i64>,
}

/// A person keys were issued to.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
#[cfg_attr(not(feature = "admin"), allow(dead_code))]
pub struct Person {
    pub email: String,
    pub created_at: Option<i64>,
    /// `person` or `service`, when the engine tells.
    pub kind: Option<String>,
}

/// A person's key, without its hash: who issued it, when it was last used, whether and why it
/// was revoked, and the Calls refused with it since.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
#[cfg_attr(not(feature = "admin"), allow(dead_code))]
pub struct ApiKey {
    pub key_id: String,
    pub user_email: String,
    pub created_at: Option<i64>,
    pub created_by: Option<String>,
    pub note: Option<String>,
    pub last_used_at: Option<i64>,
    pub revoked_at: Option<i64>,
    pub revoked_by: Option<String>,
    pub revoke_reason: Option<String>,
    pub first_denied_at: Option<i64>,
    pub last_denied_at: Option<i64>,
    pub denied_count: Option<i64>,
}

/// A key issued, revoked, or a Call refused for a revoked key: the audit's record of who stopped
/// whom.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
#[cfg_attr(not(feature = "admin"), allow(dead_code))]
pub struct KeyEvent {
    /// `issued`, `revoked` or `denied`.
    pub kind: String,
    pub actor: Option<String>,
    pub reason: Option<String>,
    pub user_email: Option<String>,
    pub key_id: Option<String>,
    pub session_id: Option<String>,
    pub client_id: Option<String>,
    pub at: Option<i64>,
}

/// A session as the inventory sees it: which machine and person, how it started and ended.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
#[cfg_attr(not(feature = "admin"), allow(dead_code))]
pub struct SessionRun {
    pub session_id: String,
    pub client_id: Option<String>,
    pub user_email: Option<String>,
    /// `startup`, `resume`, `clear`, `compact` or `fork`.
    pub start_source: Option<String>,
    pub permission_mode: Option<String>,
    pub model: Option<String>,
    pub created_at: Option<i64>,
    pub last_seen_at: Option<i64>,
    pub ended_at: Option<i64>,
    /// `clear`, `resume`, `logout`, `prompt_input_exit` or `other`.
    pub end_reason: Option<String>,
}

/// The MCP servers the machines have set up, with their tools and the tools' calls. The engine
/// does not collect this yet; only the demo has it.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
#[cfg_attr(not(feature = "admin"), allow(dead_code))]
pub struct McpCatalog {
    pub servers: Vec<McpServer>,
}

/// A server: reached at a URL (`remote`) or run on the machine (`local`).
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
#[cfg_attr(not(feature = "admin"), allow(dead_code))]
pub struct McpServer {
    pub name: String,
    pub transport: String,
    /// The URL, or the command.
    pub endpoint: String,
    /// The machines it is set up on.
    pub installs: Vec<McpInstall>,
    pub tools: Vec<McpTool>,
}

/// A server set up on a person's machine, and since when.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
#[cfg_attr(not(feature = "admin"), allow(dead_code))]
pub struct McpInstall {
    pub user: String,
    pub client: String,
    pub at: i64,
}

/// A tool a server offers.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
#[cfg_attr(not(feature = "admin"), allow(dead_code))]
pub struct McpTool {
    pub name: String,
    /// What the server tells the agent the tool does.
    pub description: String,
    /// What it does, in the engine's groups: `read`, `change`, `delete`, `send`, `transact`,
    /// `execute`, `access`, `delegate`, or `unknown`.
    pub nature: String,
    pub calls: Vec<McpUse>,
}

/// A call of a tool, and what the engine decided for it.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
#[cfg_attr(not(feature = "admin"), allow(dead_code))]
pub struct McpUse {
    pub at: i64,
    pub user: String,
    pub client: String,
    /// What it was called on, cut short.
    pub target: String,
    /// `ALLOW`, `REVIEW` or `DENY`, as for any Call.
    pub decision: String,
    pub ran: bool,
}
