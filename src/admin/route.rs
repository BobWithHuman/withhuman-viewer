//! 기록 in the security team's app: a session as a town seen from above at an angle, the way
//! through its streets the way the engine itself traced each Call back to the request. Each
//! turn is a part of town: streets crossing in a grid, the blocks between them lots for
//! buildings. Its request (with what the agent wrote it would do) is a signpost at the corner
//! the part of town starts from; the turns stand one above the last, an avenue joining their
//! corners. Every Call is a building on a lot. A Call the engine traced to the request takes
//! the nearest free lot to the signpost; one it traced to an earlier Call (the one whose
//! result told it, or the last in the SubGraph the engine chose as its parent) takes the
//! nearest free lot to that Call's building. In a long turn, Calls in a row that only followed
//! the request and were let through stand as one large building filling a block, which opens
//! into them when picked. The way to a Call is found through the streets as a route finder
//! finds one: from the signpost to the door of each Call the engine's path comes through, in
//! turn, and on to the Call's own, turning at the crossings. A Call picked is drawn round in
//! black and that way is lit. Calls the engine stopped stand on a red lot, those it held on
//! an amber one.
//!
//! This file says which buildings and streets there are and where; three.js draws them
//! (`web/hub-scene/route.js`, bundled into `hub_scene.js`).

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use dioxus::prelude::*;

use crate::flow::{Call, Flow, Tone};
use crate::ui::{AppState, Selection};

/// The eye looks down a diagonal of the floor, so what runs left to right on the page runs
/// along the floor's other diagonal: a step of 1 that way is this much along x and back along z.
const TURN: f64 = std::f64::consts::FRAC_1_SQRT_2;
/// How much room a building takes around its middle, a large one more: what the tests hold
/// the lots to.
#[cfg(test)]
const ROOM: f64 = 0.975;
#[cfg(test)]
const LARGE: f64 = 1.9;
/// A street's width and the pavement beside it; from one lot's middle to the next; and from a
/// street's middle to the next street's: two lots and their pavements between.
const WIDE: f64 = 1.5;
const PAVE: f64 = 0.32;
const PITCH: f64 = 2.1;
const BLOCK: f64 = WIDE + 2.0 * PAVE + 2.0 * PITCH;
/// From a street's middle to its kerb, and to the middle of the first lot beside it.
const KERB: f64 = WIDE / 2.0 + PAVE;
const EDGE: f64 = KERB + PITCH / 2.0;
/// How far apart the turns' parts of town keep.
const BETWEEN: f64 = 3.2;
/// A turn with more Calls than this folds its plain runs, of this many Calls or more.
const LONG: usize = 10;
const RUN: usize = 3;
/// What a large building goes by where a Call goes by its seq: this, less its first Call's seq.
const MANY: i64 = -1_000_000_000_000;

type Pt = (f64, f64);

/// What a turn's request goes by where a Call goes by its seq: on the map's list of blocks,
/// and in what the scene sends when a block is picked. Below every seq.
fn ask_key(turn: usize) -> i64 {
    -(turn as i64) - 1
}

/// What a large block goes by, from the seq of the first Call folded into it. Below every
/// request's key.
fn many_key(first: i64) -> i64 {
    MANY - first
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

/// A place on a street: where, and whether the street runs along z there (else along x). A
/// crossing is on both.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Stop {
    at: Pt,
    upright: bool,
}

impl Stop {
    /// The crossings at either end of the stretch of street it is on; itself when it is one.
    fn ends(&self) -> Vec<Pt> {
        let (x, z) = self.at;
        let near = |v: f64| [(v / BLOCK).floor() * BLOCK, (v / BLOCK).ceil() * BLOCK];
        let mut ends: Vec<Pt> = if self.upright {
            near(z).map(|z| (x, z)).to_vec()
        } else {
            near(x).map(|x| (x, z)).to_vec()
        };
        ends.dedup();
        ends
    }
}

/// How far it is along a way through the streets.
fn long(way: &[Pt]) -> f64 {
    way.windows(2)
        .map(|w| (w[1].0 - w[0].0).abs() + (w[1].1 - w[0].1).abs())
        .sum()
}

