//! 기록 in the security team's app: a session laid on the map of a real part of town, the way
//! through its streets the way the engine itself traced each Call back to the request. The
//! streets and buildings are those of a few blocks of Seongsu-dong, Seoul, from OpenStreetMap
//! (`district.json`, made by `web/hub-scene/make_district.py`). Each turn starts at a crossing
//! of its own, where its request (with what the agent wrote it would do) is marked; turns
//! start well apart. Every Call is one of the town's buildings. A Call the engine traced to
//! the request takes the nearest free building to the turn's crossing; one it traced to an
//! earlier Call (the one whose result told it, or the last in the SubGraph the engine chose
//! as its parent) takes the nearest free building to that Call's. In a long turn, Calls in a
//! row that only followed the request and were let through are one building between them,
//! which opens into them when picked. The way to a Call is the shortest way along the streets
//! from the turn's crossing to the door of each Call the engine's path comes through, in
//! turn, and on to the Call's own: what a route finder would give.
//!
//! This file says which building each Call is and what its way is; MapLibre draws the town
//! and the way (`web/hub-scene/town.js`, bundled into `hub_scene.js`).

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, OnceLock};

use dioxus::prelude::*;

use crate::flow::{Call, Flow, Tone};
use crate::ui::{AppState, Selection};

/// A turn with more Calls than this folds its plain runs, of this many Calls or more.
const LONG: usize = 10;
const RUN: usize = 3;
/// What a folded run goes by where a Call goes by its seq: this, less its first Call's seq.
const MANY: i64 = -1_000_000_000_000;
/// A building further than this from every street (in metres) has no door a way can reach.
const REACH: f64 = 45.0;

/// A place in the town, in metres east and north of its middle.
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

/// How far a place is from the stretch of street between two others, and the place on that
/// stretch nearest it.
fn nearest(p: Pt, a: Pt, b: Pt) -> (f64, Pt) {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let long = dx * dx + dy * dy;
    let t = if long == 0.0 {
        0.0
    } else {
        (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / long).clamp(0.0, 1.0)
    };
    let at = (a.0 + t * dx, a.1 + t * dy);
    ((p.0 - at.0).hypot(p.1 - at.1), at)
}

fn far(a: Pt, b: Pt) -> f64 {
    (a.0 - b.0).hypot(a.1 - b.1)
}

/// A place on a street: on the stretch between two of the town's crossings or bends (the same
/// one twice, for a place that is one), where, and how far along the stretch from each end.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Door {
    ends: (usize, usize),
    at: Pt,
    from: (f64, f64),
}

/// The town as the file has it.
#[derive(serde::Deserialize)]
struct Drawn {
    centre: [f64; 2],
    roads: Vec<DrawnRoad>,
    buildings: Vec<DrawnBuilding>,
}

#[derive(serde::Deserialize)]
struct DrawnRoad {
    points: Vec<[f64; 2]>,
}

#[derive(serde::Deserialize)]
struct DrawnBuilding {
    ring: Vec<[f64; 2]>,
}

/// A building a Call can be: which of the file's it is, its middle, how much ground it covers
/// (in square metres), and its door on the nearest street.
#[derive(Clone, Debug)]
struct Building {
    id: usize,
    mid: Pt,
    ground: f64,
    door: Door,
}

/// The town: where its middle is on the globe, the streets' crossings and bends with the
/// stretches between them, the buildings with a door on a street, and the crossings turns
/// start from, each as far from those before it as can be.
struct District {
    centre: [f64; 2],
    places: Vec<Pt>,
    next: Vec<Vec<(usize, f64)>>,
    buildings: Vec<Building>,
    starts: Vec<usize>,
}

/// Metres to a degree east and north, at the town's latitude.
const EAST: f64 = 88_270.0;
const NORTH: f64 = 110_950.0;

