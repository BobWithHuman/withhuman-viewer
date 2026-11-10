//! The demo graph: the API's answers stored whole in `mock/fixture.json`, so the app can be
//! tried, and its screens worked on, without Neo4j. The fixture ships inside the app;
//! `VIEWER_FIXTURE` points at another one, such as a graph the engine's tests exported.

use serde_json::{Map, Value, json};

use crate::graph::{ApiError, check_idle};
use crate::metro::IRREVERSIBLE;
use crate::model::parse_json;

/// What the sessions query counts from a session's Calls (`graph::SESSIONS`), from its graph:
/// allowed Calls hard to undo, Calls with no decision, Calls a tool result made, and the mode
/// of the last decision.
fn attention(graph: &Value) -> Value {
    let calls: Vec<&Map<String, Value>> = graph["nodes"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|n| n["kind"] == "Call")
        .filter_map(|n| n["props"].as_object())
        .collect();
    let trace = |c: &Map<String, Value>| parse_json(c.get("trace_json"));
    let risky = calls
        .iter()
        .filter(|c| c.get("decision").and_then(Value::as_str) == Some("ALLOW"))
        .filter(|c| trace(c)["deviation"]["irreversible"].as_f64().is_some_and(|p| p >= IRREVERSIBLE))
        .count();
    let undecided = calls.iter().filter(|c| c.get("decision").is_none_or(Value::is_null)).count();
    let from_result = calls.iter().filter(|c| trace(c)["link"] == "segment").count();
    let mode = calls
        .iter()
        .filter(|c| c.get("decision_mode").is_some_and(Value::is_string))
        .max_by_key(|c| c.get("decided_at").and_then(Value::as_i64))
        .and_then(|c| c.get("decision_mode").cloned());
    json!({"risky": risky, "undecided": undecided, "from_result": from_result, "mode": mode})
}

static FIXTURE: &str = include_str!("../mock/fixture.json");

pub struct Demo {
    data: Value,
}

impl Demo {
    pub fn load() -> Self {
        let text = match std::env::var("VIEWER_FIXTURE") {
            Ok(path) => std::fs::read_to_string(&path).unwrap_or_else(|e| {
                eprintln!("데모 데이터를 읽지 못했습니다 ({path}): {e}");
                FIXTURE.to_owned()
            }),
            Err(_) => FIXTURE.to_owned(),
        };
        Self::parse(&text)
    }

    /// A demo from fixture text; an unreadable one is an empty demo.
    pub fn parse(text: &str) -> Self {
        let data = serde_json::from_str(text).unwrap_or_else(|e| {
            eprintln!("데모 데이터 형식이 잘못됐습니다: {e}");
            json!({"sessions": [], "graphs": {}, "nodes": {}, "calls": {}})
        });
        Self { data }
    }

    /// The stored sessions, each with the counts the sessions query adds since the fixture was
    /// made, worked out from its graph.
    pub fn sessions(&self) -> Value {
        let mut list = self.data["sessions"].clone();
        if let Value::Array(sessions) = &mut list {
            for session in sessions {
                let Some(id) = session["id"].as_str().map(str::to_owned) else {
                    continue;
                };
                if let (Value::Object(row), Value::Object(counts)) =
                    (session, attention(&self.data["graphs"][&id]))
                {
                    for (key, value) in counts {
                        row.entry(key).or_insert(value);
                    }
                }
            }
        }
        list
    }

    /// Every Call as the sessions' graphs hold it, cut down as `graph::CALL_FACTS` cuts it.
    #[cfg_attr(not(feature = "admin"), allow(dead_code))]
    pub fn call_facts(&self) -> Value {
        let mut facts = Vec::new();
        let Some(graphs) = self.data["graphs"].as_object() else {
            return Value::from(facts);
        };
        for (session, graph) in graphs {
            let nodes = graph["nodes"].as_array().into_iter().flatten();
            // When each node was seen, and the first result each Call got back.
            let seen: std::collections::HashMap<i64, Option<i64>> = nodes
                .clone()
                .filter_map(|n| {
                    let p = &n["props"];
                    Some((p["seq"].as_i64()?, p["observed_at"].as_i64()))
                })
                .collect();
            let mut ran: std::collections::HashMap<i64, Option<i64>> = std::collections::HashMap::new();
            for e in graph["edges"].as_array().into_iter().flatten() {
                if e["type"] != "RESULT_OF" {
                    continue;
                }
                let (Some(result), Some(call)) = (e["source"].as_i64(), e["target"].as_i64()) else {
                    continue;
                };
                let at = seen.get(&result).copied().flatten();
                let first = ran.entry(call).or_insert(at);
                if at.is_some_and(|a| first.is_none_or(|f| a < f)) {
                    *first = at;
                }
            }
            // In the order made, so each Call takes the prompt of the last Request before it.
            let mut ordered: Vec<&Value> = nodes.collect();
            ordered.sort_by_key(|n| n["props"]["order"].as_i64());
            let mut prompt: Option<String> = None;
            for node in ordered {
                if node["kind"] == "Request" {
                    prompt = parse_json(node["props"].get("payload_json"))["prompt"]
                        .as_str()
                        .map(|p| p.chars().take(200).collect());
                    continue;
                }
                if node["kind"] != "Call" {
                    continue;
                }
                let Some(c) = node["props"].as_object() else { continue };
                let trace = parse_json(c.get("trace_json"));
                let payload = parse_json(c.get("payload_json"));
                let text = |v: &Value| v.as_str().map(str::to_owned);
                let input = &payload["tool_input"];
                let target = ["command", "file_path", "url", "pattern"]
                    .iter()
                    .find_map(|k| input[k].as_str())
                    .map(|t| t.chars().take(300).collect::<String>());
                let seq = c.get("seq").and_then(Value::as_i64).unwrap_or_default();
                facts.push(json!({
                    "session": session,
                    "seq": seq,
                    "target": target,
                    "reason": c.get("decision_reason"),
                    "prompt": prompt,
                    "ran": ran.contains_key(&seq),
                    "ran_at": ran.get(&seq).copied().flatten(),
                    "client": c.get("client_id"),
                    "cwd": c.get("cwd"),
                    "at": c.get("decided_at").filter(|v| v.is_i64()).or(c.get("observed_at")),
                    "decision": c.get("decision"),
                    "mode": c.get("decision_mode"),
                    "rule": c.get("decision_rule"),
                    "tool": text(&payload["tool_name"]),
                    "link": text(&trace["link"]),
                    "worst": text(&trace["deviation"]["worst"]),
                    "irreversible": trace["deviation"]["irreversible"]
                        .as_f64()
                        .is_some_and(|p| p >= IRREVERSIBLE),
                }));
            }
        }
        Value::from(facts)
    }