/// The shortest way through the streets from one place on them to another: along the street
/// to a crossing, across town turning once, and along the last street. Straight there when
/// both are on one stretch of street.
fn drive(a: Stop, b: Stop) -> Vec<Pt> {
    let same_line = a.upright == b.upright
        && if a.upright {
            a.at.0 == b.at.0
        } else {
            a.at.1 == b.at.1
        };
    if same_line && a.ends().iter().any(|e| b.ends().contains(e)) {
        return vec![a.at, b.at];
    }
    let mut best: Option<Vec<Pt>> = None;
    for from in a.ends() {
        for to in b.ends() {
            // Carrying on the way it left, then turning.
            let corner = if a.upright {
                (from.0, to.1)
            } else {
                (to.0, from.1)
            };
            let mut way = vec![a.at, from, corner, to, b.at];
            way.dedup();
            if best.as_ref().is_none_or(|b| long(&way) < long(b)) {
                best = Some(way);
            }
        }
    }
    best.unwrap_or_else(|| vec![a.at, b.at])
}

/// A lot in a block: where its middle is, which block, and the places on the two streets it
/// faces with the kerb before it at each.
#[derive(Clone, Debug)]
struct Plot {
    at: Pt,
    block: (usize, usize),
    doors: Vec<(Stop, Pt)>,
    taken: bool,
}

/// A turn's part of town: blocks of four lots in rows, more rows as they are needed.
struct Town {
    cols: usize,
    rows: usize,
    plots: Vec<Plot>,
}

impl Town {
    fn new(cols: usize) -> Self {
        let mut town = Town {
            cols,
            rows: 0,
            plots: Vec::new(),
        };
        town.grow();
        town
    }

    /// Another row of blocks, further from the signpost.
    fn grow(&mut self) {
        let j = self.rows;
        for i in 0..self.cols {
            for b in 0..2 {
                for a in 0..2 {
                    let x = i as f64 * BLOCK + EDGE + a as f64 * PITCH;
                    let z = -(j as f64 * BLOCK + EDGE + b as f64 * PITCH);
                    let (beside, across) = ((i + a) as f64 * BLOCK, -((j + b) as f64 * BLOCK));
                    self.plots.push(Plot {
                        at: (x, z),
                        block: (i, j),
                        doors: vec![
                            (
                                Stop {
                                    at: (beside, z),
                                    upright: true,
                                },
                                (beside + if a == 0 { KERB } else { -KERB }, z),
                            ),
                            (
                                Stop {
                                    at: (x, across),
                                    upright: false,
                                },
                                (x, across + if b == 0 { -KERB } else { KERB }),
                            ),
                        ],
                        taken: false,
                    });
                }
            }
        }
        self.rows += 1;
    }

    /// The free lot nearest a place, taken; rows are added until there is one.
    fn take(&mut self, near: Pt) -> Plot {
        loop {
            let free = self
                .plots
                .iter()
                .enumerate()
                .filter(|(_, p)| !p.taken)
                .min_by(|(_, a), (_, b)| {
                    let far = |p: &Plot| (p.at.0 - near.0).hypot(p.at.1 - near.1);
                    far(a).total_cmp(&far(b))
                })
                .map(|(i, _)| i);
            match free {
                Some(i) => {
                    self.plots[i].taken = true;
                    return self.plots[i].clone();
                }
                None => self.grow(),
            }
        }
    }

    /// The block nearest the signpost with all four lots free, taken whole: its middle, and
    /// the places on the streets round it with their kerbs.
    fn take_block(&mut self) -> (Pt, Vec<(Stop, Pt)>) {
        loop {
            let free = (0..self.rows)
                .flat_map(|j| (0..self.cols).map(move |i| (i, j)))
                .filter(|b| {
                    self.plots
                        .iter()
                        .filter(|p| p.block == *b)
                        .all(|p| !p.taken)
                })
                .min_by_key(|(i, j)| (i + j, *j));
            let Some(block) = free else {
                self.grow();
                continue;
            };
            for p in self.plots.iter_mut().filter(|p| p.block == block) {
                p.taken = true;
            }
            let (x, z) = (block.0 as f64 * BLOCK, -(block.1 as f64 * BLOCK));
            let half = BLOCK / 2.0;
            let mid = (x + half, z - half);
            return (
                mid,
                vec![
                    (
                        Stop {
                            at: (x, mid.1),
                            upright: true,
                        },
                        (x + KERB, mid.1),
                    ),
                    (
                        Stop {
                            at: (mid.0, z),
                            upright: false,
                        },
                        (mid.0, z - KERB),
                    ),
                    (
                        Stop {
                            at: (x + BLOCK, mid.1),
                            upright: true,
                        },
                        (x + BLOCK - KERB, mid.1),
                    ),
                    (
                        Stop {
                            at: (mid.0, z - BLOCK),
                            upright: false,
                        },
                        (mid.0, z - BLOCK + KERB),
                    ),
                ],
            );
        }
    }
}

