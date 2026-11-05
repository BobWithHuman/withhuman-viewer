//! A session as a metro map. Time runs left to right, and every visual channel answers one
//! question only:
//!
//! - **Line: what told the agent to do it.** The main line is the turn's request: navy when a
//!   person wrote it, teal when it is a subagent's report the engine recorded as the turn. A
//!   branch is text in a tool result, leaving the station of the Call that read it. The dotted
//!   line holds Calls traced to neither, or to a result whose Call is not in the session.
//! - **Station fill: what the engine decided.** White when allowed (ringed in its line's
//!   colour), red when denied, amber when held for review, a dotted grey ring when undecided.
//! - **Marks on a station: how it went.** A dashed amber ring when it is hard to undo, a slash
//!   when the tool failed.
//! - **Band behind stations: who did it.** A subagent's Calls stay on their line and share a
//!   band named after the agent.
//! - **Station shape: what kind of action.** Reading is a circle, changing a square, running a
//!   triangle, searching a diamond, handing work to an agent a hexagon.
//! - **Section names: what the agent said it was doing**, over the stretch it did it, so the
//!   map reads as a few sentences before any station is.
//!
//! Four or more plain allowed stations in a row on one line fold into one; a stop, an
//! undecided Call, a hard undo or a failure never folds. This module only places things.

use std::collections::{HashMap, HashSet};

use serde_json::Value;

use crate::layout::Model;
use crate::model::{Node, parse_json};

/// Space between two stations along a line.
pub const STEP: f64 = 64.0;
/// Where the main line runs (below the section names), and the space to each branch below.
pub const MAIN_Y: f64 = 92.0;
pub const LANE_GAP: f64 = 84.0;
/// The first terminus, and the room before the first station.
pub const LEFT: f64 = 30.0;
pub const FIRST: f64 = 40.0;
/// Space the dotted line of untraced Calls takes.
const LOOSE_GAP: f64 = 70.0;
/// Where a branch turns: it drops, then folds into its row at 45°.
pub const CHAMFER: f64 = 22.0;
/// The most characters a section name shows.
pub const NAME_CHARS: usize = 28;
/// At or above this, a Call's effect is shown as hard to undo.
pub const IRREVERSIBLE: f64 = 0.5;
/// This many plain stations in a row on one line fold into one.
pub const FOLD: usize = 4;

/// What kind of thing a Call does, drawn as its station's shape.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Action {
    Read,
    Change,
    Run,
    Search,
    Delegate,
    Other,
}

