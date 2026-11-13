//! 기록 in the security team's app: a session as a map seen from above at an angle. Each turn's
//! request and its Calls stand as blocks on a floor, in the order made, row after row; a Call
//! picked is drawn round in black and what told the agent to make it is lit in green, back to
//! the request. Calls the engine stopped stand on a red pad, those it held on an amber one.
//!
//! This file says which blocks there are and where they stand; three.js draws them
//! (`web/hub-scene/route.js`, bundled into `hub_scene.js`).

use std::collections::HashMap;
use std::sync::Arc;

use dioxus::prelude::*;

use crate::flow::{Call, Flow, Tone};
use crate::ui::{AppState, Selection};

/// How many blocks stand in a row before the line turns back, and how far apart blocks and rows
/// are, in the floor's units. Half the gap between rows is the aisle a lit path runs along.
const ROW: usize = 7;
const STEP: f64 = 2.5;
const ROW_GAP: f64 = 3.4;

fn kind(c: &Call) -> &'static str {
    match c.tone {
        Tone::Read => "read",
        Tone::Change => "change",
        Tone::Run => "run",
        Tone::Outside => "outside",
        Tone::Delegate => "delegate",
        _ => "other",
    }
}

fn cut(text: &str, most: usize) -> String {
    let line = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if line.chars().count() <= most {
        line
    } else {
        format!("{}…", line.chars().take(most).collect::<String>())
    }
}

/// A Call as the map needs it.
#[derive(Clone, Debug, PartialEq)]
struct Step {
    seq: i64,
    kind: &'static str,
    tool: String,
    title: String,
    /// The Call that read what told the agent to make this one.
    by: Option<i64>,
    blocked: bool,
    held: bool,
    loose: bool,
}

/// A turn as the map needs it: which it is, its request and intent, and its Calls in order.
#[derive(Clone, Debug, PartialEq)]
struct Leg {
    turn: usize,
    n: usize,
    req: String,
    intent: Option<String>,
    calls: Vec<Step>,
}

fn legs(flow: &Flow, turns: &[usize]) -> Vec<Leg> {
    turns
        .iter()
        .filter_map(|&t| Some((t, flow.turns.get(t)?)))
        .map(|(t, turn)| Leg {
            turn: t,
            n: turn.n,
            req: turn.req.clone(),
            intent: turn.intent.clone(),
            calls: turn
                .calls
                .iter()
                .map(|c| Step {
                    seq: c.seq,
                    kind: kind(c),
                    tool: c.tool.clone(),
                    title: c.title.clone(),
                    by: c.by,
                    blocked: c.blocked,
                    held: c.warn,
                    loose: c.loose,
                })
                .collect(),
        })
        .collect()
}

/// The session as the scene takes it: a block for each turn's request and each of its Calls,
/// standing in rows that run one way then back, each Call naming the block that told it. Also
/// which block each Call's seq is, to light the one picked.
fn scene(legs: &[Leg]) -> (String, HashMap<i64, usize>) {
    let mut nodes: Vec<serde_json::Value> = Vec::new();
    let mut at: HashMap<i64, usize> = HashMap::new();
    let mut by: Vec<(usize, i64)> = Vec::new();
    let mut row = 0;
    for turn in legs {
        let t = turn.turn;
        let long = turn.calls.len() + 1;
        for k in 0..long {
            let (r, c) = (k / ROW, k % ROW);
            let c = if r % 2 == 1 { ROW - 1 - c } else { c };
            let (x, z) = (c as f64 * STEP, (row + r) as f64 * ROW_GAP);
            if k == 0 {
                nodes.push(serde_json::json!({
                    "kind": "ask", "turn": t, "x": x, "z": z, "seq": null, "by": null, "state": null,
                    "label": format!("요청 {}", turn.n),
                    "tip": cut(&turn.req, 90),
                    "note": turn.intent.as_deref().map(|i| format!("의도: {}", cut(i, 70))),
                }));
                continue;
            }
            let call = &turn.calls[k - 1];
            let state = if call.blocked {
                Some("blocked")
            } else if call.held {
                Some("held")
            } else {
                None
            };
            let note = if call.blocked {
                "엔진이 차단함"
            } else if call.held {
                "사용자에게 확인을 요청함"
            } else if call.loose {
                "시킨 것을 찾지 못함"
            } else if call.by.is_some() {
                "도구 결과를 따라 한 호출"
            } else {
                "요청을 따라 한 호출"
            };
            at.insert(call.seq, nodes.len());
            if let Some(b) = call.by {
                by.push((nodes.len(), b));
            }
            nodes.push(serde_json::json!({
                "kind": call.kind, "turn": t, "x": x, "z": z, "seq": call.seq, "by": null, "state": state,
                "label": call.tool, "tip": cut(&call.title, 90), "note": note,
            }));
        }
        row += long.div_ceil(ROW);
    }
    for (i, b) in by {
        if let Some(&from) = at.get(&b) {
            nodes[i]["by"] = from.into();
        }
    }
    let json = serde_json::json!({ "nodes": nodes, "aisle": ROW_GAP / 2.0 }).to_string();
    (json, at)
}