/// A building's door: the place on the street before it, its kerb there, where the building
/// is, and the way through the streets to it from the signpost.
#[derive(Clone, Debug)]
struct Door {
    stop: Stop,
    kerb: Pt,
    lot: Pt,
    path: Vec<Pt>,
}

/// The door, of those a building has, with the shortest way to it on from a way so far.
fn nearest_door(doors: &[(Stop, Pt)], lot: Pt, from: Stop, so_far: &[Pt]) -> Door {
    let (stop, kerb, way) = doors
        .iter()
        .map(|&(stop, kerb)| (stop, kerb, drive(from, stop)))
        .min_by(|a, b| long(&a.2).total_cmp(&long(&b.2)))
        .expect("a building faces a street");
    let mut path = so_far.to_vec();
    path.extend(way.into_iter().skip(1));
    Door {
        stop,
        kerb,
        lot,
        path,
    }
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

/// A turn as the map needs it: which it is, its request, what the agent wrote it would do, and
/// its Calls in order.
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
            intent: turn.intent.clone().filter(|i| !i.trim().is_empty()),
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

/// What stands for a turn's Calls, in order: a Call, or a run of them folded into one block.
enum Unit<'a> {
    One(&'a Step),
    Many(&'a [Step]),
}

/// A turn's Calls as they stand. In a long turn, Calls in a row with nothing to single out
/// (let through, traced to the request itself, no other Call traced to them) fold into one
/// block; not the one `kept` (the Call picked), nor a run whose first Call is in `open`.
fn units<'a>(turn: &'a Leg, open: &HashSet<i64>, kept: Option<i64>) -> Vec<Unit<'a>> {
    let seqs: HashSet<i64> = turn.calls.iter().map(|c| c.seq).collect();
    let followed: HashSet<i64> = turn.calls.iter().filter_map(|c| c.parent).collect();
    let plain = |c: &Step| {
        !c.blocked
            && !c.held
            && !c.loose
            && !c.parent.is_some_and(|p| seqs.contains(&p))
            && !followed.contains(&c.seq)
            && Some(c.seq) != kept
    };
    let mut out = Vec::new();
    let mut from = 0;
    while from < turn.calls.len() {
        let run = turn.calls[from..].iter().take_while(|c| plain(c)).count();
        if turn.calls.len() > LONG && run >= RUN && !open.contains(&turn.calls[from].seq) {
            out.push(Unit::Many(&turn.calls[from..from + run]));
            from += run;
        } else {
            out.extend(turn.calls[from..from + run.max(1)].iter().map(Unit::One));
            from += run.max(1);
        }
    }
    out
}

/// The tools a run of Calls used, the most used first: "Write 8 · Bash 3 외 2".
fn tools(run: &[Step]) -> String {
    let mut count: Vec<(&str, usize)> = Vec::new();
    for c in run {
        match count.iter_mut().find(|(t, _)| *t == c.tool) {
            Some((_, n)) => *n += 1,
            None => count.push((&c.tool, 1)),
        }
    }
    count.sort_by_key(|t| std::cmp::Reverse(t.1));
    let mut text = count
        .iter()
        .take(3)
        .map(|(t, n)| format!("{t} {n}"))
        .collect::<Vec<_>>()
        .join(" · ");
    let rest: usize = count.iter().skip(3).map(|(_, n)| n).sum();
    if rest > 0 {
        text.push_str(&format!(" 외 {rest}"));
    }
    text
}

