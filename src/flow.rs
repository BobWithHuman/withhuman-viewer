//! The session as a route map in two levels, after the design in Figma Make.
//!
//! - **Turns**: each request the engine recorded, with the Calls made under it. Plain allowed
//!   Calls in a row fold into one item ("허용 39개"); a Call that was denied, held, or traced to
//!   the words of a tool result stands on its own.
//! - **Tasks**: turns in a row that worked on the same files, as a line of the overview.
//!   Seven tasks make a chapter, one overview each.
//! - **Overview** (`Flow::overview`): the chapter's tasks laid on hand-drawn line shapes (only
//!   horizontal, vertical and 45° runs). Stations are the places a task went to; a place two
//!   tasks went to is an interchange where their shapes cross.
//! - **Line** (`Flow::line`): the turns of one task as one line running right. The words a Call
//!   followed are a purple branch leaving an interchange.

use std::collections::{HashMap, HashSet};

use crate::metro::{Metro, Station, place_key, place_label};

pub const LINE_COLOR: &str = "#2f3542";
pub const LINE_MUTED: &str = "#b4b9c2";
pub const TEXT_COLOR: &str = "#8e7cc3";
pub const TEXT_MUTED: &str = "#d6d0e6";
pub const GREEN: &str = "#3e9b84";
pub const AMBER: &str = "#d9a21b";
pub const RED: &str = "#d8403a";
/// The overview's line colours, by task.
pub const LINE_COLORS: [&str; 9] = [
    "#3a95d4", "#e9833a", "#8cc43f", "#3e7d3a", "#8e5bb5", "#d0679d", "#1b9e8f", "#d4a017",
    "#4b5d73",
];
/// Tasks a chapter holds: one overview has room for this many lines.
pub const CHAPTER: usize = 7;
/// The most turns one task takes.
const TASK_TURNS: usize = 6;
/// The most stations a task's line shows on the overview.
const LINE_STOPS: usize = 9;

/// One Call as the map names it.
#[derive(Clone, Debug, PartialEq)]
pub struct Call {
    pub seq: i64,
    pub title: String,
    /// The tool as the map writes it (`metro::tool_label`).
    pub tool: String,
    /// The MCP server, for an MCP tool.
    pub server: Option<String>,
    pub arg: String,
    pub at: Option<i64>,
    pub warn: bool,
    pub blocked: bool,
    pub allowed: bool,
    /// The place the words it followed were read at, and the Call that read them.
    pub via: Option<String>,
    pub by: Option<i64>,
    /// Nothing in the session told it.
    pub loose: bool,
    pub key: String,
    pub place: String,
}

impl Call {
    fn of(m: &Metro, s: &Station) -> Self {
        let by = s.source.as_ref().map(|src| src.by).filter(|&b| b != s.seq);
        Call {
            seq: s.seq,
            title: s.name(),
            tool: crate::metro::tool_label(&s.tool),
            server: crate::metro::mcp_parts(&s.tool).map(|(srv, _)| srv.to_owned()),
            arg: s.what.clone(),
            at: s.at,
            warn: s.decision.as_deref() == Some("REVIEW"),
            blocked: s.decision.as_deref() == Some("DENY"),
            allowed: s.decision.as_deref() == Some("ALLOW"),
            via: by.and_then(|b| m.station(b)).map(|b| b.cause_name()),
            by: by.filter(|&b| m.station(b).is_some()),
            loose: s.loose || s.unsourced,
            key: place_key(s),
            place: place_label(s),
        }
    }

    /// Nothing to single out: let through, and traced to what told it. A Call that followed
    /// words in a tool result, or that failed, is folded too: an agent acting on what it read,
    /// and tools erring, are most of a real session, and the map is read for what was stopped.
    fn plain(&self) -> bool {
        !self.warn && !self.blocked && !self.loose
    }

    /// The line under its name: the tool, and the folder for a file; an MCP tool's server.
    pub fn sub(&self) -> String {
        if let Some(srv) = &self.server {
            return format!("{srv} MCP");
        }
        if !self.key.starts_with("f:") {
            return self.tool.clone();
        }
        match self.arg.rsplit_once('/') {
            Some((d, _)) if !d.is_empty() => format!("{} · {d}", self.tool),
            _ => self.tool.clone(),
        }
    }
}

/// One entry of a turn: a Call, or plain Calls in a row folded together.
#[derive(Clone, Debug, PartialEq)]
pub enum Item {
    Call(usize),
    Group {
        calls: Vec<usize>,
        label: String,
        tools: Vec<(String, usize)>,
        /// How many of them followed words in a tool result.
        followed: usize,
    },
}

impl Item {
    /// A group's line under its name: its tools, and how many followed words read.
    pub fn note(tools: &[(String, usize)], followed: usize) -> String {
        let tools = Self::sub(tools);
        if followed > 0 {
            format!("{tools} · 글 따름 {followed}")
        } else {
            tools
        }
    }