impl Action {
    pub fn of(tool: &str) -> Self {
        match tool {
            "Read" | "NotebookRead" | "WebFetch" => Action::Read,
            "Edit" | "MultiEdit" | "Write" | "NotebookEdit" => Action::Change,
            "Bash" | "BashOutput" | "KillShell" => Action::Run,
            "Grep" | "Glob" | "LS" | "WebSearch" => Action::Search,
            "Agent" | "Task" | "SubagentHandback" => Action::Delegate,
            t if t.starts_with("mcp__") => Action::Run,
            _ => Action::Other,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LineKind {
    /// The turn's request.
    Main,
    /// Text in a tool result.
    Branch,
    /// Neither the request nor a result in the session.
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
    /// The Call's number; a folded station carries its first Call's.
    pub seq: i64,
    pub x: f64,
    pub y: f64,
    pub line: usize,
    pub tool: String,
    pub what: String,
    pub action: Action,
    pub decision: Option<String>,
    pub irreversible: bool,
    /// The tool ran and came back with an error.
    pub failed: bool,
    /// The trace pointed at a tool result whose Call is not in the session.
    pub unsourced: bool,
    /// The subagent that made the Call, when one did.
    pub agent: Option<String>,
    /// The paragraph that caused it, when it runs on a branch.
    pub source: Option<Source>,
    /// The Calls a folded station stands for, in order; just its own otherwise.
    pub members: Vec<i64>,
}

impl Station {
    /// Denied or held for review: what the map is read for.
    pub fn stopped(&self) -> bool {
        matches!(self.decision.as_deref(), Some("DENY" | "REVIEW"))
    }

    pub fn folded(&self) -> bool {
        self.members.len() > 1
    }

    /// Nothing worth singling out: allowed, decided, easy to undo, and it worked.
    fn plain(&self) -> bool {
        self.decision.as_deref() == Some("ALLOW")
            && !self.irreversible
            && !self.failed
            && !self.unsourced
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
    /// What a branch is called on the map: where its instruction came from.
    pub name: String,
}

/// What the agent said it would do, over the stations it did it at.
#[derive(Clone, Debug, PartialEq)]
pub struct Section {
    pub seq: i64,
    pub text: String,
    /// Where the name starts, and the stretch of stations under it.
    pub at: f64,
    pub from: f64,
    pub to: f64,
}

/// A turn's request: the first opens the main line, a later one is a terminus on it.
#[derive(Clone, Debug, PartialEq)]
pub struct Ask {
    pub seq: i64,
    pub x: f64,
    pub text: String,
    pub intent: Option<String>,
    /// The subagent whose report the engine recorded as this turn; none when a person wrote it.
    pub agent: Option<String>,
}

/// Stations on one line made in a row by one subagent.
#[derive(Clone, Debug, PartialEq)]
pub struct Band {
    pub agent: String,
    pub line: usize,
    pub x0: f64,
    pub x1: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Metro {
    pub lines: Vec<Line>,
    pub stations: Vec<Station>,
    pub asks: Vec<Ask>,
    pub bands: Vec<Band>,
    pub sections: Vec<Section>,
    /// The agent's last message, shown where the main line ends.
    pub last_word: Option<(i64, String)>,
    pub width: f64,
    pub height: f64,
    /// Each Call's station, folded ones included.
    at: HashMap<i64, usize>,
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

/// A section's name as shown: one line, cut short.
pub fn section_name(text: &str) -> String {
    let flat: String = text
        .replace(['*', '`', '#'], "")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if flat.chars().count() > NAME_CHARS {
        format!("{}…", flat.chars().take(NAME_CHARS).collect::<String>())
    } else {
        flat
    }
}

/// The widest a section name runs before it wraps onto a second line.
pub const NAME_WIDTH: f64 = 210.0;

/// The room a name takes on the map, so the next section starts after it.
fn name_width(name: &str) -> f64 {
    (name.chars().count() as f64 * 15.0).min(NAME_WIDTH) + 22.0
}

/// The last part of a path, as a branch or a label names it.
pub fn file_name(path: &str) -> &str {
    path.rsplit(['/', '\\'])
        .find(|p| !p.is_empty())
        .unwrap_or(path)
}

/// The subagent a report came from, when a turn's text is one: `<agent-message from="…">`.
pub fn report_of(prompt: &str) -> Option<String> {
    let rest = prompt.trim_start().strip_prefix("<agent-message")?;
    let from = rest
        .split_once("from=\"")
        .and_then(|(_, r)| r.split_once('"'))
        .map(|(id, _)| id.to_owned());
    Some(from.unwrap_or_else(|| "에이전트".into()))
}

/// The best paragraph a Call was traced to, and the Call whose result holds it, when it has one.
fn traced(model: &Model, trace: &Value) -> Option<(Source, bool)> {
    if trace["link"] != "segment" {
        return None;
    }
    let best = trace["segments"].as_array()?.iter().max_by(|a, b| {
        let p = |x: &Value| x["probability"].as_f64().unwrap_or(0.0);
        p(a).total_cmp(&p(b))
    })?;
    let result = best["seq"].as_i64()?;
    let by = model.call_of.get(&result).copied();
    Some((
        Source {
            result,
            paragraph: best["paragraph"].as_i64(),
            probability: best["probability"].as_f64().unwrap_or(0.0),
            by: by.unwrap_or(-1),
        },
        by.is_some(),
    ))
}

/// One event in time order: a turn opening, or a Call on its line.
enum Item {
    Ask(Ask),
    Say(i64, String),
    Call(Station),
}

impl Metro {
    #[cfg(test)]
    pub fn build(model: &Model) -> Self {
        Self::build_with(model, &HashSet::new())
    }

    /// Lays the map out, leaving unfolded the runs whose first Call is in `open`.
    pub fn build_with(model: &Model, open: &HashSet<i64>) -> Self {
        let mut nodes: Vec<&Node> = model.nodes.iter().collect();
        nodes.sort_by_key(|n| n.seq());
        // A subagent by its id, named by the kind its start recorded.
        let kinds: HashMap<String, String> = model
            .nodes
            .iter()
            .filter_map(|n| {
                let id = n.props.get("agent_id")?.as_str()?;
                let kind = n.props.get("agent_type")?.as_str()?;
                Some((id.to_owned(), kind.to_owned()))
            })
            .collect();
        let failed: HashSet<i64> = model
            .nodes
            .iter()
            .filter(|n| n.kind == "Error")
            .filter_map(|n| model.call_of.get(&n.seq()).copied())
            .collect();

        // Lines first: which line each Call rides, in the order they happened.
        let mut lines = vec![Line {
            kind: LineKind::Main,
            y: MAIN_Y,
            parent: None,
            source: None,
            stations: Vec::new(),
            hot: false,
            name: String::new(),
        }];
        let mut line_of_result: HashMap<i64, usize> = HashMap::new();
        let mut loose = None;
        let mut line_of_call: HashMap<i64, usize> = HashMap::new();
        let mut items: Vec<Item> = Vec::new();
        let mut last_word = None;
        let new_line = |lines: &mut Vec<Line>, kind, parent, source| {
            lines.push(Line {
                kind,
                y: 0.0,
                parent,
                source,
                stations: Vec::new(),
                hot: false,
                name: String::new(),
            });
            lines.len() - 1
        };
        for node in nodes {
            match node.kind.as_str() {
                "Request" => {
                    let prompt = text(&node.summary.prompt);
                    items.push(Item::Ask(Ask {
                        seq: node.seq(),
                        x: 0.0,
                        agent: report_of(&prompt).map(|id| kinds.get(&id).cloned().unwrap_or(id)),
                        text: prompt,
                        intent: None,
                    }));
                }
                "Intent" => {
                    if let Some(Item::Ask(ask)) = items
                        .iter_mut()
                        .rev()
                        .find(|i| matches!(i, Item::Ask(_)))
                        .filter(|i| matches!(i, Item::Ask(a) if a.intent.is_none()))
                    {
                        ask.intent = node.summary.intent.clone();
                    }
                }
                "Message" => {
                    let said = text(&node.summary.text);
                    last_word = Some((node.seq(), said.clone()));
                    items.push(Item::Say(node.seq(), said));
                }
                "Call" => {
                    let trace = parse_json(node.props.get("trace_json"));
                    let traced = traced(model, &trace);
                    // Branch when the trace names a result in the session; the dotted line
                    // when it names one that is not, or names nothing; else the main line.
                    let (line, source, unsourced) = match traced {
                        Some((s, true)) => {
                            let parent = line_of_call.get(&s.by).copied().unwrap_or(0);
                            let line = *line_of_result.entry(s.result).or_insert_with(|| {
                                new_line(
                                    &mut lines,
                                    LineKind::Branch,
                                    Some(parent),
                                    Some(s.clone()),
                                )
                            });
                            (line, Some(s), false)
                        }
                        Some((_, false)) => (
                            *loose.get_or_insert_with(|| {
                                new_line(&mut lines, LineKind::Loose, None, None)
                            }),
                            None,
                            true,
                        ),
                        None if trace["link"] == "none" => (
                            *loose.get_or_insert_with(|| {
                                new_line(&mut lines, LineKind::Loose, None, None)
                            }),
                            None,
                            false,
                        ),
                        None => (0, None, false),
                    };
                    line_of_call.insert(node.seq(), line);
                    let agent = node
                        .props
                        .get("agent_id")
                        .and_then(Value::as_str)
                        .filter(|a| !a.is_empty() && *a != "main")
                        .map(|_| {
                            node.props
                                .get("agent_type")
                                .and_then(Value::as_str)
                                .unwrap_or("서브에이전트")
                                .to_owned()
                        });
                    items.push(Item::Call(Station {
                        seq: node.seq(),
                        x: 0.0,
                        y: 0.0,
                        line,
                        tool: node
                            .summary
                            .tool_name
                            .clone()
                            .unwrap_or_else(|| "Call".into()),
                        what: target(node),
                        action: Action::of(node.summary.tool_name.as_deref().unwrap_or("")),
                        decision: node.decision().map(str::to_owned),
                        irreversible: trace["deviation"]["irreversible"]
                            .as_f64()
                            .is_some_and(|p| p >= IRREVERSIBLE),
                        failed: failed.contains(&node.seq()),
                        unsourced,
                        agent,
                        source,
                        members: vec![node.seq()],
                    }));
                }
                _ => {}
            }
        }

        // Folding: plain stations in a row on one line, made by the same hand, become one
        // unless the run was opened.
        let mut folded: Vec<Item> = Vec::new();
        let mut run: Vec<Station> = Vec::new();
        let flush = |run: &mut Vec<Station>, out: &mut Vec<Item>| {
            if run.len() >= FOLD && !open.contains(&run[0].seq) {
                let mut first = run[0].clone();
                let same = run.iter().all(|s| s.tool == first.tool);
                let members: Vec<i64> = run.iter().map(|s| s.seq).collect();
                first.what = if same {
                    format!("{}개", members.len())
                } else {
                    // Each tool with how many times it ran, the most used first.
                    let mut counts: Vec<(&str, usize)> = Vec::new();
                    for t in run.iter().map(|s| s.tool.as_str()) {
                        match counts.iter_mut().find(|(n, _)| *n == t) {
                            Some((_, c)) => *c += 1,
                            None => counts.push((t, 1)),
                        }
                    }
                    counts.sort_by_key(|c| std::cmp::Reverse(c.1));
                    counts
                        .iter()
                        .map(|(t, c)| format!("{t} {c}"))
                        .collect::<Vec<_>>()
                        .join(" · ")
                };
                if !same {
                    first.tool = "Call".into();
                }
                first.members = members;
                out.push(Item::Call(first));
            } else {
                out.extend(run.drain(..).map(Item::Call));
            }
            run.clear();
        };
        for item in items {
            match item {
                Item::Call(s) if s.plain() => {
                    let joins = run
                        .last()
                        .is_some_and(|r| r.line == s.line && r.agent == s.agent);
                    if !joins {
                        flush(&mut run, &mut folded);
                    }
                    run.push(s);
                }
                other => {
                    flush(&mut run, &mut folded);
                    folded.push(other);
                }
            }
        }
        flush(&mut run, &mut folded);

        // Columns: one per station, more where a turn changes, and room for each section's
        // name before the next section starts.
        let mut stations: Vec<Station> = Vec::new();
        let mut asks: Vec<Ask> = Vec::new();
        let mut sections: Vec<Section> = Vec::new();
        let mut x = LEFT + FIRST;
        let mut free_at = 0.0_f64;
        let mut open_section: Option<usize> = None;
        for item in folded {
            match item {
                Item::Ask(mut a) => {
                    if asks.is_empty() {
                        a.x = LEFT;
                    } else {
                        x = (x + STEP * 0.6).max(free_at);
                        a.x = x;
                        x += STEP * 1.2;
                        free_at = a.x + 130.0;
                    }
                    open_section = None;
                    asks.push(a);
                }
                Item::Say(seq, said) => {
                    let name = section_name(&said);
                    if name.is_empty() {
                        continue;
                    }
                    x = x.max(free_at);
                    let at = x - 10.0;
                    free_at = at + name_width(&name);
                    sections.push(Section {
                        seq,
                        text: name,
                        at,
                        from: x,
                        to: x,
                    });
                    open_section = Some(sections.len() - 1);
                }
                Item::Call(mut s) => {
                    s.x = x;
                    if let Some(i) = open_section {
                        if sections[i].to == sections[i].from
                            && stations.last().is_none_or(|l| l.x < sections[i].from)
                        {
                            sections[i].from = x;
                        }
                        sections[i].to = x;
                    }
                    x += STEP;
                    stations.push(s);
                }
            }
        }
        // A closing message with no Call after it names nothing on the map.
        sections.retain(|sec| stations.iter().any(|s| s.x >= sec.from && s.x <= sec.to));
        let mut at: HashMap<i64, usize> = HashMap::new();
        for (i, s) in stations.iter().enumerate() {
            for &m in &s.members {
                at.insert(m, i);
            }
        }
        for s in &stations {
            lines[s.line].hot |= s.stopped();
            lines[s.line].stations.push(s.seq);
        }
        for line in lines.iter_mut().filter(|l| l.kind == LineKind::Branch) {
            let by = line
                .source
                .as_ref()
                .and_then(|s| at.get(&s.by))
                .map(|&i| &stations[i]);
            line.name = match by {
                Some(s) if !s.what.is_empty() => format!("{}의 지시", file_name(&s.what)),
                Some(s) => format!("{} 결과의 지시", s.tool),
                None => "도구 결과의 지시".into(),
            };
        }

        // Rows: each branch right under the line it leaves, siblings that leave further right
        // nearer, so no branch's drop crosses another branch. The loose line goes last.
        let x_of = |seq: i64| at.get(&seq).map_or(LEFT, |&i| stations[i].x);
        let mut next = MAIN_Y + LANE_GAP;
        fn place(parent: usize, lines: &mut [Line], x_of: &dyn Fn(i64) -> f64, next: &mut f64) {
            let mut children: Vec<usize> = (0..lines.len())
                .filter(|&i| lines[i].parent == Some(parent) && lines[i].kind == LineKind::Branch)
                .collect();
            let leaves = |i: &usize| lines[*i].source.as_ref().map_or(0.0, |s| x_of(s.by));
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

        // Bands: a subagent's stations in a row on one line.
        let mut bands: Vec<Band> = Vec::new();
        for line in 0..lines.len() {
            for s in stations.iter().filter(|s| s.line == line) {
                let Some(agent) = &s.agent else { continue };
                match bands.last_mut() {
                    Some(b) if b.line == line && &b.agent == agent && s.x - b.x1 <= STEP * 1.5 => {
                        b.x1 = s.x
                    }
                    _ => bands.push(Band {
                        agent: agent.clone(),
                        line,
                        x0: s.x,
                        x1: s.x,
                    }),
                }
            }
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
            bands,
            sections,
            last_word,
            width: right + 220.0,
            height: next.max(MAIN_Y + 180.0),
            at,
        }
    }

    /// The station a Call is drawn at: its own, or the folded one it is part of.
    pub fn station(&self, seq: i64) -> Option<&Station> {
        self.at.get(&seq).map(|&i| &self.stations[i])
    }

    /// Where the main line ends: past its last station or prompt.
    pub fn main_end(&self) -> f64 {
        self.stations
            .iter()
            .map(|s| s.x)
            .chain(self.asks.iter().map(|a| a.x))
            .fold(LEFT + FIRST, f64::max)
            + 30.0
    }

    /// The main line in turns: from each request to the next, and who wrote that request.
    pub fn main_segments(&self) -> Vec<(f64, f64, Option<String>)> {
        let end = self.main_end();
        self.asks
            .iter()
            .enumerate()
            .map(|(i, a)| {
                let to = self.asks.get(i + 1).map_or(end, |n| n.x);
                (a.x, to, a.agent.clone())
            })
            .collect()
    }

    /// Who wrote the request a main-line station's turn answers: none for a person.
    pub fn turn_agent(&self, x: f64) -> Option<String> {
        self.asks
            .iter()
            .rev()
            .find(|a| a.x <= x)
            .and_then(|a| a.agent.clone())
    }

    /// A Call's way back to the request: its station, the branches it rode and the stations
    /// they left from.
    pub fn route(&self, seq: i64) -> Route {
        let mut route = Route::default();
        let Some(mut station) = self.station(seq) else {
            return route;
        };
        route.stations.insert(station.seq);
        loop {
            route.lines.insert(station.line);
            let line = &self.lines[station.line];
            let Some(from) = line.source.as_ref().map(|s| s.by) else {
                break;
            };
            match self.station(from) {
                Some(s) => {
                    route.stations.insert(s.seq);
                    station = s;
                }
                None => break,
            }
        }
        route.lines.insert(0);
        route
    }

    /// A line's track as an SVG path. The main line is drawn turn by turn
    /// ([`Metro::main_segments`]); a branch drops from the station it leaves, turns and runs
    /// right to its last station.
    pub fn path(&self, index: usize) -> String {
        let line = &self.lines[index];
        let last = line
            .stations
            .last()
            .and_then(|&s| self.station(s))
            .map_or(LEFT + FIRST, |s| s.x)
            + 40.0;
        match line.kind {
            LineKind::Main => format!("M{},{} H{}", LEFT, MAIN_Y, self.main_end()),
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
                format!(
                    "M{x0},{y0} V{} L{},{} H{last}",
                    line.y - CHAMFER,
                    x0 + CHAMFER,
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

    fn model(session: &str) -> Model {
        let graph = Demo::load().graph(session).expect("fixture session");
        Model::build(&serde_json::from_value(graph).expect("graph shape"))
    }

    fn metro(session: &str) -> Metro {
        Metro::build(&model(session))
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

    // A subagent's Calls keep their line and share a band; the one that failed is marked.
    #[test]
    fn a_subagent_gets_a_band_and_a_failed_tool_a_mark() {
        let m = metro(&session("b6380b32"));
        assert_eq!(m.station(6).unwrap().agent.as_deref(), Some("Explore"));
        assert!(m.station(14).unwrap().agent.is_none());
        assert!(m.station(10).unwrap().failed);
        let band = &m.bands[0];
        assert_eq!(band.agent, "Explore");
        assert_eq!(
            (band.x0, band.x1),
            (m.station(6).unwrap().x, m.station(10).unwrap().x)
        );
    }

    // A subagent's report recorded as a turn is not a person's request.
    #[test]
    fn a_report_opens_its_turn_in_the_agents_name() {
        let m = metro(&session("7fcbd428"));
        assert_eq!(m.asks.len(), 2);
        assert!(m.asks[0].agent.is_none());
        assert_eq!(m.asks[1].agent.as_deref(), Some("Explore"));
        let segments = m.main_segments();
        assert_eq!(segments.len(), 2);
        assert!(segments[1].2.is_some());
        assert!(m.turn_agent(m.station(21).unwrap().x).is_some());
        assert!(m.turn_agent(m.station(4).unwrap().x).is_none());
        assert_eq!(report_of("고쳐줘"), None);
    }

    /// A session of one prompt, one message and these Calls: (tool, decision, irreversible).
    fn made_up(calls: &[(&str, &str, f64)]) -> Model {
        let mut nodes = vec![
            serde_json::json!({"kind": "Request", "props": {"seq": 1}, "summary": {"prompt": "고쳐줘"}}),
            serde_json::json!({"kind": "Message", "props": {"seq": 2}, "summary": {"text": "살펴보겠습니다."}}),
        ];
        for (i, (tool, decision, undo)) in calls.iter().enumerate() {
            let trace = serde_json::json!({"link": "request", "deviation": {"irreversible": undo}});
            nodes.push(serde_json::json!({
                "kind": "Call",
                "props": {"seq": 3 + i as i64, "decision": decision, "trace_json": trace.to_string()},
                "summary": {"tool_name": tool, "brief": format!("f{i}")},
            }));
        }
        let data = serde_json::json!({"nodes": nodes, "subgraphs": [], "edges": []});
        Model::build(&serde_json::from_value(data).unwrap())
    }

    // Plain calls in a row within one section fold into one station, unless opened; a stop
    // or a hard undo breaks the run and stays on its own.
    #[test]
    fn plain_runs_fold_and_open() {
        let plain = [
            ("Bash", "ALLOW", 0.0),
            ("Bash", "ALLOW", 0.0),
            ("Edit", "ALLOW", 0.0),
            ("Bash", "ALLOW", 0.0),
            ("Read", "ALLOW", 0.0),
        ];
        let model = made_up(&plain);
        let m = Metro::build(&model);
        let folded = m.station(3).unwrap();
        assert_eq!(folded.members, [3, 4, 5, 6, 7]);
        assert_eq!(m.station(5).unwrap().seq, 3);
        assert_eq!(folded.tool, "Call");
        assert_eq!(folded.what, "Bash 3 · Edit 1 · Read 1");
        let open = Metro::build_with(&model, &HashSet::from([3]));
        assert!(!open.station(3).unwrap().folded());
        assert!(open.station(5).unwrap().x > open.station(3).unwrap().x);

        let broken = made_up(&[
            ("Bash", "ALLOW", 0.0),
            ("Bash", "ALLOW", 0.0),
            ("Bash", "DENY", 0.0),
            ("Bash", "ALLOW", 0.9),
            ("Bash", "ALLOW", 0.0),
        ]);
        assert!(Metro::build(&broken).stations.iter().all(|s| !s.folded()));
    }

    // The agent's words between Calls end a run: a fold never hides a section's start.
    #[test]
    fn a_section_boundary_ends_a_fold() {
        let m = metro(&session("f8a97043"));
        assert!(m.stations.iter().all(|s| !s.folded()));
        assert!(m.sections.len() >= 3);
    }

    #[test]
    fn stops_and_hard_undos_never_fold() {
        let m = metro(&session("fa6576f7"));
        assert!(
            m.stations
                .iter()
                .all(|s| !s.folded() || s.members.len() >= FOLD)
        );
        assert!(m.station(12).is_some_and(|s| !s.folded()));
    }

    // The agent's words name the stretch of stations they came before, in order.
    #[test]
    fn sections_name_what_the_agent_said_it_would_do() {
        let m = metro(&session("fa6576f7"));
        let names: Vec<&str> = m.sections.iter().map(|s| s.text.as_str()).collect();
        assert_eq!(names[0], "README를 먼저 읽고 테스트를 돌려 보겠습니다.");
        assert!(names.iter().all(|n| n.chars().count() <= NAME_CHARS + 1));
        let first = &m.sections[0];
        assert_eq!(
            (first.from, first.to),
            (m.station(5).unwrap().x, m.station(7).unwrap().x)
        );
        // Each name ends before the next section starts.
        for w in m.sections.windows(2) {
            assert!(w[0].at + name_width(&w[0].text) <= w[1].at + 10.0);
        }
        assert_eq!(line_of(&m, 12).name, "README.md의 지시");
        assert_eq!(m.station(12).unwrap().action, Action::Change);
        assert_eq!(m.station(7).unwrap().action, Action::Run);
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