impl District {
    fn load() -> &'static District {
        static TOWN: OnceLock<District> = OnceLock::new();
        TOWN.get_or_init(|| District::read(include_str!("district.json")))
    }

    fn read(file: &str) -> District {
        let drawn: Drawn = serde_json::from_str(file).expect("the district file");
        let centre = drawn.centre;
        let flat = |p: &[f64; 2]| ((p[0] - centre[0]) * EAST, (p[1] - centre[1]) * NORTH);
        // The streets' points, one for each place whatever streets share it.
        let mut index: HashMap<(i64, i64), usize> = HashMap::new();
        let mut places: Vec<Pt> = Vec::new();
        let mut next: Vec<Vec<(usize, f64)>> = Vec::new();
        for road in &drawn.roads {
            let ids: Vec<usize> = road
                .points
                .iter()
                .map(|p| {
                    *index
                        .entry(((p[0] * 1e6).round() as i64, (p[1] * 1e6).round() as i64))
                        .or_insert_with(|| {
                            places.push(flat(p));
                            next.push(Vec::new());
                            places.len() - 1
                        })
                })
                .collect();
            for w in ids.windows(2).filter(|w| w[0] != w[1]) {
                let long = far(places[w[0]], places[w[1]]);
                next[w[0]].push((w[1], long));
                next[w[1]].push((w[0], long));
            }
        }
        // Only the streets that join up with the middle of town: the rest lead nowhere.
        let home = (0..places.len())
            .min_by(|&a, &b| far(places[a], (0.0, 0.0)).total_cmp(&far(places[b], (0.0, 0.0))))
            .expect("a street");
        let mut joined = vec![false; places.len()];
        let mut edge = vec![home];
        joined[home] = true;
        while let Some(at) = edge.pop() {
            for &(to, _) in &next[at] {
                if !joined[to] {
                    joined[to] = true;
                    edge.push(to);
                }
            }
        }
        for (i, links) in next.iter_mut().enumerate() {
            if !joined[i] {
                links.clear();
            }
        }
        // Each building's door: the place on a street nearest its middle.
        let buildings = drawn
            .buildings
            .iter()
            .enumerate()
            .filter_map(|(id, b)| {
                let ring: Vec<Pt> = b.ring.iter().map(flat).collect();
                let n = (ring.len() - 1).max(1) as f64;
                let mid = (
                    ring[..ring.len() - 1].iter().map(|p| p.0).sum::<f64>() / n,
                    ring[..ring.len() - 1].iter().map(|p| p.1).sum::<f64>() / n,
                );
                let ground = ring
                    .windows(2)
                    .map(|w| w[0].0 * w[1].1 - w[1].0 * w[0].1)
                    .sum::<f64>()
                    .abs()
                    / 2.0;
                let (off, door) = (0..places.len())
                    .flat_map(|a| {
                        next[a]
                            .iter()
                            .filter(move |(b, _)| a < *b)
                            .map(move |&(b, long)| (a, b, long))
                    })
                    .map(|(a, b, long)| {
                        let (off, at) = nearest(mid, places[a], places[b]);
                        let along = far(at, places[a]);
                        (
                            off,
                            Door {
                                ends: (a, b),
                                at,
                                from: (along, long - along),
                            },
                        )
                    })
                    .min_by(|a, b| a.0.total_cmp(&b.0))?;
                (off <= REACH).then_some(Building {
                    id,
                    mid,
                    ground,
                    door,
                })
            })
            .collect();
        // Where turns start: crossings, each the furthest from those already chosen.
        let crossings: Vec<usize> = (0..places.len()).filter(|&i| next[i].len() >= 3).collect();
        let mut starts = vec![
            *crossings
                .iter()
                .min_by(|&&a, &&b| {
                    far(places[a], (0.0, 0.0)).total_cmp(&far(places[b], (0.0, 0.0)))
                })
                .expect("a crossing"),
        ];
        while starts.len() < crossings.len().min(48) {
            let apart = |c: usize| {
                starts
                    .iter()
                    .map(|&s| far(places[c], places[s]))
                    .fold(f64::INFINITY, f64::min)
            };
            let furthest = crossings
                .iter()
                .copied()
                .max_by(|&a, &b| apart(a).total_cmp(&apart(b)))
                .expect("a crossing");
            starts.push(furthest);
        }
        District {
            centre,
            places,
            next,
            buildings,
            starts,
        }
    }

    /// A place on the globe, as the map takes it: degrees east and north.
    fn globe(&self, p: Pt) -> [f64; 2] {
        let round = |v: f64| (v * 1e6).round() / 1e6;
        [
            round(self.centre[0] + p.0 / EAST),
            round(self.centre[1] + p.1 / NORTH),
        ]
    }

    /// A crossing as a door: a place on a street that is the end of its stretch.
    fn crossing(&self, at: usize) -> Door {
        Door {
            ends: (at, at),
            at: self.places[at],
            from: (0.0, 0.0),
        }
    }

    /// The shortest way along the streets from one place on them to another.
    fn drive(&self, a: Door, b: Door) -> Vec<Pt> {
        let same = |x: (usize, usize), y: (usize, usize)| x == y || x == (y.1, y.0);
        if same(a.ends, b.ends) {
            return vec![a.at, b.at];
        }
        let mut reach = vec![f64::INFINITY; self.places.len()];
        let mut via: Vec<Option<usize>> = vec![None; self.places.len()];
        let mut done = vec![false; self.places.len()];
        reach[a.ends.0] = a.from.0;
        reach[a.ends.1] = reach[a.ends.1].min(a.from.1);
        while let Some(at) = (0..reach.len())
            .filter(|&i| !done[i] && reach[i].is_finite())
            .min_by(|&x, &y| reach[x].total_cmp(&reach[y]))
        {
            done[at] = true;
            for &(to, long) in &self.next[at] {
                if reach[at] + long < reach[to] {
                    reach[to] = reach[at] + long;
                    via[to] = Some(at);
                }
            }
        }
        let end = if reach[b.ends.0] + b.from.0 <= reach[b.ends.1] + b.from.1 {
            b.ends.0
        } else {
            b.ends.1
        };
        if !reach[end].is_finite() {
            return vec![a.at, b.at];
        }
        let mut way = vec![b.at];
        let mut at = Some(end);
        while let Some(i) = at {
            way.push(self.places[i]);
            at = via[i];
        }
        way.push(a.at);
        way.reverse();
        way.dedup();
        way
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

/// The session as the scene takes it: each turn's request at the crossing it starts from, and
/// each Call as one of the town's buildings (`building`: which of the district file's it is).
/// A Call the engine traced to the request is the nearest free building to the turn's
/// crossing; one it traced to an earlier Call, the nearest free building to that Call's. Each
/// Call something told carries the blocks on its way from the request (`way`: the request,
/// the Calls the engine's path comes through, the words of the result that told it, a mark at
/// the door of the Call that read them that shows only while lit, and itself) and the way
/// along the streets (`route`, as degrees east and north): from the turn's crossing to each of
/// those Calls' doors in turn and on to its own. A block that can be picked carries what the
/// scene sends back for it (`pick`): a Call its seq, a request `ask_key`, a folded run
/// `many_key`.
///
/// Also which block each Call's seq is, and each turn's request under `ask_key`, to light the
/// one picked.
fn scene(legs: &[Leg], open: &HashSet<i64>, kept: Option<i64>) -> (String, HashMap<i64, usize>) {
    let town = District::load();
    let mut nodes: Vec<serde_json::Value> = Vec::new();
    let mut at: HashMap<i64, usize> = HashMap::new();
    // The buildings Calls already are, the session over.
    let mut taken: HashSet<usize> = HashSet::new();
    // The free building nearest a place, taken: a big one when one is asked for and near.
    let mut take = |near: Pt, big: bool| -> Option<Building> {
        let free = || town.buildings.iter().filter(|b| !taken.contains(&b.id));
        let nearest = |list: Vec<&Building>| {
            list.into_iter()
                .min_by(|a, b| far(a.mid, near).total_cmp(&far(b.mid, near)))
                .cloned()
        };
        let found = big
            .then(|| {
                nearest(
                    free()
                        .filter(|b| b.ground >= 260.0 && far(b.mid, near) < 160.0)
                        .collect(),
                )
            })
            .flatten()
            .or_else(|| nearest(free().collect()))
            // More Calls than the town has buildings: the nearest, shared.
            .or_else(|| nearest(town.buildings.iter().collect()))?;
        taken.insert(found.id);
        Some(found)
    };
    let line = |way: &[Pt]| -> Vec<[f64; 2]> { way.iter().map(|&p| town.globe(p)).collect() };
    for (k, turn) in legs.iter().enumerate() {
        let t = turn.turn;
        let first = nodes.len();
        // The way so far to each Call's door, by its block.
        let mut ways: HashMap<usize, (Building, Vec<Pt>)> = HashMap::new();
        // The words to mark, once the turn is: (the Call's block, the words, where).
        let mut told: Vec<(usize, Words, Pt)> = Vec::new();
        // The request, with what the agent wrote it would do about it, is marked at the
        // crossing the turn starts from: where the way to every Call of it starts.
        let start = town.starts[k % town.starts.len()];
        let corner = town.crossing(start);
        let ask = nodes.len();
        at.insert(ask_key(t), ask);
        nodes.push(serde_json::json!({
            "kind": "ask", "turn": t, "seq": null, "pick": ask_key(t), "state": null,
            "at": town.globe(corner.at),
            "label": format!("요청 {}", turn.n),
            "tip": cut(&turn.req, 110),
            "intent": turn.intent.as_deref().map(|i| cut(i, 110)),
            "note": "사용자가 보낸 요청",
        }));
        for unit in units(turn, open, kept) {
            let call = match unit {
                Unit::One(call) => call,
                // A run of Calls as one building, a big one where there is one near.
                Unit::Many(run) => {
                    let Some(b) = take(corner.at, true) else {
                        continue;
                    };
                    let route = town.drive(corner, b.door);
                    nodes.push(serde_json::json!({
                        "kind": "many", "turn": t, "seq": null, "pick": many_key(run[0].seq), "state": null,
                        "building": b.id, "at": town.globe(b.mid), "count": run.len(), "route": line(&route),
                        "label": format!("호출 {}개", run.len()),
                        "tip": tools(run),
                        "note": "요청을 따라 한 호출을 묶었습니다. 누르면 하나씩 펼쳐집니다",
                    }));
                    continue;
                }
            };
            // The Call it follows, when that one is in this turn: it is the building nearest
            // that one's, and the way to it goes on from that one's door.
            let up = call
                .parent
                .and_then(|p| at.get(&p).copied())
                .filter(|&i| i >= first);
            let before = up.and_then(|i| ways.get(&i)).cloned();
            let Some(b) = take(before.as_ref().map_or(corner.at, |(b, _)| b.mid), false) else {
                continue;
            };
            let route = match &before {
                Some((from, so_far)) => {
                    let mut way = so_far.clone();
                    way.extend(town.drive(from.door, b.door).into_iter().skip(1));
                    way
                }
                None => town.drive(corner, b.door),
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
            if let (Some((from, _)), Some(w)) = (&before, &call.words) {
                // The words are marked at the door of the Call that read them.
                told.push((here, w.clone(), from.door.at));
            }
            at.insert(call.seq, here);
            nodes.push(serde_json::json!({
                "kind": call.kind, "turn": t, "seq": call.seq, "pick": call.seq, "state": state,
                "building": b.id, "at": town.globe(b.mid),
                "label": call.tool, "tip": cut(&call.title, 90), "note": note, "way": way,
                "route": (!call.loose).then(|| line(&route)),
            }));
            ways.insert(here, (b, route));
        }
        // The words, each at its door, and into the way of the Call they told just before it.
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
                "at": town.globe(place), "label": label,
                "tip": quote.unwrap_or(named),
                "note": note,
            }));
            nodes[call]["over"] = w.into();
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
    let json = serde_json::json!({ "nodes": nodes }).to_string();
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

    /// A way as the scene gives it, back in metres from the middle of town.
    fn pts(v: &serde_json::Value) -> Vec<Pt> {
        let c = District::load().centre;
        v.as_array().map_or(Vec::new(), |p| {
            p.iter()
                .map(|p| {
                    (
                        (p[0].as_f64().unwrap() - c[0]) * EAST,
                        (p[1].as_f64().unwrap() - c[1]) * NORTH,
                    )
                })
                .collect()
        })
    }

    fn way(n: &serde_json::Value) -> Vec<usize> {
        n["way"].as_array().map_or(Vec::new(), |w| {
            w.iter().map(|i| i.as_u64().unwrap() as usize).collect()
        })
    }

    /// How far a place is from the nearest street.
    fn off_street(p: Pt) -> f64 {
        let town = District::load();
        (0..town.places.len())
            .flat_map(|a| town.next[a].iter().map(move |&(b, _)| (a, b)))
            .map(|(a, b)| nearest(p, town.places[a], town.places[b]).0)
            .fold(f64::INFINITY, f64::min)
    }

    #[test]
    fn the_town_is_streets_that_join_up_and_buildings_with_doors_on_them() {
        let town = District::load();
        let joined = town.next.iter().filter(|n| !n.is_empty()).count();
        assert!(joined > 200, "{joined} places on streets that join up");
        assert!(
            town.buildings.len() > 400,
            "{} buildings with a door",
            town.buildings.len()
        );
        assert!(
            town.starts.len() >= 20,
            "{} crossings to start from",
            town.starts.len()
        );
        // Turns start well apart: the first few crossings are a hundred metres and more apart.
        for (i, &a) in town.starts[..6].iter().enumerate() {
            for &b in &town.starts[i + 1..6] {
                assert!(far(town.places[a], town.places[b]) > 100.0);
            }
        }
        // A way across town keeps to the streets and is not much longer than it need be.
        let (a, b) = (town.crossing(town.starts[0]), town.crossing(town.starts[1]));
        let way = town.drive(a, b);
        assert_eq!((way[0], way[way.len() - 1]), (a.at, b.at));
        for w in way.windows(2) {
            let (i, j) = (
                town.places.iter().position(|p| *p == w[0]).unwrap(),
                town.places.iter().position(|p| *p == w[1]).unwrap(),
            );
            assert!(
                town.next[i].iter().any(|&(to, _)| to == j),
                "no street from {i} to {j}"
            );
        }
        let long: f64 = way.windows(2).map(|w| far(w[0], w[1])).sum();
        assert!(
            long < 2.2 * far(a.at, b.at),
            "{long} m for {} m as the crow flies",
            far(a.at, b.at)
        );
    }

    #[test]
    fn every_call_is_a_building_of_its_own_and_its_way_keeps_to_the_streets() {
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
        let mut used = HashSet::new();
        for n in nodes.iter().filter(|n| n["kind"] != "ask") {
            assert!(
                used.insert(n["building"].as_u64().unwrap()),
                "two Calls in one building"
            );
            let route = pts(&n["route"]);
            // From the turn's request, every place on a street, to the building's door.
            let ask = pts(&serde_json::json!([
                nodes[at[&ask_key(n["turn"].as_u64().unwrap() as usize)]]["at"]
            ]))[0];
            assert!(far(route[0], ask) < 0.2);
            assert!(
                route.iter().all(|&p| off_street(p) < 0.3),
                "{} leaves the streets",
                n["seq"]
            );
            let door = route[route.len() - 1];
            let home = pts(&serde_json::json!([n["at"]]))[0];
            assert!(far(door, home) <= REACH + 0.2);
            // The way to a Call that follows another goes by that one's door first.
            if way(n).len() > 2 {
                let before = pts(&nodes[way(n)[way(n).len() - 2]]["route"]);
                assert_eq!(route[..before.len()], before[..]);
            }
        }
        // The two turns start at crossings well apart, and each one's Calls are near its own.
        let starts = pts(&serde_json::json!([nodes[0]["at"], nodes[41]["at"]]));
        assert!(far(starts[0], starts[1]) > 100.0);
        for (t, own) in [(0usize, starts[0]), (1, starts[1])] {
            let mean = nodes
                .iter()
                .filter(|n| n["turn"] == t && n["kind"] != "ask")
                .map(|n| far(pts(&serde_json::json!([n["at"]]))[0], own))
                .sum::<f64>()
                / 40.0;
            assert!(
                mean < 150.0,
                "turn {t}'s Calls are {mean} m from its request on average"
            );
        }
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
        assert_eq!(three[..one.len()], one[..]);
        assert_eq!(four[..three.len()], three[..]);
        assert!(nodes[words]["note"].as_str().unwrap().contains("90%"));
        // The words are marked at the door of the Call that read them: where its way ends.
        assert_eq!(
            pts(&serde_json::json!([nodes[words]["at"]]))[0],
            one[one.len() - 1]
        );

        // A Call following one of an earlier turn starts from its own turn's request.
        let (v, at, _) = drawn(&[
            leg(0, vec![call(1, None, false)]),
            leg(1, vec![call(2, Some(1), false)]),
        ]);
        let nodes = v["nodes"].as_array().unwrap();
        assert_eq!(way(&nodes[at[&1]]), [0, at[&1]]);
        assert_eq!(way(&nodes[at[&2]]), [2, at[&2]]);
        assert_eq!(nodes[at[&2]]["route"][0], nodes[2]["at"]);
    }

    /// Every session of the demo data stands on the map with every Call a building.
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
            let (json, at) = scene(&legs(&flow, &turns), &HashSet::new(), None);
            let v: serde_json::Value = serde_json::from_str(&json).unwrap();
            let nodes = v["nodes"].as_array().unwrap();
            assert!(
                nodes
                    .iter()
                    .all(|n| n["at"][0].as_f64().is_some_and(f64::is_finite)),
                "{id}"
            );
            assert!(at.len() >= flow.turns.len(), "{id}");
        }
    }

    #[test]
    fn a_long_turn_folds_its_plain_runs_into_one_building_that_opens() {
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
        // A folded run picked opens into its Calls.
        assert_eq!(
            kinds(&[7], None),
            ["ask", "many 5", "6", "7", "8", "9", "10", "11", "many 3"]
        );
        // A short turn folds nothing.
        let (v, _, _) = drawn(&[leg(0, (1..=10).map(|s| call(s, None, false)).collect())]);
        assert_eq!(v["nodes"].as_array().unwrap().len(), 11);

        // A folded run is picked by its first Call's seq, and says what it holds.
        let (v, at, _) = drawn(&[leg(0, calls())]);
        let nodes = v["nodes"].as_array().unwrap();
        assert_eq!(nodes[1]["pick"], many_key(1));
        assert_eq!(
            (nodes[1]["label"].as_str(), nodes[1]["tip"].as_str()),
            (Some("호출 5개"), Some("Bash 5"))
        );
        assert!(!at.contains_key(&1) && at.contains_key(&6));
    }
}