/// The map in place of the route map: the session's blocks, a heading over them, and the
/// controls to zoom. A block picked is the app's selection, as a station picked is.
#[component]
pub fn RouteScene(
    flow: Arc<Flow>,
    turns: Vec<usize>,
    title: String,
    detail: Signal<bool>,
) -> Element {
    let mut state = use_context::<AppState>();
    let mut detail = detail;
    use_future(move || async move {
        let mut said = document::eval(
            "window.__routePick = (seq) => dioxus.send(seq); await new Promise(() => {});",
        );
        while let Ok(seq) = said.recv::<i64>().await {
            state.selected.set(Selection::Node(seq));
            detail.set(true);
        }
    });
    let (json, at) = scene(&legs(&flow, &turns));
    let model = serde_json::to_string(&json).unwrap_or_else(|_| "\"{}\"".into());
    let chosen = match (state.selected)() {
        Selection::Node(seq) => at.get(&seq).map_or("null".to_owned(), |i| i.to_string()),
        Selection::None => "null".to_owned(),
    };
    use_effect(use_reactive!(|(model, chosen)| {
        document::eval(&format!(
            r#"const el = document.getElementById("route-scene");
            if (el && window.WHRoute) {{
                if (!window.__route || window.__route.container !== el) {{
                    window.__route?.dispose();
                    window.__route = window.WHRoute.mount(el);
                    window.__routeModel = null;
                }}
                const r = window.__route;
                r.onPick = (seq) => window.__routePick?.(seq);
                const model = {model};
                if (window.__routeModel !== model) {{
                    window.__routeModel = model;
                    r.setData(JSON.parse(model));
                }}
                r.setPicked({chosen});
            }}"#
        ));
    }));
    let calls: usize = turns
        .iter()
        .filter_map(|t| flow.turns.get(*t))
        .map(|t| t.calls.len())
        .sum();
    let zoom = |f: f64| {
        move |_| {
            document::eval(&format!("window.__route?.zoomBy({f})"));
        }
    };
    rsx! {
        div { class: "rt-stage",
            div { id: "route-scene" }
            header { class: "rt-head",
                h2 { "{title}" }
                p { "요청 {turns.len()}개, 호출 {calls}개가 일어난 순서대로 서 있습니다. 호출을 누르면 그 호출을 시킨 길이 초록으로 이어집니다." }
            }
            div { class: "rt-keys",
                span { i { class: "way" } "호출을 시킨 길" }
                span { i { class: "blocked" } "차단" }
                span { i { class: "held" } "확인 요청" }
            }
            div { class: "rt-zoom",
                button { title: "확대", onclick: zoom(1.25), "+" }
                button { title: "축소", onclick: zoom(0.8), "−" }
                button { class: "fit", title: "전체 보기", onclick: move |_| { document::eval("window.__route?.fit()"); },
                    svg { width: "14", height: "14", view_box: "0 0 24 24",
                        path { d: "M4.5 9V4.5H9M15 4.5h4.5V9M19.5 15v4.5H15M9 19.5H4.5V15", fill: "none", stroke: "currentColor", stroke_width: "1.8", stroke_linecap: "round", stroke_linejoin: "round" }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn call(seq: i64, by: Option<i64>, blocked: bool) -> Step {
        Step {
            seq,
            kind: "run",
            tool: "Bash".into(),
            title: format!("call {seq}"),
            by,
            blocked,
            held: false,
            loose: false,
        }
    }

    #[test]
    fn a_session_stands_in_rows_that_turn_back_and_calls_name_what_told_them() {
        let calls: Vec<Step> = (1..=8)
            .map(|s| call(s, (s == 8).then_some(3), s == 5))
            .collect();
        let leg = Leg {
            turn: 0,
            n: 1,
            req: "고쳐줘".into(),
            intent: None,
            calls,
        };
        let (json, at) = scene(&[leg]);
        let told: serde_json::Value = serde_json::from_str(&json).unwrap();
        let nodes = told["nodes"].as_array().unwrap();
        assert_eq!(nodes.len(), 9, "the request and its eight Calls");
        assert_eq!(
            (nodes[0]["kind"].as_str(), nodes[0]["x"].as_f64()),
            (Some("ask"), Some(0.0))
        );
        // Seven to a row; the eighth and ninth stand in the next, running back from the right.
        assert_eq!(nodes[6]["z"].as_f64(), Some(0.0));
        assert_eq!(
            (nodes[7]["x"].as_f64(), nodes[7]["z"].as_f64()),
            (Some(6.0 * STEP), Some(ROW_GAP))
        );
        assert_eq!(nodes[8]["x"].as_f64(), Some(5.0 * STEP));
        assert_eq!(nodes[at[&5]]["state"], "blocked");
        assert_eq!(
            nodes[at[&8]]["by"].as_u64(),
            Some(at[&3] as u64),
            "the Call that read what told it"
        );
        assert!(nodes[at[&1]]["by"].is_null());
    }
}
