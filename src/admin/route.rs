//! 기록 in the security team's app: a session as a town seen from above at an angle, its roads
//! the way the engine itself traced each Call back to the request. Each turn has a wide street
//! of its own that runs round the town's blocks, turning and doubling back, its request (with
//! what the agent wrote it would do, as one block) where it starts; the turns stand one above
//! the last, a road up the left joining their requests. Every Call is a building at the side
//! of a road, never on one. A Call the engine traced to the request stands along its turn's
//! street. A Call it traced to an earlier Call (the one whose result told it, or the last in
//! the SubGraph the engine chose as its parent) stands along a road that forks off beside that
//! Call's building: so each fork is a line of the agent's work that followed from one Call. In
//! a long turn, Calls in a row that only followed the request and were let through stand as
//! one large building, which opens into them when picked. A Call picked is drawn round in
//! black and the way to it from the request, fork by fork, is lit. Calls the engine stopped
//! stand on a red lot, those it held on an amber one.
//!
//! This file says which buildings and roads there are and where; three.js draws them
//! (`web/hub-scene/route.js`, bundled into `hub_scene.js`).

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use dioxus::prelude::*;

use crate::flow::{Call, Flow, Tone};
use crate::ui::{AppState, Selection};

/// The eye looks down a diagonal of the floor, so what runs left to right on the page runs
/// along the floor's other diagonal: a step of 1 that way is this much along x and back along z.
const TURN: f64 = std::f64::consts::FRAC_1_SQRT_2;
/// How far apart the street's points are given to the scene, in steps along it.
const SAMPLE: f64 = 0.5;
/// How much room a block takes around its middle, a large one more; how far a Call stands
/// from the one it follows; and how far apart the turns' streets keep their blocks.
const ROOM: f64 = 0.975;
const LARGE: f64 = 1.9;
/// Half the width of a turn's street and of a road forking off, and the pavement beside both.
const MAIN: f64 = 1.05;
const FORK: f64 = 0.75;
const PAVE: f64 = 0.32;
const BETWEEN: f64 = 1.7;
/// A turn with more Calls than this folds its plain runs, of this many Calls or more.
const LONG: usize = 10;
const RUN: usize = 3;
/// What a large block goes by where a Call goes by its seq: this, less its first Call's seq.
const MANY: i64 = -1_000_000_000_000;

type Pt = (f64, f64);

/// How far apart a turn's street runs from itself where it doubles back.
const ROW: f64 = 13.0;

/// Where a turn's street is on the floor `u` steps along it, and which way it is heading
/// there. It runs like a street through a town's blocks: `wide` along x, a turn and `ROW` back
/// along z, a turn and `wide` back the way it came, and so on, every stretch along the floor's
/// own lines.
fn street(u: f64, wide: f64) -> (Pt, Pt) {
    let lap = 2.0 * (wide + ROW);
    let (laps, on) = ((u / lap).floor(), u.rem_euclid(lap));
    let z = -laps * 2.0 * ROW;
    if on < wide {
        ((on, z), (1.0, 0.0))
    } else if on < wide + ROW {
        ((wide, z - (on - wide)), (0.0, -1.0))
    } else if on < 2.0 * wide + ROW {
        ((wide - (on - wide - ROW), z - ROW), (-1.0, 0.0))
    } else {
        ((0.0, z - ROW - (on - 2.0 * wide - ROW)), (0.0, -1.0))
    }
}

/// How wide a turn's street runs before it doubles back, for this many blocks along it: wide
/// enough that the turn is about as deep as it is wide.
fn width(blocks: usize) -> f64 {
    let long = 3.0 + 0.8 * blocks as f64;
    (ROW * long).sqrt().max(8.0).ceil()
}

