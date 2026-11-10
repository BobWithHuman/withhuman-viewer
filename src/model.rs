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
