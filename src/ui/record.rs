//! 기록's graph for the security team: the session's steps drawn by a page built from
//! `web/record` (`npm run build` there writes `record.html`), shown in a frame over the work area
//! and handed the session as JSON. The page keeps its own styles, so the app's do not reach it.

use dioxus::prelude::*;
use serde_json::{Value, json};

use super::{AppState, Load, Selection};
use crate::layout::Model;
use crate::metro::{AskParent, Metro, ParentOf, Station, mcp_parts};
use crate::model::Session;

/// The page, one file with its script and styles inside.
const PAGE: &str = include_str!("record.html");

/// What goes in the app's head: the page as a string, and how it is put in its frame and given
/// its data. `</` is escaped so the page's own closing tags do not end this script.
pub fn head_script() -> String {
    let page = serde_json::to_string(PAGE)
        .unwrap_or_default()
        .replace("</", "<\\/");
    format!(
        "<script>window.__WH_RECORD_HTML = {page};\n\
         window.__whRecordSet = function (d) {{\n\
           window.__whRecordData = d;\n\
           const host = document.getElementById('wh-record'); if (!host) return;\n\
           let f = host.querySelector('iframe');\n\
           if (!f) {{\n\
             f = document.createElement('iframe'); f.className = 'wh-record-frame';\n\
             f.addEventListener('load', () => window.__whFontInto && window.__whFontInto(f.contentDocument));\n\
             f.addEventListener('load', () => f.contentWindow.WHRecord && f.contentWindow.WHRecord.render(window.__whRecordData));\n\
             f.addEventListener('load', () => f.contentWindow.focus());\n\
             f.srcdoc = window.__WH_RECORD_HTML; host.appendChild(f); return;\n\
           }}\n\
           if (f.contentWindow && f.contentWindow.WHRecord) f.contentWindow.WHRecord.render(d);\n\
         }};\n\
         window.__whHooksOn = null;\n\
         window.__whHooksSend = function (m) {{ if (window.__whHooksOn) window.__whHooksOn(m); }};\n\
         window.__whHooksSet = function (d) {{\n\
           window.__whHooksData = d;\n\
           const host = document.getElementById('wh-hooks'); if (!host) return;\n\
           let f = host.querySelector('iframe');\n\
           if (!f) {{\n\
             f = document.createElement('iframe'); f.className = 'wh-record-frame';\n\
             f.addEventListener('load', () => window.__whFontInto && window.__whFontInto(f.contentDocument));\n\
             f.addEventListener('load', () => f.contentWindow.WHHooks && f.contentWindow.WHHooks.render(window.__whHooksData));\n\
             f.srcdoc = window.__WH_RECORD_HTML; host.appendChild(f); return;\n\
           }}\n\
           if (f.contentWindow && f.contentWindow.WHHooks) f.contentWindow.WHHooks.render(d);\n\
         }};</script>"
    )
}