/// The session as the scene takes it: the streets, the avenue joining the turns, and the
/// buildings. Each turn's part of town starts at a corner where its request stands as a
/// signpost (carrying what the agent wrote it would do); the next turn's lies above, clear of
/// this one's. A Call the engine traced to the request stands on the nearest free lot to the
/// signpost; one it traced to an earlier Call on the nearest free lot to that Call. Each Call
/// something told carries the blocks on its way from the request (`way`: the request, the
/// Calls the engine's path comes through, the words of the result that told it, a small block
/// before the door of the Call that read them that shows only while lit, and itself) and the
/// way through the streets (`route`): from the signpost's corner to each of those Calls' doors
/// in turn and on to its own kerb. A block that can be picked carries what the scene sends
/// back for it (`pick`): a Call its seq, a request `ask_key`, a large building `many_key`.
///
/// Also which block each Call's seq is, and each turn's request under `ask_key`, to light the
/// one picked.
fn scene(legs: &[Leg], open: &HashSet<i64>, kept: Option<i64>) -> (String, HashMap<i64, usize>) {
    let mut nodes: Vec<serde_json::Value> = Vec::new();
    let mut at: HashMap<i64, usize> = HashMap::new();
    let mut roads: Vec<serde_json::Value> = Vec::new();
    let mut avenue: Vec<[f64; 2]> = Vec::new();
    // How far from the eye, up the page, the turns so far reach.
    let mut above = 0.0;
    for turn in legs {
        let t = turn.turn;
        let first = nodes.len();
        // Each Call's door, by its block.
        let mut doors: HashMap<usize, Door> = HashMap::new();
        // The words to stand, once the turn is: (the Call's block, the words, where).
        let mut told: Vec<(usize, Words, Pt)> = Vec::new();
        // The request, with what the agent wrote it would do about it, is a signpost beside
        // the corner the part of town starts from: where the way to every Call starts.
        let ask = nodes.len();
        let corner = Stop {
            at: (0.0, 0.0),
            upright: true,
        };
        let sign = (-1.2, 1.2);
        at.insert(ask_key(t), ask);
        nodes.push(serde_json::json!({
            "kind": "ask", "turn": t, "seq": null, "pick": ask_key(t), "state": null, "x": sign.0, "z": sign.1,
            "label": format!("요청 {}", turn.n),
            "tip": cut(&turn.req, 110),
            "intent": turn.intent.as_deref().map(|i| cut(i, 110)),
            "note": "사용자가 보낸 요청",
        }));
        let units = units(turn, open, kept);
        // About as many blocks across as up, for what is to stand.
        let lots: usize = units
            .iter()
            .map(|u| if matches!(u, Unit::Many(_)) { 4 } else { 1 })
            .sum();
        let mut town = Town::new(((lots as f64 / 4.0 * 1.3).sqrt().ceil() as usize).max(2));
        for unit in units {
            let call = match unit {
                Unit::One(call) => call,
                // A large building, a block to itself.
                Unit::Many(run) => {
                    let (lot, faces) = town.take_block();
                    let door = nearest_door(&faces, lot, corner, &[corner.at]);
                    let mut route = door.path.clone();
                    route.push(door.kerb);
                    nodes.push(serde_json::json!({
                        "kind": "many", "turn": t, "seq": null, "pick": many_key(run[0].seq), "state": null,
                        "x": lot.0, "z": lot.1, "count": run.len(), "route": route,
                        "label": format!("호출 {}개", run.len()),
                        "tip": tools(run),
                        "note": "요청을 따라 한 호출을 묶었습니다. 누르면 하나씩 펼쳐집니다",
                    }));
                    continue;
                }
            };
            // The Call it follows, when that one stands in this turn: it takes the lot nearest
            // that one's, and the way to it goes on from that one's door.
            let up = call
                .parent
                .and_then(|p| at.get(&p).copied())
                .filter(|&i| i >= first);
            let before = up.and_then(|i| doors.get(&i)).cloned();
            let plot = town.take(before.as_ref().map_or(sign, |b| b.lot));
            let door = match &before {
                Some(b) => nearest_door(&plot.doors, plot.at, b.stop, &b.path),
                None => nearest_door(&plot.doors, plot.at, corner, &[corner.at]),
            };
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
            // The blocks on its way from the request: the Calls the engine's path comes through.
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
                        .unwrap_or_else(|| vec![ask, i]),
                    None => vec![ask],
                };
                way.push(here);
                way
            });
            if let (Some(b), Some(w)) = (&before, &call.words) {
                // The words stand on the pavement before the door of the Call that read them.
                let along = if b.stop.upright {
                    (0.0, 0.8)
                } else {
                    (0.8, 0.0)
                };
                told.push((here, w.clone(), (b.kerb.0 + along.0, b.kerb.1 + along.1)));
            }
            // The way through the streets, ending at the kerb before its door.
            let mut route = door.path.clone();
            route.push(door.kerb);
            at.insert(call.seq, here);
            nodes.push(serde_json::json!({
                "kind": call.kind, "turn": t, "seq": call.seq, "pick": call.seq, "state": state,
                "x": plot.at.0, "z": plot.at.1,
                "label": call.tool, "tip": cut(&call.title, 90), "note": note, "way": way,
                "route": (!call.loose).then_some(route),
            }));
            doors.insert(here, door);
        }
        // The words, each before its door, and into the way of the Call they told just before it.
        for (call, words, place) in told {
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
            nodes.push(serde_json::json!({
                "kind": "words", "ghost": true, "turn": t, "seq": null, "state": null,
                "x": place.0, "z": place.1, "label": label,
                "tip": quote.unwrap_or(named),
                "note": note,
            }));
            nodes[call]["over"] = w.into();
        }
        // The streets: every line of the grid as far as the last block built on, each way,
        // so every street runs from one street to another. A turn with no Calls has none.
        let built: Vec<(usize, usize)> = town
            .plots
            .iter()
            .filter(|p| p.taken)
            .map(|p| p.block)
            .collect();
        let cols = built.iter().map(|b| b.0 + 1).max().unwrap_or(0);
        let rows = built.iter().map(|b| b.1 + 1).max().unwrap_or(0);
        let (wide, deep) = (cols as f64 * BLOCK, rows as f64 * BLOCK);
        let mut streets: Vec<[Pt; 2]> = Vec::new();
        if cols > 0 {
            streets
                .extend((0..=cols).map(|i| [(i as f64 * BLOCK, 0.0), (i as f64 * BLOCK, -deep)]));
            streets.extend(
                (0..=rows).map(|j| [(0.0, -(j as f64 * BLOCK)), (wide, -(j as f64 * BLOCK))]),
            );
        }
        // The turn moves away from the eye, up the page, until it is clear of the turns below.
        let near = |p: Pt| (p.0 + p.1) * TURN;
        let reach: Vec<f64> = [(0.0, 0.0), (wide, 0.0), (0.0, -deep), (wide, -deep), sign]
            .map(near)
            .to_vec();
        let (back, front) = (
            reach.iter().copied().fold(f64::INFINITY, f64::min) - KERB,
            reach.iter().copied().fold(f64::NEG_INFINITY, f64::max) + KERB,
        );
        let up = if first == 0 {
            0.0
        } else {
            above - BETWEEN - front
        };
        above = up + back;
        let shift = up * TURN;
        let moved = |p: Pt| [p.0 + shift, p.1 + shift];
        for n in &mut nodes[first..] {
            for key in ["x", "z"] {
                n[key] = (n[key].as_f64().unwrap_or(0.0) + shift).into();
            }
            if let Some(route) = n["route"].as_array() {
                let route: Vec<[f64; 2]> = route
                    .iter()
                    .map(|p| moved((p[0].as_f64().unwrap_or(0.0), p[1].as_f64().unwrap_or(0.0))))
                    .collect();
                n["route"] = serde_json::json!(route);
            }
        }
        for street in streets {
            roads.push(serde_json::json!({ "turn": t, "wide": WIDE, "points": street.map(moved) }));
        }
        avenue.push(moved(corner.at));
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
    let json = serde_json::json!({ "nodes": nodes, "roads": roads, "avenue": avenue }).to_string();
    (json, at)
}