    /// The tools a group ran: "Read 24 · Grep 7 · 외 4".
    pub fn sub(tools: &[(String, usize)]) -> String {
        let mut parts: Vec<String> = tools
            .iter()
            .take(3)
            .map(|(t, n)| format!("{t} {n}"))
            .collect();
        let rest: usize = tools.iter().skip(3).map(|(_, n)| n).sum();
        if rest > 0 {
            parts.push(format!("외 {rest}"));
        }
        parts.join(" · ")
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Turn {
    /// Its number, from 1.
    pub n: usize,
    pub req: String,
    /// The subagent whose report the engine recorded as this turn; none when a person wrote it.
    pub agent: Option<String>,
    /// When the engine saw the request, in milliseconds.
    pub asked: Option<i64>,
    pub calls: Vec<Call>,
    pub items: Vec<Item>,
}

impl Turn {
    pub fn warn(&self) -> usize {
        self.calls.iter().filter(|c| c.warn).count()
    }
    pub fn blocked(&self) -> usize {
        self.calls.iter().filter(|c| c.blocked).count()
    }
    pub fn ok(&self) -> usize {
        self.calls.iter().filter(|c| c.allowed).count()
    }
    /// When it began: the request, or its first Call when the request has no time.
    pub fn at(&self) -> Option<i64> {
        self.asked.or_else(|| self.calls.iter().find_map(|c| c.at))
    }
}

/// Turns in a row that worked on the same files: a line of the overview.
#[derive(Clone, Debug, PartialEq)]
pub struct Task {
    pub name: String,
    /// Its first and last turn, by index.
    pub from: usize,
    pub to: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Flow {
    pub turns: Vec<Turn>,
    pub tasks: Vec<Task>,
    /// Each Call's turn and index in it.
    at: HashMap<i64, (usize, usize)>,
}

fn short(t: &str, n: usize) -> String {
    let line = t
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("");
    if line.chars().count() > n {
        format!("{}…", line.chars().take(n - 1).collect::<String>())
    } else {
        line.to_owned()
    }
}

impl Flow {
    /// From a map with every run unfolded: one station per Call.
    pub fn build(m: &Metro) -> Self {
        let mut order: Vec<&Station> = m.stations.iter().collect();
        order.sort_by(|a, b| a.x.total_cmp(&b.x));
        let mut turns: Vec<Turn> = m
            .asks
            .iter()
            .enumerate()
            .map(|(i, a)| Turn {
                n: i + 1,
                req: a.text.clone(),
                agent: a.agent.clone(),
                asked: a.at,
                calls: Vec::new(),
                items: Vec::new(),
            })
            .collect();
        if turns.is_empty() {
            turns.push(Turn {
                n: 1,
                req: String::new(),
                agent: None,
                asked: None,
                calls: Vec::new(),
                items: Vec::new(),
            });
        }
        let mut plain: Vec<Vec<bool>> = vec![Vec::new(); turns.len()];
        let mut at = HashMap::new();
        for s in order {
            let t = m.asks.iter().rposition(|a| a.x <= s.x).unwrap_or(0);
            let c = Call::of(m, s);
            plain[t].push(c.plain());
            at.insert(c.seq, (t, turns[t].calls.len()));
            turns[t].calls.push(c);
        }
        for (t, turn) in turns.iter_mut().enumerate() {
            let mut run: Vec<usize> = Vec::new();
            let flush = |run: &mut Vec<usize>, items: &mut Vec<Item>, calls: &[Call]| {
                if run.len() >= 2 {
                    let mut tools: Vec<(String, usize)> = Vec::new();
                    for &i in run.iter() {
                        match tools.iter_mut().find(|(t, _)| *t == calls[i].tool) {
                            Some((_, n)) => *n += 1,
                            None => tools.push((calls[i].tool.clone(), 1)),
                        }
                    }
                    tools.sort_by_key(|t| std::cmp::Reverse(t.1));
                    let all_ok = run.iter().all(|&i| calls[i].allowed);
                    let label = if all_ok {
                        format!("허용 {}개", run.len())
                    } else {
                        format!("호출 {}개", run.len())
                    };
                    let followed = run.iter().filter(|&&i| calls[i].by.is_some()).count();
                    items.push(Item::Group {
                        calls: std::mem::take(run),
                        label,
                        tools,
                        followed,
                    });
                } else {
                    items.extend(run.drain(..).map(Item::Call));
                }
            };
            let mut items = Vec::new();
            for (i, &p) in plain[t].iter().enumerate() {
                if p {
                    run.push(i);
                } else {
                    flush(&mut run, &mut items, &turn.calls);
                    items.push(Item::Call(i));
                }
            }
            flush(&mut run, &mut items, &turn.calls);
            turn.items = items;
        }
        let tasks = Self::tasks(&turns);
        Flow { turns, tasks, at }
    }

    /// Turns in a row that touched the same files are one task; a person's request that
    /// touches none of the task's files starts the next, and so does a task grown long.
    fn tasks(turns: &[Turn]) -> Vec<Task> {
        let files = |t: &Turn| -> HashSet<String> {
            t.calls
                .iter()
                .filter(|c| c.key.starts_with("f:"))
                .map(|c| c.key.clone())
                .collect()
        };
        let mut out: Vec<Task> = Vec::new();
        let mut seen: HashSet<String> = HashSet::new();
        for (i, t) in turns.iter().enumerate() {
            let keys = files(t);
            let fresh = match out.last() {
                None => true,
                Some(task) => {
                    i - task.from >= TASK_TURNS
                        || (t.agent.is_none() && !keys.is_empty() && keys.is_disjoint(&seen))
                }
            };
            if fresh {
                let name = short(&t.req, 22);
                out.push(Task {
                    name: if name.is_empty() {
                        format!("턴 {}", t.n)
                    } else {
                        name
                    },
                    from: i,
                    to: i,
                });
                seen.clear();
            }
            out.last_mut().unwrap().to = i;
            seen.extend(keys);
        }
        out
    }

    pub fn call(&self, seq: i64) -> Option<&Call> {
        self.at.get(&seq).map(|&(t, i)| &self.turns[t].calls[i])
    }

    /// The turn a Call was made in.
    pub fn turn_of(&self, seq: i64) -> Option<usize> {
        self.at.get(&seq).map(|&(t, _)| t)
    }

    pub fn task_of(&self, turn: usize) -> usize {
        self.tasks
            .iter()
            .position(|t| turn >= t.from && turn <= t.to)
            .unwrap_or(0)
    }

    pub fn chapters(&self) -> usize {
        self.tasks.len().div_ceil(CHAPTER).max(1)
    }

    pub fn chapter_of(task: usize) -> usize {
        task / CHAPTER
    }

    /// The tasks of a chapter, by index.
    pub fn chapter_tasks(&self, ch: usize) -> std::ops::Range<usize> {
        let a = (ch * CHAPTER).min(self.tasks.len());
        a..((ch + 1) * CHAPTER).min(self.tasks.len())
    }

    pub fn task_turns(&self, task: usize) -> std::ops::RangeInclusive<usize> {
        let t = &self.tasks[task];
        t.from..=t.to
    }
}

// ── The overview ──────────────────────────────────────────────────────────────────────────

type Pt = (f64, f64);

/// The line shapes the overview lays tasks on, on a 1000 × 640 canvas: horizontal, vertical and
/// 45° runs only, crossing one another as a city's lines do.
const TEMPLATES: [&[Pt]; CHAPTER] = [
    &[
        (300., 40.),
        (300., 140.),
        (390., 230.),
        (620., 230.),
        (720., 330.),
        (720., 520.),
        (640., 600.),
    ],
    &[
        (60., 600.),
        (230., 430.),
        (330., 430.),
        (450., 310.),
        (940., 310.),
    ],
    &[
        (80., 40.),
        (80., 200.),
        (240., 360.),
        (470., 360.),
        (720., 610.),
    ],
    &[
        (230., 620.),
        (110., 500.),
        (110., 330.),
        (270., 170.),
        (440., 170.),
        (560., 50.),
        (840., 50.),
    ],
    &[(40., 520.), (380., 520.), (450., 590.), (960., 590.)],
    &[(960., 450.), (800., 450.), (660., 310.), (660., 120.)],
    &[(560., 625.), (560., 400.), (480., 320.), (480., 110.)],
];
const TW: f64 = 1000.0;
const TH: f64 = 640.0;

/// A place on the overview.
#[derive(Clone, Debug, PartialEq)]
pub struct Place {
    pub key: String,
    pub name: String,
    /// The tasks that went there.
    pub tasks: Vec<usize>,
    pub blocked: bool,
    pub warn: bool,
    /// Words read there told a Call.
    pub text: bool,
    /// The first turn each task went there in.
    first: HashMap<usize, usize>,
    /// How many Calls went there.
    visits: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct OLine {
    pub task: usize,
    pub color: &'static str,
    pub d: String,
    /// Each end: the terminal, and where its number badge sits.
    pub ends: [(Pt, Pt); 2],
}

#[derive(Clone, Debug, PartialEq)]
pub struct OStop {
    pub place: usize,
    pub task: usize,
    pub x: f64,
    pub y: f64,
    /// The way the line runs there: names go aslant along a level run, beside a steep one.
    pub level: bool,
    /// The turn picking it opens.
    pub turn: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct OXfer {
    pub place: usize,
    pub x: f64,
    pub y: f64,
    pub tasks: [usize; 2],
}

#[derive(Clone, Debug, PartialEq)]
pub struct Overview {
    pub places: Vec<Place>,
    pub lines: Vec<OLine>,
    pub stops: Vec<OStop>,
    pub xfers: Vec<OXfer>,
    /// The chapter's tasks that went nowhere, talk only: listed, but drawn as no line.
    pub bare: Vec<usize>,
}

impl Overview {
    pub fn turn_for(&self, place: usize, task: usize, flow: &Flow) -> usize {
        self.places[place]
            .first
            .get(&task)
            .copied()
            .unwrap_or(flow.tasks[task].from)
    }
}

fn seg_len(a: Pt, b: Pt) -> f64 {
    (b.0 - a.0).hypot(b.1 - a.1)
}

fn path_len(p: &[Pt]) -> f64 {
    p.windows(2).map(|w| seg_len(w[0], w[1])).sum()
}

/// The point `t` along a path, and the way the path runs there.
fn along(p: &[Pt], mut t: f64) -> (Pt, Pt) {
    for i in 1..p.len() {
        let l = seg_len(p[i - 1], p[i]);
        if t <= l || i == p.len() - 1 {
            let r = if l > 0.0 { (t / l).min(1.0) } else { 0.0 };
            let (a, b) = (p[i - 1], p[i]);
            return (
                (a.0 + (b.0 - a.0) * r, a.1 + (b.1 - a.1) * r),
                (b.0 - a.0, b.1 - a.1),
            );
        }
        t -= l;
    }
    (p[0], (1.0, 0.0))
}

/// How far along a path the point nearest `q` is.
fn project(p: &[Pt], q: Pt) -> f64 {
    let (mut acc, mut best) = (0.0, (0.0, f64::INFINITY));
    for i in 1..p.len() {
        let (a, b) = (p[i - 1], p[i]);
        let l = seg_len(a, b);
        let u = if l > 0.0 {
            (((q.0 - a.0) * (b.0 - a.0) + (q.1 - a.1) * (b.1 - a.1)) / (l * l)).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let d = (a.0 + (b.0 - a.0) * u - q.0).hypot(a.1 + (b.1 - a.1) * u - q.1);
        if d < best.1 {
            best = (acc + u * l, d);
        }
        acc += l;
    }
    best.0
}

/// Where two paths cross.
fn crossings(p: &[Pt], q: &[Pt]) -> Vec<Pt> {
    let mut out = Vec::new();
    for i in 1..p.len() {
        for j in 1..q.len() {
            let (a, b, c, d) = (p[i - 1], p[i], q[j - 1], q[j]);
            let den = (b.0 - a.0) * (d.1 - c.1) - (b.1 - a.1) * (d.0 - c.0);
            if den.abs() < 1e-9 {
                continue;
            }
            let u = ((c.0 - a.0) * (d.1 - c.1) - (c.1 - a.1) * (d.0 - c.0)) / den;
            let v = ((c.0 - a.0) * (b.1 - a.1) - (c.1 - a.1) * (b.0 - a.0)) / den;
            if u > 0.02 && u < 0.98 && v > 0.02 && v < 0.98 {
                out.push((a.0 + (b.0 - a.0) * u, a.1 + (b.1 - a.1) * u));
            }
        }
    }
    out
}

/// A path through `pts` with each corner rounded.
pub fn rounded(pts: &[Pt], r: f64) -> String {
    let mut p: Vec<Pt> = Vec::new();
    for &q in pts {
        if p.last().is_none_or(|&l| seg_len(l, q) > 0.5) {
            p.push(q);
        }
    }
    if p.len() < 2 {
        return String::new();
    }
    let mut d = format!("M{:.1} {:.1}", p[0].0, p[0].1);
    for i in 1..p.len() - 1 {
        let (a, b, c) = (p[i - 1], p[i], p[i + 1]);
        let (l1, l2) = (seg_len(a, b), seg_len(b, c));
        let rr = r.min(l1 / 2.0).min(l2 / 2.0);
        d += &format!(
            " L{:.1} {:.1} Q{:.1} {:.1} {:.1} {:.1}",
            b.0 - (b.0 - a.0) / l1 * rr,
            b.1 - (b.1 - a.1) / l1 * rr,
            b.0,
            b.1,
            b.0 + (c.0 - b.0) / l2 * rr,
            b.1 + (c.1 - b.1) / l2 * rr
        );
    }
    let z = p[p.len() - 1];
    d + &format!(" L{:.1} {:.1}", z.0, z.1)
}

impl Flow {
    /// The places a chapter's tasks went to, and each task's way through them.
    fn places(&self, tasks: std::ops::Range<usize>) -> (Vec<Place>, HashMap<usize, Vec<usize>>) {
        let mut places: Vec<Place> = Vec::new();
        let mut index: HashMap<String, usize> = HashMap::new();
        let mut paths: HashMap<usize, Vec<usize>> = HashMap::new();
        let mut visit =
            |task: usize, turn: usize, c: &Call, text: bool, places: &mut Vec<Place>| {
                let i = *index.entry(c.key.clone()).or_insert_with(|| {
                    places.push(Place {
                        key: c.key.clone(),
                        name: c.place.clone(),
                        tasks: Vec::new(),
                        blocked: false,
                        warn: false,
                        text: false,
                        first: HashMap::new(),
                        visits: 0,
                    });
                    places.len() - 1
                });
                let p = &mut places[i];
                if !p.tasks.contains(&task) {
                    p.tasks.push(task);
                }
                p.first.entry(task).or_insert(turn);
                if text {
                    p.text = true;
                } else {
                    p.blocked |= c.blocked;
                    p.warn |= c.warn;
                    p.visits += 1;
                }
                let path = paths.entry(task).or_default();
                if !path.contains(&i) {
                    path.push(i);
                }
            };
        for task in tasks {
            for turn in self.task_turns(task) {
                for c in &self.turns[turn].calls {
                    if let Some(b) = c.by.and_then(|b| self.call(b)) {
                        visit(task, turn, b, true, &mut places);
                    }
                    visit(task, turn, c, false, &mut places);
                }
            }
        }
        // A line shows what matters on it: its stops, the words it followed, the places it
        // shares; then the places it went to most.
        for path in paths.values_mut() {
            let keep = |i: &usize| {
                let p = &places[*i];
                p.blocked || p.warn || p.text || p.tasks.len() > 1
            };
            // Stops and the words followed first, then what is shared, as long as there is room.
            let rank = |i: &usize| {
                let p = &places[*i];
                if p.blocked || p.warn || p.text { 0 } else { 1 }
            };
            let mut kept: Vec<usize> = path.iter().copied().filter(keep).collect();
            kept.sort_by_key(rank);
            kept.truncate(LINE_STOPS);
            let mut rest: Vec<usize> = path.iter().copied().filter(|i| !keep(i)).collect();
            rest.sort_by_key(|&i| std::cmp::Reverse(places[i].visits));
            let room = LINE_STOPS.saturating_sub(kept.len());
            kept.extend(rest.into_iter().take(room));
            path.retain(|i| kept.contains(i));
        }
        (places, paths)
    }

    /// The overview of a chapter on a `w` × `h` canvas.
    pub fn overview(&self, chapter: usize, w: f64, h: f64) -> Overview {
        let tasks = self.chapter_tasks(chapter);
        let (places, paths) = self.places(tasks.clone());
        let k = ((w - 90.0) / TW).min((h - 210.0) / TH).max(0.2);
        let (ox, oy) = ((w - TW * k) / 2.0, 92.0);
        let s = |p: &Pt| (ox + p.0 * k, oy + p.1 * k);
        let path = |t: usize| paths.get(&t).cloned().unwrap_or_default();
        // A task keeps its shape and colour by its place in the chapter, drawn or not.
        let (drawn, bare): (Vec<_>, Vec<_>) =
            tasks.clone().enumerate().partition(|(_, t)| !path(*t).is_empty());
        let bare: Vec<usize> = bare.into_iter().map(|(_, t)| t).collect();
        let lines: Vec<(usize, usize, Vec<Pt>)> = drawn
            .into_iter()
            .map(|(i, t)| (i, t, TEMPLATES[i].iter().map(s).collect()))
            .collect();

        // Interchanges: where two shapes cross, a place both tasks went to.
        let mut fixed: HashMap<usize, Vec<(usize, f64)>> = HashMap::new();
        let mut xfers: Vec<OXfer> = Vec::new();
        for (i, (_, a, ta)) in lines.iter().enumerate() {
            for (_, b, tb) in lines.iter().skip(i + 1) {
                let mut shared: Vec<usize> = path(*a)
                    .into_iter()
                    .filter(|&p| places[p].tasks.contains(b) && !xfers.iter().any(|x| x.place == p))
                    .collect();
                for pt in crossings(ta, tb) {
                    if shared.is_empty() {
                        break;
                    }
                    let p = shared.remove(0);
                    xfers.push(OXfer {
                        place: p,
                        x: pt.0,
                        y: pt.1,
                        tasks: [*a, *b],
                    });
                    fixed.entry(*a).or_default().push((p, project(ta, pt)));
                    fixed.entry(*b).or_default().push((p, project(tb, pt)));
                }
            }
        }

        // The rest of each line's stations, evenly along it, stepping aside from interchanges.
        let mut stops = Vec::new();
        for (_, t, tpl) in &lines {
            let l = path_len(tpl);
            let fx = fixed.get(t).cloned().unwrap_or_default();
            let margin = 36.0 * k.max(0.8);
            let gap = 24.0 * k.max(0.8);
            let others: Vec<usize> = path(*t)
                .into_iter()
                .filter(|p| !fx.iter().any(|f| f.0 == *p))
                .collect();
            let n = others.len().max(1) as f64;
            for (j, p) in others.iter().enumerate() {
                let mut tt = margin + (l - 2.0 * margin) * (j as f64 + 0.5) / n;
                for f in &fx {
                    if (tt - f.1).abs() < gap {
                        tt = f.1 + if tt < f.1 { -gap } else { gap };
                    }
                }
                let ((x, y), (dx, dy)) = along(tpl, tt.min(l - margin * 0.6).max(margin * 0.6));
                stops.push(OStop {
                    place: *p,
                    task: *t,
                    x,
                    y,
                    level: dx.abs() > dy.abs() * 1.5,
                    turn: places[*p]
                        .first
                        .get(t)
                        .copied()
                        .unwrap_or(self.tasks[*t].from),
                });
            }
        }

        let lines = lines
            .iter()
            .map(|(i, t, tpl)| {
                let end = |e: Pt, n: Pt| {
                    let (dx, dy) = (e.0 - n.0, e.1 - n.1);
                    let l = dx.hypot(dy).max(1.0);
                    (e, (e.0 + dx / l * 28.0, e.1 + dy / l * 28.0))
                };
                let z = tpl.len() - 1;
                OLine {
                    task: *t,
                    color: LINE_COLORS[i % LINE_COLORS.len()],
                    d: rounded(tpl, 34.0 * k),
                    ends: [end(tpl[0], tpl[1]), end(tpl[z], tpl[z - 1])],
                }
            })
            .collect();
        Overview {
            places,
            lines,
            stops,
            xfers,
            bare,
        }
    }
}

// ── One task's line ───────────────────────────────────────────────────────────────────────

const LEFT: f64 = 120.0;
const TOP: f64 = 200.0;
const ROW_H: f64 = 220.0;
const BRANCH_ROW_H: f64 = 340.0;
const BR_DY: f64 = 120.0;
const BR_STEP: f64 = 160.0;
const MARK_STEP: f64 = 80.0;
const TICK: f64 = 72.0;
const STOP: f64 = 100.0;
const IMPORTANT: f64 = 150.0;
const PRE: f64 = 40.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Start,
    Tick,
    Stop,
    Transfer,
    Warn,
    Blocked,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Stop {
    pub id: String,
    pub turn: usize,
    pub x: f64,
    pub y: f64,
    pub kind: Kind,
    pub label: String,
    pub sub: String,
    /// On a words' branch.
    pub branch: bool,
    /// Its name over the line, else under it; none for a turn's start.
    pub up: Option<bool>,
    /// The way from the turn's start to it.
    pub route: String,
    /// The Call it is, or the group's index in its turn's items.
    pub seq: Option<i64>,
    pub group: Option<usize>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Seg {
    pub turn: usize,
    pub d: String,
    pub branch: bool,
    pub end: Option<Pt>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TurnMark {
    pub turn: usize,
    pub x: f64,
    pub y: f64,
    /// The room to the next turn's mark.
    pub avail: f64,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BranchLabel {
    pub turn: usize,
    pub x: f64,
    pub y: f64,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Line {
    pub stops: Vec<Stop>,
    pub segs: Vec<Seg>,
    pub marks: Vec<TurnMark>,
    pub branch_labels: Vec<BranchLabel>,
    pub width: f64,
    pub height: f64,
}

impl Flow {
    /// The turns given, by index, as one line running right.
    pub fn line(&self, turns: &[usize]) -> Line {
        let x = |o: f64| LEFT + o;
        let by = TOP + BR_DY;
        let mut stops: Vec<Stop> = Vec::new();
        let mut marks: Vec<(usize, f64)> = Vec::new();
        // Each interchange: its turn, where on the line, the place, how many branch stops.
        let mut xfers: Vec<(usize, f64, String, i64, usize)> = Vec::new();
        let mut off = 0.0_f64;
        let mut branch_end = 0.0_f64;
        let mut parity = 0usize;
        let route_to = |mark: f64, at: f64| format!("M{} {TOP} H{}", x(mark), x(at));
        let branch_path = |tr: f64, upto: f64| {
            let p = x(tr);
            format!(
                "L{} {} Q{} {} {} {} H{}",
                p + 50.0,
                by - 10.0,
                p + 60.0,
                by,
                p + 75.0,
                by,
                upto
            )
        };
        let branch_x = |tr: f64, bi: usize| x(tr) + 75.0 + 85.0 + bi as f64 * BR_STEP;

        for &t in turns {
            let turn = &self.turns[t];
            let mark = off;
            marks.push((t, off));
            stops.push(Stop {
                id: format!("t{t}-s"),
                turn: t,
                x: x(off),
                y: TOP,
                kind: Kind::Start,
                label: String::new(),
                sub: String::new(),
                branch: false,
                up: None,
                route: String::new(),
                seq: None,
                group: None,
            });
            off += MARK_STEP;
            for (gi, item) in turn.items.iter().enumerate() {
                match item {
                    Item::Group { label, tools, followed, .. } => {
                        stops.push(Stop {
                            id: format!("t{t}-{gi}"),
                            turn: t,
                            x: x(off),
                            y: TOP,
                            kind: Kind::Tick,
                            label: label.clone(),
                            sub: Item::note(tools, *followed),
                            branch: false,
                            up: {
                                parity += 1;
                                Some(parity % 2 == 1)
                            },
                            route: route_to(mark, off),
                            seq: None,
                            group: Some(gi),
                        });
                        off += TICK;
                    }
                    Item::Call(i) => {
                        let c = &turn.calls[*i];
                        let name = short(
                            c.title.trim_end_matches(" 읽기").trim_end_matches(" 수정"),
                            28,
                        );
                        if let (Some(via), Some(b)) = (&c.via, c.by) {
                            // The words it followed: a branch off the place they were read at.
                            let found = xfers.iter().position(|x| x.0 == t && x.3 == b);
                            let xi = match found {
                                Some(xi) => xi,
                                None => {
                                    if off > 0.0 {
                                        off += PRE;
                                    }
                                    off = off.max(branch_end + PRE);
                                    stops.push(Stop {
                                        id: format!("t{t}-via{b}"),
                                        turn: t,
                                        x: x(off),
                                        y: TOP,
                                        kind: Kind::Transfer,
                                        label: via.clone(),
                                        sub: "읽은 글".into(),
                                        branch: false,
                                        // Over the line: its branch leaves under it.
                                        up: {
                                            parity = 1;
                                            Some(true)
                                        },
                                        route: route_to(mark, off),
                                        seq: Some(b),
                                        group: None,
                                    });
                                    xfers.push((t, off, via.clone(), b, 0));
                                    off += IMPORTANT;
                                    xfers.len() - 1
                                }
                            };
                            let (_, tr, _, _, bi) = xfers[xi];
                            xfers[xi].4 += 1;
                            let bx = branch_x(tr, bi);
                            branch_end = branch_end
                                .max(bx + 90.0 + 60.0 + via.chars().count() as f64 * 13.0 - LEFT);
                            stops.push(Stop {
                                id: format!("t{t}-{gi}"),
                                turn: t,
                                x: bx,
                                y: by,
                                kind: if c.blocked {
                                    Kind::Blocked
                                } else if c.warn {
                                    Kind::Warn
                                } else {
                                    Kind::Stop
                                },
                                label: name,
                                sub: c.sub(),
                                branch: true,
                                up: None,
                                route: format!("{} {}", route_to(mark, tr), branch_path(tr, bx)),
                                seq: Some(c.seq),
                                group: None,
                            });
                            continue;
                        }
                        let important = c.blocked || c.warn;
                        if important && off > 0.0 {
                            off += PRE;
                        }
                        stops.push(Stop {
                            id: format!("t{t}-{gi}"),
                            turn: t,
                            x: x(off),
                            y: TOP,
                            kind: if c.blocked {
                                Kind::Blocked
                            } else if c.warn {
                                Kind::Warn
                            } else {
                                Kind::Stop
                            },
                            label: name,
                            sub: if c.blocked {
                                format!("{} · 차단", c.sub())
                            } else if c.warn {
                                format!("{} · 확인 요청", c.sub())
                            } else if c.loose {
                                format!("{} · 시킨 것 없음", c.sub())
                            } else {
                                c.sub()
                            },
                            branch: false,
                            up: {
                                parity += 1;
                                Some(parity % 2 == 1)
                            },
                            route: route_to(mark, off),
                            seq: Some(c.seq),
                            group: None,
                        });
                        off += if important { IMPORTANT } else { STOP };
                    }
                }
            }
        }

        let end = stops
            .iter()
            .filter(|s| !s.branch)
            .map(|s| s.x - LEFT)
            .fold(0.0, f64::max)
            + 50.0;
        let mut segs: Vec<Seg> = marks
            .iter()
            .enumerate()
            .map(|(i, &(t, o))| {
                let next = marks.get(i + 1).map(|m| m.1);
                Seg {
                    turn: t,
                    d: route_to(o, next.unwrap_or(end)),
                    branch: false,
                    end: next.is_none().then_some((x(end), TOP)),
                }
            })
            .collect();
        let mut branch_labels = Vec::new();
        for &(t, tr, ref via, _, count) in &xfers {
            let end_x = branch_x(tr, count.saturating_sub(1)) + 90.0;
            segs.push(Seg {
                turn: t,
                d: format!("M{} {TOP} {}", x(tr), branch_path(tr, end_x)),
                branch: true,
                end: Some((end_x, by)),
            });
            branch_labels.push(BranchLabel {
                turn: t,
                x: end_x + 18.0,
                y: by,
                text: format!("{via}에서 읽은 글"),
            });
        }
        let marks = marks
            .iter()
            .enumerate()
            .map(|(i, &(t, o))| TurnMark {
                turn: t,
                x: x(o),
                y: TOP,
                avail: marks.get(i + 1).map_or(f64::INFINITY, |m| m.1 - o),
                text: short(&self.turns[t].req, 80),
            })
            .collect();
        let width = x(end.max(branch_end)) + 420.0;
        let height = TOP
            + if xfers.is_empty() {
                ROW_H
            } else {
                BRANCH_ROW_H
            };
        Line {
            stops,
            segs,
            marks,
            branch_labels,
            width,
            height,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::demo::Demo;
    use crate::layout::Model;

    fn flow_of(text: &str, prefix: &str) -> Flow {
        let demo = Demo::parse(text);
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
        let all: HashSet<i64> = model.nodes.iter().map(|n| n.seq()).collect();
        Flow::build(&Metro::build_with(&model, &all))
    }

    fn long(prefix: &str) -> Flow {
        let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/mock/long.json"))
            .expect("mock/long.json: run python3 -I mock/make_long.py");
        flow_of(&text, prefix)
    }

    #[test]
    fn every_call_is_in_its_turn_once() {
        let f = long("c4000a1b");
        assert_eq!(f.turns.len(), 40);
        for t in &f.turns {
            let mut seen: Vec<usize> = Vec::new();
            for item in &t.items {
                match item {
                    Item::Call(i) => seen.push(*i),
                    Item::Group { calls, .. } => {
                        assert!(calls.len() >= 2);
                        seen.extend(calls)
                    }
                }
            }
            seen.sort();
            assert_eq!(seen, (0..t.calls.len()).collect::<Vec<_>>(), "turn {}", t.n);
        }
    }

    #[test]
    fn tasks_cover_the_turns_in_order() {
        let f = long("c4000a1b");
        assert_eq!(f.tasks[0].from, 0);
        assert_eq!(f.tasks.last().unwrap().to, f.turns.len() - 1);
        for w in f.tasks.windows(2) {
            assert_eq!(w[1].from, w[0].to + 1);
        }
        assert!(f.tasks.iter().all(|t| t.to - t.from < TASK_TURNS));
    }

    #[test]
    fn a_stop_is_never_folded_away() {
        let f = long("c4000a1b");
        let stopped: usize = f.turns.iter().map(|t| t.warn() + t.blocked()).sum();
        let shown: usize = f
            .turns
            .iter()
            .flat_map(|t| t.items.iter().map(move |i| (t, i)))
            .filter(|(t, i)| matches!(i, Item::Call(c) if t.calls[*c].warn || t.calls[*c].blocked))
            .count();
        assert!(stopped > 0);
        assert_eq!(stopped, shown);
    }

    #[test]
    fn words_followed_branch_off_where_they_were_read() {
        let f = flow_of(
            &std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/mock/fixture.json"))
                .unwrap(),
            "fa6576f7",
        );
        let turns: Vec<usize> = (0..f.turns.len()).collect();
        let line = f.line(&turns);
        let branch: Vec<&Stop> = line.stops.iter().filter(|s| s.branch).collect();
        assert!(
            !branch.is_empty(),
            "fa6576f7 has a Call told by a tool result"
        );
        assert!(line.stops.iter().any(|s| s.kind == Kind::Transfer));
        assert!(line.segs.iter().any(|s| s.branch));
        for s in branch {
            assert!(s.y > TOP);
        }
    }

    #[test]
    fn the_overview_draws_a_line_per_task_of_the_chapter() {
        let f = long("c4000a1b");
        let o = f.overview(0, 1100.0, 760.0);
        assert_eq!(o.lines.len() + o.bare.len(), f.chapter_tasks(0).len());
        for l in &o.lines {
            assert!(o.stops.iter().any(|s| s.task == l.task) || o.xfers.iter().any(|x| x.tasks.contains(&l.task)));
        }
        for x in &o.xfers {
            for t in x.tasks {
                assert!(o.places[x.place].tasks.contains(&t));
            }
        }
        // No line shows more stations than it has room for.
        for l in &o.lines {
            let n = o.stops.iter().filter(|s| s.task == l.task).count()
                + o.xfers.iter().filter(|x| x.tasks.contains(&l.task)).count();
            assert!(n <= LINE_STOPS + CHAPTER, "line {} has {n}", l.task);
            assert!(o.stops.iter().filter(|s| s.task == l.task).count() <= LINE_STOPS);
        }
    }
}