/// The work area of 기록: the frame, handed the open session each time it changes.
#[component]
pub fn RecordGraph() -> Element {
    let state = use_context::<AppState>();
    let backend = use_context::<std::sync::Arc<crate::api::Backend>>();
    // The machines' host names, from the inventory, so a session's machine is named as its list
    // names it rather than by its client ID.
    let hosts = use_resource({
        let backend = backend.clone();
        move || {
            let b = backend.clone();
            let _ = (state.reload)();
            async move {
                let inv = b.inventory().await.ok().flatten().unwrap_or_default();
                inv.clients
                    .into_iter()
                    .filter_map(|c| Some((c.client_id, c.hostname?)))
                    .collect::<std::collections::HashMap<String, String>>()
            }
        }
    });
    let data = use_memo(move || {
        let Load::Ready(model) = &*state.graph.read() else {
            return None;
        };
        let metro = unfolded(model);
        let id = (state.current)();
        let session = match &*state.sessions.read() {
            Load::Ready(list) => list.iter().find(|s| Some(&s.id) == id.as_ref()).cloned(),
            _ => None,
        };
        let mut out = record_data(model, &metro, session.as_ref(), id.as_deref().unwrap_or(""));
        if let Some(client) = session.as_ref().and_then(|s| s.client.as_deref()) {
            let host = hosts.read().as_ref().and_then(|h| h.get(client).cloned());
            out["session"]["device"] = json!(host.unwrap_or_else(|| short_client(client)));
        }
        // The Call opened from another page (민감 정보, 메인): the step it stands in, to open on.
        if let Selection::Node(seq) = (state.selected)() {
            let step = metro
                .stations
                .iter()
                .find(|st| st.seq == seq || st.members.contains(&seq))
                .map(|st| format!("s{}", st.seq))
                .or_else(|| {
                    metro
                        .asks
                        .iter()
                        .any(|a| a.seq == seq)
                        .then(|| format!("a{seq}"))
                });
            out["focus"] = json!(step);
        }
        Some(out.to_string())
    });
    use_effect(move || {
        if let Some(json) = data() {
            document::eval(&format!(
                "requestAnimationFrame(() => window.__whRecordSet && window.__whRecordSet({json}))"
            ));
        }
    });
    let waiting = data().is_none();
    // Why nothing came: the store's own words, and the way to its settings.
    let failed = match (&*state.sessions.read(), &*state.graph.read()) {
        (Load::Failed(e), _) | (_, Load::Failed(e)) => Some(e.clone()),
        _ => None,
    };
    let mut settings = state.settings_open;
    rsx! {
        div { id: "wh-record", class: "wh-record",
            if let Some(e) = failed {
                div { class: "wh-record-wait",
                    div { class: "wh-record-fail",
                        b { "연결 안 됨" }
                        code { "{e}" }
                        button { onclick: move |_| settings.set(true), "연결 설정" }
                    }
                }
            } else if waiting {
                div { class: "wh-record-wait", "세션을 불러오는 중" }
            }
        }
    }
}

/// The session laid out with every Call its own step: 기록 lists each one, none folded.
fn unfolded(model: &Model) -> Metro {
    let calls = model
        .nodes
        .iter()
        .filter(|n| n.kind == "Call")
        .map(|n| n.seq())
        .collect();
    Metro::build_with(model, &calls)
}

/// When, as the timeline writes it: the date and the time.
fn stamp(ms: Option<i64>) -> Option<String> {
    use chrono::{Local, TimeZone};
    let t = Local.timestamp_millis_opt(ms?).single()?;
    Some(t.format("%Y년 %-m월 %-d일 %H:%M:%S").to_string())
}

/// A service with a logo on the page, from an MCP server's name.
fn logo_of(server: &str) -> Option<&'static str> {
    let s = server.to_lowercase();
    ["notion", "slack", "github", "postgres"]
        .into_iter()
        .find(|k| s.contains(k))
        .map(|k| if k == "postgres" { "postgresql" } else { k })
}

fn verdict(decision: Option<&str>) -> Option<&'static str> {
    match decision {
        Some("DENY") => Some("deny"),
        Some("REVIEW") => Some("review"),
        Some("ALLOW") => Some("allow"),
        _ => None,
    }
}

/// A line beside the main one: the first subagent below, the next above, and so on outward.
fn lane(line: usize) -> i64 {
    if line == 0 {
        0
    } else if line % 2 == 1 {
        line.div_ceil(2) as i64
    } else {
        -((line / 2) as i64)
    }
}

fn clip(text: &str, n: usize) -> String {
    let one = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if one.chars().count() > n {
        format!("{}…", one.chars().take(n).collect::<String>())
    } else {
        one
    }
}

