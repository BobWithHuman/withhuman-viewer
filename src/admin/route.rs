//! 기록 in the security team's app: a session as a map seen from above at an angle. One street
//! runs through the whole session from left to right; each turn's request, what the agent wrote
//! it would do and its last word to the person stand on the street, and the turn's Calls stand
//! off it to either side, scattered, each at the end of its own lane, in the order made. A Call
//! picked is drawn round in black and what told the agent to make it is lit in green along the
//! street, back to the request. Calls the engine stopped stand on a red lot, those it held on
//! an amber one.
//!
//! This file says which blocks there are and where they stand; three.js draws them
//! (`web/hub-scene/route.js`, bundled into `hub_scene.js`).

use std::collections::HashMap;
use std::sync::Arc;

use dioxus::prelude::*;

use crate::flow::{Call, Flow, Tone};
use crate::ui::{AppState, Selection};

/// The eye looks down a diagonal of the floor, so what runs left to right on the page runs
/// along the floor's other diagonal: a step of 1 that way is this much along x and back along z.
const TURN: f64 = std::f64::consts::FRAC_1_SQRT_2;
/// How far apart the street's points are given to the scene, in steps along it.
const SAMPLE: f64 = 0.5;
/// How near two blocks may stand, and how near a lane may pass a block.
const APART: f64 = 1.95;
const LANE: f64 = 0.95;

/// Where the street is on the floor `u` steps along it: left to right on the page, swaying a
/// little toward and away from the eye.
fn street(u: f64) -> (f64, f64) {
    let v = 1.3 * (u / 6.5).sin();
    ((u + v) * TURN, (v - u) * TURN)
}

