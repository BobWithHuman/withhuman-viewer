//! A session as a metro map: time runs left to right, and each line is where an instruction
//! came from. The person's request is the main line; text in a tool result that drove the
//! agent is a branch leaving the station of the Call that read it; a Call traced to neither
//! runs on a dotted line of its own. Following a Call's line back is following its cause.
//!
//! This module only places things; the canvas draws them.

use std::collections::{HashMap, HashSet};

use serde_json::Value;

use crate::layout::Model;
use crate::model::{Node, parse_json};

/// Space between two stations along a line.
pub const STEP: f64 = 150.0;
/// Where the main line runs, and the space between it and each branch below.
pub const MAIN_Y: f64 = 110.0;
pub const LANE_GAP: f64 = 190.0;
/// Left of the first terminus, and the room the opening request takes before the first station.
pub const LEFT: f64 = 40.0;
pub const FIRST: f64 = 330.0;
/// Space the dotted line of untraced Calls takes.
const LOOSE_GAP: f64 = 110.0;
/// At or above this, a Call's effect is shown as hard to undo.
pub const IRREVERSIBLE: f64 = 0.5;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LineKind {
    /// The person's request.
    Main,
    /// Text in a tool result.
    Branch,
    /// Neither the request nor a result.
    Loose,
}

/// What a branch's Calls were traced to: a paragraph of one tool result.
#[derive(Clone, Debug, PartialEq)]
pub struct Source {
    pub result: i64,
    pub paragraph: Option<i64>,
    pub probability: f64,
    /// The Call whose result it is: where the branch leaves its line.
    pub by: i64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Station {
    pub seq: i64,
    pub x: f64,
    pub y: f64,
    pub line: usize,
    pub tool: String,
    pub what: String,
    pub decision: Option<String>,
    pub irreversible: bool,
    /// The paragraph that caused it, when it runs on a branch.
    pub source: Option<Source>,
}

impl Station {
    /// Denied or held for review: what the map is read for.
    pub fn stopped(&self) -> bool {
        matches!(self.decision.as_deref(), Some("DENY" | "REVIEW"))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Line {
    pub kind: LineKind,
    pub y: f64,
    /// The line it leaves, and the station it leaves from; none for the main and loose lines.
    pub parent: Option<usize>,
    pub source: Option<Source>,
    /// Its stations, left to right.
    pub stations: Vec<i64>,
    /// A station on it was denied or held: its source is shown as the likely culprit.
    pub hot: bool,
}

/// A prompt: the first one opens the main line, a later one is a terminus on it.
#[derive(Clone, Debug, PartialEq)]
pub struct Ask {
    pub seq: i64,
    pub x: f64,
    pub text: String,
    pub intent: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Metro {
    pub lines: Vec<Line>,
    pub stations: Vec<Station>,
    pub asks: Vec<Ask>,
    /// The agent's last message, shown where the main line ends.
    pub last_word: Option<(i64, String)>,
    pub width: f64,
    pub height: f64,
}

/// What a selection keeps lit: its stations and lines, back to the request.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Route {
    pub stations: HashSet<i64>,
    pub lines: HashSet<usize>,
}

fn text(v: &Option<String>) -> String {
    v.clone().unwrap_or_default()
}

/// What a Call acts on, written from the session's folder rather than the root.
fn target(node: &Node) -> String {
    let brief = text(&node.summary.brief);
    match node.props.get("cwd").and_then(Value::as_str) {
        Some(cwd) if !cwd.is_empty() && brief == cwd => ".".into(),
        Some(cwd) if !cwd.is_empty() => brief.replace(&format!("{cwd}/"), ""),
        _ => brief,
    }
}

/// The paragraph a Call was traced to, when the trace links it to a tool result.
fn source_of(model: &Model, trace: &Value) -> Option<Source> {
    if trace["link"] != "segment" {
        return None;
    }
    let best = trace["segments"].as_array()?.iter().max_by(|a, b| {
        let p = |x: &Value| x["probability"].as_f64().unwrap_or(0.0);
        p(a).total_cmp(&p(b))
    })?;
    let result = best["seq"].as_i64()?;
    Some(Source {
        result,
        paragraph: best["paragraph"].as_i64(),
        probability: best["probability"].as_f64().unwrap_or(0.0),
        by: *model.call_of.get(&result)?,
    })
}

impl Metro {
    pub fn build(model: &Model) -> Self {
        let mut nodes: Vec<&Node> = model.nodes.iter().collect();
        nodes.sort_by_key(|n| n.seq());

        let mut lines = vec![Line {
            kind: LineKind::Main,
            y: MAIN_Y,
            parent: None,
            source: None,
            stations: Vec::new(),
            hot: false,
        }];
        let mut line_of_result: HashMap<i64, usize> = HashMap::new();
        let mut loose = None;
        let mut stations: Vec<Station> = Vec::new();
        let mut asks: Vec<Ask> = Vec::new();
        let mut last_word = None;
        let mut column = 0.0;
        let mut station_line: HashMap<i64, usize> = HashMap::new();

        for node in nodes {
            match node.kind.as_str() {
                "Request" => {
                    let x = if asks.is_empty() {
                        LEFT + 12.0
                    } else {
                        // A later prompt takes two columns: the turn changes there.
                        let x = LEFT + FIRST + column * STEP + 40.0;
                        column += 2.0;
                        x
                    };
                    asks.push(Ask {
                        seq: node.seq(),
                        x,
                        text: text(&node.summary.prompt),
                        intent: None,
                    });
                }
                "Intent" => {
                    if let Some(ask) = asks.last_mut().filter(|a| a.intent.is_none()) {
                        ask.intent = node.summary.intent.clone();
                    }
                }
                "Message" => last_word = Some((node.seq(), text(&node.summary.text))),
                "Call" => {
                    let trace = parse_json(node.props.get("trace_json"));
                    let source = source_of(model, &trace);
                    let line = match (&source, trace["link"].as_str()) {
                        (Some(s), _) => {
                            let parent = station_line.get(&s.by).copied().unwrap_or(0);
                            *line_of_result.entry(s.result).or_insert_with(|| {
                                lines.push(Line {
                                    kind: LineKind::Branch,
                                    y: 0.0,
                                    parent: Some(parent),
                                    source: Some(s.clone()),
                                    stations: Vec::new(),
                                    hot: false,
                                });
                                lines.len() - 1
                            })
                        }
                        (None, Some("none")) => *loose.get_or_insert_with(|| {
                            lines.push(Line {
                                kind: LineKind::Loose,
                                y: 0.0,
                                parent: None,
                                source: None,
                                stations: Vec::new(),
                                hot: false,
                            });
                            lines.len() - 1
                        }),
                        _ => 0,
                    };
                    let irreversible = trace["deviation"]["irreversible"]
                        .as_f64()
                        .is_some_and(|p| p >= IRREVERSIBLE);
                    let station = Station {
                        seq: node.seq(),
                        x: LEFT + FIRST + column * STEP,
                        y: 0.0,
                        line,
                        tool: node
                            .summary
                            .tool_name
                            .clone()
                            .unwrap_or_else(|| "Call".into()),
                        what: target(node),
                        decision: node.decision().map(str::to_owned),
                        irreversible,
                        source,
                    };
                    column += 1.0;
                    lines[line].hot |= station.stopped();
                    lines[line].stations.push(station.seq);
                    station_line.insert(station.seq, line);
                    stations.push(station);
                }
                _ => {}
            }
        }

        // Rows: each branch right under the line it leaves, siblings that leave further right
        // nearer, so no branch's drop crosses another branch. The loose line goes last.
        let mut next = MAIN_Y + LANE_GAP;
        let x_of: HashMap<i64, f64> = stations.iter().map(|s| (s.seq, s.x)).collect();
        fn place(parent: usize, lines: &mut [Line], x_of: &HashMap<i64, f64>, next: &mut f64) {
            let mut children: Vec<usize> = (0..lines.len())
                .filter(|&i| lines[i].parent == Some(parent) && lines[i].kind == LineKind::Branch)
                .collect();
            let leaves = |i: &usize| lines[*i].source.as_ref().map_or(0.0, |s| x_of[&s.by]);
            children.sort_by(|a, b| leaves(b).total_cmp(&leaves(a)));
            for child in children {
                lines[child].y = *next;
                *next += LANE_GAP;
                place(child, lines, x_of, next);
            }
        }
        place(0, &mut lines, &x_of, &mut next);
        if let Some(i) = loose {
            lines[i].y = next;
            next += LOOSE_GAP;
        }
        for s in &mut stations {
            s.y = lines[s.line].y;
        }
        let right = stations
            .iter()
            .map(|s| s.x)
            .chain(asks.iter().map(|a| a.x))
            .fold(LEFT + FIRST, f64::max);
        Metro {
            lines,
            stations,
            asks,
            last_word,
            width: right + 320.0,
            height: next.max(MAIN_Y + 180.0),
        }
    }

    pub fn station(&self, seq: i64) -> Option<&Station> {
        self.stations.iter().find(|s| s.seq == seq)
    }

    /// Where the main line ends: past its last station or prompt.
    pub fn main_end(&self) -> f64 {
        self.stations
            .iter()
            .map(|s| s.x)
            .chain(self.asks.iter().map(|a| a.x))
            .fold(LEFT + FIRST, f64::max)
            + 60.0
    }

    /// A Call's way back to the request: its station, the branches it rode and the stations
    /// they left from.
    pub fn route(&self, seq: i64) -> Route {
        let mut route = Route::default();
        let Some(mut station) = self.station(seq) else {
            return route;
        };
        route.stations.insert(seq);
        loop {
            route.lines.insert(station.line);
            let line = &self.lines[station.line];
            let Some(from) = line.source.as_ref().map(|s| s.by) else {
                break;
            };
            route.stations.insert(from);
            match self.station(from) {
                Some(s) => station = s,
                None => break,
            }
        }
        route.lines.insert(0);
        route
    }

    /// A line's track as an SVG path: a branch drops from the station it leaves, turns and
    /// runs right to its last station.
    pub fn path(&self, index: usize) -> String {
        let line = &self.lines[index];
        let last = line
            .stations
            .last()
            .and_then(|&s| self.station(s))
            .map_or(LEFT + FIRST, |s| s.x)
            + 40.0;
        match line.kind {
            LineKind::Main => format!("M{},{} H{}", LEFT + 12.0, MAIN_Y, self.main_end()),
            LineKind::Loose => {
                let first = line
                    .stations
                    .first()
                    .and_then(|&s| self.station(s))
                    .map_or(LEFT + FIRST, |s| s.x);
                format!("M{},{} H{}", first - 50.0, line.y, last)
            }
            LineKind::Branch => {
                let source = line.source.as_ref().expect("a branch has a source");
                let x0 = self.station(source.by).map_or(LEFT, |s| s.x);
                let y0 = self.lines[line.parent.unwrap_or(0)].y;
                let r = 26.0;
                format!(
                    "M{x0},{y0} V{} Q{x0},{} {},{} H{last}",
                    line.y - r,
                    line.y,
                    x0 + r,
                    line.y
                )
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::demo::Demo;

    fn metro(session: &str) -> Metro {
        let graph = Demo::load().graph(session).expect("fixture session");
        Metro::build(&Model::build(
            &serde_json::from_value(graph).expect("graph shape"),
        ))
    }

    fn session(prefix: &str) -> String {
        let sessions = Demo::load().sessions();
        sessions
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|s| s["id"].as_str())
            .find(|id| id.starts_with(prefix))
            .expect("fixture session")
            .to_owned()
    }

    fn line_of(m: &Metro, seq: i64) -> &Line {
        &m.lines[m.station(seq).unwrap().line]
    }

    // The README's hidden instruction drove the denied Write and Edit: they ride a branch
    // leaving the Read of the README, and the test output drives another.
    #[test]
    fn branches_leave_the_call_that_read_their_source() {
        let m = metro(&session("fa6576f7"));
        assert_eq!(line_of(&m, 5).kind, LineKind::Main);
        let readme = line_of(&m, 12);
        assert_eq!(readme.kind, LineKind::Branch);
        assert_eq!(readme.source.as_ref().unwrap().by, 5);
        assert!(readme.hot);
        assert_eq!(readme.stations, [11, 12]);
        let tests = line_of(&m, 16);
        assert_eq!(tests.source.as_ref().unwrap().by, 7);
        assert!(!tests.hot);
        // The branch leaving further right sits nearer the main line.
        assert!(tests.y < readme.y);
    }

    #[test]
    fn a_route_runs_back_to_the_request() {
        let m = metro(&session("fa6576f7"));
        let route = m.route(12);
        assert!(route.stations.contains(&12) && route.stations.contains(&5));
        assert!(!route.stations.contains(&11));
        assert_eq!(route.lines.len(), 2);
    }

    #[test]
    fn untraced_calls_ride_the_loose_line_and_hard_undos_show() {
        let m = metro(&session("b6380b32"));
        assert_eq!(line_of(&m, 16).kind, LineKind::Loose);
        assert!(m.station(14).unwrap().irreversible);
        assert!(!m.station(6).unwrap().irreversible);
    }

    #[test]
    fn a_later_prompt_opens_a_turn_on_the_main_line() {
        let m = metro(&session("f8a97043"));
        assert_eq!(m.asks.len(), 2);
        assert!(m.asks[1].x > m.station(14).unwrap().x);
        assert!(m.asks[0].intent.is_some());
    }

    #[test]
    fn paths_turn_into_their_row() {
        let m = metro(&session("fa6576f7"));
        let i = m.station(12).unwrap().line;
        let path = m.path(i);
        assert!(path.starts_with(&format!("M{},{MAIN_Y}", m.station(5).unwrap().x)));
        assert!(path.contains(&format!("{}", m.lines[i].y)));
    }
}