fn station_step(st: &Station) -> Value {
    let mcp = mcp_parts(&st.tool);
    let decision = st.decision.as_deref();
    let decided = match decision {
        Some("DENY") => "차단",
        Some("REVIEW") => "승인 요청",
        Some("ALLOW") => "허용",
        _ => "판정 없음",
    };
    // An MCP tool by its server and its own name, as people read them.
    let tool = mcp.map_or_else(
        || st.tool.clone(),
        |(server, name)| format!("{server} · {name}"),
    );
    let mut detail = vec![json!(["판정", decided]), json!(["도구", tool])];
    if let Some(src) = &st.source {
        detail.push(json!([
            "출처",
            format!(
                "도구 응답 · 호출 #{} ({}%)",
                src.by,
                (src.probability * 100.0).round()
            )
        ]));
    } else if st.loose {
        detail.push(json!(["출처", "찾지 못함"]));
    } else if decision.is_none() {
        detail.push(json!(["출처", "추적 없음"]));
    } else {
        detail.push(json!(["출처", "사용자 요청"]));
    }
    if st.irreversible {
        detail.push(json!(["되돌리기", "어려움"]));
    }
    if st.failed {
        detail.push(json!(["결과", "오류"]));
    }
    if let Some(agent) = &st.agent {
        detail.push(json!(["에이전트", agent]));
    }
    if st.folded() {
        detail.push(json!(["호출", st.members.len()]));
    }
    let catch = st.source.as_ref().map(|src| {
        json!({
            "name": if src.error { "도구 오류" } else { "도구 응답" },
            "sub": src.quote.clone().unwrap_or_default(),
            "score": (src.probability * 100.0).round() as i64,
        })
    });
    json!({
        "id": format!("s{}", st.seq),
        "kind": if st.folded() { "group" } else { "call" },
        "name": st.name(),
        "type": match mcp { Some((server, _)) => format!("MCP · {server}"), None => st.tool.clone() },
        "at": st.at,
        "svc": mcp.and_then(|(server, _)| logo_of(server)),
        "verdict": verdict(decision),
        "undecided": decision.is_none(),
        "catch": catch,
        "lane": lane(st.line),
        "detail": detail,
        "text": (!st.what.is_empty()).then(|| json!({ "label": "대상", "body": st.what })),
        "inside": st.folded().then(|| st.members.iter().map(|m| format!("#{m}")).collect::<Vec<_>>()),
    })
}

/// The session as the page reads it: its head, its steps in order, the lines between them, and
/// the causes the engine traced.
/// A client ID (`<team>/<uuid>`) with its UUID cut to the first part, for a machine the
/// inventory has no name for; any other ID as it is.
fn short_client(client: &str) -> String {
    let (team, id) = client.split_once('/').unwrap_or(("", client));
    let uuid = id.len() == 36 && id.matches('-').count() == 4;
    match (uuid, team) {
        (false, _) => client.to_owned(),
        (true, "") => id[..8].to_owned(),
        (true, team) => format!("{team}/{}", &id[..8]),
    }
}