/// A number from 0 to 1 that is always the same for the same Call, to scatter the blocks the
/// same way each time the session is drawn.
fn chance(seq: i64, salt: u64) -> f64 {
    let mut h =
        (seq as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ salt.wrapping_mul(0xD1B5_4A32_D192_ED03);
    h ^= h >> 31;
    h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    h ^= h >> 29;
    (h >> 11) as f64 / (1u64 << 53) as f64
}

/// Where a block stands, and which side of the street: 1 in front of it (nearer the eye), -1
/// behind, 0 on it. Its lane leaves the street going on the way the street runs: to a block in
/// front along x, to one behind back along z.
#[derive(Clone, Copy, Debug)]
struct Lot {
    x: f64,
    z: f64,
    side: f64,
}

impl Lot {
    /// The lot `reach` off the street from where its lane meets it.
    fn off(meets: (f64, f64), side: f64, reach: f64) -> Self {
        if side > 0.0 {
            Lot {
                x: meets.0 + reach,
                z: meets.1,
                side,
            }
        } else {
            Lot {
                x: meets.0,
                z: meets.1 - reach,
                side,
            }
        }
    }

    /// Whether this block, standing in front of the street, hides `other`, a block on it:
    /// near it across the page and not far nearer the eye.
    fn hides(&self, other: &Lot) -> bool {
        let across = ((self.x - self.z) - (other.x - other.z)) * TURN;
        let nearer = ((self.x + self.z) - (other.x + other.z)) * TURN;
        self.side > 0.0 && other.side == 0.0 && across.abs() < 1.15 && (0.0..2.9).contains(&nearer)
    }

    /// Whether this block's lane runs too near `other`, a block on its side or on the street.
    fn lane_meets(&self, other: &Lot) -> bool {
        let across = if self.side > 0.0 {
            self.z - other.z
        } else {
            self.x - other.x
        };
        self.side != 0.0 && (other.side == 0.0 || other.side == self.side) && across.abs() < LANE
    }
}

/// Whether a block can stand here: clear of the blocks near it, no lane running through a
/// block or ending under one on the street, and no block on the street hidden behind another.
fn clear(lot: Lot, taken: &[Lot]) -> bool {
    taken.iter().rev().take(32).all(|t| {
        (lot.x - t.x).hypot(lot.z - t.z) >= APART
            && !lot.lane_meets(t)
            && !t.lane_meets(&lot)
            && !lot.hides(t)
            && !t.hides(&lot)
    })
}

/// Stands a block on the floor: where it is, where its lane meets the street, and how far
/// along the street that is.
fn stand(
    mut node: serde_json::Value,
    at: (f64, f64),
    meets: (f64, f64),
    u: f64,
) -> serde_json::Value {
    for (key, value) in [
        ("x", at.0),
        ("z", at.1),
        ("jx", meets.0),
        ("jz", meets.1),
        ("u", u),
    ] {
        node[key] = value.into();
    }
    node
}

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

/// Stands a block on the street at the first clear place from `u` steps along it, moving `u`
/// there; which block it is.
fn on_street(
    u: &mut f64,
    node: serde_json::Value,
    nodes: &mut Vec<serde_json::Value>,
    taken: &mut Vec<Lot>,
) -> usize {
    let lot = loop {
        let (x, z) = street(*u);
        let lot = Lot { x, z, side: 0.0 };
        if clear(lot, taken) {
            break lot;
        }
        *u += 0.3;
    };
    taken.push(lot);
    nodes.push(stand(node, (lot.x, lot.z), (lot.x, lot.z), *u));
    nodes.len() - 1
}

/// The words in a tool result that told the agent to make a Call, as the engine traced them.
#[derive(Clone, Debug, PartialEq)]
struct Words {
    quote: Option<String>,
    /// Which paragraph of the result they are, when the engine named one.
    paragraph: Option<i64>,
    /// How sure the engine was that these were the words, from 0 to 1.
    p: f64,
    /// The result was the tool failing.
    error: bool,
}

/// A Call as the map needs it.
#[derive(Clone, Debug, PartialEq)]
struct Step {
    seq: i64,
    kind: &'static str,
    tool: String,
    title: String,
    /// The Call that read what told the agent to make this one, and the words that did.
    by: Option<i64>,
    words: Option<Words>,
    blocked: bool,
    held: bool,
    loose: bool,
}

/// A turn as the map needs it: which it is, its request, what the agent wrote it would do, its
/// Calls in order, and the agent's last word to the person.
#[derive(Clone, Debug, PartialEq)]
struct Leg {
    turn: usize,
    n: usize,
    req: String,
    intent: Option<String>,
    calls: Vec<Step>,
    said: Option<String>,
}

fn legs(flow: &Flow, turns: &[usize]) -> Vec<Leg> {
    turns
        .iter()
        .filter_map(|&t| Some((t, flow.turns.get(t)?)))
        .map(|(t, turn)| Leg {
            turn: t,
            n: turn.n,
            req: turn.req.clone(),
            intent: turn.intent.clone().filter(|i| !i.trim().is_empty()),
            said: turn.said.last().map(|s| s.text.clone()),
            calls: turn
                .calls
                .iter()
                .map(|c| Step {
                    seq: c.seq,
                    kind: kind(c),
                    tool: c.tool.clone(),
                    title: c.title.clone(),
                    by: c.by,
                    words: c.cause.as_ref().filter(|_| c.by.is_some()).map(|w| Words {
                        quote: w.quote.clone(),
                        paragraph: w.paragraph,
                        p: w.p,
                        error: w.error,
                    }),
                    blocked: c.blocked,
                    held: c.warn,
                    loose: c.loose,
                })
                .collect(),
        })
        .collect()
}

/// The session as the scene takes it: the street's points, and the blocks. Along the street
/// each turn stands in the order it happened: its request, what the agent wrote it would do,
/// its Calls off to either side, and the agent's last word; the next turn follows on the same
/// street. Each block names the one that told it (`by`), so the scene can light the way back
/// from a Call:
/// - a Call made for the request names the intent (the request when there is none);
/// - a Call a tool result told names that result's words, a small block in its own lane that
///   shows only while lit, which names the Call that read them;
/// - a Call nothing told names nothing.
///
/// Also which block each Call's seq is, to light the one picked.
fn scene(legs: &[Leg]) -> (String, HashMap<i64, usize>) {
    let mut nodes: Vec<serde_json::Value> = Vec::new();
    let mut at: HashMap<i64, usize> = HashMap::new();
    // The Calls a result told: (the Call's block, the Call that read the result, the words).
    let mut told: Vec<(usize, i64, Words)> = Vec::new();
    let mut taken: Vec<Lot> = Vec::new();
    let mut u = 0.0;
    for turn in legs {
        let t = turn.turn;
        let ask = on_street(
            &mut u,
            serde_json::json!({
                "kind": "ask", "turn": t, "seq": null, "by": null, "state": null,
                "label": format!("요청 {}", turn.n),
                "tip": cut(&turn.req, 110),
                "note": "사용자가 보낸 요청",
            }),
            &mut nodes,
            &mut taken,
        );
        u += 2.4;
        // What the Calls made for the request name: the intent when the agent wrote one.
        let mut lead = ask;
        if let Some(intent) = &turn.intent {
            lead = on_street(
                &mut u,
                serde_json::json!({
                    "kind": "intent", "turn": t, "seq": null, "by": ask, "state": null,
                    "label": "의도",
                    "tip": cut(intent, 110),
                    "note": "에이전트가 적은 의도. 엔진은 호출을 이것과 견줍니다",
                }),
                &mut nodes,
                &mut taken,
            );
            u += 2.4;
        }
        for call in &turn.calls {
            // Near or far, one side or the other, as the Call's own number has it; the next
            // place along the street that is clear.
            // A tall block stands only behind the street, where it hides none of it; a low one
            // in front stands a little further off, for the same reason.
            let tall = matches!(call.kind, "change" | "outside" | "delegate");
            let first = (chance(call.seq, 1) * 4.0) as usize;
            let (lot, meets) = loop {
                let meets = street(u);
                let found = (0..4).find_map(|k| {
                    let c = (first + k) % 4;
                    let side = if c.is_multiple_of(2) { 1.0 } else { -1.0 };
                    if tall && side > 0.0 {
                        return None;
                    }
                    let reach = if c < 2 {
                        2.3 + 0.5 * chance(call.seq, 2)
                    } else {
                        4.7 + 0.6 * chance(call.seq, 3)
                    } + if side > 0.0 { 0.6 } else { 0.0 };
                    let lot = Lot::off(meets, side, reach);
                    clear(lot, &taken).then_some(lot)
                });
                match found {
                    Some(lot) => break (lot, meets),
                    None => u += 0.45,
                }
            };
            taken.push(lot);
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
            let by = (call.by.is_none() && !call.loose).then_some(lead);
            at.insert(call.seq, nodes.len());
            if let (Some(b), Some(w)) = (call.by, &call.words) {
                told.push((nodes.len(), b, w.clone()));
            }
            nodes.push(stand(
                serde_json::json!({
                    "kind": call.kind, "turn": t, "seq": call.seq, "by": by, "state": state,
                    "label": call.tool, "tip": cut(&call.title, 90), "note": note,
                }),
                (lot.x, lot.z),
                meets,
                u,
            ));
            u += 0.35 + 0.4 * chance(call.seq, 4);
        }
        if let Some(said) = &turn.said {
            u += 1.6;
            on_street(
                &mut u,
                serde_json::json!({
                    "kind": "say", "turn": t, "seq": null, "by": null, "state": null,
                    "label": "답변",
                    "tip": cut(said, 110),
                    "note": "에이전트가 사용자에게 한 말",
                }),
                &mut nodes,
                &mut taken,
            );
        }
        u += 3.6;
    }
    // The words stand in the lane of the Call they told, a little off the street, so the lit
    // way runs from the Call that read them, along the street, through them, into that Call.
    for (call, reader, words) in told {
        let Some(&from) = at.get(&reader) else {
            continue;
        };
        let num = |key: &str| nodes[call][key].as_f64().unwrap_or(0.0);
        let meets = (num("jx"), num("jz"));
        let side = if num("x") > meets.0 { 1.0 } else { -1.0 };
        let place = Lot::off(meets, side, 1.25);
        let quote = words.quote.as_deref().map(|q| cut(q, 120));
        let named = match words.paragraph {
            _ if words.error => "오류 메시지".to_owned(),
            Some(n) => format!("결과의 문단 {n}"),
            None => "도구 결과".to_owned(),
        };
        let label = match &quote {
            Some(q) => format!("“{}”", cut(q, 16)),
            None => named.clone(),
        };
        let note = format!(
            "{}이 다음 호출을 시켰다고 엔진이 본 확률 {:.0}%",
            if words.error {
                "도구의 오류"
            } else {
                "도구 결과의 이 문단"
            },
            words.p * 100.0
        );
        let w = nodes.len();
        nodes.push(stand(
            serde_json::json!({
                "kind": "words", "ghost": true, "turn": nodes[call]["turn"], "seq": null,
                "by": from, "state": null, "label": label,
                "tip": quote.unwrap_or(named),
                "note": note,
            }),
            (place.x, place.z),
            meets,
            num("u"),
        ));
        nodes[call]["by"] = w.into();
    }
    let from = -3.0;
    let points: Vec<[f64; 2]> = (0..)
        .map(|k| from + k as f64 * SAMPLE)
        .take_while(|&s| s <= u)
        .map(|s| street(s).into())
        .collect();
    let json = serde_json::json!({
        "nodes": nodes,
        "street": { "from": from, "step": SAMPLE, "points": points },
    })
    .to_string();
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
                p { "요청 {turns.len()}개와 호출 {calls}개가 왼쪽에서 오른쪽으로, 일어난 순서대로 서 있습니다. 호출을 누르면 그 호출을 시킨 요청·의도·도구 결과의 문단이 초록 길로 이어집니다." }
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
            words: by.map(|_| Words {
                quote: Some("이 파일도 고쳐라".into()),
                paragraph: Some(3),
                p: 0.9,
                error: false,
            }),
            blocked,
            held: false,
            loose: false,
        }
    }

    fn told(json: &str) -> Vec<serde_json::Value> {
        let v: serde_json::Value = serde_json::from_str(json).unwrap();
        v["nodes"].as_array().unwrap().clone()
    }

    fn num(n: &serde_json::Value, key: &str) -> f64 {
        n[key].as_f64().unwrap()
    }

    #[test]
    fn a_session_runs_left_to_right_turn_after_turn_with_no_block_on_another() {
        let leg = |turn: usize, first: i64| Leg {
            turn,
            n: turn + 1,
            req: "고쳐줘".into(),
            intent: Some("고친다".into()),
            calls: (first..first + 40).map(|s| call(s, None, s == 5)).collect(),
            said: Some("고쳤습니다".into()),
        };
        let (json, at) = scene(&[leg(0, 1), leg(1, 41)]);
        let nodes = told(&json);
        assert_eq!(
            nodes.len(),
            2 * 43,
            "each turn's request, intent, forty Calls and answer"
        );
        // Further along the street is further right on the page, and the order made is the
        // order along the street, the second turn after the first.
        let along: Vec<f64> = nodes.iter().map(|n| num(n, "u")).collect();
        assert!(along.windows(2).all(|w| w[0] < w[1]), "{along:?}");
        for n in &nodes {
            let right = (num(n, "jx") - num(n, "jz")) * TURN;
            assert!((right - num(n, "u")).abs() < 1e-9);
        }
        assert_eq!(nodes[42]["kind"], "say");
        assert_eq!(
            (nodes[43]["kind"].as_str(), nodes[43]["turn"].as_u64()),
            (Some("ask"), Some(1))
        );
        // Calls stand off the street on both sides; the turn's own words stand on it.
        let sides: Vec<f64> = nodes
            .iter()
            .map(|n| (num(n, "x") - num(n, "jx")) - (num(n, "jz") - num(n, "z")))
            .collect();
        assert!(sides.iter().any(|s| *s > 2.0) && sides.iter().any(|s| *s < -2.0));
        assert_eq!((sides[0], sides[1], sides[42]), (0.0, 0.0, 0.0));
        for (i, a) in nodes.iter().enumerate() {
            for b in &nodes[i + 1..] {
                let apart = (num(a, "x") - num(b, "x")).hypot(num(a, "z") - num(b, "z"));
                assert!(
                    apart >= APART,
                    "{} and {} stand {apart} apart",
                    a["label"],
                    b["label"]
                );
            }
        }
        assert_eq!(nodes[at[&5]]["state"], "blocked");
        // The same session is scattered the same way each time.
        assert_eq!(scene(&[leg(0, 1), leg(1, 41)]).0, json);
    }

    #[test]
    fn the_way_back_from_a_call_runs_through_the_words_and_the_intent() {
        let mut loose = call(4, None, false);
        loose.loose = true;
        let leg = Leg {
            turn: 0,
            n: 1,
            req: "고쳐줘".into(),
            intent: Some("테스트를 고친다".into()),
            calls: vec![
                call(1, None, false),
                call(2, None, false),
                call(3, Some(1), true),
                loose,
            ],
            said: Some("고쳤습니다".into()),
        };
        let (json, at) = scene(&[leg]);
        let nodes = told(&json);
        let kinds: Vec<&str> = nodes.iter().map(|n| n["kind"].as_str().unwrap()).collect();
        assert_eq!(
            kinds,
            ["ask", "intent", "run", "run", "run", "run", "say", "words"]
        );
        let by = |i: usize| nodes[i]["by"].as_u64().map(|b| b as usize);
        // The Call a result told → that result's words → the Call that read them → the intent → the request.
        let words = by(at[&3]).unwrap();
        assert_eq!(nodes[words]["ghost"], true);
        assert_eq!(by(words), Some(at[&1]));
        assert_eq!(by(at[&1]), Some(1));
        assert_eq!(by(1), Some(0));
        assert_eq!(by(at[&4]), None, "a Call nothing told names nothing");
        assert_eq!(by(at[&2]), Some(1));
        // The words stand in the lane of the Call they told, between it and the street.
        let (toldc, w) = (&nodes[at[&3]], &nodes[words]);
        let off = |n: &serde_json::Value| (num(n, "x") - num(n, "jx"), num(n, "z") - num(n, "jz"));
        assert_eq!(
            (num(w, "jx"), num(w, "jz"), num(w, "u")),
            (num(toldc, "jx"), num(toldc, "jz"), num(toldc, "u"))
        );
        let (lane, short) = (off(toldc), off(w));
        assert!(short.0.hypot(short.1) < lane.0.hypot(lane.1));
        assert!(
            short.0 * lane.0 + short.1 * lane.1 > 0.0,
            "on the same side of the street"
        );
        assert!(w["note"].as_str().unwrap().contains("90%"));

        // With no intent written, the request is what a Call made for it names.
        let bare = Leg {
            turn: 0,
            n: 1,
            req: "고쳐줘".into(),
            intent: None,
            calls: vec![call(1, None, false)],
            said: None,
        };
        let (json, at) = scene(&[bare]);
        assert_eq!(told(&json)[at[&1]]["by"].as_u64(), Some(0));
    }
}
