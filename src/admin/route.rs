//! 기록 in the security team's app: a session as a map seen from above at an angle, its roads
//! the way the engine itself traced each Call back to the request. One street runs through the
//! whole session from left to right; each turn's request (with what the agent wrote it would
//! do, as one block) and its last word to the person stand on the street. A Call the engine traced to the request
//! stands off the street at the end of a lane of its own; a Call it traced to an earlier Call
//! (the one whose result told it, or the last in the SubGraph the engine chose as its parent)
//! stands on from that Call, the road carrying on. So a road leaving the street is one line of
//! the agent's work, each Call on it following from the one before. A Call picked is drawn
//! round in black and its way back to the request is lit. Calls the engine stopped stand on a
//! red lot, those it held on an amber one.
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
/// How near two blocks may stand, how near a road may pass a block, and how far a Call stands
/// from the one it follows.
const APART: f64 = 1.95;
const LANE: f64 = 0.95;
const ON: f64 = 2.3;

type Pt = (f64, f64);

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

/// How far a point is from the stretch of road between two others.
fn gap(p: Pt, a: Pt, b: Pt) -> f64 {
    let (dx, dz) = (b.0 - a.0, b.1 - a.1);
    let long = dx * dx + dz * dz;
    let t = if long == 0.0 {
        0.0
    } else {
        (((p.0 - a.0) * dx + (p.1 - a.1) * dz) / long).clamp(0.0, 1.0)
    };
    (p.0 - a.0 - t * dx).hypot(p.1 - a.1 - t * dz)
}

/// Where a block stands, and which side of the street: 1 in front of it (nearer the eye), -1
/// behind, 0 on it.
#[derive(Clone, Copy, Debug)]
struct Lot {
    x: f64,
    z: f64,
    side: f64,
}

impl Lot {
    fn at(&self) -> Pt {
        (self.x, self.z)
    }

    /// The lot `reach` on from a place off this side of the street. Roads leave the street
    /// going on the way it runs, to the front along x and behind back along z; a road that
    /// `turns` off another goes further from the street the other way.
    fn on(from: Pt, side: f64, reach: f64, turns: bool) -> Self {
        let (x, z) = match (side > 0.0, turns) {
            (true, false) => (from.0 + reach, from.1),
            (true, true) => (from.0, from.1 + reach),
            (false, false) => (from.0, from.1 - reach),
            (false, true) => (from.0 - reach, from.1),
        };
        Lot { x, z, side }
    }

    /// Whether this block, standing in front of the street, hides `other`, a block on it:
    /// near it across the page and not far nearer the eye.
    fn hides(&self, other: &Lot) -> bool {
        let across = ((self.x - self.z) - (other.x - other.z)) * TURN;
        let nearer = ((self.x + self.z) - (other.x + other.z)) * TURN;
        self.side > 0.0 && other.side == 0.0 && across.abs() < 1.15 && (0.0..2.9).contains(&nearer)
    }
}

/// What stands on the floor so far: the blocks, and the roads to them.
#[derive(Default)]
struct Town {
    lots: Vec<Lot>,
    roads: Vec<(Pt, Pt)>,
}

impl Town {
    /// Whether a block can stand here with its road coming from `from`: clear of every block,
    /// its road passing none but the one it leaves, itself on no other road, and no block on
    /// the street hidden behind one in front.
    fn clear(&self, lot: Lot, from: Pt) -> bool {
        self.apart(lot)
            && self.lots.iter().all(|t| {
                (t.at() == from || gap(t.at(), from, lot.at()) >= LANE)
                    && !lot.hides(t)
                    && !t.hides(&lot)
            })
            && self.roads.iter().all(|&(a, b)| gap(lot.at(), a, b) >= LANE)
    }

    fn apart(&self, lot: Lot) -> bool {
        self.lots
            .iter()
            .all(|t| (lot.x - t.x).hypot(lot.z - t.z) >= APART)
    }