fn record_data(model: &Model, m: &Metro, session: Option<&Session>, id: &str) -> Value {
    // Requests and Calls by when they came, with the step each became.
    let mut items: Vec<(i64, i64, Value)> = Vec::new();
    for ask in &m.asks {
        items.push((
            ask.seq,
            0,
            json!({
                "id": format!("a{}", ask.seq),
                "kind": "prompt",
                // Named by its intent, or quoted by its first words.
                "name": ask.intent.as_deref().map(crate::model::short_intent).filter(|i| !i.is_empty())
                    .map_or_else(|| clip(&ask.text, 30), |i| clip(i, 40)),
                "quote": ask.intent.as_deref().map(crate::model::short_intent).is_none_or(str::is_empty),
                "type": if ask.agent.is_some() { "보고" } else { "프롬프트" },
                "fresh": ask.fresh,
                "at": ask.at,
                "detail": ask.intent.iter().map(|i| json!(["의도", i])).collect::<Vec<_>>(),
                "text": { "label": "입력 프롬프트", "body": ask.text },
            }),
        ));
    }
    for st in &m.stations {
        items.push((st.seq, lane(st.line), station_step(st)));
    }
    // The agent's messages as steps of their own: each turn's answer (its last message), and
    // any a later prompt was written from; the session's last is the answer already.
    let last = m.last_word.as_ref().map(|(seq, _)| *seq);
    let mut said: Vec<i64> = m
        .asks
        .iter()
        .filter_map(|a| match &a.parent {
            Some(AskParent {
                from: ParentOf::Message(seq),
                ..
            }) => Some(*seq),
            _ => None,
        })
        .collect();
    for (i, ask) in m.asks.iter().enumerate() {
        let until = m.asks.get(i + 1).map_or(i64::MAX, |next| next.seq);
        if let Some(end) = m
            .says
            .iter()
            .filter(|s| s.seq > ask.seq && s.seq < until)
            .map(|s| s.seq)
            .max()
        {
            said.push(end);
        }
    }
    said.sort_unstable();
    said.dedup();
    for seq in &said {
        if Some(*seq) != last
            && let Some(said) = m.says.iter().find(|s| s.seq == *seq)
        {
            items.push((
                *seq,
                0,
                json!({
                    "id": format!("m{seq}"), "kind": "answer", "name": clip(&said.text, 24),
                    "type": "답변", "at": said.at, "detail": [],
                    "text": { "label": "답변", "body": said.text },
                }),
            ));
        }
    }
    items.sort_by_key(|(seq, _, _)| *seq);
    if let Some((seq, text)) = &m.last_word {
        items.push((
            *seq,
            0,
            json!({
                "id": "answer", "kind": "answer", "name": "답변", "type": "답변",
                "detail": [], "text": { "label": "답변", "body": text },
            }),
        ));
    }

    // Each line in order; a line beside the main one leaves from the main step before it.
    let mut links: Vec<(String, String)> = Vec::new();
    let mut last_on: std::collections::HashMap<i64, String> = std::collections::HashMap::new();
    for (_, l, step) in &items {
        let sid = step["id"].as_str().unwrap_or_default().to_owned();
        let from = last_on.get(l).cloned().or_else(|| last_on.get(&0).cloned());
        if let Some(from) = from {
            links.push((from, sid.clone()));
        }
        last_on.insert(*l, sid);
    }

    // A Call a tool result made: from the station of the Call that brought the result back.
    let station_of = |seq: i64| {
        m.stations
            .iter()
            .find(|s| s.seq == seq || s.members.contains(&seq))
            .map(|s| format!("s{}", s.seq))
    };
    let causes: Vec<(String, String)> = m
        .stations
        .iter()
        .filter_map(|st| {
            let from = station_of(st.source.as_ref()?.by)?;
            let to = format!("s{}", st.seq);
            (from != to).then_some((from, to))
        })
        .collect();

    // Each prompt's parent: the step of the turn before its writer built on.
    let parents: Vec<Value> = m
        .asks
        .iter()
        .enumerate()
        .filter_map(|(i, ask)| {
            let p = ask.parent.as_ref()?;
            let (from, via) = match &p.from {
                ParentOf::Request => (format!("a{}", m.asks[..i].last()?.seq), "request"),
                ParentOf::Message(seq) if Some(*seq) == last => ("answer".to_owned(), "message"),
                ParentOf::Message(seq) => (format!("m{seq}"), "message"),
                ParentOf::Result { call, .. } => (station_of(*call)?, "result"),
            };
            Some(json!({ "prompt": format!("a{}", ask.seq), "from": from, "p": p.probability, "via": via }))
        })
        .collect();

    // The SubGraphs the engine cut the session into, each with its steps and the one it was
    // traced to: a prompt's to what of the turn before it was written from, the others to the
    // SubGraph their Calls were traced to (the prompt's, or one holding a tool result).
    let mut steps_of: std::collections::BTreeMap<i64, Vec<String>> =
        std::collections::BTreeMap::new();
    for (seq, _, step) in &items {
        if let (Some(g), Some(id)) = (model.subgraph_of(*seq), step["id"].as_str()) {
            steps_of.entry(g).or_default().push(id.to_owned());
        }
    }
    let ask_in = |g: i64| {
        m.asks
            .iter()
            .position(|a| model.subgraph_of(a.seq) == Some(g))
    };
    let undecided: std::collections::HashSet<&str> = items
        .iter()
        .filter(|(_, _, s)| s["undecided"] == true)
        .filter_map(|(_, _, s)| s["id"].as_str())
        .collect();
    let subgraphs: Vec<Value> = steps_of
        .iter()
        .map(|(&g, ids)| {
            let (parent, via, p) = match ask_in(g) {
                Some(i) => match m.asks[i].parent.as_ref() {
                    Some(AskParent { from, probability }) => {
                        let (to, via) = match from {
                            ParentOf::Request => (
                                i.checked_sub(1).and_then(|j| model.subgraph_of(m.asks[j].seq)),
                                "request",
                            ),
                            ParentOf::Message(seq) => (model.subgraph_of(*seq), "message"),
                            ParentOf::Result { result, .. } => (model.subgraph_of(*result), "result"),
                        };
                        (to, via, Some(*probability))
                    }
                    None => (None, "none", None),
                },
                None => match model.subgraph_parent(g) {
                    Some(to) => (Some(to), if ask_in(to).is_some() { "ask" } else { "cause" }, None),
                    // The engine traces no message: the agent's answer is its turn's, so it
                    // hangs from the turn's prompt.
                    None if ids.iter().any(|id| id == "answer" || id.starts_with('m')) => (
                        m.asks
                            .iter()
                            .filter_map(|a| model.subgraph_of(a.seq))
                            .filter(|&a| a < g)
                            .max(),
                        "answer",
                        None,
                    ),
                    // A Call the engine did not trace (it decided none), or traced to nothing
                    // that asked for it: its turn's, left untied.
                    None if ids.iter().any(|id| id.starts_with('s')) => (
                        m.asks
                            .iter()
                            .filter_map(|a| model.subgraph_of(a.seq))
                            .filter(|&a| a < g)
                            .max(),
                        if ids.iter().any(|id| undecided.contains(id.as_str())) { "untraced" } else { "loose" },
                        None,
                    ),
                    None => (None, "none", None),
                },
            };
            let parent = parent.filter(|&to| to != g && steps_of.contains_key(&to));
            json!({ "n": g, "parent": parent, "via": if parent.is_some() { via } else { "none" }, "p": p, "steps": ids })
        })
        .collect();

    // The session's name, as its list gives it: the intent it did the most under, or its first
    // prompt, quoted.
    let named = session.map(|s| s.title()).filter(|t| !t.is_empty());
    let quote = named.is_none_or(|t| session.and_then(|s| s.prompt.as_deref()) == Some(t));
    let title = named
        .map(|t| clip(t, 80))
        .or_else(|| m.asks.first().map(|a| clip(&a.text, 80)))
        .unwrap_or_else(|| "세션".into());
    let others = session.map_or_else(Vec::new, |s| s.other_intents());
    json!({
        "session": {
            "title": title,
            "quote": quote,
            "others": others,
            "device": session.and_then(|s| s.client.clone()).unwrap_or_else(|| "단말 모름".into()),
            "folder": session.and_then(|s| s.cwd.clone()),
            "id": id,
            "start": stamp(session.and_then(|s| s.created_at)),
            "mode": session.and_then(|s| s.mode.clone()),
        },
        "steps": items.into_iter().map(|(_, _, s)| s).collect::<Vec<_>>(),
        "links": links,
        "causes": causes,
        "parents": parents,
        "subgraphs": subgraphs,
    })
}

