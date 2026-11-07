//! The session as a metro map of what told the agent what to do.
//!
//! A station is a place the agents went to: a file or a page, a command, a kind of search.
//! Calls to one place are one station, whoever made them and whenever. A line is what told the
//! agent to go there: each turn's request is a line, and so are the words in a tool result that
//! the engine traced a later Call to. The words' line leaves the station where they were read.
//! Lines meet where two of them sent the agent to one place, and those stations are the
//! transfers.
//!
//! The way to a Call is the engine's own: from the turn's request along its line, and where
//! the Call followed words in a result, changing at the station they were read at onto the
//! words' line.

use std::collections::{HashMap, HashSet};

use crate::layout::Model;
use crate::metro::{FIRST, LEFT, MAIN_Y, Metro, STEP, Station, place_key, place_label, rounded};

/// The space between two rows of lines.
pub const ROW_GAP: f64 = 110.0;
/// How far beside a station's own line another line runs through it, a row of difference apart.
pub const BESIDE: f64 = 7.0;

/// What told the agent to go somewhere.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Through {
    /// A turn's request, by the turn's index.
    Turn(usize),
    /// The words in the result of a Call, by the Call.
    Told(i64),
}

#[derive(Clone, Debug, PartialEq)]
pub struct NetStation {
    /// The place, as the stations are told apart.
    pub key: String,
    pub x: f64,
    pub y: f64,
    pub row: usize,
    /// Its Calls, in order.
    pub calls: Vec<i64>,
    /// The gravest of its Calls, placed and named as the station.
    pub rep: Station,
    /// The lines through it, by index.
    pub lines: Vec<usize>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NetLine {
    pub through: Through,
    pub row: usize,
    /// Its stations, left to right.
    pub stations: Vec<usize>,
    /// Where it is drawn: its terminal (or, for the words' line, the station the words were
    /// read at), then a point at each station, with the corners between.
    pub points: Vec<(f64, f64)>,
    /// For each of its stations, which point is that station's.
    pub at: Vec<usize>,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Net {
    pub stations: Vec<NetStation>,
    pub lines: Vec<NetLine>,
    pub width: f64,
    pub top: f64,
    pub bottom: f64,
    /// Each Call's station.
    station_of: HashMap<i64, usize>,
    /// Each Call's line; none when the engine traced it to nothing.
    line_of: HashMap<i64, usize>,
    /// The Calls as the call map has them, for their sources.
    calls: HashMap<i64, Station>,
}

impl Net {
    pub fn build(model: &Model) -> Self {
        let all: HashSet<i64> = model.nodes.iter().map(|n| n.seq()).collect();
        let m = Metro::build_with(model, &all);
        let mut order: Vec<&Station> = m.stations.iter().collect();
        order.sort_by(|a, b| a.x.total_cmp(&b.x));
        let turn_of = |s: &Station| m.asks.iter().rposition(|a| a.x <= s.x).unwrap_or(0);
        let through = |s: &Station| -> Option<Through> {
            match &s.source {
                Some(src) if src.by != s.seq && m.station(src.by).is_some() => {
                    Some(Through::Told(src.by))
                }
                _ if s.loose || s.unsourced => None,
                _ => Some(Through::Turn(turn_of(s))),
            }
        };

        // The places, in the order they were first gone to, and the lines in the order they
        // first sent the agent anywhere.
        let mut keys: Vec<String> = Vec::new();
        let mut calls_at: HashMap<String, Vec<&Station>> = HashMap::new();
        let mut throughs: Vec<Through> = Vec::new();
        for s in &order {
            let key = place_key(s);
            if !keys.contains(&key) {
                keys.push(key.clone());
            }
            calls_at.entry(key).or_default().push(s);
            if let Some(t) = through(s)
                && !throughs.contains(&t)
            {
                throughs.push(t);
            }
        }
        let row_of_line: HashMap<Through, usize> =
            throughs.iter().enumerate().map(|(i, t)| (*t, i)).collect();
        // A place no line went to sits in a row of its own under the rest.
        let lost_row = throughs.len();

        // Each station: on the row of the line that first went there, spaced along its row
        // for the names, and never before a place gone to earlier.
        let mut stations: Vec<NetStation> = Vec::new();
        let mut station_of: HashMap<i64, usize> = HashMap::new();
        let mut row_end: HashMap<usize, (f64, f64)> = HashMap::new();
        let mut last_x = LEFT + FIRST - STEP;
        for key in &keys {
            let group = &calls_at[key];
            let first = group[0];
            let row = through(first).map_or(lost_row, |t| row_of_line[&t]);
            let rep = grave(group);
            let name = place_label(rep);
            let half = width_of(&name) / 2.0;
            let x = match row_end.get(&row) {
                Some(&(end, prev_half)) => {
                    (end + STEP.max(prev_half + half + 28.0)).max(last_x + 36.0)
                }
                None => (last_x + 36.0).max(LEFT + FIRST),
            };
            last_x = x;
            row_end.insert(row, (x, half));
            let y = MAIN_Y + row as f64 * ROW_GAP;
            let i = stations.len();
            for s in group {
                station_of.insert(s.seq, i);
            }
            stations.push(NetStation {
                key: key.clone(),
                x,
                y,
                row,
                calls: group.iter().map(|s| s.seq).collect(),
                rep: Station {
                    x,
                    y,
                    // The call map's agent lines mean nothing here.
                    line: 0,
                    label: Some(name),
                    members: group.iter().map(|s| s.seq).collect(),
                    ..rep.clone()
                },
                lines: Vec::new(),
            });
        }

        // Each line through its stations, left to right, beside the stations of other rows.
        let mut line_of: HashMap<i64, usize> = HashMap::new();
        let mut lines: Vec<NetLine> = Vec::new();
        for (li, t) in throughs.iter().enumerate() {
            let mut on: Vec<usize> = Vec::new();
            for s in &order {
                if through(s) == Some(*t) {
                    line_of.insert(s.seq, li);
                    let st = station_of[&s.seq];
                    if !on.contains(&st) {
                        on.push(st);
                    }
                }
            }
            // The words' line starts where the words were read.
            let source = match t {
                Through::Told(by) => station_of.get(by).copied(),
                Through::Turn(_) => None,
            };
            if let Some(src) = source {
                on.retain(|&s| s != src);
            }
            on.sort_by(|a, b| stations[*a].x.total_cmp(&stations[*b].x));
            let row = li;
            let row_y = MAIN_Y + row as f64 * ROW_GAP;
            let point = |s: &NetStation| {
                let off = (row as f64 - s.row as f64).clamp(-3.0, 3.0) * BESIDE;
                (s.x, s.y + off)
            };
            let mut stops: Vec<(f64, f64)> = Vec::new();
            let mut ids: Vec<usize> = Vec::new();
            if let Some(src) = source {
                stops.push(point(&stations[src]));
                ids.push(src);
            }
            for &s in &on {
                stops.push(point(&stations[s]));
                ids.push(s);
            }
            let Some(&(fx, fy)) = stops.first() else {
                continue;
            };
            // A turn's line sets out from its terminal, on its own row a little before its
            // first station, far enough back to come round to a station on another row.
            let mut points: Vec<(f64, f64)> = Vec::new();
            let mut at: Vec<usize> = Vec::new();
            if source.is_none() {
                let back = (STEP * 0.6).max((fy - row_y).abs() + 30.0);
                points.push((fx - back, row_y));
            }
            for &p in &stops {
                if let Some(&q) = points.last()
                    && let Some(c) = corner(q, p, row_y)
                {
                    points.push(c);
                }
                at.push(points.len());
                points.push(p);
            }
            // A line runs on a little past its last station, to its end.
            if let Some(&(lx, ly)) = points.last() {
                points.push((lx + 30.0, ly));
            }
            for &s in &ids {
                if !stations[s].lines.contains(&lines.len()) {
                    stations[s].lines.push(lines.len());
                }
            }
            lines.push(NetLine {
                through: *t,
                row,
                stations: ids,
                points,
                at,
            });
        }
        let width = stations
            .iter()
            .map(|s| s.x)
            .chain(lines.iter().flat_map(|l| l.points.iter().map(|p| p.0)))
            .fold(LEFT + FIRST, f64::max)
            + 40.0;
        let rows = if stations.iter().any(|s| s.row == lost_row) {
            lost_row + 1
        } else {
            lost_row.max(1)
        };
        Net {
            stations,
            lines,
            width,
            top: MAIN_Y,
            bottom: MAIN_Y + (rows - 1) as f64 * ROW_GAP,
            station_of,
            line_of,
            calls: m.stations.iter().map(|s| (s.seq, s.clone())).collect(),
        }
    }

    /// The station a Call was made at.
    pub fn station_of(&self, call: i64) -> Option<&NetStation> {
        self.station_of.get(&call).map(|&i| &self.stations[i])
    }

    pub fn station_index(&self, call: i64) -> Option<usize> {
        self.station_of.get(&call).copied()
    }

    /// The line a Call rode: what told the agent to make it.
    pub fn line_of(&self, call: i64) -> Option<usize> {
        self.line_of.get(&call).copied()
    }

    /// A line as an SVG path, its turns rounded.
    pub fn path(&self, line: usize) -> String {
        rounded(&self.lines[line].points)
    }

    /// The stretch of a line from its point `a` to its point `b`.
    pub fn stretch(&self, line: usize, a: usize, b: usize) -> String {
        let pts = &self.lines[line].points;
        let (a, b) = (a.min(b), a.max(b));
        rounded(&pts[a..=b.min(pts.len() - 1)])
    }

    /// Which point of a line is a station's.
    pub fn point_at(&self, line: usize, station: usize) -> Option<usize> {
        let l = &self.lines[line];
        l.stations
            .iter()
            .position(|&s| s == station)
            .map(|k| l.at[k])
    }

    /// The way to a Call, as the engine traced it: each stretch ridden as (line, from point,
    /// to point), the first from the turn's terminal. Empty when the engine traced it to
    /// nothing in the session.
    pub fn route(&self, call: i64) -> Vec<(usize, usize, usize)> {
        let mut out = Vec::new();
        let mut seen = HashSet::new();
        let mut here = call;
        while seen.insert(here) {
            let (Some(li), Some(st)) = (self.line_of(here), self.station_index(here)) else {
                break;
            };
            let Some(to) = self.point_at(li, st) else {
                break;
            };
            out.push((li, 0, to));
            match self.lines[li].through {
                Through::Turn(_) => break,
                Through::Told(by) => here = by,
            }
        }
        out.reverse();
        out
    }

    /// The turn whose request the way to a Call sets out from.
    pub fn start_of(&self, call: i64) -> Option<usize> {
        let first = self.route(call).first().copied()?;
        match self.lines[first.0].through {
            Through::Turn(n) => Some(n),
            Through::Told(_) => None,
        }
    }

    /// The Call whose result held the words a line follows.
    pub fn source_call(&self, line: usize) -> Option<&Station> {
        match self.lines[line].through {
            Through::Told(by) => self.calls.get(&by),
            Through::Turn(_) => None,
        }
    }
}

/// The gravest of a station's Calls: denied over held over undecided over allowed, the
/// latest of the gravest.
fn grave<'a>(group: &[&'a Station]) -> &'a Station {
    let weight = |s: &Station| match s.decision.as_deref() {
        Some("DENY") => 3,
        Some("REVIEW") => 2,
        None => 1,
        _ => 0,
    };
    group
        .iter()
        .copied()
        .max_by(|a, b| weight(a).cmp(&weight(b)).then(a.x.total_cmp(&b.x)))
        .expect("a station has a Call")
}

