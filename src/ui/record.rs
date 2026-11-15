//! 기록's graph for the security team: the session's steps drawn by a page built from
//! `web/record` (`npm run build` there writes `record.html`), shown in a frame over the work area
//! and handed the session as JSON. The page keeps its own styles, so the app's do not reach it.

use dioxus::prelude::*;
use serde_json::{Value, json};

use super::{AppState, Load};
use crate::metro::{Metro, Station, mcp_parts};
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
             f.srcdoc = window.__WH_RECORD_HTML; host.appendChild(f); return;\n\
           }}\n\
           if (f.contentWindow && f.contentWindow.WHRecord) f.contentWindow.WHRecord.render(d);\n\
         }};</script>"
    )
}

/// The work area of 기록: the frame, handed the open session each time it changes.
#[component]
pub fn RecordGraph() -> Element {
    let state = use_context::<AppState>();
    let data = use_memo(move || {
        let Load::Ready(model) = &*state.graph.read() else {
            return None;
        };
        let metro = Metro::build_with(model, &std::collections::HashSet::new());
        let id = (state.current)();
        let session = match &*state.sessions.read() {
            Load::Ready(list) => list.iter().find(|s| Some(&s.id) == id.as_ref()).cloned(),
            _ => None,
        };
        Some(record_data(&metro, session.as_ref(), id.as_deref().unwrap_or("")).to_string())
    });
    use_effect(move || {
        if let Some(json) = data() {
            document::eval(&format!(
                "requestAnimationFrame(() => window.__whRecordSet && window.__whRecordSet({json}))"
            ));
        }
    });
    let waiting = data().is_none();
    rsx! {
        div { id: "wh-record", class: "wh-record",
            if waiting {
                div { class: "wh-record-wait", "세션을 불러오는 중" }
            }
        }
    }
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
        Some("DENY") => "막음",
        Some("REVIEW") => "확인",
        Some("ALLOW") => "허용",
        _ => "판정 없음",
    };
    let mut detail = vec![json!(["판정", decided]), json!(["도구", st.tool])];
    if let Some(src) = &st.source {
        detail.push(json!([
            "원인",
            format!(
                "도구 결과 · 호출 #{} ({}%)",
                src.by,
                (src.probability * 100.0).round()
            )
        ]));
    } else if st.loose {
        detail.push(json!(["원인", "찾지 못함"]));
    } else {
        detail.push(json!(["원인", "사용자 요청"]));
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
            "name": if src.error { "도구 오류가 시킴" } else { "도구 결과가 시킴" },
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
        "catch": catch,
        "lane": lane(st.line),
        "detail": detail,
        "text": (!st.what.is_empty()).then(|| json!({ "label": "대상", "body": st.what })),
        "inside": st.folded().then(|| st.members.iter().map(|m| format!("#{m}")).collect::<Vec<_>>()),
    })
}

/// The session as the page reads it: its head, its steps in order, the lines between them, and
/// the causes the engine traced.
fn record_data(m: &Metro, session: Option<&Session>, id: &str) -> Value {
    // Requests and Calls by when they came, with the step each became.
    let mut items: Vec<(i64, i64, Value)> = Vec::new();
    for ask in &m.asks {
        items.push((
            ask.seq,
            0,
            json!({
                "id": format!("a{}", ask.seq),
                "kind": "prompt",
                "name": clip(&ask.text, 30),
                "type": if ask.agent.is_some() { "보고" } else { "프롬프트" },
                "at": ask.at,
                "detail": ask.intent.iter().map(|i| json!(["의도", i])).collect::<Vec<_>>(),
                "text": { "label": "입력 프롬프트", "body": ask.text },
            }),
        ));
    }
    for st in &m.stations {
        items.push((st.seq, lane(st.line), station_step(st)));
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

    let title = m
        .asks
        .first()
        .map(|a| clip(&a.text, 80))
        .or_else(|| session.and_then(|s| s.prompt.clone()))
        .unwrap_or_else(|| "세션".into());
    json!({
        "session": {
            "title": title,
            "device": session.and_then(|s| s.client.clone()).unwrap_or_else(|| "단말 모름".into()),
            "folder": session.and_then(|s| s.cwd.clone()),
            "id": id,
            "start": stamp(session.and_then(|s| s.created_at)),
            "mode": session.and_then(|s| s.mode.clone()).map(|m| match m.as_str() {
                "enforce" => "적용".to_owned(),
                "shadow" => "그림자".to_owned(),
                "record" => "기록만".to_owned(),
                _ => m,
            }),
        },
        "steps": items.into_iter().map(|(_, _, s)| s).collect::<Vec<_>>(),
        "links": links,
        "causes": causes,
    })
}
