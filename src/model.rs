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

    pub fn title(&self) -> &str {
        self.prompt
            .as_deref()
            .or(self.intent.as_deref())
            .unwrap_or("")
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
    pub client: Option<String>,
    pub at: Option<i64>,
    pub tool: Option<String>,
    /// What the Call acted on (masked as stored).
    pub target: Option<String>,
    /// A result came back.
    pub ran: bool,
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

#[cfg(test)]
mod tests {
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

/// The MCP servers and tools the gateway lets the agents use, as its allowlist keeps them, with
/// the requests developers sent to have more let through, and the calls the gateway refused.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
#[cfg_attr(not(feature = "admin"), allow(dead_code))]
pub struct McpCatalog {
    /// The allowlist's version, and when it last changed.
    pub version: String,
    pub updated_at: Option<i64>,
    pub servers: Vec<McpServer>,
    pub requests: Vec<McpRequest>,
    pub refused: Vec<McpRefusal>,
}

/// An approved server: reached at a URL (`remote`) or run by the gateway (`local`).
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
#[cfg_attr(not(feature = "admin"), allow(dead_code))]
pub struct McpServer {
    pub name: String,
    pub transport: String,
    /// The URL, or the command.
    pub endpoint: String,
    pub approved_by: Option<String>,
    pub approved_at: Option<i64>,
    /// The request it was approved on, if one.
    pub request: Option<String>,
    pub tools: Vec<McpTool>,
}

/// A tool of an approved server, and how far it is let through.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
#[cfg_attr(not(feature = "admin"), allow(dead_code))]
pub struct McpTool {
    pub name: String,
    /// What the server tells the agent the tool does.
    pub description: String,
    /// `read`, `write`, `delete` or `send` (out of the company).
    pub nature: String,
    /// `allow`, `review` (a person confirms each call) or `deny`.
    pub level: String,
    /// The teams or people it is for; everyone when empty.
    pub scope: Vec<String>,
    pub approved_by: Option<String>,
    pub approved_at: Option<i64>,
    pub calls: Vec<McpUse>,
}

/// A call of a tool that went through the gateway.
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

/// A developer's request, sent from their machine, to have a server or a tool let through, or a
/// tool opened to them, and why; and what the security team answered.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
#[cfg_attr(not(feature = "admin"), allow(dead_code))]
pub struct McpRequest {
    pub id: String,
    pub at: i64,
    pub user: String,
    pub client: String,
    /// The project it was asked from.
    pub cwd: Option<String>,
    /// `server` (a new server), `tool` (a new tool of an approved server) or `scope` (an
    /// approved tool opened to the person).
    pub kind: String,
    pub server: String,
    /// A new server's URL or command, and `remote` or `local`.
    pub endpoint: Option<String>,
    pub transport: Option<String>,
    /// The tools the server offers, as the gateway listed them when the request came.
    pub offered: Vec<McpOffered>,
    /// The tools asked for.
    pub wanted: Vec<String>,
    /// Why, in the developer's words.
    pub message: String,
    /// The calls refused for this before it was asked.
    pub attempts: i64,
    /// `pending`, `approved` or `rejected`.
    pub status: String,
    pub decided_by: Option<String>,
    pub decided_at: Option<i64>,
    /// The security team's answer to the developer.
    pub answer: Option<String>,
}

/// A tool a server offers, before it is approved.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
#[cfg_attr(not(feature = "admin"), allow(dead_code))]
pub struct McpOffered {
    pub name: String,
    pub description: String,
    pub nature: String,
}

/// Calls the gateway refused, the same person, server, tool and reason counted together.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
#[cfg_attr(not(feature = "admin"), allow(dead_code))]
pub struct McpRefusal {
    pub first_at: i64,
    pub last_at: i64,
    pub count: i64,
    pub user: String,
    pub client: String,
    pub server: String,
    pub tool: Option<String>,
    /// `unapproved` (not on the allowlist), `pending` (asked for, not yet decided), `scope`
    /// (approved, not for this person) or `deny` (a tool set to deny).
    pub reason: String,
    /// The request made for it, if one.
    pub request: Option<String>,
}

/// A change the MCP page makes to the allowlist.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(not(feature = "admin"), allow(dead_code))]
pub enum McpChange {
    /// How far a tool is let through: `allow`, `review` or `deny`.
    Level { server: String, tool: String, level: String },
    /// A request approved, with the level for each tool offered and whom they are for, or
    /// rejected; with the answer to the developer.
    Decide { id: String, approve: bool, actor: String, answer: String, levels: Vec<(String, String)>, scope: Vec<String> },
}
