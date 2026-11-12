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

#[cfg_attr(not(feature = "admin"), allow(dead_code))]
mod mcp;

static FIXTURE: &str = include_str!("../mock/fixture.json");

pub struct Demo {
    data: Value,
    /// The demo's agent inventory, made when first asked for and changed by the demo's key
    /// actions until the app restarts.
    #[cfg_attr(not(feature = "admin"), allow(dead_code))]
    inventory: Option<crate::model::Inventory>,
    /// The demo's MCP allowlist, likewise ([`mcp`]).
    #[cfg_attr(not(feature = "admin"), allow(dead_code))]
    mcp: Option<crate::model::McpCatalog>,
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
        Self { data, inventory: None, mcp: None }
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

    /// The demo's agent inventory: the fixture's sessions on two registered machines (the newest
    /// one still running), a machine installed but never heard from, a benchmark's unregistered
    /// ID, two sessions that came without an ID, and keys for four people, one of them stopped.
    #[cfg_attr(not(feature = "admin"), allow(dead_code))]
    pub fn inventory(&mut self, now: i64) -> crate::model::Inventory {
        if self.inventory.is_none() {
            self.inventory = Some(self.made_inventory(now));
        }
        self.inventory.clone().unwrap_or_default()
    }

    #[cfg_attr(not(feature = "admin"), allow(dead_code))]
    fn made_inventory(&self, now: i64) -> crate::model::Inventory {
        use crate::model::{ApiKey, Device, DeviceProject, Inventory, KeyEvent, Person, SessionRun};
        const MIN: i64 = 60_000;
        const DAY: i64 = 24 * 60 * MIN;
        let person = |client: &str| match client {
            "claude-code-mock" => Some("dev.kim@example.com".to_owned()),
            "personal-mock" => Some("dev.lee@example.com".to_owned()),
            _ => None,
        };
        let mut sessions: Vec<SessionRun> = Vec::new();
        let list = self.data["sessions"].as_array().cloned().unwrap_or_default();
        let newest = list.iter().filter_map(|s| s["created_at"].as_i64()).max();
        for s in &list {
            let id = s["id"].as_str().unwrap_or_default().to_owned();
            let client = s["client"].as_str().map(str::to_owned);
            let created = s["created_at"].as_i64();
            let running = created.is_some() && created == newest;
            let last = s["last_at"].as_i64().or(created);
            sessions.push(SessionRun {
                session_id: id,
                user_email: client.as_deref().and_then(person),
                client_id: client,
                start_source: Some("startup".into()),
                permission_mode: Some(if running { "acceptEdits" } else { "default" }.into()),
                model: Some("claude-sonnet".into()),
                created_at: created,
                last_seen_at: if running { Some(now - 2 * MIN) } else { last },
                ended_at: if running { None } else { last.map(|l| l + 1_000) },
                end_reason: if running { None } else { Some("prompt_input_exit".into()) },
            });
        }
        // Two sessions whose hooks sent no ID (an app launched outside a shell, a container).
        for (i, age) in [(1, 3 * DAY), (2, DAY + 5 * MIN)] {
            sessions.push(SessionRun {
                session_id: format!("demo-no-id-{i}"),
                start_source: Some("startup".into()),
                created_at: Some(now - age),
                last_seen_at: Some(now - age + 20 * MIN),
                ended_at: Some(now - age + 21 * MIN),
                end_reason: Some("other".into()),
                ..SessionRun::default()
            });
        }
        let seen = |client: &str| {
            sessions
                .iter()
                .filter(|r| r.client_id.as_deref() == Some(client))
                .filter_map(|r| r.last_seen_at)
                .max()
        };
        let first = |client: &str| {
            sessions
                .iter()
                .filter(|r| r.client_id.as_deref() == Some(client))
                .filter_map(|r| r.created_at)
                .min()
        };
        let device = |id: &str, host: &str, os: &str, user: &str, registered: Option<i64>| Device {
            client_id: id.into(),
            team: id.split_once('/').map(|(t, _)| t.to_owned()).or(Some("lab".into())),
            hostname: Some(host.into()),
            os: Some(os.into()),
            os_user: Some(host.replace("dev-", "")),
            hook_version: Some("0.9.2".into()),
            agent: Some("claude-code".into()),
            user_email: Some(user.into()),
            registered_at: registered,
            updated_at: registered,
            first_seen: first(id),
            last_seen: seen(id),
        };
        let clients = vec![
            device("claude-code-mock", "dev-mac-07", "macOS 15", "dev.kim@example.com", Some(now - 30 * DAY)),
            device("personal-mock", "dev-mac-02", "macOS 14", "dev.lee@example.com", Some(now - 21 * DAY)),
            device("lab/7c1e9a2e-4f1b-4c55-9d0e-2b8f3a61c0d4", "dev-win-11", "Windows 11", "dev.park@example.com", Some(now - 9 * DAY)),
            Device {
                client_id: "repoguard-codex-main/s14".into(),
                first_seen: Some(now - 4 * DAY),
                last_seen: Some(now - 4 * DAY + 15 * MIN),
                ..Device::default()
            },
        ];
        let project = |id: &str, path: &str, at: i64| DeviceProject {
            client_id: id.into(),
            project: path.into(),
            agent: Some("claude-code".into()),
            hook_version: Some("0.9.2".into()),
            installed_at: Some(at),
            updated_at: Some(at),
        };
        let projects = vec![
            project("claude-code-mock", "/Users/dev/src/payments-service", now - 30 * DAY),
            project("claude-code-mock", "/Users/dev/src/infra", now - 12 * DAY),
            project("personal-mock", "/Users/dev/src/payments-service", now - 21 * DAY),
            project("lab/7c1e9a2e-4f1b-4c55-9d0e-2b8f3a61c0d4", "C:/work/billing-api", now - 9 * DAY),
        ];
        let users = ["dev.kim", "dev.lee", "dev.park", "ci-bot"]
            .iter()
            .map(|u| Person {
                email: format!("{u}@example.com"),
                created_at: Some(now - 30 * DAY),
                kind: Some(if *u == "ci-bot" { "service" } else { "person" }.into()),
            })
            .collect();
        let key = |id: &str, user: &str, made: i64, used: Option<i64>| ApiKey {
            key_id: id.into(),
            user_email: format!("{user}@example.com"),
            created_at: Some(made),
            created_by: Some("security-admin".into()),
            note: None,
            last_used_at: used,
            ..ApiKey::default()
        };
        let mut keys = vec![
            key("k-7f3a", "dev.kim", now - 30 * DAY, Some(now - 2 * MIN)),
            key("k-21c9", "dev.lee", now - 21 * DAY, seen("personal-mock")),
            key("k-9e04", "dev.park", now - 9 * DAY, Some(now - DAY - 2 * 60 * MIN)),
            key("k-c1b0", "ci-bot", now - 14 * DAY, Some(now - 6 * 60 * MIN)),
        ];
        let stopped = now - DAY - 60 * MIN;
        if let Some(k) = keys.iter_mut().find(|k| k.key_id == "k-9e04") {
            k.revoked_at = Some(stopped);
            k.revoked_by = Some("security-admin".into());
            k.revoke_reason = Some("사용자가 승인했지만 main 브랜치 강제 푸시였음".into());
            k.first_denied_at = Some(stopped + 40 * MIN);
            k.last_denied_at = Some(stopped + 55 * MIN);
            k.denied_count = Some(2);
        }
        let event = |kind: &str, user: &str, key: &str, at: i64, actor: Option<&str>, reason: Option<&str>| KeyEvent {
            kind: kind.into(),
            actor: actor.map(str::to_owned),
            reason: reason.map(str::to_owned),
            user_email: Some(format!("{user}@example.com")),
            key_id: Some(key.into()),
            session_id: None,
            client_id: None,
            at: Some(at),
        };
        let mut events: Vec<KeyEvent> = keys
            .iter()
            .map(|k| {
                let user = k.user_email.trim_end_matches("@example.com").to_owned();
                event("issued", &user, &k.key_id, k.created_at.unwrap_or(now), Some("security-admin"), None)
            })
            .collect();
        events.push(event("revoked", "dev.park", "k-9e04", stopped, Some("security-admin"),
            Some("사용자가 승인했지만 main 브랜치 강제 푸시였음")));
        for (i, at) in [stopped + 40 * MIN, stopped + 55 * MIN].into_iter().enumerate() {
            let mut e = event("denied", "dev.park", "k-9e04", at, None, None);
            e.client_id = Some("lab/7c1e9a2e-4f1b-4c55-9d0e-2b8f3a61c0d4".into());
            e.session_id = Some(format!("demo-denied-{i}"));
            events.push(e);
        }
        events.sort_by_key(|e| std::cmp::Reverse(e.at));
        Inventory { clients, projects, users, keys, events, sessions }
    }