    /// The sensitive values the graphs' token lists hold, one mention each (a fixture keeps each
    /// value's first mention per writer), cut as `graph::SECRETS` cuts them.
    #[cfg_attr(not(feature = "admin"), allow(dead_code))]
    pub fn secrets(&self) -> Value {
        let mut out = Vec::new();
        let Some(graphs) = self.data["graphs"].as_object() else {
            return Value::from(out);
        };
        for (session, graph) in graphs {
            let nodes: Vec<&Value> = graph["nodes"].as_array().into_iter().flatten().collect();
            let ran: std::collections::HashSet<i64> = graph["edges"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|e| e["type"] == "RESULT_OF")
                .filter_map(|e| e["target"].as_i64())
                .collect();
            for t in graph["tokens"].as_array().into_iter().flatten() {
                let category = t["category"].as_str().unwrap_or("");
                if !matches!(category, "credential" | "pii" | "financial" | "device") {
                    continue;
                }
                let seq = t["seq"].as_i64().unwrap_or_default();
                let node = nodes.iter().find(|n| n["props"]["seq"].as_i64() == Some(seq));
                let props = node.map(|n| &n["props"]);
                let payload = parse_json(props.and_then(|p| p.get("payload_json")));
                let input = &payload["tool_input"];
                let target = ["command", "url", "file_path"].iter().find_map(|k| input[k].as_str());
                out.push(json!({
                    "session": session,
                    "seq": seq,
                    "node": node.and_then(|n| n["kind"].as_str()),
                    "slot": t["slot"],
                    "key": t["key"],
                    "kind": t["kind"],
                    "category": category,
                    "text": t["text"],
                    "decision": props.and_then(|p| p.get("decision")),
                    "client": props.and_then(|p| p.get("client_id")),
                    "at": props.and_then(|p| p.get("decided_at").filter(|v| v.is_i64()).or(p.get("observed_at"))),
                    "tool": payload["tool_name"],
                    "target": target,
                    "ran": ran.contains(&seq),
                }));
            }
        }
        Value::from(out)
    }

    pub fn graph(&self, session: &str) -> Result<Value, ApiError> {
        self.data["graphs"]
            .get(session)
            .cloned()
            .ok_or_else(|| ApiError::new(404, "세션을 찾지 못했습니다."))
    }

    pub fn node(&self, session: &str, seq: i64) -> Value {
        self.data["nodes"][session]
            .get(seq.to_string())
            .cloned()
            .unwrap_or(Value::Null)
    }

    pub fn calls(&self, sessions: &[String]) -> Value {
        let found: Map<String, Value> = sessions
            .iter()
            .map(|s| {
                (
                    s.clone(),
                    self.data["calls"]
                        .get(s)
                        .cloned()
                        .unwrap_or_else(|| json!([])),
                )
            })
            .collect();
        Value::Object(found)
    }

    /// Forgets a session until the app restarts, with the same refusal as Neo4j for an active one.
    pub fn delete(&mut self, session: &str) -> Result<Value, ApiError> {
        let list = self.data["sessions"]
            .as_array_mut()
            .ok_or_else(|| ApiError::new(500, "데모 데이터가 손상됐습니다."))?;
        let index = list
            .iter()
            .position(|s| s["id"] == session)
            .ok_or_else(|| ApiError::new(404, "세션을 찾지 못했습니다."))?;
        let s = &list[index];
        check_idle(s["last_at"].as_i64().or_else(|| s["created_at"].as_i64()))?;
        list.remove(index);
        let deleted = self.data["graphs"][session]["nodes"]
            .as_array()
            .map_or(0, Vec::len);
        for key in ["graphs", "nodes", "calls"] {
            if let Some(map) = self.data[key].as_object_mut() {
                map.remove(session);
            }
        }
        Ok(json!({"deleted": deleted}))
    }
}

#[cfg(test)]
mod tests {
    use super::Demo;

    #[test]
    fn fixture_answers_every_session() {
        let demo = Demo::load();
        let sessions = demo.sessions();
        let sessions = sessions.as_array().expect("sessions list");
        assert!(!sessions.is_empty(), "mock/fixture.json has sessions");
        for s in sessions {
            let id = s["id"].as_str().unwrap();
            let graph = demo.graph(id).expect("graph");
            let nodes = graph["nodes"].as_array().unwrap();
            assert_eq!(nodes.len() as i64, s["nodes"].as_i64().unwrap());
            let seq = nodes[0]["props"]["seq"].as_i64().unwrap();
            assert_eq!(demo.node(id, seq)["kind"], nodes[0]["kind"]);
            assert!(
                graph["edges"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|e| e["props"]["kind"] == "cause")
            );
        }
    }
}