/// The map in place of the route map: the session's blocks, a heading over them, and the
/// controls to zoom. A Call picked is the app's selection, as a station picked is; with no Call
/// picked, the request of the turn picked is the one marked. Picking a request on the map picks
/// its turn, lets the Call go and folds the turn's runs again; picking a large block opens it
/// into its Calls.
#[component]
pub fn RouteScene(
    flow: Arc<Flow>,
    turns: Vec<usize>,
    title: String,
    turn: Signal<usize>,
    detail: Signal<bool>,
) -> Element {
    let mut state = use_context::<AppState>();
    let mut detail = detail;
    let mut turn = turn;
    // The large blocks opened into their Calls, by the seq of their first.
    let mut open = use_signal(HashSet::<i64>::new);
    use_future(move || async move {
        let mut said = document::eval(
            "window.__routePick = (seq) => dioxus.send(seq); await new Promise(() => {});",
        );
        while let Ok(seq) = said.recv::<i64>().await {
            if seq <= MANY {
                open.write().insert(MANY - seq);
            } else if seq < 0 {
                turn.set((-seq - 1) as usize);
                state.selected.set(Selection::None);
                open.write().clear();
            } else {
                state.selected.set(Selection::Node(seq));
                detail.set(true);
            }
        }
    });
    let kept = match (state.selected)() {
        Selection::Node(seq) => Some(seq),
        Selection::None => None,
    };
    let (json, at) = scene(&legs(&flow, &turns), &open.read(), kept);
    let model = serde_json::to_string(&json).unwrap_or_else(|_| "\"{}\"".into());
    let chosen = match (state.selected)() {
        Selection::Node(seq) => at.get(&seq),
        Selection::None => at.get(&ask_key(turn())),
    }
    .map_or("null".to_owned(), |i| i.to_string());
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
                p { "요청 {turns.len()}개, 호출 {calls}개. 호출을 고르면 요청에서 그 호출까지 엔진이 고른 길을 안내합니다." }
            }
            div { class: "rt-keys",
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

    fn leg(turn: usize, calls: Vec<Step>) -> Leg {
        Leg {
            turn,
            n: turn + 1,
            req: "고쳐줘".into(),
            intent: Some("고친다".into()),
            calls,
        }
    }

    fn drawn(legs: &[Leg]) -> (serde_json::Value, HashMap<i64, usize>, String) {
        let (json, at) = scene(legs, &HashSet::new(), None);
        (serde_json::from_str(&json).unwrap(), at, json)
    }

    fn num(n: &serde_json::Value, key: &str) -> f64 {
        n[key].as_f64().unwrap()
    }

    fn pts(v: &serde_json::Value) -> Vec<Pt> {
        v.as_array().map_or(Vec::new(), |p| {
            p.iter()
                .map(|p| (p[0].as_f64().unwrap(), p[1].as_f64().unwrap()))
                .collect()
        })
    }

    fn way(n: &serde_json::Value) -> Vec<usize> {
        n["way"].as_array().map_or(Vec::new(), |w| {
            w.iter().map(|i| i.as_u64().unwrap() as usize).collect()
        })
    }

    #[test]
    fn the_way_through_the_streets_turns_at_crossings() {
        let at = |x: f64, z: f64, upright: bool| Stop {
            at: (x * BLOCK, z * BLOCK),
            upright,
        };
        // Along one stretch: straight there.
        assert_eq!(drive(at(0.0, -0.2, true), at(0.0, -0.7, true)).len(), 2);
        // Round a corner: to the crossing, and on.
        let way = drive(at(0.0, -0.5, true), at(0.5, -1.0, false));
        assert_eq!(
            way,
            [(0.0, -0.5 * BLOCK), (0.0, -BLOCK), (0.5 * BLOCK, -BLOCK)]
        );
        // Across town: every step along a street, the shortest of the ways.
        let way = drive(at(0.0, -0.5, true), at(2.5, -3.0, false));
        assert!(way.windows(2).all(|w| w[0].0 == w[1].0 || w[0].1 == w[1].1));
        assert!((long(&way) - 5.0 * BLOCK).abs() < 1e-9, "{way:?}");
    }

    #[test]
    fn every_building_stands_on_a_lot_and_its_way_keeps_to_the_streets() {
        // Every third Call follows the one before it; the rest follow the request.
        let turn = |turn: usize, first: i64| {
            leg(
                turn,
                (first..first + 40)
                    .map(|s| call(s, (s % 3 == 0 && s > first).then_some(s - 1), s == 5))
                    .collect(),
            )
        };
        let legs = [turn(0, 1), turn(1, 41)];
        let (v, at, json) = drawn(&legs);
        let nodes = v["nodes"].as_array().unwrap();
        assert_eq!(nodes.len(), 2 * 41, "each turn's request and forty Calls");
        assert_eq!((at[&ask_key(0)], at[&ask_key(1)]), (0, 41));
        assert_eq!(nodes[0]["intent"], "고친다");
        let corners = pts(&v["avenue"]);
        assert_eq!(corners.len(), 2);
        for (i, a) in nodes.iter().enumerate() {
            for b in &nodes[i + 1..] {
                let apart = (num(a, "x") - num(b, "x")).hypot(num(a, "z") - num(b, "z"));
                assert!(
                    apart >= 2.0 * ROOM,
                    "{} and {} stand {apart} apart",
                    a["label"],
                    b["label"]
                );
            }
        }
        let mut turned = 0;
        for n in nodes.iter().filter(|n| n["kind"] != "ask") {
            let corner = corners[n["turn"].as_u64().unwrap() as usize];
            let route = pts(&n["route"]);
            // From the turn's corner, every step along a street of the grid, to the kerb
            // before the building.
            assert_eq!(route[0], corner);
            let on_grid = |v: f64| ((v / BLOCK).round() * BLOCK - v).abs() < 1e-9;
            for w in route[..route.len() - 1].windows(2) {
                let (a, b) = (
                    (w[0].0 - corner.0, w[0].1 - corner.1),
                    (w[1].0 - corner.0, w[1].1 - corner.1),
                );
                assert!(
                    (a.0 == b.0 && on_grid(a.0)) || (a.1 == b.1 && on_grid(a.1)),
                    "{:?} leaves the streets",
                    n["seq"]
                );
            }
            let kerb = route[route.len() - 1];
            let far = (kerb.0 - num(n, "x")).hypot(kerb.1 - num(n, "z"));
            assert!(
                (far - PITCH / 2.0).abs() < 1e-9,
                "{:?}'s way ends {far} from it",
                n["seq"]
            );
            turned += usize::from(route.len() > 3);
            // The way to a Call that follows another goes by that one's door first.
            if way(n).len() > 2 {
                let before = pts(&nodes[way(n)[way(n).len() - 2]]["route"]);
                assert_eq!(route[..before.len() - 1], before[..before.len() - 1]);
            }
        }
        assert!(turned > 20, "only {turned} ways turn a corner");
        // Every street runs the whole of its part of town, from one street to another.
        let roads = v["roads"].as_array().unwrap();
        assert!(roads.len() >= 12);
        for r in roads {
            let line = pts(&r["points"]);
            assert!(long(&line) >= BLOCK - 1e-9);
        }
        // The second turn stands wholly further from the eye than the first, up the page, its
        // corner straight above the first's.
        let near = |n: &serde_json::Value| (num(n, "x") + num(n, "z")) * TURN;
        let (under, over) = nodes.split_at(41);
        let highest = under.iter().map(near).fold(f64::INFINITY, f64::min);
        assert!(over.iter().all(|n| near(n) < highest - 1.0));
        assert!((corners[1].0 - corners[1].1).abs() < 1e-9);
        assert_eq!(nodes[at[&5]]["state"], "blocked");
        assert_eq!(nodes[at[&5]]["pick"], 5);
        // The same session is laid out the same way each time.
        assert_eq!(drawn(&legs).2, json);
    }

    #[test]
    fn the_way_to_a_call_goes_by_the_doors_of_the_calls_the_engine_traced() {
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
        let (v, at, _) = drawn(&[leg(
            0,
            vec![
                call(1, None, false),
                call(2, None, false),
                third,
                call(4, Some(3), false),
                loose,
            ],
        )]);
        let nodes = v["nodes"].as_array().unwrap();
        let kinds: Vec<&str> = nodes.iter().map(|n| n["kind"].as_str().unwrap()).collect();
        assert_eq!(kinds, ["ask", "run", "run", "run", "run", "run", "words"]);
        let words = 6;
        assert_eq!(nodes[words]["ghost"], true);
        assert_eq!(way(&nodes[at[&1]]), [0, at[&1]]);
        assert_eq!(way(&nodes[at[&2]]), [0, at[&2]]);
        assert_eq!(way(&nodes[at[&3]]), [0, at[&1], words, at[&3]]);
        assert_eq!(way(&nodes[at[&4]]), [0, at[&1], words, at[&3], at[&4]]);
        assert!(
            way(&nodes[at[&5]]).is_empty() && nodes[at[&5]]["route"].is_null(),
            "a Call nothing told has no way to it"
        );
        let (one, three, four) = (
            pts(&nodes[at[&1]]["route"]),
            pts(&nodes[at[&3]]["route"]),
            pts(&nodes[at[&4]]["route"]),
        );
        assert_eq!(three[..one.len() - 1], one[..one.len() - 1]);
        assert_eq!(four[..three.len() - 1], three[..three.len() - 1]);
        assert!(nodes[words]["note"].as_str().unwrap().contains("90%"));
        // The words stand by the building of the Call that read them.
        let far = (num(&nodes[words], "x") - num(&nodes[at[&1]], "x"))
            .hypot(num(&nodes[words], "z") - num(&nodes[at[&1]], "z"));
        assert!(far < PITCH);

        // A Call following one of an earlier turn stands in its own turn's part of town.
        let (v, at, _) = drawn(&[
            leg(0, vec![call(1, None, false)]),
            leg(1, vec![call(2, Some(1), false)]),
        ]);
        let nodes = v["nodes"].as_array().unwrap();
        assert_eq!(way(&nodes[at[&1]]), [0, at[&1]]);
        assert_eq!(way(&nodes[at[&2]]), [2, at[&2]]);
        assert_eq!(pts(&nodes[at[&2]]["route"])[0], pts(&v["avenue"])[1]);

        // A turn with no Calls has no streets.
        let (v, _, _) = drawn(&[leg(0, vec![call(1, None, false)]), leg(1, Vec::new())]);
        assert!(
            v["roads"]
                .as_array()
                .unwrap()
                .iter()
                .all(|r| r["turn"] == 0)
        );
    }

    /// Every session of the demo data stands on the map with every block somewhere.
    #[test]
    fn every_demo_session_stands_whole() {
        let demo = crate::demo::Demo::load();
        for row in demo.sessions().as_array().unwrap() {
            let id = row["id"].as_str().unwrap();
            let graph = demo.graph(id).unwrap();
            let model = crate::layout::Model::build(&serde_json::from_value(graph).unwrap());
            let all: HashSet<i64> = model.nodes.iter().map(|n| n.seq()).collect();
            let flow = Flow::build(&crate::metro::Metro::build_with(&model, &all));
            let turns: Vec<usize> = (0..flow.turns.len()).collect();
            let (json, _) = scene(&legs(&flow, &turns), &HashSet::new(), None);
            let v: serde_json::Value = serde_json::from_str(&json).unwrap();
            for n in v["nodes"].as_array().unwrap() {
                for key in ["x", "z"] {
                    assert!(
                        n[key].as_f64().is_some_and(f64::is_finite),
                        "{id}: {key} of {} is {}",
                        n["label"],
                        n[key]
                    );
                }
            }
        }
    }

    #[test]
    fn a_long_turn_folds_its_plain_runs_into_large_buildings_that_open() {
        // Fourteen Calls: 1-5 plain, 6 stopped, 7-9 plain, 10 followed by 11, 12-14 plain.
        let calls = || -> Vec<Step> {
            (1..=14)
                .map(|s| call(s, (s == 11).then_some(10), s == 6))
                .collect()
        };
        let kinds = |open: &[i64], kept: Option<i64>| -> Vec<String> {
            let (json, _) = scene(&[leg(0, calls())], &open.iter().copied().collect(), kept);
            let v: serde_json::Value = serde_json::from_str(&json).unwrap();
            v["nodes"]
                .as_array()
                .unwrap()
                .iter()
                .map(|n| match n["kind"].as_str().unwrap() {
                    "many" => format!("many {}", n["count"]),
                    _ if n["seq"].is_i64() => n["seq"].to_string(),
                    kind => kind.to_owned(),
                })
                .collect()
        };
        assert_eq!(
            kinds(&[], None),
            ["ask", "many 5", "6", "many 3", "10", "11", "many 3"]
        );
        // The Call picked stands by itself, what is left of its run folding only if long enough.
        assert_eq!(
            kinds(&[], Some(3)),
            [
                "ask", "1", "2", "3", "4", "5", "6", "many 3", "10", "11", "many 3"
            ]
        );
        assert_eq!(
            kinds(&[], Some(1)),
            ["ask", "1", "many 4", "6", "many 3", "10", "11", "many 3"]
        );
        // A large building picked opens into its Calls.
        assert_eq!(
            kinds(&[7], None),
            ["ask", "many 5", "6", "7", "8", "9", "10", "11", "many 3"]
        );
        // A short turn folds nothing.
        let (v, _, _) = drawn(&[leg(0, (1..=10).map(|s| call(s, None, false)).collect())]);
        assert_eq!(v["nodes"].as_array().unwrap().len(), 11);

        // A large building is picked by its first Call's seq, has a block to itself, and says
        // what it holds.
        let (v, at, _) = drawn(&[leg(0, calls())]);
        let nodes = v["nodes"].as_array().unwrap();
        assert_eq!(nodes[1]["pick"], many_key(1));
        assert_eq!(
            (nodes[1]["label"].as_str(), nodes[1]["tip"].as_str()),
            (Some("호출 5개"), Some("Bash 5"))
        );
        assert!(!at.contains_key(&1) && at.contains_key(&6));
        for other in nodes.iter().skip(2) {
            let apart = (num(&nodes[1], "x") - num(other, "x"))
                .hypot(num(&nodes[1], "z") - num(other, "z"));
            assert!(apart >= LARGE + ROOM);
        }
    }
}