    fn take(&mut self, lot: Lot, from: Pt) {
        self.lots.push(lot);
        if from != lot.at() {
            self.roads.push((from, lot.at()));
        }
    }
}

/// Stands a block on the floor: where it is, where its road leaves the street, and how far
/// along the street that is.
fn stand(mut node: serde_json::Value, at: Pt, meets: Pt, u: f64) -> serde_json::Value {
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

/// Stands a block on the street at the first clear place from `u` steps along it, moving `u`
/// there; which block it is.
fn on_street(
    u: &mut f64,
    node: serde_json::Value,
    nodes: &mut Vec<serde_json::Value>,
    town: &mut Town,
) -> usize {
    let lot = loop {
        let (x, z) = street(*u);
        let lot = Lot { x, z, side: 0.0 };
        if town.clear(lot, lot.at()) {
            break lot;
        }
        *u += 0.3;
    };
    town.take(lot, lot.at());
    nodes.push(stand(node, lot.at(), lot.at(), *u));
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
    /// The Call the engine's path to this one comes through last (`Call::parent`), and the
    /// words of its result that told this one, when the engine traced it to them.
    parent: Option<i64>,
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
                    parent: c.parent,
                    words: c
                        .cause
                        .as_ref()
                        .filter(|_| c.by.is_some() && c.by == c.parent)
                        .map(|w| Words {
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
/// each turn stands in the order it happened: its request (carrying what the agent wrote it
/// would do), its Calls off to either side, and the agent's last word; the next turn follows on the same
/// street. Each Call's road comes from where the engine traced it to: from the street for one
/// it traced to the request, from the Call before it on the engine's path (`up`) for one it
/// traced to that Call. Each Call that something told also carries its whole way from the
/// request (`way`): the request, the Calls its road comes through, the words of
/// the result that told it (a small block on its road that shows only while lit), and itself.
///
/// Also which block each Call's seq is, to light the one picked.
fn scene(legs: &[Leg]) -> (String, HashMap<i64, usize>) {
    let mut nodes: Vec<serde_json::Value> = Vec::new();
    let mut at: HashMap<i64, usize> = HashMap::new();
    let mut town = Town::default();
    // Each Call's lot, by its block.
    let mut lots: HashMap<usize, Lot> = HashMap::new();
    // The words to stand, once every Call is: (the Call's block, the words).
    let mut told: Vec<(usize, Words)> = Vec::new();
    let mut u = 0.0;
    for turn in legs {
        let t = turn.turn;
        let first = nodes.len();
        // The request and what the agent wrote it would do about it stand as one block: where
        // the way to every Call of the turn starts.
        let ask = on_street(
            &mut u,
            serde_json::json!({
                "kind": "ask", "turn": t, "seq": null, "up": null, "state": null,
                "label": format!("요청 {}", turn.n),
                "tip": cut(&turn.req, 110),
                "intent": turn.intent.as_deref().map(|i| cut(i, 110)),
                "note": "사용자가 보낸 요청",
            }),
            &mut nodes,
            &mut town,
        );
        u += 2.6;
        let start = vec![ask];
        for call in &turn.calls {
            // The Call it follows, when that one stands in this turn.
            let up = call
                .parent
                .and_then(|p| at.get(&p).copied())
                .filter(|&i| i >= first);
            // A tall block stands only behind the street, where it hides none of it.
            let tall = matches!(call.kind, "change" | "outside" | "delegate");
            let (lot, from, meets, along) = match up.and_then(|i| Some((i, *lots.get(&i)?))) {
                // On from the Call it follows: the road carrying on, or turning off when the
                // way on is taken; further on when words stand between the two.
                Some((i, before)) => {
                    let near = if call.words.is_some() { ON + 0.7 } else { ON };
                    let found = (0..24)
                        .flat_map(|k| [(k, false), (k, true)])
                        .map(|(k, turns)| {
                            Lot::on(before.at(), before.side, near + 0.6 * k as f64, turns)
                        })
                        .enumerate()
                        // The first six places must be clear of everything; after those, of
                        // the blocks alone, so there is always a place.
                        .find(|(n, lot)| {
                            if *n < 12 {
                                town.clear(*lot, before.at())
                            } else {
                                town.apart(*lot)
                            }
                        })
                        .map_or(
                            Lot::on(before.at(), before.side, 60.0, false),
                            |(_, lot)| lot,
                        );
                    let num = |key: &str| nodes[i][key].as_f64().unwrap_or(0.0);
                    (found, before.at(), (num("jx"), num("jz")), num("u"))
                }
                // Off the street: near or far, one side or the other, as the Call's own number
                // has it; the next place along the street that is clear.
                None => {
                    let turn_at = (chance(call.seq, 1) * 4.0) as usize;
                    loop {
                        let meets = street(u);
                        let found = (0..4).find_map(|k| {
                            let c = (turn_at + k) % 4;
                            let side = if c.is_multiple_of(2) { 1.0 } else { -1.0 };
                            if tall && side > 0.0 {
                                return None;
                            }
                            let reach = if c < 2 {
                                2.3 + 0.5 * chance(call.seq, 2)
                            } else {
                                4.7 + 0.6 * chance(call.seq, 3)
                            } + if side > 0.0 { 0.6 } else { 0.0 };
                            let lot = Lot::on(meets, side, reach, false);
                            town.clear(lot, meets).then_some(lot)
                        });
                        match found {
                            Some(lot) => break (lot, meets, meets, u),
                            None => u += 0.45,
                        }
                    }
                }
            };
            town.take(lot, from);
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
            } else if call.words.is_some() {
                "앞선 호출의 결과를 따라 한 호출"
            } else if up.is_some() {
                "앞선 호출에서 이어진 호출"
            } else {
                "요청을 따라 한 호출"
            };
            let here = nodes.len();
            // Its way from the request: through the Calls its road comes through.
            let way = (!call.loose).then(|| {
                let mut way = match up {
                    Some(i) => nodes[i]["way"]
                        .as_array()
                        .map(|w| {
                            w.iter()
                                .filter_map(|i| i.as_u64())
                                .map(|i| i as usize)
                                .collect()
                        })
                        .unwrap_or_else(|| start.iter().copied().chain([i]).collect()),
                    None => start.clone(),
                };
                way.push(here);
                way
            });
            at.insert(call.seq, here);
            lots.insert(here, lot);
            if let (Some(_), Some(w)) = (up, &call.words) {
                told.push((here, w.clone()));
            }
            nodes.push(stand(
                serde_json::json!({
                    "kind": call.kind, "turn": t, "seq": call.seq, "up": up, "state": state,
                    "label": call.tool, "tip": cut(&call.title, 90), "note": note, "way": way,
                }),
                lot.at(),
                meets,
                along,
            ));
            if up.is_none() {
                u += 0.35 + 0.4 * chance(call.seq, 4);
            }
        }
        if let Some(said) = &turn.said {
            u += 1.6;
            on_street(
                &mut u,
                serde_json::json!({
                    "kind": "say", "turn": t, "seq": null, "up": null, "state": null,
                    "label": "답변",
                    "tip": cut(said, 110),
                    "note": "에이전트가 사용자에게 한 말",
                }),
                &mut nodes,
                &mut town,
            );
        }
        u += 3.6;
    }
    // The words stand on the road between the Call that read them and the Call they told,
    // and go into that Call's way just before it.
    for (call, words) in told {
        let Some(from) = nodes[call]["up"].as_u64().map(|i| i as usize) else {
            continue;
        };
        let num = |i: usize, key: &str| nodes[i][key].as_f64().unwrap_or(0.0);
        let place = (
            (num(from, "x") + num(call, "x")) / 2.0,
            (num(from, "z") + num(call, "z")) / 2.0,
        );
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
                "up": from, "state": null, "label": label,
                "tip": quote.unwrap_or(named),
                "note": note,
            }),
            place,
            (num(call, "jx"), num(call, "jz")),
            num(call, "u"),
        ));
        nodes[call]["over"] = w.into();
    }
    // A Call's way carries the words on it: before each Call a result told, that result's.
    let over: HashMap<usize, usize> = nodes
        .iter()
        .enumerate()
        .filter_map(|(i, n)| Some((i, n["over"].as_u64()? as usize)))
        .collect();
    for n in nodes.iter_mut() {
        let Some(way) = n["way"].as_array() else {
            continue;
        };
        let with: Vec<usize> = way
            .iter()
            .filter_map(|i| i.as_u64())
            .flat_map(|i| {
                over.get(&(i as usize))
                    .copied()
                    .into_iter()
                    .chain([i as usize])
            })
            .collect();
        n["way"] = with.into();
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
                p { "요청 {turns.len()}개와 호출 {calls}개입니다. 회색 선은 일어난 순서입니다. 호출을 누르면 요청에서 그 호출까지 엔진이 고른 길만 색으로 켜집니다." }
            }
            div { class: "rt-keys",
                span { i { class: "order" } "일어난 순서" }
                span { i { class: "way" } "엔진이 고른 길" }
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

    fn call(seq: i64, parent: Option<i64>, blocked: bool) -> Step {
        Step {
            seq,
            kind: "run",
            tool: "Bash".into(),
            title: format!("call {seq}"),
            parent,
            words: None,
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

    fn way(n: &serde_json::Value) -> Vec<usize> {
        n["way"].as_array().map_or(Vec::new(), |w| {
            w.iter().map(|i| i.as_u64().unwrap() as usize).collect()
        })
    }

    #[test]
    fn a_session_runs_left_to_right_turn_after_turn_with_no_block_on_another() {
        // Every third Call follows the one before it; the rest follow the request.
        let leg = |turn: usize, first: i64| Leg {
            turn,
            n: turn + 1,
            req: "고쳐줘".into(),
            intent: Some("고친다".into()),
            calls: (first..first + 40)
                .map(|s| call(s, (s % 3 == 0 && s > first).then_some(s - 1), s == 5))
                .collect(),
            said: Some("고쳤습니다".into()),
        };
        let (json, at) = scene(&[leg(0, 1), leg(1, 41)]);
        let nodes = told(&json);
        assert_eq!(
            nodes.len(),
            2 * 42,
            "each turn's request, forty Calls and answer"
        );
        // Further along the street is further right on the page; what stands on the street
        // and the Calls leaving it are in the order made, the second turn after the first.
        let along: Vec<f64> = nodes
            .iter()
            .filter(|n| n["up"].is_null())
            .map(|n| num(n, "u"))
            .collect();
        assert!(along.windows(2).all(|w| w[0] < w[1]), "{along:?}");
        for n in &nodes {
            let right = (num(n, "jx") - num(n, "jz")) * TURN;
            assert!((right - num(n, "u")).abs() < 1e-9);
        }
        assert_eq!(nodes[41]["kind"], "say");
        assert_eq!(
            (nodes[42]["kind"].as_str(), nodes[42]["turn"].as_u64()),
            (Some("ask"), Some(1))
        );
        // Calls stand off the street on both sides; the turn's own words stand on it.
        let sides: Vec<f64> = nodes
            .iter()
            .map(|n| (num(n, "x") - num(n, "jx")) - (num(n, "jz") - num(n, "z")))
            .collect();
        assert!(sides.iter().any(|s| *s > 2.0) && sides.iter().any(|s| *s < -2.0));
        assert_eq!((sides[0], sides[41]), (0.0, 0.0));
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
        // A Call that follows another stands on from it, further from the street, its road
        // leaving the street where that one's does; no road runs through a third block.
        for (i, n) in nodes.iter().enumerate() {
            let Some(up) = n["up"].as_u64().map(|u| u as usize) else {
                continue;
            };
            let before = &nodes[up];
            assert_eq!(
                (num(n, "jx"), num(n, "u")),
                (num(before, "jx"), num(before, "u"))
            );
            let off = |m: &serde_json::Value| {
                (num(m, "x") - num(m, "jx")).hypot(num(m, "z") - num(m, "jz"))
            };
            assert!(off(n) > off(before));
            for (k, other) in nodes.iter().enumerate() {
                if k != i && k != up {
                    let near = gap(
                        (num(other, "x"), num(other, "z")),
                        (num(before, "x"), num(before, "z")),
                        (num(n, "x"), num(n, "z")),
                    );
                    assert!(near >= LANE, "the road to {i} passes {k} at {near}");
                }
            }
        }
        assert_eq!(nodes[at[&5]]["state"], "blocked");
        // The same session is scattered the same way each time.
        assert_eq!(scene(&[leg(0, 1), leg(1, 41)]).0, json);
    }

    #[test]
    fn the_way_to_a_call_is_the_road_the_engine_traced_back_to_the_request() {
        let mut loose = call(5, None, false);
        loose.loose = true;
        // 1 and 2 follow the request; 3 follows 1 because of words in its result; 4 follows 3.
        let mut third = call(3, Some(1), true);
        third.words = Some(Words {
            quote: Some("이 파일도 고쳐라".into()),
            paragraph: Some(3),
            p: 0.9,
            error: false,
        });
        let leg = Leg {
            turn: 0,
            n: 1,
            req: "고쳐줘".into(),
            intent: Some("테스트를 고친다".into()),
            calls: vec![
                call(1, None, false),
                call(2, None, false),
                third,
                call(4, Some(3), false),
                loose,
            ],
            said: Some("고쳤습니다".into()),
        };
        let (json, at) = scene(&[leg]);
        let nodes = told(&json);
        let kinds: Vec<&str> = nodes.iter().map(|n| n["kind"].as_str().unwrap()).collect();
        assert_eq!(
            kinds,
            ["ask", "run", "run", "run", "run", "run", "say", "words"]
        );
        let words = 7;
        assert_eq!(nodes[0]["intent"], "테스트를 고친다");
        assert_eq!(nodes[words]["ghost"], true);
        assert_eq!(way(&nodes[at[&1]]), [0, at[&1]]);
        assert_eq!(way(&nodes[at[&2]]), [0, at[&2]]);
        assert_eq!(way(&nodes[at[&3]]), [0, at[&1], words, at[&3]]);
        assert_eq!(way(&nodes[at[&4]]), [0, at[&1], words, at[&3], at[&4]]);
        assert!(
            way(&nodes[at[&5]]).is_empty(),
            "a Call nothing told has no way to it"
        );
        assert_eq!(nodes[at[&3]]["up"].as_u64(), Some(at[&1] as u64));
        assert_eq!(nodes[at[&4]]["up"].as_u64(), Some(at[&3] as u64));
        assert!(nodes[at[&2]]["up"].is_null());
        // The words stand on the road between the Call that read them and the one they told.
        let (a, b, w) = (&nodes[at[&1]], &nodes[at[&3]], &nodes[words]);
        assert!(
            gap(
                (num(w, "x"), num(w, "z")),
                (num(a, "x"), num(a, "z")),
                (num(b, "x"), num(b, "z"))
            ) < 1e-9
        );
        assert!(w["note"].as_str().unwrap().contains("90%"));

        // With no intent written, the way starts at the request alone; a Call following one
        // of an earlier turn leaves the street like any other.
        let bare = Leg {
            turn: 0,
            n: 1,
            req: "고쳐줘".into(),
            intent: None,
            calls: vec![call(1, None, false)],
            said: None,
        };
        let next = Leg {
            turn: 1,
            n: 2,
            req: "더".into(),
            intent: None,
            calls: vec![call(2, Some(1), false)],
            said: None,
        };
        let (json, at) = scene(&[bare, next]);
        let nodes = told(&json);
        assert_eq!(way(&nodes[at[&1]]), [0, at[&1]]);
        assert!(nodes[at[&2]]["up"].is_null());
        assert_eq!(way(&nodes[at[&2]]), [2, at[&2]]);
    }
}