/// The two ways off a street heading `along`: toward the eye, and away from it.
fn sides(along: Pt) -> [Pt; 2] {
    if along.0 == 0.0 {
        [(1.0, 0.0), (-1.0, 0.0)]
    } else {
        [(0.0, 1.0), (0.0, -1.0)]
    }
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

/// Which way three places turn: over zero to the left, under to the right.
fn turn_of(p: Pt, q: Pt, r: Pt) -> f64 {
    (q.0 - p.0) * (r.1 - p.1) - (q.1 - p.1) * (r.0 - p.0)
}

/// How far apart two stretches of road are; nothing when they cross.
fn apart(a: Pt, b: Pt, c: Pt, d: Pt) -> f64 {
    if turn_of(a, b, c) * turn_of(a, b, d) < 0.0 && turn_of(c, d, a) * turn_of(c, d, b) < 0.0 {
        return 0.0;
    }
    gap(a, c, d)
        .min(gap(b, c, d))
        .min(gap(c, a, b))
        .min(gap(d, a, b))
}

/// Where a building stands and how much room it takes.
#[derive(Clone, Copy, Debug)]
struct Lot {
    x: f64,
    z: f64,
    room: f64,
}

impl Lot {
    fn at(&self) -> Pt {
        (self.x, self.z)
    }
}

/// A building's place on its road: the point of the road it fronts, which way the road runs
/// there, which side of it the building is on, half the road's width, and the way along the
/// roads from the request to that point.
#[derive(Clone, Debug)]
struct Front {
    at: Pt,
    along: Pt,
    side: Pt,
    half: f64,
    path: Vec<Pt>,
    /// The road that forks off beside it, once a Call follows it: which road of the town,
    /// and how far it runs.
    fork: Option<(usize, f64)>,
}

/// What stands in one turn so far: the buildings, and the stretches of road with half their
/// widths.
#[derive(Default)]
struct Town {
    lots: Vec<Lot>,
    roads: Vec<(Pt, Pt, f64)>,
}

impl Town {
    /// Whether a building can stand here: clear of every building, and off every road by
    /// that road's pavement.
    fn lot_clear(&self, lot: Lot) -> bool {
        self.lots
            .iter()
            .all(|t| (lot.x - t.x).hypot(lot.z - t.z) >= lot.room + t.room)
            && self
                .roads
                .iter()
                .all(|&(a, b, half)| gap(lot.at(), a, b) >= half + PAVE + lot.room - 0.05)
    }

    /// The same for a building along the road numbered `own`, which is about to run past it.
    fn lot_clear_but(&self, lot: Lot, own: Option<usize>) -> bool {
        self.lots
            .iter()
            .all(|t| (lot.x - t.x).hypot(lot.z - t.z) >= lot.room + t.room)
            && self.roads.iter().enumerate().all(|(i, &(a, b, half))| {
                Some(i) == own || gap(lot.at(), a, b) >= half + PAVE + lot.room - 0.05
            })
    }

    /// Whether a stretch of road can run here: off every building by its pavement.
    fn road_clear(&self, a: Pt, b: Pt, half: f64) -> bool {
        self.lots
            .iter()
            .all(|t| gap(t.at(), a, b) >= half + PAVE + t.room - 0.05)
    }

    /// Whether a stretch of road keeps off every other road but the one numbered `own`.
    fn roads_clear(&self, a: Pt, b: Pt, half: f64, own: Option<usize>) -> bool {
        self.roads.iter().enumerate().all(|(i, &(c, d, other))| {
            Some(i) == own || apart(a, b, c, d) >= half + other + 2.0 * PAVE + 0.4
        })
    }
}

/// A place `far` on from another, the way given.
fn on(from: Pt, way: Pt, far: f64) -> Pt {
    (from.0 + way.0 * far, from.1 + way.1 * far)
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

/// The session as the scene takes it: the roads, and the buildings. Each turn's street starts
/// at its request (which carries what the agent wrote it would do) and runs round the town's
/// blocks; the next turn's lies above, clear of this one's buildings. A Call the engine traced
/// to the request stands at the side of the street; one it traced to an earlier Call stands
/// at the side of the road that forks off beside that Call's building. Each Call something
/// told carries the blocks on its way from the request (`way`: the request, the Calls whose
/// forks it is down, the words of the result that told it, a small block at the fork that
/// shows only while lit, and itself) and the way along the roads (`route`). A block that can
/// be picked carries what the scene sends back for it (`pick`): a Call its seq, a request
/// `ask_key`, a large building `many_key`.
///
/// Also which block each Call's seq is, and each turn's request under `ask_key`, to light the
/// one picked.
fn scene(legs: &[Leg], open: &HashSet<i64>, kept: Option<i64>) -> (String, HashMap<i64, usize>) {
    let mut nodes: Vec<serde_json::Value> = Vec::new();
    let mut at: HashMap<i64, usize> = HashMap::new();
    let mut roads: Vec<serde_json::Value> = Vec::new();
    // How far from the eye, up the page, the turns so far reach.
    let mut above = 0.0;
    for turn in legs {
        let t = turn.turn;
        let first = nodes.len();
        let mut town = Town::default();
        // Each Call's place on its road, by its block.
        let mut fronts: HashMap<usize, Front> = HashMap::new();
        // The words to stand, once the turn is: (the Call's block, the words, where).
        let mut told: Vec<(usize, Words, Pt)> = Vec::new();
        // The request and what the agent wrote it would do about it stand as one block where
        // the street starts: where the way to every Call of the turn starts.
        let ask = nodes.len();
        town.lots.push(Lot {
            x: 0.0,
            z: 0.0,
            room: 1.3,
        });
        at.insert(ask_key(t), ask);
        nodes.push(serde_json::json!({
            "kind": "ask", "turn": t, "seq": null, "pick": ask_key(t), "state": null, "x": 0.0, "z": 0.0,
            "label": format!("요청 {}", turn.n),
            "tip": cut(&turn.req, 110),
            "intent": turn.intent.as_deref().map(|i| cut(i, 110)),
            "note": "사용자가 보낸 요청",
        }));
        let units = units(turn, open, kept);
        // Only the buildings along the street itself take room along it.
        let seqs: HashSet<i64> = turn.calls.iter().map(|c| c.seq).collect();
        let wide = width(
            units
                .iter()
                .filter(|unit| !matches!(unit, Unit::One(c) if c.parent.is_some_and(|p| seqs.contains(&p))))
                .count(),
        );
        // The street's stretches, as far as the turn could reach: no building stands on them.
        let far = 8.0 + 2.5 * turn.calls.len() as f64 + 2.0 * (wide + ROW);
        let mut corner = street(0.0, wide).0;
        for k in 1..=(far / SAMPLE) as usize {
            let (next, _) = street(k as f64 * SAMPLE, wide);
            let (after, _) = street((k + 1) as f64 * SAMPLE, wide);
            let straight = |p: Pt| p.0 == corner.0 || p.1 == corner.1;
            if straight(next) && !straight(after) {
                town.roads.push((corner, next, MAIN));
                corner = next;
            }
        }
        // The street's points up to a place along it, for the way to a building there.
        let along_street = |u: f64| -> Vec<Pt> {
            (0..)
                .map(|k| k as f64 * SAMPLE)
                .take_while(|&s| s < u)
                .chain([u])
                .map(|s| street(s, wide).0)
                .collect()
        };
        let mut u = 3.4;
        let mut last = 0.0;
        for unit in units {
            let call = match unit {
                Unit::One(call) => call,
                // A large building, behind the street where it hides none of it.
                Unit::Many(run) => {
                    let (lot, front) = loop {
                        let (meets, along) = street(u, wide);
                        let side = sides(along)[1];
                        let place = on(meets, side, MAIN + PAVE + LARGE);
                        let lot = Lot {
                            x: place.0,
                            z: place.1,
                            room: LARGE,
                        };
                        if town.lot_clear(lot) {
                            break (lot, on(meets, side, MAIN + PAVE));
                        }
                        u += 0.45;
                    };
                    town.lots.push(lot);
                    let mut route = along_street(u);
                    route.push(front);
                    nodes.push(serde_json::json!({
                        "kind": "many", "turn": t, "seq": null, "pick": many_key(run[0].seq), "state": null,
                        "x": lot.x, "z": lot.z, "count": run.len(),
                        "label": format!("호출 {}개", run.len()),
                        "tip": tools(run),
                        "note": "요청을 따라 한 호출을 묶었습니다. 누르면 하나씩 펼쳐집니다",
                    }));
                    last = u;
                    u += 2.0;
                    continue;
                }
            };
            // The Call it follows, when that one stands in this turn.
            let up = call
                .parent
                .and_then(|p| at.get(&p).copied())
                .filter(|&i| i >= first);
            // A tall building stands only behind its road, where it hides none of it.
            let tall = matches!(call.kind, "change" | "outside" | "delegate");
            let nearer = |way: Pt| way.0 + way.1 > 0.0;
            // Down the road that forks off beside the Call it follows: at the first place
            // along it, on either side, that is clear. When there is none, it stands along the
            // street like a Call the request told, and only its way says what it follows.
            let forked = up
                .and_then(|i| Some((i, fronts.get(&i)?.clone())))
                .and_then(|(i, before)| {
                    let start = on(before.at, before.along, FORK + PAVE + ROOM);
                    let way = before.side;
                    let (road, far) = before.fork.unzip();
                    let found =
                        (0..16)
                            .flat_map(|k| [(k, 1.0), (k, -1.0)])
                            .find_map(|(k, hand)| {
                                let reach = before.half + PAVE + ROOM + 0.55 * k as f64;
                                let side = (before.along.0 * hand, before.along.1 * hand);
                                if tall && nearer(side) {
                                    return None;
                                }
                                let meets = on(start, way, reach);
                                let place = on(meets, side, FORK + PAVE + ROOM);
                                let lot = Lot {
                                    x: place.0,
                                    z: place.1,
                                    room: ROOM,
                                };
                                let end = reach.max(far.unwrap_or(0.0));
                                let clear = town.lot_clear_but(lot, road)
                                    && town.road_clear(
                                        on(start, way, before.half + PAVE),
                                        on(start, way, end),
                                        FORK,
                                    )
                                    && (end <= 3.0
                                        || town.roads_clear(
                                            on(start, way, 3.0),
                                            on(start, way, end),
                                            FORK,
                                            road,
                                        ));
                                clear.then_some((lot, meets, side, end))
                            })?;
                    Some((i, before, start, way, road, found))
                });
            let (lot, front) = match forked {
                Some((i, before, start, way, road, (lot, meets, side, end))) => {
                    // The fork runs as far as the furthest building along it.
                    let road = match road {
                        Some(road) => {
                            town.roads[road].1 = on(start, way, end);
                            road
                        }
                        None => {
                            town.roads.push((start, on(start, way, end), FORK));
                            town.roads.len() - 1
                        }
                    };
                    if let Some(parent) = fronts.get_mut(&i) {
                        parent.fork = Some((road, end));
                    }
                    if let Some(w) = &call.words {
                        // The words stand on the pavement at the corner where the fork leaves.
                        let corner = on(
                            on(start, before.along, -(FORK + PAVE / 2.0)),
                            way,
                            before.half + PAVE / 2.0,
                        );
                        told.push((nodes.len(), w.clone(), corner));
                    }
                    let mut path = before.path.clone();
                    path.extend([start, meets]);
                    (
                        lot,
                        Front {
                            at: meets,
                            along: way,
                            side,
                            half: FORK,
                            path,
                            fork: None,
                        },
                    )
                }
                // Along the street, on one side or the other as the Call's own number has it:
                // the next place that is clear.
                None => {
                    let first_side = (chance(call.seq, 1) * 2.0) as usize;
                    let (lot, meets, along, side) = loop {
                        let (meets, along) = street(u, wide);
                        let found = (0..2).find_map(|k| {
                            let side = sides(along)[(first_side + k) % 2];
                            if tall && nearer(side) {
                                return None;
                            }
                            let place = on(meets, side, MAIN + PAVE + ROOM);
                            let lot = Lot {
                                x: place.0,
                                z: place.1,
                                room: ROOM,
                            };
                            town.lot_clear(lot).then_some((lot, meets, along, side))
                        });
                        match found {
                            Some(found) => break found,
                            None => u += 0.45,
                        }
                    };
                    let path = along_street(u);
                    last = u;
                    u += 1.1 + 0.3 * chance(call.seq, 4);
                    (
                        lot,
                        Front {
                            at: meets,
                            along,
                            side,
                            half: MAIN,
                            path,
                            fork: None,
                        },
                    )
                }
            };
            town.lots.push(lot);
            let state = if call.blocked {
                Some("blocked")
            } else if call.held {
                Some("held")
            } else {
                None
            };
            let joined = forked_at(&front, up, &fronts);
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
            // The blocks on its way from the request: the Calls whose forks it is down.
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
            // The way along the roads, ending at the kerb before its door.
            let mut route = front.path.clone();
            route.push(on(front.at, front.side, front.half + PAVE));
            at.insert(call.seq, here);
            nodes.push(serde_json::json!({
                "kind": call.kind, "turn": t, "seq": call.seq, "pick": call.seq, "state": state,
                "x": lot.x, "z": lot.z, "forked": joined,
                "label": call.tool, "tip": cut(&call.title, 90), "note": note, "way": way,
                "route": (!call.loose).then_some(route),
            }));
            fronts.insert(here, front);
        }
        // The words, each at its corner, and into the way of the Call they told just before it.
        for (call, words, corner) in told {
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
                "x": corner.0, "z": corner.1, "label": label,
                "tip": quote.unwrap_or(named),
                "note": note,
            }));
            nodes[call]["over"] = w.into();
        }
        // The turn's roads: its street as far as the last building along it and no further,
        // so no stretch leads nowhere (a turn with no Calls has no street), and its forks.
        let mut mine: Vec<(Vec<Pt>, f64)> = Vec::new();
        if last > 0.0 {
            mine.push((along_street(last), MAIN));
        }
        mine.extend(
            town.roads
                .iter()
                .filter(|r| r.2 == FORK)
                .map(|&(a, b, half)| (vec![a, b], half)),
        );
        // The turn moves away from the eye, up the page, until it is clear of the turns below.
        let reach = |pick: fn(f64, f64) -> f64, start: f64| {
            town.lots.iter().fold(start, |far, l| {
                pick(far, (l.x + l.z) * TURN - l.room.copysign(start))
            })
        };
        let (back, front) = (
            reach(f64::min, f64::INFINITY),
            reach(f64::max, f64::NEG_INFINITY),
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
        for (points, half) in mine {
            let points: Vec<[f64; 2]> = points.into_iter().map(moved).collect();
            roads.push(serde_json::json!({ "turn": t, "wide": 2.0 * half, "points": points }));
        }
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
    let json = serde_json::json!({ "nodes": nodes, "roads": roads }).to_string();
    (json, at)
}

/// Whether a building stands down the fork of the Call it follows, not along the street.
fn forked_at(front: &Front, up: Option<usize>, fronts: &HashMap<usize, Front>) -> bool {
    up.and_then(|i| fronts.get(&i))
        .is_some_and(|before| front.half == FORK && front.path.len() > before.path.len())
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

    /// How near a place is to a road given as its points.
    fn off(p: Pt, road: &[Pt]) -> f64 {
        road.windows(2)
            .map(|w| gap(p, w[0], w[1]))
            .fold(f64::INFINITY, f64::min)
    }

    #[test]
    fn every_building_stands_beside_a_road_and_none_on_one() {
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
        let roads: Vec<(u64, f64, Vec<Pt>)> = v["roads"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| {
                (
                    r["turn"].as_u64().unwrap(),
                    r["wide"].as_f64().unwrap() / 2.0,
                    pts(&r["points"]),
                )
            })
            .collect();
        for n in nodes.iter().filter(|n| n["kind"] != "ask") {
            let here = (num(n, "x"), num(n, "z"));
            // Off every road by its pavement; right beside one.
            let mine = roads.iter().filter(|r| r.0 == n["turn"].as_u64().unwrap());
            let clear: Vec<f64> = mine
                .map(|(_, half, road)| off(here, road) - half - PAVE - ROOM)
                .collect();
            assert!(
                clear.iter().all(|c| *c > -0.06),
                "{} stands on a road: {clear:?}",
                n["seq"]
            );
            assert!(
                clear.iter().any(|c| c.abs() < 0.06),
                "{} stands beside no road: {clear:?}",
                n["seq"]
            );
            // Its way along the roads starts at the request and ends at its kerb.
            let route = pts(&n["route"]);
            let ask = &nodes[at[&ask_key(n["turn"].as_u64().unwrap() as usize)]];
            assert_eq!(route[0], (num(ask, "x"), num(ask, "z")));
            let kerb = route[route.len() - 1];
            assert!(((kerb.0 - here.0).hypot(kerb.1 - here.1) - ROOM).abs() < 1e-6);
        }
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
        // A Call that follows another stands down the fork beside it: the way to it goes
        // first to that one, then on down a road of the narrower kind. Most find room to.
        let followers: Vec<&serde_json::Value> =
            nodes.iter().filter(|n| way(n).len() > 2).collect();
        let forked: Vec<&&serde_json::Value> =
            followers.iter().filter(|n| n["forked"] == true).collect();
        assert!(
            forked.len() * 4 >= followers.len() * 3,
            "{} of {} forked",
            forked.len(),
            followers.len()
        );
        for n in forked {
            let before = &nodes[way(n)[way(n).len() - 2]];
            let (mine, theirs) = (pts(&n["route"]), pts(&before["route"]));
            assert_eq!(mine[..theirs.len() - 1], theirs[..theirs.len() - 1]);
            let here = (num(n, "x"), num(n, "z"));
            assert!(roads.iter().any(|(_, half, road)| *half == FORK
                && (off(here, road) - FORK - PAVE - ROOM).abs() < 0.06));
        }
        // The second turn stands wholly further from the eye than the first, up the page, its
        // request straight above the first's.
        let near = |n: &serde_json::Value| (num(n, "x") + num(n, "z")) * TURN;
        let (under, over) = nodes.split_at(41);
        let highest = under.iter().map(near).fold(f64::INFINITY, f64::min);
        assert!(over.iter().all(|n| near(n) < highest - 1.0));
        assert!((num(&nodes[41], "x") - num(&nodes[41], "z")).abs() < 1e-9);
        assert_eq!(nodes[at[&5]]["state"], "blocked");
        assert_eq!(nodes[at[&5]]["pick"], 5);
        // The same session is laid out the same way each time.
        assert_eq!(drawn(&legs).2, json);
    }

    #[test]
    fn the_way_to_a_call_goes_down_the_forks_the_engine_traced() {
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
        // 3 stands down the fork beside 1, and 4 down the fork beside 3: two forks, each way
        // going on from the last.
        assert_eq!(
            (
                nodes[at[&3]]["forked"].as_bool(),
                nodes[at[&4]]["forked"].as_bool()
            ),
            (Some(true), Some(true))
        );
        assert_eq!(
            v["roads"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|r| r["wide"] == 2.0 * FORK)
                .count(),
            2
        );
        let (one, three, four) = (
            pts(&nodes[at[&1]]["route"]),
            pts(&nodes[at[&3]]["route"]),
            pts(&nodes[at[&4]]["route"]),
        );
        assert_eq!(three[..one.len() - 1], one[..one.len() - 1]);
        assert_eq!(four[..three.len() - 1], three[..three.len() - 1]);
        assert!(nodes[words]["note"].as_str().unwrap().contains("90%"));

        // A Call following one of an earlier turn stands along its own turn's street.
        let (v, at, _) = drawn(&[
            leg(0, vec![call(1, None, false)]),
            leg(1, vec![call(2, Some(1), false)]),
        ]);
        let nodes = v["nodes"].as_array().unwrap();
        assert_eq!(way(&nodes[at[&1]]), [0, at[&1]]);
        assert_eq!(nodes[at[&2]]["forked"], false);
        assert_eq!(way(&nodes[at[&2]]), [2, at[&2]]);

        // A turn's street ends at its last building; a turn with no Calls has none.
        let (v, at, _) = drawn(&[
            leg(0, vec![call(1, None, false), call(2, None, false)]),
            leg(1, Vec::new()),
        ]);
        let roads = v["roads"].as_array().unwrap();
        assert_eq!(roads.len(), 1);
        let end = *pts(&roads[0]["points"]).last().unwrap();
        let last = pts(&v["nodes"][at[&2]]["route"]);
        assert_eq!(end, last[last.len() - 2]);
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

        // A large building is picked by its first Call's seq, takes more room, and says what it holds.
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