/// How wide a name is drawn, at the map's 11.5px.
fn width_of(text: &str) -> f64 {
    text.chars()
        .map(|c| if c.is_ascii() { 7.0 } else { 12.5 })
        .sum::<f64>()
        + 10.0
}

/// The corner a line turns at going from `p` to `q` on another row, as a metro map turns: at
/// 45° across, the straight run kept on the line's own row (`home`). Leaving a station of
/// another row it turns away at once; coming to one it runs along its own row first. None
/// when they are on one row, or too close for a 45° turn.
fn corner(p: (f64, f64), q: (f64, f64), home: f64) -> Option<(f64, f64)> {
    let dy = (q.1 - p.1).abs();
    if dy < 0.5 || q.0 - p.0 <= dy + 1.0 {
        return None;
    }
    if (p.1 - home).abs() > (q.1 - home).abs() {
        Some((p.0 + dy, q.1))
    } else {
        Some((q.0 - dy, p.1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::demo::Demo;

    fn net(prefix: &str) -> (Net, Model) {
        let demo = Demo::load();
        let id = demo
            .sessions()
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|s| s["id"].as_str())
            .find(|id| id.starts_with(prefix))
            .unwrap()
            .to_owned();
        let model = Model::build(&serde_json::from_value(demo.graph(&id).unwrap()).unwrap());
        (Net::build(&model), model)
    }

    // The README session: the request's line goes to the README, make test and the test files;
    // the README's hidden comment is a line of its own out of the README's station, to the two
    // denied writes; make test's output is another, to the test fixes. The way to a denied
    // write changes at the README.
    #[test]
    fn the_words_in_a_result_are_a_line_out_of_where_they_were_read() {
        let (n, model) = net("fa6576f7");
        assert!(matches!(n.lines[0].through, Through::Turn(0)));
        let readme = n.station_index(5).unwrap();
        let told: Vec<usize> = (0..n.lines.len())
            .filter(|&l| n.lines[l].through == Through::Told(5))
            .collect();
        assert_eq!(told.len(), 1, "README.md's words");
        assert_eq!(
            n.lines[told[0]].stations[0], readme,
            "it leaves the README's station"
        );
        assert!(n.stations[readme].lines.len() >= 2, "a transfer");
        let route = n.route(11);
        assert_eq!(route.len(), 2);
        assert_eq!(route[0].0, 0);
        assert_eq!(route[1].0, told[0]);
        assert_eq!(n.start_of(11), Some(0));
        // Every Call is at a station.
        let calls = model.nodes.iter().filter(|x| x.kind == "Call").count();
        assert_eq!(
            n.stations.iter().map(|s| s.calls.len()).sum::<usize>(),
            calls
        );
    }

    // A file read in one turn and again in the next is one station both turns' lines run
    // through.
    #[test]
    fn a_place_two_turns_went_to_is_a_transfer() {
        let (n, _) = net("7fcbd428");
        let turns = n
            .lines
            .iter()
            .filter(|l| matches!(l.through, Through::Turn(_)))
            .count();
        assert_eq!(turns, 2);
        assert!(n.stations.iter().all(|s| !s.lines.is_empty()));
    }

    // A Call the engine traced to nothing is a station no line reaches, with no way to it.
    #[test]
    fn a_call_traced_to_nothing_stands_apart() {
        let (n, _) = net("b6380b32");
        let rm = n
            .stations
            .iter()
            .find(|s| s.rep.decision.as_deref() == Some("DENY"))
            .unwrap();
        assert!(rm.lines.is_empty());
        assert!(n.route(rm.calls[0]).is_empty());
    }
}