    /// The demo's key actions, so the pages can be tried: issue a key, revoke one, stop a person.
    #[cfg_attr(not(feature = "admin"), allow(dead_code))]
    pub fn issue_key(&mut self, now: i64, email: &str, actor: &str, note: &str) -> (String, String) {
        let inventory = self.inventory.get_or_insert_with(Default::default);
        let key_id = format!("k-{:04x}", (now / 1000) % 0xffff);
        let secret = format!("whk_demo_{:x}", now);
        if !inventory.users.iter().any(|u| u.email == email) {
            inventory.users.push(crate::model::Person { email: email.into(), created_at: Some(now), kind: Some("person".into()) });
        }
        inventory.keys.push(crate::model::ApiKey {
            key_id: key_id.clone(),
            user_email: email.into(),
            created_at: Some(now),
            created_by: Some(actor.into()),
            note: Some(note.into()).filter(|n: &String| !n.is_empty()),
            ..Default::default()
        });
        inventory.events.insert(0, crate::model::KeyEvent {
            kind: "issued".into(),
            actor: Some(actor.into()),
            user_email: Some(email.into()),
            key_id: Some(key_id.clone()),
            at: Some(now),
            ..Default::default()
        });
        (key_id, secret)
    }

    #[cfg_attr(not(feature = "admin"), allow(dead_code))]
    pub fn revoke(&mut self, now: i64, key_id: Option<&str>, email: Option<&str>, actor: &str, reason: &str) -> usize {
        let inventory = self.inventory.get_or_insert_with(Default::default);
        let mut done = Vec::new();
        for k in inventory.keys.iter_mut().filter(|k| k.revoked_at.is_none()) {
            if key_id.is_some_and(|id| k.key_id == id) || email.is_some_and(|e| k.user_email == e) {
                k.revoked_at = Some(now);
                k.revoked_by = Some(actor.into());
                k.revoke_reason = Some(reason.into());
                done.push((k.key_id.clone(), k.user_email.clone()));
            }
        }
        for (key, user) in &done {
            inventory.events.insert(0, crate::model::KeyEvent {
                kind: "revoked".into(),
                actor: Some(actor.into()),
                reason: Some(reason.into()),
                user_email: Some(user.clone()),
                key_id: Some(key.clone()),
                at: Some(now),
                ..Default::default()
            });
        }
        done.len()
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
