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
    /// What in a tool result told it, when the engine traced it to one.
    pub cause: Option<Cause>,
    /// A value it used (an address, a key, a path outside the project) that first came out of a
    /// tool result in an earlier turn and never from the person: where that was.
    pub origin: Option<Origin>,
    /// What kind of thing it did, for its colour.
    pub tone: Tone,
    /// Which kind of tool, for its icon.
    pub glyph: Glyph,
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
            origin: None,
            cause: s.source.as_ref().filter(|_| by.is_some()).map(|src| Cause {
                result: src.result,
                paragraph: src.paragraph,
                p: src.probability,
                error: src.error,
                quote: src.quote.clone(),
            }),
            tone: Tone::of(&s.tool, s.action),
            glyph: Glyph::of(&s.tool),
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

    /// Nothing to single out: let through, told by the request. One that followed words read
    /// rides a branch of its own; one that failed is folded with the rest.
    fn plain(&self) -> bool {
        !self.warn && !self.blocked && !self.loose && self.by.is_none()
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

/// Why a Call rode a branch: the result it read (by seq, and paragraph), how sure the engine
/// was, whether that result was the tool failing, and the words in a line.
#[derive(Clone, Debug, PartialEq)]
pub struct Cause {
    pub result: i64,
    pub paragraph: Option<i64>,
    pub p: f64,
    pub error: bool,
    pub quote: Option<String>,
}

/// Where a value a Call used first came out, in an earlier turn: the value as indexed, its kind,
/// the tool result it was in, the Call that result came from, and that Call's turn.
#[derive(Clone, Debug, PartialEq)]
pub struct Origin {
    pub value: String,
    pub kind: String,
    pub result: i64,
    pub from: i64,
    pub turn: usize,
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
    /// The tools a group ran, short enough to sit under its station: "Read 24 · Grep 7 외 4".
    pub fn brief(tools: &[(String, usize)]) -> String {
        let mut text = tools
            .iter()
            .take(2)
            .map(|(t, n)| format!("{t} {n}"))
            .collect::<Vec<_>>()
            .join(" · ");
        let rest: usize = tools.iter().skip(2).map(|(_, n)| n).sum();
        if rest > 0 {
            text.push_str(&format!(" 외 {rest}"));
        }
        text
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Turn {
    /// Its number, from 1.
    pub n: usize,
    pub req: String,
    /// The subagent whose report the engine recorded as this turn; none when a person wrote it.
    pub agent: Option<String>,
    /// What the agent wrote it would do this turn: the request as the engine measured Calls
    /// against it.
    pub intent: Option<String>,
    /// When the engine saw the request, in milliseconds.
    pub asked: Option<i64>,
    pub calls: Vec<Call>,
    pub items: Vec<Item>,
    /// What the agent said to the person in this turn, in order; the last is its answer.
    pub said: Vec<crate::metro::Said>,
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

/// How wide a name is drawn at `size` px: Latin, digits and marks about 0.6 of the size,
/// Hangul and other wide letters the whole size.
pub fn text_width(text: &str, size: f64) -> f64 {
    text.chars()
        .map(|c| if c.is_ascii() { 0.6 } else { 1.0 })
        .sum::<f64>()
        * size
}

/// An intent without the "(확신도: …)" the engine appends to it.
pub fn without_confidence(text: &str) -> &str {
    let text = text.trim();
    match text.rfind('(') {
        Some(i) if text.ends_with(')') && (text[i..].contains("확신도") || text[i..].contains("confidence")) => text[..i].trim_end(),
        _ => text,
    }
}

/// A long text cut to its gist for a heading: its first sentence, without a trailing
/// "(확신도: …)", at most `n` characters and cut between words.
pub fn gist(text: &str, n: usize) -> String {
    let text = without_confidence(text);
    let first = text
        .split_inclusive(['\n', '。'])
        .next()
        .unwrap_or(text)
        .trim();
    let first = match first.find(". ") {
        Some(i) => &first[..=i],
        None => first,
    };
    if first.chars().count() <= n {
        return first.to_owned();
    }
    let cut: String = first.chars().take(n).collect();
    let cut = match cut.rfind(' ') {
        Some(i) if i > cut.len() / 2 => cut[..i].to_owned(),
        _ => cut,
    };
    format!("{}…", cut.trim_end_matches([',', '.', ' ']))
}

/// What a message to the person says came of the turn, in a sentence: past code, tables, rules
/// and lines that open a list ("…:"), a short label taken off ("현황: …"), the first sentence
/// that tells an outcome (done, failed, blocked, the cause, "…했습니다"), else the first one.
pub fn outcome(text: &str, n: usize) -> String {
    const CUES: [&str; 18] = [
        "완료", "성공", "실패", "차단", "막혔", "막았", "막혀", "오류", "에러", "원인", "결과",
        "했습니다", "됐습니다", "되었습니다", "없습니다", "끝났", "확인했", "찾았",
    ];
    // Each line kept, and whether it was an item of a list.
    let mut lines: Vec<(String, bool, bool)> = Vec::new();
    let mut fenced = false;
    for raw in text.lines() {
        let l = raw.trim();
        if l.starts_with("```") {
            fenced = !fenced;
            continue;
        }
        if fenced || l.is_empty() || l.starts_with('|') || l.chars().all(|c| "-=_*~|: ".contains(c)) {
            continue;
        }
        // A list's numbering, "1. " or "2) ", not a number the words begin with.
        let digits = l.chars().take_while(char::is_ascii_digit).count();
        let numbered = digits > 0
            && l[digits..].starts_with(['.', ')'])
            && l[digits + 1..].starts_with(' ');
        // A heading ("## …", or a line all in bold) names a part rather than saying how it went;
        // so does a question. They count as list items: taken only when nothing else is there.
        // ("## 현황: Judge 진행 중" says how it stands, so it is no mere heading.)
        let heading = (l.starts_with('#') && !l.contains(": "))
            || (l.starts_with("**") && l.ends_with("**") && l.matches("**").count() == 2)
            || l.trim_end_matches(['*', ' ']).ends_with(['?', '？']);
        let item = l.starts_with(['-', '*', '•']) && !heading || numbered || heading;
        let l = if numbered { &l[digits + 2..] } else { l };
        // Marks at the start: headings, bullets, quotes, emoji.
        let l = l
            .trim_start_matches(|c: char| !c.is_alphanumeric() && c != '"' && c != '\'' && c != '(' && c != '`')
            .replace("**", "")
            .replace("__", "")
            .replace('`', "");
        let l = l.trim();
        // A line that opens a list ("…:") keeps a sentence it ends one before:
        // "INFUSE — 흥미로운 프로젝트네요! 요약:" is "INFUSE — 흥미로운 프로젝트네요!".
        let l = if l.ends_with(':') || l.ends_with('：') {
            // The last sentence end followed by a space.
            let chars: Vec<(usize, char)> = l.char_indices().collect();
            let end = chars
                .windows(2)
                .rev()
                .find(|w| ['!', '.', '다', '요'].contains(&w[0].1) && w[1].1.is_whitespace())
                .map(|w| w[1].0);
            match end {
                Some(i) => &l[..i],
                None => continue,
            }
        } else {
            l
        };
        if l.is_empty() {
            continue;
        }
        // A short label before the words: "현황: Judge 진행 중".
        let l = match l.split_once(": ") {
            Some((label, rest)) if label.chars().count() <= 10 && !rest.trim().is_empty() => rest.trim(),
            _ => l,
        };
        lines.push((l.to_owned(), item, heading));
        if lines.len() >= 40 {
            break;
        }
    }
    // An outcome said, else a whole sentence not in a list, else the first line.
    let sentence = |l: &str| l.ends_with(['다', '요', '.', '!']) || l.ends_with("다.") || l.ends_with("요.");
    // An outcome said, else a whole sentence not in a list, else any line that is not a
    // heading, else the first.
    let pick = lines
        .iter()
        .find(|(l, _, h)| !h && CUES.iter().any(|c| l.contains(c)))
        .or_else(|| lines.iter().find(|(l, item, _)| !item && sentence(l)))
        .or_else(|| lines.iter().find(|(_, _, h)| !h))
        .or(lines.first())
        .map(|(l, _, _)| l.clone())
        .unwrap_or_default();
    gist(&pick, n)
}

/// A value without the quotes a command line wrapped it in.
fn bare(s: &str) -> &str {
    s.trim().trim_matches(|c| matches!(c, '\'' | '"' | '`'))
}

/// A turn's heading: what it was for (the intent, else the request) and how it ended.
fn heading_of(t: &Turn) -> (String, Option<String>) {
    let text = t.intent.as_deref().unwrap_or(&t.req);
    // An aside after the gist ("… (구체적 목표는 모호함 — …)") is left for the full card.
    let text = match text.find(" (") {
        Some(i) if text[..i].chars().count() >= 12 => &text[..i],
        _ => text,
    };
    let head = gist(text, 130);
    let res = t
        .said
        .last()
        .map(|m| outcome(&m.text, 60))
        .filter(|g| !g.is_empty());
    (head, res)
}

/// A message as plain words: without Markdown's bold, code and heading marks.
pub fn plain_words(text: &str) -> String {
    text.replace("**", "")
        .replace("__", "")
        .replace('`', "")
        .lines()
        .map(|l| l.trim_start_matches('#').trim())
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

/// What a station's icon shows: the kind of tool, finer than its colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Glyph {
    Doc,
    Search,
    Edit,
    Run,
    Web,
    Plug,
    Agent,
    Other,
}

impl Glyph {
    pub const ALL: [Glyph; 8] = [
        Glyph::Doc,
        Glyph::Search,
        Glyph::Edit,
        Glyph::Run,
        Glyph::Web,
        Glyph::Plug,
        Glyph::Agent,
        Glyph::Other,
    ];

    fn of(tool: &str) -> Self {
        if crate::metro::mcp_parts(tool).is_some() {
            return Glyph::Plug;
        }
        match tool {
            "Read" | "NotebookRead" => Glyph::Doc,
            "Grep" | "Glob" | "LS" | "WebSearch" | "ToolSearch" => Glyph::Search,
            "Edit" | "MultiEdit" | "Write" | "NotebookEdit" => Glyph::Edit,
            "Bash" | "BashOutput" | "KillShell" => Glyph::Run,
            "WebFetch" => Glyph::Web,
            "Agent" | "Task" | "SubagentHandback" => Glyph::Agent,
            _ => Glyph::Other,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Glyph::Doc => "파일 읽기",
            Glyph::Search => "찾기",
            Glyph::Edit => "수정·작성",
            Glyph::Run => "명령 실행",
            Glyph::Web => "웹 페이지",
            Glyph::Plug => "MCP 도구",
            Glyph::Agent => "에이전트",
            Glyph::Other => "그 밖의 도구",
        }
    }

    /// Its colour: the colour of the kind of thing it does.
    pub fn tone(self) -> Tone {
        match self {
            Glyph::Doc | Glyph::Search => Tone::Read,
            Glyph::Edit => Tone::Change,
            Glyph::Run => Tone::Run,
            Glyph::Web | Glyph::Plug => Tone::Outside,
            Glyph::Agent => Tone::Delegate,
            Glyph::Other => Tone::Other,
        }
    }
}

/// What a station did, as a metro map colours its lines: the person's request, reading or
/// searching, changing files, running a command, a tool outside (MCP, the web), handing to an
/// agent. Red, amber and the words' lavender stay for the verdicts and the branch.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Tone {
    Ask,
    Report,
    Read,
    Change,
    Run,
    Outside,
    Delegate,
    Other,
    /// The agent saying something to the person.
    Say,
    /// A stretch to a Call that followed words read: not a kind of tool, the words' line.
    Words,
}

impl Tone {
    pub const ALL: [Tone; 8] = [
        Tone::Ask,
        Tone::Read,
        Tone::Change,
        Tone::Run,
        Tone::Outside,
        Tone::Delegate,
        Tone::Other,
        Tone::Say,
    ];

    fn of(tool: &str, action: crate::metro::Action) -> Self {
        use crate::metro::Action;
        if crate::metro::mcp_parts(tool).is_some() || matches!(tool, "WebFetch" | "WebSearch") {
            return Tone::Outside;
        }
        match action {
            Action::Read | Action::Search => Tone::Read,
            Action::Change => Tone::Change,
            Action::Run => Tone::Run,
            Action::Delegate => Tone::Delegate,
            Action::Other => Tone::Other,
        }
    }

    pub fn color(self) -> &'static str {
        match self {
            Tone::Ask => "#16a34a",
            Tone::Report => "#6b7280",
            Tone::Read => "#2563eb",
            Tone::Change => "#c026d3",
            Tone::Run => "#334155",
            Tone::Outside => "#0891b2",
            Tone::Delegate => "#a16207",
            Tone::Other => "#94a3b8",
            Tone::Say => "#7c3aed",
            Tone::Words => "#8e7cc3",
        }
    }
}

/// One leg of a task's summary, as a route finder lists a route's rides: the request, the
/// words read that a stopped Call followed, the stopped Call, or the Calls let through.
#[derive(Clone, Debug, PartialEq)]
pub enum Leg {
    Ask(usize),
    Text(String),
    /// Calls denied or held in a row at one place count as one leg.
    Stopped { blocked: bool, name: String, times: usize },
    Allowed(usize),
}

/// A task at a glance: how long it took, its legs (at most `LEGS`, with how many more), and
/// its counts.
#[derive(Clone, Debug, PartialEq)]
pub struct Summary {
    pub took: Option<i64>,
    pub legs: Vec<Leg>,
    pub more: usize,
    pub turns: usize,
    pub calls: usize,
    pub blocked: usize,
    pub warn: usize,
}

/// How many legs a summary shows.
const LEGS: usize = 5;

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
                intent: a.intent.clone().filter(|i| !i.trim().is_empty()),
                asked: a.at,
                calls: Vec::new(),
                items: Vec::new(),
                said: Vec::new(),
            })
            .collect();
        if turns.is_empty() {
            turns.push(Turn {
                n: 1,
                req: String::new(),
                agent: None,
                intent: None,
                asked: None,
                calls: Vec::new(),
                items: Vec::new(),
                said: Vec::new(),
            });
        }
        let mut at = HashMap::new();
        for s in order {
            let t = m.asks.iter().rposition(|a| a.x <= s.x).unwrap_or(0);
            let c = Call::of(m, s);
            at.insert(c.seq, (t, turns[t].calls.len()));
            turns[t].calls.push(c);
        }
        // A Call stands alone when it is worth seeing: stopped, told by nothing, told by words
        // it read (it rides a branch), or the one whose result told another (an interchange).
        let parents: HashSet<i64> = turns
            .iter()
            .flat_map(|t| t.calls.iter().filter_map(|c| c.by))
            .collect();
        let plain: Vec<Vec<bool>> = turns
            .iter()
            .map(|t| {
                t.calls
                    .iter()
                    .map(|c| c.plain() && !parents.contains(&c.seq))
                    .collect()
            })
            .collect();
        // Each message to the turn it was said in: the last request at or before its column.
        for sd in &m.says {
            let t = m.asks.iter().rposition(|a| a.x <= sd.x).unwrap_or(0);
            if let Some(turn) = turns.get_mut(t) {
                turn.said.push(sd.clone());
            }
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

    /// Finds, for each Call, a value it used that came from a tool result of an earlier turn
    /// (see `Origin`), from the values the Generator indexed. A value the person wrote is
    /// theirs; paths inside the session's folder or under /tmp are everyday and skipped.
    pub fn trace_values(&mut self, model: &crate::layout::Model) {
        use crate::model::TokenRow;
        let cwd = model
            .nodes
            .iter()
            .find_map(|n| n.props.get("cwd").and_then(serde_json::Value::as_str).map(str::to_owned))
            .unwrap_or_default();
        // A path inside the project, relative or under its folder, or a scratch one, is what any
        // turn names anyway; it says nothing of where the value came from.
        let everyday = |t: &TokenRow| {
            let v = bare(&t.text);
            matches!(t.kind.as_str(), "path" | "unc_path")
                && (!(v.starts_with('/') || v.starts_with('~') || v.starts_with('\\'))
                    || v.starts_with("/tmp")
                    || v.starts_with("/private/tmp")
                    || (!cwd.is_empty() && (v.starts_with(&cwd) || cwd.starts_with(v))))
        };
        let mut by_key: HashMap<&str, Vec<&TokenRow>> = HashMap::new();
        for t in &model.tokens {
            by_key.entry(t.key.as_str()).or_default().push(t);
        }
        // What a value says, for choosing among a Call's: secrets and personal data first, then
        // where data can go, then paths; a fragment too short to mean anything not at all.
        let weight = |t: &TokenRow| -> i32 {
            let meaningful = bare(&t.text).chars().filter(|c| c.is_alphanumeric()).count() >= 4;
            if !meaningful {
                return -1;
            }
            match t.category.as_str() {
                "credential" | "pii" | "financial" | "device" => 4,
                _ => match t.kind.as_str() {
                    "url" | "email" | "socket_address" | "db_dsn" | "cloud_arn" | "ip" => 3,
                    _ => 1,
                },
            }
        };
        let mut found: Vec<(i64, i32, Origin)> = Vec::new();
        for t in model.tokens.iter().filter(|t| t.slot == "agent" && !everyday(t) && weight(t) >= 0) {
            let Some(turn) = self.turn_of(t.seq) else { continue };
            let same = &by_key[t.key.as_str()];
            if same.iter().any(|o| o.slot == "user" && o.seq <= t.seq) {
                continue;
            }
            let Some(first) = same.iter().filter(|o| o.slot == "external" && o.seq < t.seq).min_by_key(|o| o.seq) else {
                continue;
            };
            let Some(&from) = model.call_of.get(&first.seq) else { continue };
            let Some(from_turn) = self.turn_of(from) else { continue };
            if from_turn >= turn {
                continue;
            }
            let w = weight(t) * 1000 + t.text.chars().count().min(999) as i32;
            let origin = Origin { value: bare(&t.text).to_owned(), kind: t.kind.clone(), result: first.seq, from, turn: from_turn };
            match found.iter_mut().find(|(s, _, _)| *s == t.seq) {
                Some(slot) if slot.1 < w => *slot = (t.seq, w, origin),
                Some(_) => {}
                None => found.push((t.seq, w, origin)),
            }
        }
        for (seq, _, origin) in found {
            if let Some(&(ti, ci)) = self.at.get(&seq) {
                self.turns[ti].calls[ci].origin = Some(origin);
            }
        }
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

    /// A task's summary. The legs are the turns with a Call denied or held, each with the words
    /// those Calls followed and the Calls; a task with none is its request and the Calls let
    /// through.
    pub fn summary(&self, task: usize) -> Summary {
        let turns: Vec<&Turn> = self.task_turns(task).map(|i| &self.turns[i]).collect();
        let start = turns.iter().find_map(|t| t.at());
        let end = turns
            .iter()
            .flat_map(|t| t.calls.iter().filter_map(|c| c.at))
            .max();
        let took = match (start, end) {
            (Some(a), Some(b)) if b >= a => Some(b - a),
            _ => None,
        };
        let calls: usize = turns.iter().map(|t| t.calls.len()).sum();
        let mut legs = Vec::new();
        for t in &turns {
            let stopped: Vec<&Call> = t.calls.iter().filter(|c| c.blocked || c.warn).collect();
            if stopped.is_empty() {
                continue;
            }
            legs.push(Leg::Ask(t.n));
            let mut read: Option<&String> = None;
            for c in stopped {
                if let Some(via) = c.via.as_ref().filter(|v| read != Some(*v)) {
                    legs.push(Leg::Text(short(via, 18)));
                    read = Some(via);
                }
                let name = short(&c.title, 24);
                match legs.last_mut() {
                    Some(Leg::Stopped { blocked, name: n, times })
                        if *blocked == c.blocked && *n == name =>
                    {
                        *times += 1
                    }
                    _ => legs.push(Leg::Stopped {
                        blocked: c.blocked,
                        name,
                        times: 1,
                    }),
                }
            }
        }
        if legs.is_empty() {
            legs.push(Leg::Ask(turns.first().map_or(1, |t| t.n)));
            if calls > 0 {
                legs.push(Leg::Allowed(calls));
            }
        }
        let more = legs.len().saturating_sub(LEGS);
        legs.truncate(LEGS);
        Summary {
            took,
            legs,
            more,
            turns: turns.len(),
            calls,
            blocked: turns.iter().map(|t| t.blocked()).sum(),
            warn: turns.iter().map(|t| t.warn()).sum(),
        }
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
/// The room added between turns, and how far a turn's zone begins before its start.
const TURN_GAP: f64 = 60.0;
/// From one row of the folded line to the next, and how far a zone reaches above its row.
const ROW_SP: f64 = 370.0;
/// The width to height the folded line aims at, about a screen's.
const ASPECT: f64 = 1.6;
const ZONE_UP: f64 = 206.0;
const ZONE_LEAD: f64 = 70.0;
const BR_DY: f64 = 120.0;
/// How far under the lower of its two ends a passage from a cause runs.
const LINK_DY: f64 = 112.0;
const MARK_STEP: f64 = 80.0;
const TICK: f64 = 72.0;
/// The step between a group's small stations, and the largest group always shown open.
const MINI: f64 = 64.0;
const OPEN_ALWAYS: usize = 6;
/// Whether no group is ever shown folded: every Call its own small station.
const FOLD_NONE: bool = true;
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
    /// The agent's answer to the person: the turn's last message.
    Say,
    /// A Call let through, a small station in an opened group.
    Mini,
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
    /// What it did: a turn's start is the request, a group the tool it ran most.
    pub tone: Tone,
    /// Its result held words another Call followed: a branch leaves it.
    pub xfer: bool,
    /// The icon of the tool it ran, for a Call's station.
    pub glyph: Option<Glyph>,
    /// The Calls after `seq` folded into it: the same thing done again right after.
    pub also: Vec<i64>,
    /// How many names' height further from the line its name stands, to clear a neighbour's.
    pub tier: u8,
    /// Its Call followed words read in a tool result: the stretch to it is the words' violet.
    pub follows: bool,
    /// A value it used first came out of a tool result in this earlier turn (by index).
    pub crossing: Option<usize>,
}

impl Stop {
    /// Whether it stands for the Call.
    pub fn is(&self, seq: i64) -> bool {
        self.seq == Some(seq) || self.also.contains(&seq)
    }
}

/// Folds a Call into the station just laid when it did the same thing there again: a
/// plain station of the same turn and lane, the same name. Its name counts the times.
fn fold_into_last(stops: &mut [Stop], turn: usize, y: f64, label: &str, seq: i64) -> bool {
    let Some(last) = stops.last_mut() else {
        return false;
    };
    let base = match last.also.len() {
        0 => last.label.as_str(),
        _ => last.label.rsplit_once(" ×").map_or(last.label.as_str(), |(b, _)| b),
    };
    if last.turn != turn
        || (last.y - y).abs() > 0.5
        || base != label
        || last.xfer
        || !matches!(last.kind, Kind::Mini | Kind::Stop)
    {
        return false;
    }
    last.also.push(seq);
    last.label = format!("{label} ×{}", last.also.len() + 1);
    true
}

/// A stretch of a turn's line coloured for the station it leads to.
#[derive(Clone, Debug, PartialEq)]
pub struct ToneSeg {
    pub turn: usize,
    pub d: String,
    pub tone: Tone,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Seg {
    pub turn: usize,
    pub d: String,
    pub branch: bool,
    pub end: Option<Pt>,
    /// The way back from a row's end to the next row's start.
    pub ret: bool,
    /// A branch the engine was less than even sure of: drawn dashed.
    pub unsure: bool,
    /// A passage from a cause to the Call that followed its words, under the line.
    pub link: bool,
    /// For a passage: the Calls it is about (the cause and those that followed it), shown only
    /// when one of them is picked.
    pub about: Vec<i64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TurnMark {
    pub turn: usize,
    pub x: f64,
    pub y: f64,
    /// The turn's zone, a rectangle (x, y, width, height) on each row it runs along; its
    /// heading sits at the top of the first.
    pub zones: Vec<(f64, f64, f64, f64)>,
    /// Its first zone is on a row running left: its heading stands at the zone's right, where
    /// the turn starts.
    pub rev: bool,
    /// The request, whole: the zone's heading wraps it.
    pub text: String,
    /// What the turn was for: the agent's intent, else the request, cut to its gist.
    pub head: String,
    /// How the turn ended: the gist of the agent's last word to the person.
    pub result: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BranchLabel {
    pub turn: usize,
    pub x: f64,
    pub y: f64,
    /// On a row running left: the label ends at `x`.
    pub rev: bool,
    pub text: String,
    /// Whether the label at the branch's end is drawn (not when the next branch comes down
    /// into it).
    pub end: bool,
    /// What told the branch, quoted where it bends away: the box's corner and width.
    pub cause: Option<Cause>,
    pub qx: f64,
    pub qy: f64,
    pub qw: f64,
    /// The Calls its passage is about: shown only when one of them is picked.
    pub about: Vec<i64>,
}

/// The quote box of a branch: its width for the words.
pub fn quote_width(c: &Cause) -> f64 {
    let words = c.quote.as_deref().unwrap_or("결과 보기");
    (text_width(words, 11.5) + 120.0).clamp(150.0, 320.0)
}

#[derive(Clone, Debug, PartialEq)]
pub struct Line {
    pub stops: Vec<Stop>,
    pub segs: Vec<Seg>,
    pub marks: Vec<TurnMark>,
    pub branch_labels: Vec<BranchLabel>,
    /// The main line in the colours of what was done along it.
    pub tones: Vec<ToneSeg>,
    /// Small arrows on the line, (x, y, degrees), saying which way it is read.
    pub arrows: Vec<(f64, f64, f64)>,
    pub width: f64,
    pub height: f64,
}

impl Flow {
    /// The turns given, by index, as one line running right, its groups folded, unwrapped.
    #[cfg(test)]
    pub fn line(&self, turns: &[usize]) -> Line {
        self.line_open(turns, &HashSet::new(), f64::INFINITY)
    }

    /// The line with the groups given, by (turn, item), opened into their Calls. A group of
    /// `OPEN_ALWAYS` Calls or fewer is always open.
    ///
    /// Rows are `wrap` wide: the line runs right, turns down at the edge and runs back, as a
    /// metro map folds a long line; an infinite `wrap` keeps it one row.
    pub fn line_open(&self, turns: &[usize], open: &HashSet<(usize, usize)>, wrap: f64) -> Line {
        let x = |o: f64| LEFT + o;
        let mut stops: Vec<Stop> = Vec::new();
        let mut marks: Vec<(usize, f64)> = Vec::new();
        // Each Call told by words it read in another of its turn: (the cause's stop, its stop).
        let mut links: Vec<(usize, usize)> = Vec::new();
        let mut off = 0.0_f64;
        let route_to = |mark: f64, at: f64| format!("M{} {TOP} H{}", x(mark), x(at));
        // Names all stand over the line (the links run under it), each clear of the next.
        let step_after = |label: &str, size: f64, base: f64| base.max(text_width(label, size) + 26.0);

        for &t in turns {
            let turn = &self.turns[t];
            if !marks.is_empty() {
                off += TURN_GAP;
            }
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
                tone: if turn.agent.is_some() { Tone::Report } else { Tone::Ask },
                xfer: false,
                glyph: None,
                also: Vec::new(),
                tier: 0,
                follows: false,
                crossing: None,
            });
            off += MARK_STEP;
            for (gi, item) in turn.items.iter().enumerate() {
                match item {
                    // Every group is laid open: a small station for each Call, the same thing done
                    // again in a row counted on one (×n).
                    Item::Group { label, calls, .. } if FOLD_NONE || calls.len() <= OPEN_ALWAYS || open.contains(&(t, gi)) => {
                        if !FOLD_NONE && calls.len() > OPEN_ALWAYS {
                            stops.push(Stop {
                                id: format!("t{t}-{gi}"),
                                turn: t,
                                x: x(off),
                                y: TOP,
                                kind: Kind::Tick,
                                label: label.clone(),
                                sub: "접기".into(),
                                branch: false,
                                up: Some(true),
                                route: route_to(mark, off),
                                seq: None,
                                group: Some(gi),
                                tone: Tone::Other,
                                xfer: false,
                                glyph: None,
                                also: Vec::new(),
                                tier: 0,
                                follows: false,
                                crossing: None,
                            });
                            off += step_after(label, 14.0, MINI);
                        }
                        for &i in calls {
                            let c = &turn.calls[i];
                            if fold_into_last(&mut stops, t, TOP, &short(&c.title, 26), c.seq) {
                                continue;
                            }
                            stops.push(Stop {
                                id: format!("t{t}-{gi}-{i}"),
                                turn: t,
                                x: x(off),
                                y: TOP,
                                kind: Kind::Mini,
                                // What it did; the tool is its icon and colour.
                                label: short(&c.title, 26),
                                sub: match &c.server {
                                    Some(srv) => short(srv, 14),
                                    None => c.tool.clone(),
                                },
                                branch: false,
                                up: Some(true),
                                route: route_to(mark, off),
                                seq: Some(c.seq),
                                group: None,
                                tone: c.tone,
                                xfer: false,
                                glyph: Some(c.glyph),
                                also: Vec::new(),
                                tier: 0,
                                follows: false,
                                crossing: c.origin.as_ref().map(|o| o.turn),
                            });
                            off += step_after(&stops[stops.len() - 1].label, 12.0, MINI);
                        }
                    }
                    Item::Group { label, tools, calls, .. } => {
                        let mut counts: HashMap<Tone, usize> = HashMap::new();
                        for &i in calls {
                            *counts.entry(turn.calls[i].tone).or_default() += 1;
                        }
                        let tone = Tone::ALL
                            .into_iter()
                            .max_by_key(|t| counts.get(t).copied().unwrap_or(0))
                            .unwrap_or(Tone::Other);
                        stops.push(Stop {
                            id: format!("t{t}-{gi}"),
                            turn: t,
                            x: x(off),
                            y: TOP,
                            kind: Kind::Tick,
                            label: label.clone(),
                            sub: Item::brief(tools),
                            branch: false,
                            up: Some(true),
                            route: route_to(mark, off),
                            seq: None,
                            group: Some(gi),
                            tone,
                            xfer: false,
                            glyph: None,
                            also: Vec::new(),
                            tier: 0,
                            follows: false,
                            crossing: None,
                        });
                        off += step_after(label, 14.0, TICK);
                    }
                    Item::Call(i) => {
                        let c = &turn.calls[*i];
                        let name = short(
                            c.title.trim_end_matches(" 읽기").trim_end_matches(" 수정"),
                            28,
                        );
                        let kind = if c.blocked {
                            Kind::Blocked
                        } else if c.warn {
                            Kind::Warn
                        } else {
                            Kind::Stop
                        };
                        // Told by words it read in a Call of this turn: that Call becomes an
                        // interchange, and a passage runs from it to this one under the line.
                        let cause = c.by.and_then(|b| stops.iter().position(|st| st.turn == t && st.is(b)));
                        let important = c.blocked || c.warn;
                        if cause.is_none()
                            && !important
                            && !c.loose
                            && c.via.is_none()
                            && fold_into_last(&mut stops, t, TOP, &name, c.seq)
                        {
                            continue;
                        }
                        if let Some(ci) = cause {
                            if !stops[ci].xfer {
                                stops[ci].sub = "환승".into();
                            }
                            stops[ci].xfer = true;
                            if matches!(stops[ci].kind, Kind::Stop | Kind::Mini) {
                                stops[ci].kind = Kind::Transfer;
                            }
                            links.push((ci, stops.len()));
                        }
                        if important && off > 0.0 {
                            off += PRE;
                        }
                        stops.push(Stop {
                            id: format!("t{t}-{gi}"),
                            turn: t,
                            x: x(off),
                            y: TOP,
                            kind,
                            label: name,
                            sub: if c.blocked {
                                format!("{} · 차단", c.sub())
                            } else if c.warn {
                                format!("{} · 확인 요청", c.sub())
                            } else if c.loose {
                                format!("{} · 시킨 것 없음", c.sub())
                            } else if c.via.is_some() {
                                format!("{} · 글을 따름", c.sub())
                            } else {
                                c.sub()
                            },
                            branch: false,
                            up: Some(true),
                            route: route_to(mark, off),
                            seq: Some(c.seq),
                            group: None,
                            tone: c.tone,
                            xfer: false,
                            glyph: Some(c.glyph),
                            also: Vec::new(),
                            tier: 0,
                            follows: c.via.is_some(),
                            crossing: c.origin.as_ref().map(|o| o.turn),
                        });
                        let base = if important { IMPORTANT } else { STOP };
                        off += step_after(&stops[stops.len() - 1].label, 15.5, base);
                    }
                }
            }
            // The turn ends where the agent answered the person: its last message.
            if !turn.said.is_empty() {
                let earlier = turn.said.len() - 1;
                stops.push(Stop {
                    id: format!("t{t}-say"),
                    turn: t,
                    x: x(off),
                    y: TOP,
                    kind: Kind::Say,
                    // The heading above says what it ended on, whole; all of it opens from here.
                    label: "답변".into(),
                    sub: if earlier > 0 {
                        format!("중간 메시지 {earlier}개")
                    } else {
                        String::new()
                    },
                    branch: false,
                    up: Some(true),
                    route: route_to(mark, off),
                    seq: None,
                    group: None,
                    tone: Tone::Say,
                    xfer: false,
                    glyph: None,
                    also: Vec::new(),
                    tier: 0,
                    follows: false,
                    crossing: None,
                });
                off += STOP;
            }
            // A turn's zone is wide enough for its heading, so what it was for and how it ended
            // read whole: short turns get room rather than a cut heading.
            let (head, res) = heading_of(turn);
            let w = text_width(&head, 13.5).max(res.as_deref().map_or(0.0, |r| text_width(r, 12.5)));
            let need = if w + 40.0 <= 420.0 {
                w + 40.0
            } else if w / 2.0 + 70.0 <= 620.0 {
                (w / 2.0 + 70.0).max(420.0)
            } else {
                (w / 3.0 + 80.0).clamp(620.0, 760.0)
            };
            off = off.max(mark + need - TURN_GAP);
        }

        // Names that would still meet (an interchange's name beside a long one) stand a step
        // further up.
        let span = |st: &Stop| {
            let size = if st.kind == Kind::Mini { 12.0 } else { 15.5 };
            let half = text_width(&st.label, size) / 2.0 + 6.0;
            (st.x - half, st.x + half)
        };
        let named: Vec<usize> = (0..stops.len()).filter(|&i| !stops[i].label.is_empty()).collect();
        for k in 1..named.len() {
            let (p, c) = (named[k - 1], named[k]);
            if span(&stops[p]).1 > span(&stops[c]).0 {
                stops[c].tier = (stops[p].tier + 1) % 2;
            }
        }

        // Where the line runs: on the main track, or down on the track of words followed, for
        // the Calls that followed words read in a tool result. One train, one line: it goes down
        // when the agent acts on what it read and comes back up when it acts on the request.
        let o_of = |x: f64| x - LEFT;
        let level: Vec<f64> = stops.iter().map(|s| if s.follows { 1.0 } else { 0.0 }).collect();
        // Each passage from a cause to the Call that followed it, as one with the run of Calls
        // after it that followed the same words: (cause, first follower, last follower).
        let mut passages: Vec<(usize, usize, usize)> = Vec::new();
        for &(ci, fi) in &links {
            if let Some(p) = passages.iter().position(|&(c, _, l)| c == ci && (l + 1..fi).all(|k| !stops[k].follows || links.iter().any(|&(c2, f2)| c2 == ci && f2 == k))) {
                passages[p].2 = fi;
                continue;
            }
            passages.push((ci, fi, fi));
        }

        let end = stops.iter().map(|s| s.x - LEFT).fold(0.0, f64::max) + 50.0;

        // The line folded into rows as a snake: row 0 runs right, row 1 back left under it, and
        // so on, each row as long as the turns on it. Rows are cut between turns where one fits,
        // else between two stations, and only where the line is on its main track.
        let mut spans: Vec<(f64, f64)> = Vec::new();
        for i in 0..stops.len() {
            if level[i] > 0.0 && (i == 0 || level[i - 1] == 0.0) {
                let a = if i > 0 { o_of(stops[i - 1].x) } else { o_of(stops[i].x) };
                let j = (i..stops.len()).find(|&j| level[j] == 0.0).unwrap_or(stops.len() - 1);
                spans.push((a, o_of(stops[j].x)));
            }
        }
        let far = end + 60.0;
        let start = -ZONE_LEAD - 10.0;
        let wrap = if wrap.is_finite() {
            wrap.max((ASPECT * (far - start) * ROW_SP).sqrt())
        } else {
            wrap
        };
        let mut main_os: Vec<f64> = stops.iter().map(|s| o_of(s.x)).collect();
        main_os.sort_by(f64::total_cmp);
        main_os.dedup();
        let turn_cuts: Vec<f64> = marks.iter().skip(1).map(|m| m.1 - ZONE_LEAD - 5.0).collect();
        let mut cuts = vec![start];
        if wrap.is_finite() {
            let mut row = start;
            while far - row > wrap {
                if let Some(c) = turn_cuts
                    .iter()
                    .copied()
                    .filter(|&c| c > row + 1.0 && c - row <= wrap)
                    .reduce(f64::max)
                {
                    cuts.push(c);
                    row = c;
                    continue;
                }
                let mut best = None;
                let mut over = None;
                for w in main_os.windows(2) {
                    let c = (w[0] + w[1]) / 2.0;
                    if c <= row + 1.0 || spans.iter().any(|&(a, b)| a < c && c < b) {
                        continue;
                    }
                    if c - row <= wrap {
                        best = Some(c);
                    } else {
                        over = Some(c);
                        break;
                    }
                }
                let best = best.filter(|&c| c - row >= wrap * 0.4);
                match best.or(over) {
                    Some(c) => {
                        cuts.push(c);
                        row = c;
                    }
                    None => break,
                }
            }
        }
        let rows = cuts.len();
        let dir = |r: usize| if r.is_multiple_of(2) { 1.0 } else { -1.0 };
        let len = |r: usize| cuts.get(r + 1).copied().unwrap_or(far) - cuts[r];
        let mut anchor = vec![0.0_f64];
        for r in 0..rows {
            anchor.push(anchor[r] + dir(r) * len(r));
        }
        let lo = anchor.iter().copied().fold(f64::INFINITY, f64::min);
        let hi = anchor.iter().copied().fold(f64::NEG_INFINITY, f64::max) - lo;
        let x0 = 80.0 - lo;
        let row_of = |o: f64| cuts.iter().rposition(|&c| c <= o).unwrap_or(0);
        // Each row as deep as its lower track, when the line goes down on it.
        let row_h: Vec<f64> = (0..rows)
            .map(|r| {
                let down = (0..stops.len()).any(|i| level[i] > 0.0 && row_of(o_of(stops[i].x)) == r);
                let base = ROW_SP - BR_DY + 40.0;
                if down { base + BR_DY + 100.0 } else { base }
            })
            .collect();
        let mut tops = vec![TOP];
        for r in 0..rows {
            tops.push(tops[r] + row_h[r]);
        }
        let row_y = |r: usize| tops[r];
        let map_x = |r: usize, o: f64| x0 + anchor[r] + dir(r) * (o - cuts[r]);
        let edge = |r: usize| x0 + anchor[r + 1] + dir(r) * 30.0;
        // Every point the line passes, in order: each station at its track, and the line's end.
        let mut along: Vec<(f64, f64)> = stops.iter().zip(&level).map(|(s, &l)| (o_of(s.x), l)).collect();
        along.push((end, 0.0));
        along.sort_by(|a, b| a.0.total_cmp(&b.0));
        let at = |o: f64, l: f64| {
            let r = row_of(o);
            (map_x(r, o), row_y(r) + l * BR_DY)
        };
        // The line from one offset to another through the stations between: along a track,
        // ramping down or up just before a station on the other track, and turning at the rows'
        // ends.
        let track = |a: f64, b: f64| {
            let pts_in: Vec<(f64, f64)> = along.iter().copied().filter(|&(o, _)| o >= a - 0.5 && o <= b + 0.5).collect();
            if pts_in.len() < 2 {
                return String::new();
            }
            let mut pts = vec![at(pts_in[0].0, pts_in[0].1)];
            for w in pts_in.windows(2) {
                let ((oa, la), (ob, lb)) = (w[0], w[1]);
                let (ra, rb) = (row_of(oa), row_of(ob));
                if ra != rb {
                    for r in ra..rb {
                        pts.push((edge(r), row_y(r)));
                        pts.push((edge(r), row_y(r + 1)));
                    }
                } else if (la - lb).abs() > 0.1 {
                    let d = dir(ra);
                    let (xb, yb) = at(ob, lb);
                    let ya = row_y(ra) + la * BR_DY;
                    let run = ((ob - oa) * 0.7).min(70.0);
                    pts.push((xb - d * run, ya));
                    pts.push((xb - d * 14.0, yb));
                }
                pts.push(at(ob, lb));
            }
            rounded(&pts, 22.0)
        };
        let mark_of: HashMap<usize, f64> = marks.iter().copied().collect();
        let offsets: Vec<f64> = stops.iter().map(|s| o_of(s.x)).collect();
        for (i, st) in stops.iter_mut().enumerate() {
            let o = offsets[i];
            if !st.route.is_empty() {
                let m = mark_of.get(&st.turn).copied().unwrap_or(o);
                st.route = track(m, o);
            }
            (st.x, st.y) = at(o, level[i]);
        }
        let mut segs: Vec<Seg> = marks
            .iter()
            .enumerate()
            .map(|(i, &(t, o))| {
                let next = marks.get(i + 1).map(|m| m.1);
                Seg {
                    turn: t,
                    d: track(o, next.unwrap_or(end)),
                    branch: false,
                    end: next.is_none().then(|| at(end, 0.0)),
                    ret: false,
                    unsure: false,
                    link: false,
                    about: Vec::new(),
                }
            })
            .collect();
        // A thin passage, for the Call picked, from the station whose words it followed (the
        // interchange) under the line to it. And over each stretch on the lower track that
        // followed the same words, a quote of them where the line goes down.
        let mut branch_labels = Vec::new();
        for &(ci, fi, li) in &passages {
            let (c, f) = (&stops[ci], &stops[fi]);
            let mut about: Vec<i64> = c.seq.into_iter().chain(c.also.iter().copied()).collect();
            for &(c2, f2) in &links {
                if c2 == ci && (fi..=li).contains(&f2) {
                    about.extend(stops[f2].seq.into_iter().chain(stops[f2].also.iter().copied()));
                }
            }
            let cause = self.call(f.seq.unwrap_or_default()).and_then(|call| call.cause.clone());
            // (Shown only when the way back is asked for.) Within a row it runs under both ends;
            // across rows, under the cause's row and then down into the follower from above.
            {
                let pts = if row_of(offsets[ci]) == row_of(offsets[fi]) {
                    let floor = c.y.max(f.y) + LINK_DY;
                    vec![(c.x, c.y + 14.0), (c.x, floor), (f.x, floor), (f.x, f.y + 14.0)]
                } else {
                    let below = row_y(row_of(offsets[ci])) + BR_DY + LINK_DY;
                    vec![(c.x, c.y + 14.0), (c.x, below), (f.x, below), (f.x, f.y - 16.0)]
                };
                segs.push(Seg {
                    turn: c.turn,
                    d: rounded(&pts, 16.0),
                    branch: true,
                    end: None,
                    ret: false,
                    unsure: cause.as_ref().is_some_and(|k| k.p < 0.5),
                    link: true,
                    about: about.clone(),
                });
            }
            // The quote hangs under the lower track from where the stretch starts (the names
            // stand over it). Where the line goes down it always shows; further along the lower
            // track, a change of words read shows only for the Call picked.
            let dips_here = fi == 0 || level[fi - 1] == 0.0;
            let last = &stops[li];
            let d = dir(row_of(offsets[fi]));
            let _ = last;
            let qw = cause.as_ref().map_or(0.0, quote_width).max(200.0);
            let qx = if d > 0.0 { f.x - 40.0 } else { f.x + 40.0 - qw };
            branch_labels.push(BranchLabel {
                turn: c.turn,
                x: f.x,
                y: f.y,
                rev: d < 0.0,
                // Where the words were read: the station they came out of.
                text: c.label.clone(),
                end: false,
                cause,
                qx,
                // Under the end pin that stands under a picked station.
                qy: f.y + 54.0,
                qw,
                about: if dips_here { Vec::new() } else { about },
            });
        }
        // A value that came from a tool result of an earlier turn: a passage (shown with the way
        // back) from the station whose result held it, and a note of the value under the Call.
        for (fi, f) in stops.iter().enumerate() {
            let Some(origin) = f.seq.and_then(|q| self.call(q)).and_then(|c| c.origin.clone()) else {
                continue;
            };
            let Some(ci) = stops.iter().position(|st| st.is(origin.from)) else { continue };
            let c = &stops[ci];
            let below = row_y(row_of(offsets[ci])) + BR_DY + LINK_DY;
            let pts = vec![(c.x, c.y + 14.0), (c.x, below), (f.x, below), (f.x, f.y - 16.0)];
            let about = vec![origin.from, f.seq.unwrap_or_default()];
            segs.push(Seg {
                turn: f.turn,
                d: rounded(&pts, 16.0),
                branch: true,
                end: None,
                ret: false,
                unsure: false,
                link: true,
                about: about.clone(),
            });
            let _ = fi;
            let cause = Cause {
                result: origin.result,
                paragraph: None,
                p: -1.0,
                error: false,
                quote: Some(origin.value.clone()),
            };
            let qw = quote_width(&cause).max(240.0);
            branch_labels.push(BranchLabel {
                turn: f.turn,
                x: f.x,
                y: f.y,
                rev: false,
                text: format!("턴 {} · {}", origin.turn + 1, c.label),
                end: false,
                cause: Some(cause),
                qx: f.x - 40.0,
                qy: f.y + 54.0,
                qw,
                about,
            });
        }
        // The arrows: between stations far enough apart on one track, and down each turn.
        let mut arrows: Vec<(f64, f64, f64)> = Vec::new();
        for w in along.windows(2) {
            let ((oa, la), (ob, lb)) = (w[0], w[1]);
            let (ra, rb) = (row_of(oa), row_of(ob));
            if ra == rb {
                if ob - oa > 70.0 && (la - lb).abs() < 0.1 {
                    let (x, y) = at((oa + ob) / 2.0, la);
                    arrows.push((x, y, if dir(ra) > 0.0 { 0.0 } else { 180.0 }));
                }
            } else {
                for r in ra..rb {
                    arrows.push((edge(r), (row_y(r) + row_y(r + 1)) / 2.0, 90.0));
                }
            }
        }
        // Each stretch of the line in the colour of the station it reaches: the tool's, or the
        // words' violet on the lower track.
        let mut tones = Vec::new();
        for &(t, _) in &marks {
            let mut run: Vec<(f64, Tone)> = Vec::new();
            for (st, o) in stops.iter().zip(offsets.iter()) {
                if st.turn == t {
                    run.push((*o, if st.follows { Tone::Words } else { st.tone }));
                }
            }
            run.sort_by(|a, b| a.0.total_cmp(&b.0));
            for w in run.windows(2) {
                tones.push(ToneSeg {
                    turn: t,
                    d: track(w[0].0, w[1].0),
                    tone: w[1].1,
                });
            }
        }
        let map = |o: f64| at(o, 0.0);
        let marks = marks
            .iter()
            .enumerate()
            .map(|(i, &(t, o))| {
                let z0 = o - ZONE_LEAD;
                let z1 = marks.get(i + 1).map_or(far, |m| m.1 - ZONE_LEAD);
                let mut zones = Vec::new();
                for r in row_of(z0.max(start))..=row_of((z1 - 1.0).max(start)) {
                    let lo = z0.max(cuts[r]);
                    let hi = z1.min(cuts.get(r + 1).copied().unwrap_or(far));
                    if hi <= lo {
                        continue;
                    }
                    let (a, b) = (map_x(r, lo), map_x(r, hi));
                    zones.push((a.min(b), row_y(r) - ZONE_UP, (a - b).abs(), row_h[r] - 30.0));
                }
                let (x, y) = map(o);
                TurnMark {
                    turn: t,
                    x,
                    y,
                    zones,
                    rev: dir(row_of(o)) < 0.0,
                    head: heading_of(&self.turns[t]).0,
                    result: heading_of(&self.turns[t]).1,
                    text: self.turns[t].req.trim().to_owned(),
                }
            })
            .collect();
        let width = 80.0 + hi + 110.0;
        // Tall enough for the quote boxes under the last row, a few stacked.
        let boxes = branch_labels.iter().map(|b| b.qy + 150.0).fold(0.0, f64::max);
        let height = (tops[rows] - ZONE_UP + 10.0).max(boxes);
        Line {
            stops,
            segs,
            marks,
            branch_labels,
            tones,
            arrows,
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
    fn words_followed_run_on_the_line_with_a_passage_from_where_they_were_read() {
        let f = flow_of(
            &std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/mock/fixture.json"))
                .unwrap(),
            "fa6576f7",
        );
        let turns: Vec<usize> = (0..f.turns.len()).collect();
        let line = f.line(&turns);
        // One line in the order made: on the main track, and down on the lower one exactly for
        // the Calls that followed words read.
        assert!(line.stops.iter().all(|s| !s.branch));
        let main_y = line.stops[0].y;
        for s in &line.stops {
            let want = if s.follows { main_y + BR_DY } else { main_y };
            assert!((s.y - want).abs() < 0.5, "{} at {} not {}", s.label, s.y, want);
        }
        // The Call told by a tool result follows words, and a passage leads to it from an
        // interchange.
        assert!(line.stops.iter().any(|s| s.follows), "fa6576f7 has a Call told by a tool result");
        assert!(line.stops.iter().any(|s| s.kind == Kind::Transfer || s.xfer));
        assert!(line.segs.iter().any(|s| s.link));
    }

    #[test]
    fn a_value_from_an_earlier_turns_result_is_traced_across_the_turn() {
        use crate::model::TokenRow;
        let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/mock/long.json")).unwrap();
        let demo = Demo::parse(&text);
        let id = demo
            .sessions()
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|s| s["id"].as_str())
            .find(|id| id.starts_with("c4000a1b"))
            .unwrap()
            .to_owned();
        let model = Model::build(&serde_json::from_value(demo.graph(&id).unwrap()).unwrap());
        let all: HashSet<i64> = model.nodes.iter().map(|n| n.seq()).collect();
        let base = Flow::build(&Metro::build_with(&model, &all));
        // A result of a Call in an earlier turn, and a Call some turns later.
        let (&result, &from) = model
            .call_of
            .iter()
            .filter(|(_, c)| base.turn_of(**c).is_some_and(|t| t < 3))
            .min_by_key(|(r, _)| **r)
            .unwrap();
        let later = base.turns[6].calls[0].seq;
        let token = |seq: i64, slot: &str| TokenRow {
            seq,
            key: "https://evil.example/upload".into(),
            kind: "url".into(),
            slot: slot.into(),
            category: "address".into(),
            text: "https://evil.example/upload".into(),
        };
        let mut m = model.clone();
        m.tokens = vec![token(result, "external"), token(later, "agent")];
        let mut f = base.clone();
        f.trace_values(&m);
        let origin = f.call(later).and_then(|c| c.origin.clone()).expect("traced across turns");
        assert_eq!((origin.from, origin.result), (from, result));
        assert_eq!(Some(origin.turn), base.turn_of(from));
        // The person wrote the value: it is theirs, not the result's.
        m.tokens.push(token(base.turns[0].calls[0].seq.min(result) - 1, "user"));
        let mut f = base.clone();
        f.trace_values(&m);
        assert!(f.call(later).and_then(|c| c.origin.clone()).is_none());
    }

    #[test]
    fn every_group_is_laid_open_into_its_calls() {
        let f = long("c0001a7e");
        let turns: Vec<usize> = (0..f.turns.len()).collect();
        let line = f.line_open(&turns, &HashSet::new(), f64::INFINITY);
        assert!(!line.stops.iter().any(|s| s.kind == Kind::Tick), "no group head is drawn");
        for (t, turn) in f.turns.iter().enumerate() {
            for item in &turn.items {
                if let Item::Group { calls, .. } = item {
                    for &i in calls {
                        assert!(
                            line.stops.iter().any(|s| s.turn == t && s.is(turn.calls[i].seq)),
                            "turn {} call {} has a station",
                            t + 1,
                            turn.calls[i].seq
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn a_long_line_folds_into_rows() {
        let f = long("c4000a1b");
        let turns: Vec<usize> = (0..f.turns.len()).collect();
        let line = f.line_open(&turns, &HashSet::new(), 900.0);
        let mut rows: Vec<f64> = line.stops.iter().map(|s| s.y).collect();
        rows.sort_by(f64::total_cmp);
        rows.dedup_by(|a, b| (*a - *b).abs() < 0.5);
        assert!(rows.len() > 1, "forty turns fold into more than one row");
        for m in &line.marks {
            assert!(!m.zones.is_empty(), "turn {} has a zone", m.turn + 1);
        }
        assert!(line.width / line.height > 0.8, "{} x {}", line.width, line.height);
    }

    #[test]
    fn an_answer_is_summed_up_by_what_came_of_it() {
        assert_eq!(
            outcome("**기억하기 (메모리 저장):**\n- 키 파일 위치\n- 실행 명령\n\n메모리에 저장해 두었습니다.", 40),
            "메모리에 저장해 두었습니다."
        );
        assert_eq!(outcome("## 현황: Judge 진행 중 ⏳\n남은 공격 12개", 40), "Judge 진행 중 ⏳");
        assert_eq!(
            outcome("좋은 질문입니다!\n\n| a | b |\n|---|---|\n\n훅이 rm -rf를 차단했습니다.", 40),
            "훅이 rm -rf를 차단했습니다."
        );
        assert_eq!(outcome("```\ncode\n```\n네, 알겠습니다", 40), "네, 알겠습니다");
        assert_eq!(outcome("13개 공격 trajectory 완료", 40), "13개 공격 trajectory 완료");
        assert_eq!(outcome("1. 첫째 할 일\n2. 둘째 할 일", 40), "첫째 할 일");
        assert_eq!(
            outcome("**INFUSE** — 흥미로운 프로젝트네요! 요약:\n\n### 🎯 이게 뭐하는 프로젝트\n- **목적**: 공격 벤치마크\n\n**더 도와드릴 수 있습니다!**", 60),
            "INFUSE — 흥미로운 프로젝트네요!"
        );
        assert_eq!(
            outcome("## 이게 뭐하는 프로젝트?\n\nINFUSE는 에이전트 공격을 재현하는 벤치마크입니다.", 60),
            "INFUSE는 에이전트 공격을 재현하는 벤치마크입니다."
        );
    }

    #[test]
    fn a_heading_takes_the_gist_of_a_long_intent() {
        assert_eq!(gist("타입 힌트 추가해줘", 40), "타입 힌트 추가해줘");
        assert_eq!(
            gist("강사 레포로 STEP 1~5 리허설을 돌려 ref 문제를 실제로 확인하고 싶음 (확신도: 높음)", 60),
            "강사 레포로 STEP 1~5 리허설을 돌려 ref 문제를 실제로 확인하고 싶음"
        );
        assert_eq!(gist("Open it. Then handle the rest.", 40), "Open it.");
        let long = gist("파트너로부터의 구금형 판례 연구 과제를 받고 그 과제를 완료하여 법원 법정 시스템에 저장하기", 24);
        assert!(long.ends_with('…') && long.chars().count() <= 25, "{long}");
    }

    #[test]
    fn a_summary_leads_with_what_was_stopped() {
        let f = long("c4000a1b");
        for task in 0..f.tasks.len() {
            let s = f.summary(task);
            assert!(!s.legs.is_empty() && s.legs.len() <= LEGS);
            assert!(matches!(s.legs[0], Leg::Ask(_)));
            let stopped: usize = s
                .legs
                .iter()
                .map(|l| match l {
                    Leg::Stopped { times, .. } => *times,
                    _ => 0,
                })
                .sum();
            if s.blocked + s.warn == 0 {
                assert_eq!(stopped, 0);
            } else if s.more == 0 {
                assert_eq!(stopped, s.blocked + s.warn);
            }
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