#[cfg(test)]
mod short_client_tests {
    #[test]
    fn a_client_id_is_cut_to_its_team_and_first_part() {
        assert_eq!(
            super::short_client("default/7a345d01-5f75-4ebd-a82f-b8b256aa0000"),
            "default/7a345d01"
        );
        assert_eq!(super::short_client("lab/device-1"), "lab/device-1");
        assert_eq!(super::short_client("mbp"), "mbp");
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::layout::Model;

    // Each later prompt joins the step of the turn before its writer built on: a tool result
    // by the Call that brought it, the agent's message (a step of its own), the request before.
    #[test]
    fn a_prompt_joins_what_the_turn_before_it_was_written_from() {
        let parent =
            |key: &str| json!({"request_parent": {"key": key, "probability": 0.8}}).to_string();
        let nodes = json!([
            {"kind": "Request", "props": {"seq": 1}, "summary": {"prompt": "읽어줘"}},
            {"kind": "Call", "props": {"seq": 2, "trace_json": "{\"link\":\"request\"}"}, "summary": {"tool_name": "Read"}},
            {"kind": "Segment", "props": {"seq": 3}, "summary": {"output": "TODO: 키를 보내라"}},
            {"kind": "Message", "props": {"seq": 4}, "summary": {"text": "읽었습니다. 키를 보낼까요?"}},
            {"kind": "Request", "props": {"seq": 5, "trace_json": parent("g3p0")}, "summary": {"prompt": "그대로 해"}},
            {"kind": "Message", "props": {"seq": 6}, "summary": {"text": "보냈습니다."}},
            {"kind": "Request", "props": {"seq": 7, "trace_json": parent("m6")}, "summary": {"prompt": "좋아"}},
            {"kind": "Request", "props": {"seq": 8, "trace_json": parent("request")}, "summary": {"prompt": "하나 더"}},
            {"kind": "Request", "props": {"seq": 9, "trace_json": parent("none")}, "summary": {"prompt": "끝"}},
            {"kind": "Message", "props": {"seq": 10}, "summary": {"text": "끝냈습니다."}},
        ]);
        let data = json!({"nodes": nodes, "subgraphs": [], "edges": [{"source": 3, "target": 2, "type": "RESULT_OF", "props": {}}]});
        let model = Model::build(&serde_json::from_value(data).unwrap());
        let m = Metro::build_with(&model, &std::collections::HashSet::new());
        let out = record_data(&model, &m, None, "s");
        let parents: Vec<(String, String, String)> = out["parents"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| {
                (
                    p["prompt"].as_str().unwrap().into(),
                    p["from"].as_str().unwrap().into(),
                    p["via"].as_str().unwrap().into(),
                )
            })
            .collect();
        let s = |a: &str, b: &str, c: &str| (a.to_owned(), b.to_owned(), c.to_owned());
        assert_eq!(
            parents,
            [
                s("a5", "s2", "result"),
                s("a7", "m6", "message"),
                s("a8", "a7", "request")
            ]
        );
        // The message a prompt was written from is a step, in its place; the last is the answer.
        let ids: Vec<&str> = out["steps"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s["id"].as_str().unwrap())
            .collect();
        assert!(ids.iter().position(|&i| i == "m6") < ids.iter().position(|&i| i == "a7"));
        assert_eq!(ids.last(), Some(&"answer"));
    }
}

/// Every demo session as the page is handed it, written to VIEWER_DUMP_RECORDS, to try the page
/// on outside the app.
#[cfg(test)]
mod dump {
    #[test]
    #[ignore = "writes a file"]
    fn every_demo_session_as_the_page_gets_it() {
        let to = std::env::var("VIEWER_DUMP_RECORDS").expect("VIEWER_DUMP_RECORDS");
        let demo = crate::demo::Demo::load();
        let sessions: Vec<crate::model::Session> = serde_json::from_value(demo.sessions()).unwrap();
        let all: Vec<serde_json::Value> = sessions
            .iter()
            .filter_map(|s| {
                let graph = serde_json::from_value(demo.graph(&s.id).ok()?).ok()?;
                let model = crate::layout::Model::build(&graph);
                let m = super::unfolded(&model);
                Some(super::record_data(&model, &m, Some(s), &s.id))
            })
            .collect();
        std::fs::write(to, serde_json::to_string(&all).unwrap()).unwrap();
    }
}
