//! A session as a metro map. Time runs left to right, one column per Call in the order the
//! engine saw them, and every visual channel answers one question only:
//!
//! - **Line: which agent did it.** The main agent rides the main line; each subagent run is a
//!   branch that drops from the main line where the run starts and climbs back where it ends.
//!   Branches share lanes: a run that starts after another has ended takes its lane again.
//! - **Station mark: what the engine decided.** A white ring when allowed, a red disc with ✕
//!   when denied, an amber diamond when held for review, a dotted ring when undecided: shape and
//!   colour both, so a verdict never depends on colour alone or blends with a line's.
//! - **Capsule: one subgraph.** Calls the engine grouped into one subgraph sit in one capsule.
//! - **Interchange: a cause.** A station whose tool result told the agent to make a later Call
//!   (a `cause` link) is ringed as an interchange. The links themselves stay hidden, or the map
//!   becomes a web; picking a Call draws its own link and lights the engine's `path`.
//! - **Turn boundary: a new request.** A vertical rule where a later turn starts.
//!
//! Four or more plain allowed stations in a row on one line fold into one express station; a
//! stop, an undecided Call, a hard undo, a failure or an interchange never folds. A Call's place
//! depends only on what came before it, so a live session grows to the right and never moves
//! what is drawn. This module only places things.

use std::collections::{HashMap, HashSet};

use serde_json::Value;

use crate::layout::Model;
use crate::model::{Node, parse_json};

/// Space between two stations along a line.
pub const STEP: f64 = 120.0;
/// Where the main line runs, and the space to each lane below it.
pub const MAIN_Y: f64 = 200.0;
pub const LANE_GAP: f64 = 120.0;
/// The first terminus, and the room before the first station.
pub const LEFT: f64 = 30.0;
pub const FIRST: f64 = 40.0;
/// Where a branch turns: it drops, then folds into its lane at 45°.
pub const CHAMFER: f64 = 22.0;
/// The most characters a section name shows.
pub const NAME_CHARS: usize = 28;
/// At or above this, a Call's effect is shown as hard to undo.
pub const IRREVERSIBLE: f64 = 0.5;
/// This many plain stations in a row on one line fold into one.
pub const FOLD: usize = 4;

/// What kind of thing a Call does: how its station is named.
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
    /// The main agent.
    Main,
    /// One subagent run.
    Branch,
}

/// What a Call was traced to: a paragraph of a tool result, and the Call that produced it.
#[derive(Clone, Debug, PartialEq)]
pub struct Source {
    pub result: i64,
    pub paragraph: Option<i64>,
    pub probability: f64,
    /// The result was the tool failing: its error message told the Call.
    pub error: bool,
    /// The words that told it, in a line: the error, else the output's first telling line.
    pub quote: Option<String>,
    /// The Call whose result it is: the interchange the cause link leaves from.
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
    /// The trace found nothing in the session that asked for it.
    pub loose: bool,
    /// The subagent that made the Call, by its kind, when one did.
    pub agent: Option<String>,
    /// The tool result that caused it, when the engine traced it to one in the session.
    pub source: Option<Source>,
    /// The subgraph the engine put it in, and the subgraphs on its path back to the request.
    pub subgraph: Option<i64>,
    pub path: Vec<i64>,
    /// How much the engine found each thing the Call could have followed to weigh on it
    /// (the trace's `influence`): a SubGraph by its number, or nothing (`None`), and the
    /// weight from 0 to 1.
    pub weights: Vec<(Option<i64>, f64)>,
    /// The Calls a folded station stands for, in order; just its own otherwise.
    pub members: Vec<i64>,
    /// When the engine saw the Call, in milliseconds.
    pub at: Option<i64>,
    /// On a map of places or steps, the station's name: the places its Calls went to.
    pub label: Option<String>,
}

impl Station {
    /// The station's name on the map, short as a metro station's: what it did, to what.
    pub fn name(&self) -> String {
        if self.folded() {
            return self.what.clone();
        }
        let short = |t: &str, n: usize| {
            if t.chars().count() > n {
                format!("{}…", t.chars().take(n).collect::<String>())
            } else {
                t.to_owned()
            }
        };
        // The working folder itself ("."), not a file: say so.
        let file = match place_name(&self.what).as_str() {
            "." | "./" | "" => "작업 폴더".to_owned(),
            name => clip_file(name, 26),
        };
        match (self.tool.as_str(), self.action) {
            ("Write", _) => format!("{file} 작성"),
            (_, Action::Change) => format!("{file} 수정"),
            (_, Action::Read) => format!("{file} 읽기"),
            ("SubagentHandback", _) => "보고 받음".into(),
            (_, Action::Delegate) => "에이전트에 맡김".into(),
            // Claude Code loading a tool before it uses it: `select:WebFetch,Read`.
            ("ToolSearch", _) => match self.what.strip_prefix("select:") {
                Some(names) => {
                    let names: Vec<&str> = names
                        .split(',')
                        .map(str::trim)
                        .filter(|n| !n.is_empty())
                        .collect();
                    let first = names
                        .first()
                        .map_or("", |n| mcp_parts(n).map_or(*n, |(_, op)| op));
                    match names.len() {
                        0 => "도구 불러오기".into(),
                        1 => format!("{} 도구 불러오기", short(first, 16)),
                        k => format!("{} 외 {} 도구 불러오기", short(first, 14), k - 1),
                    }
                }
                None => format!("‘{}’ 도구 찾기", short(&self.what, 12)),
            },
            (_, Action::Search) => format!("‘{}’ 찾기", short(&self.what, 12)),
            (_, Action::Run) => {
                let name = command_label(&self.what);
                if let Some((_, op)) = mcp_parts(&self.tool) {
                    op.to_owned()
                } else if name.is_empty() {
                    self.tool.clone()
                } else {
                    name
                }
            }
            _ if self.what.is_empty() => self.tool.clone(),
            _ => short(&self.what, 16),
        }
    }

    /// What a cause link from this station is called: the file it read, or the command.
    pub fn cause_name(&self) -> String {
        match self.action {
            Action::Read | Action::Change if !self.what.is_empty() => place_name(&self.what),
            _ => self.name(),
        }
    }

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
            // A Call a tool result told the agent to make is worth seeing on its own.
            && self.source.is_none()
            && !self.irreversible
            && !self.failed
            && !self.unsourced
            && !self.loose
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Line {
    pub kind: LineKind,
    pub y: f64,
    /// The subagent run's id; none for the main agent.
    pub agent_id: Option<String>,
    /// Where a branch leaves the main line and where it rejoins it.
    pub x0: f64,
    pub x1: f64,
    /// Its stations, left to right.
    pub stations: Vec<i64>,
    /// A station on it was denied or held.
    pub hot: bool,
    /// The agent's name on the map.
    pub name: String,
    /// Its badge: 메 for the main agent, the subagent kind's initial for a run.
    pub badge: String,
    /// On the map of places: how far beside the main line a branch runs while it shares the
    /// main line's stations (none when it shares none), and those stations, by their Calls.
    pub off: f64,
    pub shares: Vec<i64>,
}

/// What the agent said it would do, over the stations it did it at.
#[derive(Clone, Debug, PartialEq)]
pub struct Section {
    pub seq: i64,
    pub text: String,
    pub at: f64,
    pub from: f64,
    pub to: f64,
}

/// A message the agent showed the person: at the column of the Call after it.
#[derive(Clone, Debug, PartialEq)]
pub struct Said {
    pub seq: i64,
    pub x: f64,
    pub text: String,
    pub at: Option<i64>,
}

/// A turn's request: the first opens the main line, a later one is a turn boundary on it.
#[derive(Clone, Debug, PartialEq)]
pub struct Ask {
    pub seq: i64,
    pub x: f64,
    pub text: String,
    pub intent: Option<String>,
    /// The subagent whose report the engine recorded as this turn; none when a person wrote it.
    pub agent: Option<String>,
    /// When the engine saw the request, in milliseconds.
    pub at: Option<i64>,
}

/// Kept for the drawing, which no longer needs bands: a branch is its subagent.
#[derive(Clone, Debug, PartialEq)]
pub struct Band {
    pub agent: String,
    pub line: usize,
    pub x0: f64,
    pub x1: f64,
}

/// Calls of one subgraph in a row on one line: drawn as one long station.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Capsule {
    pub line: usize,
    pub x0: f64,
    pub x1: f64,
    pub y: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Metro {
    pub lines: Vec<Line>,
    pub stations: Vec<Station>,
    pub asks: Vec<Ask>,
    pub bands: Vec<Band>,
    pub sections: Vec<Section>,
    pub capsules: Vec<Capsule>,
    /// The agent's last message.
    pub last_word: Option<(i64, String)>,
    /// What the agent said to the person, where it said it.
    pub says: Vec<Said>,
    pub width: f64,
    pub height: f64,
    /// Each Call's station, folded ones included.
    at: HashMap<i64, usize>,
}

/// Where a leg of a journey starts.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Board {
    /// A turn's request: the journey's start, by the turn's index.
    Ask(usize),
    /// A station: a cause link leaves it, or a subagent run starts from it.
    Station(i64),
    /// Nothing: the Call was traced to nothing in the session.
    Nowhere,
}

/// One line ridden on the way from the request to a Call: where it was boarded, the stations
/// passed, and the station got off at.
#[derive(Clone, Debug, PartialEq)]
pub struct Leg {
    pub line: usize,
    pub board: Board,
    pub passed: Vec<i64>,
    pub alight: i64,
}

/// Calls by verdict, as the route list's tabs count them.
#[cfg(test)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Verdicts {
    pub all: usize,
    pub deny: usize,
    pub review: usize,
    pub allow: usize,
    pub none: usize,
}

/// What makes Calls the same station: the file or page they acted on, the command they ran,
/// the kind of search; a hand-off to a subagent is a station of its own.
pub fn place_key(s: &Station) -> String {
    match s.action {
        Action::Read | Action::Change if !s.what.is_empty() => format!("f:{}", s.what),
        // Each MCP tool is a place of its own; it has no command to name it by.
        Action::Run if mcp_parts(&s.tool).is_some() => format!("o:{}", s.tool),
        Action::Run => format!("r:{}", command_name(&s.what)),
        Action::Search => format!("s:{}", s.tool),
        Action::Delegate => format!("d:{}", s.seq),
        _ => format!("o:{}", s.tool),
    }
}

/// A place's name on the map: the file or page, the command, the search.
pub fn place_label(s: &Station) -> String {
    if let Some(l) = &s.label {
        return l.clone();
    }
    let short = |t: &str, n: usize| {
        if t.chars().count() > n {
            format!("{}…", t.chars().take(n).collect::<String>())
        } else {
            t.to_owned()
        }
    };
    match s.action {
        Action::Read | Action::Change if !s.what.is_empty() => clip_file(&place_name(&s.what), 26),
        Action::Run => {
            let c = command_name(&s.what);
            if let Some((_, op)) = mcp_parts(&s.tool) {
                short(op, 20)
            } else if c.is_empty() {
                s.tool.clone()
            } else if !c.contains(' ') && c.contains('.') {
                // A script run is told apart from the script read.
                format!("{} 실행", short(&c, 16))
            } else {
                short(&c, 20)
            }
        }
        Action::Search => format!("{} 검색", s.tool),
        Action::Delegate if s.tool == "SubagentHandback" => "보고 받음".to_owned(),
        Action::Delegate if s.what.trim().is_empty() => "에이전트에 맡김".to_owned(),
        Action::Delegate => format!("맡김 · {}", short(&s.what, 14)),
        _ => s.tool.clone(),
    }
}

/// An MCP tool's server and operation: `mcp__gmail__send_email` is `gmail` and `send_email`.
pub fn mcp_parts(tool: &str) -> Option<(&str, &str)> {
    let (server, op) = tool.strip_prefix("mcp__")?.split_once("__")?;
    (!server.is_empty() && !op.is_empty()).then_some((server, op))
}

/// A tool as the map writes it: an MCP tool by its server and operation, others as they are.
pub fn tool_label(tool: &str) -> String {
    match mcp_parts(tool) {
        Some((server, op)) => format!("{server} · {op}"),
        None => tool.to_owned(),
    }
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

/// A command as a station names it: the program, one leading flag, and what it acts on (`cat
/// -n refund.py`, `rm -rf backups`, `make test`), or just the script an interpreter runs
/// (`migrate.py`). It stops at the first quoted word, redirection or chained command.
pub fn command_name(command: &str) -> String {
    let main = main_part(command);
    match inline_script(main) {
        Some((lang, code)) => script_name(lang, code),
        None => command_name_of(main),
    }
}

/// A script given inline to its interpreter: `python3 -c "..."`, `node -e '...'`,
/// `python3 - <<EOF ... EOF`. Its language and its code.
fn inline_script(command: &str) -> Option<(&'static str, &str)> {
    let first = command.split_whitespace().next()?;
    let lang = match file_name(first) {
        p if p.starts_with("python") => "python",
        "node" | "deno" | "bun" => "node",
        "ruby" => "ruby",
        "perl" => "perl",
        "bash" | "sh" | "zsh" => "sh",
        _ => return None,
    };
    if let Some(i) = command.find("<<") {
        let code = command[i..].split_once('\n').map(|(_, c)| c)?;
        return Some((lang, code));
    }
    // `-c` gives python and the shells code; `-e` gives it to node, ruby and perl (python's
    // `-e` is pip's "editable", not code).
    let flag = if matches!(lang, "python" | "sh") {
        " -c "
    } else {
        " -e "
    };
    let i = command.find(flag)?;
    let rest = command[i + 4..].trim_start();
    let q = rest.chars().next()?;
    let code = if q == '"' || q == '\'' {
        let body = &rest[1..];
        body.rfind(q).map_or(body, |j| &body[..j])
    } else {
        rest
    };
    Some((lang, code))
}

/// An inline script named by what it works on: the file it names (`python · traj.json`), else
/// its first call that is not an import (`python · json.load`), else its length.
fn script_name(lang: &str, code: &str) -> String {
    // A quoted path or file name, the last one being the most particular.
    let mut found: Option<&str> = None;
    for q in ['\'', '"'] {
        for part in code.split(q).skip(1).step_by(2) {
            let p = part.trim();
            let looks = !p.contains(char::is_whitespace)
                && !p.contains(['{', '}', '%', '\\'])
                && (p.contains('/')
                    || p.rsplit_once('.').is_some_and(|(a, e)| {
                        !a.is_empty()
                            && (1..=5).contains(&e.len())
                            && e.chars().all(|c| c.is_ascii_alphanumeric())
                    }));
            if looks && found.is_none_or(|f| p.len() > f.len()) {
                found = Some(p);
            }
        }
    }
    if let Some(p) = found {
        let name = file_name(p.trim_end_matches('/'));
        return format!(
            "{lang} · {}",
            clip_file(if name.is_empty() { p } else { name }, 22)
        );
    }
    // Else its first call that says something: not an import, not a builtin every script
    // makes.
    let skip = ["import ", "from ", "require(", "#", "//", "use "];
    const PLAIN: [&str; 14] = [
        "print",
        "range",
        "len",
        "open",
        "str",
        "int",
        "float",
        "list",
        "dict",
        "set",
        "sorted",
        "enumerate",
        "zip",
        "require",
    ];
    let call = code
        .lines()
        .map(str::trim)
        .filter(|l| !skip.iter().any(|k| l.starts_with(k)))
        .flat_map(|l| {
            let mut calls = Vec::new();
            let mut word = String::new();
            for c in l.chars() {
                if c.is_alphanumeric() || c == '_' || c == '.' {
                    word.push(c);
                } else {
                    if c == '(' && word.chars().next().is_some_and(char::is_alphabetic) {
                        calls.push(word.clone());
                    }
                    word.clear();
                }
            }
            calls
        })
        .find(|w| !PLAIN.contains(&w.as_str()));
    match call {
        Some(c) => format!("{lang} · {}", clip_file(&c, 22)),
        None => format!("{lang} 스크립트 {}줄", code.lines().count().max(1)),
    }
}

/// The part of a command that does the work: past comments, and past setting up (`cd`,
/// `export`, `source`, `set`) joined to it by `&&`, `;` or a new line.
/// `cd /tmp/x && export K=v && python3 run.py` is `python3 run.py`.
fn main_part(command: &str) -> &str {
    const SETUP: [&str; 7] = ["cd", "pushd", "export", "source", ".", "set", "unset"];
    let mut rest = command.trim_start();
    loop {
        // Comment lines first.
        while rest.starts_with('#') {
            match rest.split_once('\n') {
                Some((_, after)) => rest = after.trim_start(),
                None => return command,
            }
        }
        let first = rest.split_whitespace().next().unwrap_or("");
        // A heading printed before the work (`echo "=== 결과 ===" && wc -l x`) is passed over
        // too, but not a line written into a file.
        let cut_at = ["&&", ";", "\n"]
            .iter()
            .filter_map(|sep| rest.find(sep))
            .min();
        let heading =
            ["echo", "printf"].contains(&first) && cut_at.is_some_and(|i| !rest[..i].contains('>'));
        if !SETUP.contains(&first) && !heading {
            return if rest.is_empty() { command } else { rest };
        }
        // Past this part, to what follows its `&&`, `;` or line end.
        let cut = ["&&", ";", "\n"]
            .iter()
            .filter_map(|sep| rest.find(sep).map(|i| (i, sep.len())))
            .min();
        match cut {
            Some((i, n)) => rest = rest[i + n..].trim_start(),
            None => return command,
        }
    }
}

fn command_name_of(command: &str) -> String {
    let plain = |w: &str| {
        !w.is_empty()
            && w.chars()
                .all(|c| c.is_alphanumeric() || "-_./:@".contains(c))
    };
    let mut words = command
        .split_whitespace()
        .filter(|w| !(w.contains('=') && !w.starts_with('-')))
        .map(|w| w.trim_end_matches([';', '&', '|']));
    let Some(program) = words.next() else {
        return String::new();
    };
    let program = file_name(program);
    let interpreter = [
        "python", "python3", "node", "bash", "sh", "zsh", "ruby", "deno", "bun",
    ]
    .contains(&program);
    let mut flag = None;
    let mut target = None;
    for w in words {
        if !plain(w) {
            break;
        }
        if w.starts_with('-') {
            if flag.is_none() && target.is_none() {
                flag = Some(w);
            }
            continue;
        }
        target = Some(if w.contains('/') || arg_is_script(w) {
            file_name(w)
        } else {
            w
        });
        break;
    }
    match (interpreter, flag, target) {
        (true, _, Some(script)) if arg_is_script(script) => script.to_owned(),
        (_, flag, target) => [Some(program), flag, target]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(" "),
    }
}

/// A word that names a script to run.
fn arg_is_script(w: &str) -> bool {
    [".py", ".sh", ".js", ".ts", ".rb"]
        .iter()
        .any(|ext| w.ends_with(ext))
        && w.chars().all(|c| c.is_alphanumeric() || "-_./".contains(c))
}

/// What a read or a change acts on, as a station names it: a file by its name, a web page by
/// the last two parts of its path ("issues/42"), or its host when it has no path.
/// A file's name cut to `n` characters in its middle, so its kind stays:
/// `diagnostic_payload_backup.json` is `diagnostic_payl….json`.
pub fn clip_file(name: &str, n: usize) -> String {
    let len = name.chars().count();
    if len <= n {
        return name.to_owned();
    }
    let ext = name
        .rfind('.')
        .filter(|&i| i > 0 && name.len() - i <= 6)
        .map_or("", |i| &name[i..]);
    let mut keep = n.saturating_sub(ext.chars().count() + 1).max(1);
    // Cut before a run of digits (a date, a number) rather than inside it, so files told
    // apart only by their numbers read alike and one file always reads the same:
    // `batch_20261103_1506.jsonl` is `batch_….jsonl`.
    let chars: Vec<char> = name.chars().collect();
    if let Some(i) = (3..keep.min(chars.len())).find(|&i| {
        chars[i].is_ascii_digit()
            && chars[i..].iter().take_while(|c| c.is_ascii_digit()).count() >= 3
    }) {
        keep = i;
    }
    format!("{}…{ext}", chars[..keep].iter().collect::<String>())
}

/// A command as what it does, in plain words: `tail -5 log.jsonl` is "log.jsonl 내용 보기",
/// `wc -l x` "x 줄 세기", `ps aux` "실행 중인 작업 확인". Setting up before it is passed over,
/// a script given inline is named by what it works on, and a command not known keeps its
/// short name (`command_name`).
pub fn command_label(command: &str) -> String {
    let main = main_part(command);
    // A loop is what it does each time, done again: `for i in 1 2 3; do cat $i.json; done`.
    if let Some(head) = ["for ", "while "].iter().find(|h| main.starts_with(**h)) {
        let body = main
            .split_once("; do ")
            .or_else(|| main.split_once("\ndo "))
            .or_else(|| main.split_once(" do "))
            .map(|(_, b)| b)
            .unwrap_or("");
        let body = body
            .rsplit_once("done")
            .map_or(body, |(b, _)| b)
            .trim_end_matches([';', ' ', '\n']);
        if !body.trim().is_empty() {
            return format!("{} 반복", command_label(body));
        }
        return if *head == "for " {
            "반복 실행".into()
        } else {
            "반복 대기".into()
        };
    }
    if let Some((lang, code)) = inline_script(main) {
        return script_name(lang, code);
    }
    // The first part of a pipe or a chain is the one that does the work.
    let part = main
        .split("&&")
        .next()
        .unwrap_or(main)
        .split(['|', ';', '\n'])
        .next()
        .unwrap_or(main);
    let words = shell_words(part);
    let mut words = words
        .iter()
        .map(String::as_str)
        .skip_while(|w| w.contains('=') && !w.starts_with('-'));
    let Some(program) = words.next().map(file_name) else {
        return command_name(command);
    };
    // Redirections (`2>/dev/null`, `> out.txt`, `< in`) are not what the command works on.
    let mut rest: Vec<&str> = Vec::new();
    let mut skip_next = false;
    for w in words {
        if skip_next {
            skip_next = false;
            continue;
        }
        if w.contains(['>', '<']) {
            skip_next = w
                .trim_start_matches(|c: char| c.is_ascii_digit())
                .trim_matches(['>', '<', '&'])
                .is_empty()
                && !w.ends_with('1');
            continue;
        }
        rest.push(w);
    }
    let args: Vec<&str> = rest
        .iter()
        .copied()
        .filter(|w| !w.starts_with('-'))
        .collect();
    // A word as an object: a path by its file name, cut alike everywhere.
    let obj = |w: &str| {
        let w = w.trim_end_matches('/');
        let name = if w.contains('/') { file_name(w) } else { w };
        clip_file(if name.is_empty() { w } else { name }, 18)
    };
    let last = args.last().map(|w| obj(w));
    let first = args.first().map(|w| obj(w));
    let on = |verb: &str, what: Option<String>| match what {
        Some(w) => format!("{w} {verb}"),
        None => verb.to_owned(),
    };
    match program {
        "tail" | "head" | "cat" | "less" | "more" | "bat" | "nl" => on("내용 보기", last),
        "wc" => on("줄 세기", last),
        "find" | "fd" => match first {
            Some(w) if w != "." => format!("{w}에서 파일 찾기"),
            _ => "파일 찾기".into(),
        },
        "ls" | "tree" => on("목록 보기", first.filter(|w| w != ".")),
        "grep" | "rg" | "egrep" => {
            let pat = args
                .first()
                .map(|p| short_word(p.trim_matches(['"', '\'']), 14));
            match (pat, args.get(1).map(|w| obj(w))) {
                (Some(p), Some(f)) => format!("{f}에서 ‘{p}’ 찾기"),
                (Some(p), None) => format!("‘{p}’ 찾기"),
                _ => "찾기".into(),
            }
        }
        "ps" | "pgrep" | "top" => "실행 중인 작업 확인".into(),
        "kill" | "pkill" | "killall" => "작업 멈추기".into(),
        "pip" | "pip3" | "npm" | "yarn" | "pnpm" | "brew" | "apt" | "apt-get" | "cargo" | "uv"
            if rest
                .first()
                .is_some_and(|w| ["install", "add", "i"].contains(w)) =>
        {
            on(
                "설치",
                args.get(1)
                    .filter(|w| !w.starts_with('.'))
                    .map(|w| short_word(w, 16)),
            )
        }
        "python" | "python3" if rest.first() == Some(&"-m") => {
            // A module run as a program: pip installing, pytest testing, else the module.
            let module = args.first().copied().unwrap_or("");
            match module {
                "pip" | "pip3" | "uv" if args.get(1) == Some(&"install") => on(
                    "설치",
                    args.get(2)
                        .filter(|w| !w.starts_with('.'))
                        .map(|w| short_word(w, 16)),
                ),
                "pip" | "pip3" => format!("pip {}", args.get(1).copied().unwrap_or(""))
                    .trim_end()
                    .to_owned(),
                "pytest" | "unittest" => "테스트 실행".into(),
                "" => "python 실행".into(),
                m => format!("{} 실행", short_word(m, 16)),
            }
        }
        "which" | "type" | "command" => on(
            "있는지 확인",
            args.iter()
                .rev()
                .find(|w| **w != "-v")
                .map(|w| short_word(w, 16)),
        ),
        "pytest" => "테스트 실행".into(),
        "python" | "python3" | "node" | "ruby" | "bash" | "sh" | "zsh" | "deno" | "bun" => {
            match args.first() {
                Some(w) if w.contains('.') || w.contains('/') => format!("{} 실행", obj(w)),
                Some(w) if *w == "-m" || rest.first() == Some(&"-m") => {
                    on("실행", args.first().map(|w| short_word(w, 16)))
                }
                _ => format!("{program} 실행"),
            }
        }
        "rm" | "rmdir" => on("삭제", last),
        "mkdir" => on("폴더 만들기", last),
        "touch" => on("만들기", last),
        "cp" => on("복사", first),
        "mv" => on("옮기기", first),
        "chmod" | "chown" => on("권한 바꾸기", last),
        "cd" | "pushd" => on("로 이동", first).replace(" 로", "로"),
        "sleep" => "기다리기".into(),
        "curl" | "wget" => match args.iter().find(|w| w.contains("://")) {
            Some(u) => format!(
                "{} 요청",
                u.split("://")
                    .nth(1)
                    .unwrap_or(u)
                    .split('/')
                    .next()
                    .unwrap_or(u)
            ),
            None => "웹 요청".into(),
        },
        "echo" | "printf" => match part.split_once('>') {
            Some((_, to)) => {
                on("에 쓰기", shell_words(to).first().map(|w| obj(w))).replace(" 에", "에")
            }
            None => "출력".into(),
        },
        "git" | "gh" | "docker" | "kubectl" | "make" | "npm" | "cargo" | "go" => {
            let sub: Vec<&str> = rest
                .iter()
                .copied()
                .filter(|w| !w.starts_with('-'))
                .take(2)
                .collect();
            [program]
                .into_iter()
                .chain(sub)
                .collect::<Vec<_>>()
                .join(" ")
        }
        _ => command_name(command),
    }
}

/// A command's words as the shell splits them, quotes taken off.
fn shell_words(text: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut word = String::new();
    let mut quote: Option<char> = None;
    let mut any = false;
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        // A backslash takes the next character as it is, except inside single quotes.
        if c == '\\' && quote != Some('\'') {
            if let Some(n) = chars.next() {
                word.push(n);
            }
            continue;
        }
        match quote {
            Some(q) if c == q => quote = None,
            Some(_) => word.push(c),
            None if c == '\'' || c == '"' => {
                quote = Some(c);
                any = true;
            }
            None if c.is_whitespace() => {
                if !word.is_empty() || any {
                    words.push(std::mem::take(&mut word));
                }
                any = false;
            }
            None => word.push(c),
        }
    }
    if !word.is_empty() || any {
        words.push(word);
    }
    words
}

/// A word cut to `n` characters.
fn short_word(w: &str, n: usize) -> String {
    if w.chars().count() > n {
        format!("{}…", w.chars().take(n - 1).collect::<String>())
    } else {
        w.to_owned()
    }
}

pub fn place_name(what: &str) -> String {
    let Some(rest) = what.split_once("://").map(|(_, r)| r) else {
        return file_name(what).to_owned();
    };
    let rest = rest.split(['?', '#']).next().unwrap_or(rest);
    let parts: Vec<&str> = rest.split('/').filter(|p| !p.is_empty()).collect();
    match parts.len() {
        0 => what.to_owned(),
        1 => parts[0].to_owned(),
        n => parts[n.saturating_sub(2).max(1)..].join("/"),
    }
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
    let node = model.nodes.iter().find(|n| n.seq() == result);
    let telling = node
        .and_then(|n| n.summary.error.as_deref().or(n.summary.output.as_deref()))
        .and_then(telling_line);
    // A tool that failed, or output that says something went wrong (a command echoing itself
    // and then failing still exits cleanly).
    let error =
        node.is_some_and(|n| n.kind == "Error") || telling.as_ref().is_some_and(|(_, bad)| *bad);
    let quote = telling.map(|(l, _)| {
        if l.chars().count() > 70 {
            format!("{}…", l.chars().take(69).collect::<String>())
        } else {
            l
        }
    });
    Some((
        Source {
            result,
            paragraph: best["paragraph"].as_i64(),
            probability: best["probability"].as_f64().unwrap_or(0.0),
            error,
            quote,
            by: by.unwrap_or(-1),
        },
        by.is_some(),
    ))
}

/// The line of a result most likely to have told the agent what to do next: the first that
/// reports something wrong (an error, a thing not found or missing), else the first that says
/// anything; and whether it reports something wrong.
pub fn telling_line(text: &str) -> Option<(String, bool)> {
    const BAD: [&str; 18] = [
        "error",
        "not found",
        "no such",
        "no module",
        "failed",
        "failure",
        "denied",
        "cannot",
        "can't",
        "unknown",
        "invalid",
        "fatal",
        "traceback",
        "exception",
        "없",
        "실패",
        "오류",
        "찾을 수",
    ];
    let lines: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|l| {
            !l.is_empty()
                && !l.starts_with("Exit code")
                && !l.starts_with("Shell cwd was reset")
                && !l.chars().all(|c| "-=_*~#. ".contains(c))
                // A line of brackets and marks (`{`, `],`) says nothing.
                && l.chars().filter(|c| c.is_alphanumeric()).count() >= 3
        })
        .collect();
    let bad = lines.iter().find(|l| {
        let low = l.to_lowercase();
        BAD.iter().any(|b| low.contains(b)) && !low.contains("0 error")
    });
    // A path the line begins with is its program's name: "/usr/…/python3: No module named x"
    // is "python3: No module named x".
    let tidy = |l: &str| match l.split_once(": ") {
        Some((head, rest)) if head.contains('/') && !head.contains(' ') => {
            format!("{}: {rest}", file_name(head))
        }
        _ => l.to_owned(),
    };
    match bad {
        Some(l) => Some((tidy(l), true)),
        None => lines.first().map(|l| (tidy(l), false)),
    }
}

/// One event in time order: a turn opening, the agent speaking, a Call, or a subagent run
/// starting or ending. (A Call is the large one and the common one; boxing it buys nothing.)
#[allow(clippy::large_enum_variant)]
enum Item {
    Ask(Ask),
    Say(i64, String, Option<i64>),
    Call(Station),
    Start(String),
    Stop(String),
}

/// The run a node belongs to, when a subagent's: its id.
fn run_of(node: &Node) -> Option<String> {
    node.props
        .get("agent_id")
        .and_then(Value::as_str)
        .filter(|a| !a.is_empty() && *a != "main")
        .map(str::to_owned)
}

impl Metro {
    #[cfg(test)]
    pub fn build(model: &Model) -> Self {
        Self::build_with(model, &HashSet::new())
    }

    /// Lays the map out, leaving unfolded the runs whose first Call is in `open`.
    pub fn build_with(model: &Model, open: &HashSet<i64>) -> Self {
        let mut nodes: Vec<&Node> = model.nodes.iter().collect();
        // In the order the engine saw them; `order` when recorded, the sequence otherwise.
        nodes.sort_by_key(|n| {
            (
                n.props
                    .get("order")
                    .and_then(Value::as_i64)
                    .unwrap_or(n.seq()),
                n.seq(),
            )
        });
        // A subagent run by its id, named by the kind its start recorded.
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

        // Lines first: the main agent, then each subagent run in the order it appears.
        let mut lines = vec![Line {
            kind: LineKind::Main,
            y: MAIN_Y,
            agent_id: None,
            x0: LEFT,
            x1: LEFT,
            stations: Vec::new(),
            hot: false,
            name: "메인 에이전트".into(),
            badge: "메".into(),
            off: 0.0,
            shares: Vec::new(),
        }];
        let mut line_of_run: HashMap<String, usize> = HashMap::new();
        let mut line_for = |lines: &mut Vec<Line>, id: &str| -> usize {
            *line_of_run.entry(id.to_owned()).or_insert_with(|| {
                let kind = kinds.get(id).cloned().unwrap_or_else(|| "서브".into());
                let initial: String = kind.chars().take(1).collect::<String>().to_uppercase();
                let taken = lines
                    .iter()
                    .filter(|l| l.badge.starts_with(&initial))
                    .count();
                lines.push(Line {
                    kind: LineKind::Branch,
                    y: 0.0,
                    agent_id: Some(id.to_owned()),
                    x0: f64::NAN,
                    x1: f64::NAN,
                    stations: Vec::new(),
                    hot: false,
                    off: 0.0,
                    shares: Vec::new(),
                    name: format!("{kind} 에이전트"),
                    badge: if taken == 0 {
                        initial
                    } else {
                        format!("{initial}{}", taken + 1)
                    },
                });
                lines.len() - 1
            })
        };
        let mut items: Vec<Item> = Vec::new();
        let mut last_word = None;
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
                        at: node.props.get("observed_at").and_then(Value::as_i64),
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
                    items.push(Item::Say(
                        node.seq(),
                        said,
                        node.props.get("observed_at").and_then(Value::as_i64),
                    ));
                }
                "SubagentStart" => {
                    if let Some(id) = run_of(node) {
                        line_for(&mut lines, &id);
                        items.push(Item::Start(id));
                    }
                }
                "SubagentStop" => {
                    if let Some(id) = run_of(node) {
                        items.push(Item::Stop(id));
                    }
                }
                "Call" => {
                    let trace = parse_json(node.props.get("trace_json"));
                    let (source, unsourced) = match traced(model, &trace) {
                        Some((s, true)) => (Some(s), false),
                        Some((_, false)) => (None, true),
                        None => (None, false),
                    };
                    let run = run_of(node);
                    let line = run.as_deref().map_or(0, |id| line_for(&mut lines, id));
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
                        loose: trace["link"] == "none",
                        agent: run.as_deref().map(|id| {
                            kinds
                                .get(id)
                                .cloned()
                                .unwrap_or_else(|| "서브에이전트".into())
                        }),
                        source,
                        subgraph: node.props.get("subgraph").and_then(Value::as_i64),
                        path: trace["path"]
                            .as_array()
                            .map(|p| p.iter().filter_map(Value::as_i64).collect())
                            .unwrap_or_default(),
                        weights: trace["influence"]
                            .as_object()
                            .map(|w| {
                                w.iter()
                                    .filter_map(|(key, p)| {
                                        let from = match key.as_str() {
                                            "none" => None,
                                            key => Some(key.strip_prefix('s')?.parse().ok()?),
                                        };
                                        Some((from, p.as_f64()?))
                                    })
                                    .collect()
                            })
                            .unwrap_or_default(),
                        members: vec![node.seq()],
                        at: node.props.get("observed_at").and_then(Value::as_i64),
                        label: None,
                    }));
                }
                _ => {}
            }
        }

        // An interchange never folds: the Calls whose results caused later ones.
        let sources: HashSet<i64> = items
            .iter()
            .filter_map(|i| match i {
                Item::Call(s) => s.source.as_ref().map(|src| src.by),
                _ => None,
            })
            .collect();

        // Express stretches: plain stations in a row on one line fold into one, unless opened.
        // Each line keeps its own stretch, so two runs working at once still fold.
        let line_of_item =
            |item: &Item,
             lines: &mut Vec<Line>,
             line_for: &mut dyn FnMut(&mut Vec<Line>, &str) -> usize| match item {
                Item::Start(id) | Item::Stop(id) => Some(line_for(lines, id)),
                _ => None,
            };
        let mut stretches: Vec<Vec<usize>> = Vec::new();
        let mut open_runs: HashMap<usize, Vec<usize>> = HashMap::new();
        let close =
            |line: usize, open_runs: &mut HashMap<usize, Vec<usize>>, out: &mut Vec<Vec<usize>>| {
                if let Some(run) = open_runs.remove(&line)
                    && run.len() >= FOLD
                {
                    out.push(run);
                }
            };
        for (i, item) in items.iter().enumerate() {
            match item {
                Item::Call(s) if s.plain() && !sources.contains(&s.seq) => {
                    open_runs.entry(s.line).or_default().push(i);
                }
                Item::Call(s) => close(s.line, &mut open_runs, &mut stretches),
                // Speaking does not interrupt an express stretch.
                Item::Say(..) => {}
                Item::Ask(_) => {
                    let all: Vec<usize> = open_runs.keys().copied().collect();
                    for l in all {
                        close(l, &mut open_runs, &mut stretches);
                    }
                }
                other => {
                    if let Some(l) = line_of_item(other, &mut lines, &mut line_for) {
                        close(l, &mut open_runs, &mut stretches);
                    }
                }
            }
        }
        let rest: Vec<usize> = open_runs.keys().copied().collect();
        for l in rest {
            close(l, &mut open_runs, &mut stretches);
        }
        // Fold each stretch into its first station, unless that one was opened.
        let mut gone: HashSet<usize> = HashSet::new();
        let mut merged: HashMap<usize, Station> = HashMap::new();
        for run in stretches {
            let stations: Vec<&Station> = run
                .iter()
                .map(|&i| match &items[i] {
                    Item::Call(s) => s,
                    _ => unreachable!("a stretch holds Calls"),
                })
                .collect();
            if open.contains(&stations[0].seq) {
                continue;
            }
            let mut first = stations[0].clone();
            let same = stations.iter().all(|s| s.tool == first.tool);
            let members: Vec<i64> = stations.iter().map(|s| s.seq).collect();
            first.what = if same {
                format!("{}개", members.len())
            } else {
                // Each tool with how many times it ran, the most used first.
                let mut counts: Vec<(&str, usize)> = Vec::new();
                for t in stations.iter().map(|s| s.tool.as_str()) {
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
            merged.insert(run[0], first);
            gone.extend(run[1..].iter().copied());
        }
        let folded: Vec<Item> = items
            .into_iter()
            .enumerate()
            .filter(|(i, _)| !gone.contains(i))
            .map(|(i, item)| match merged.remove(&i) {
                Some(s) => Item::Call(s),
                None => item,
            })
            .collect();

        // Columns: one per station, in order, with room where a turn or a run changes.
        let mut stations: Vec<Station> = Vec::new();
        let mut asks: Vec<Ask> = Vec::new();
        let mut says: Vec<Said> = Vec::new();
        let mut sections: Vec<Section> = Vec::new();
        let mut x = LEFT + FIRST;
        let mut open_section: Option<usize> = None;
        for item in folded {
            match item {
                Item::Ask(mut a) => {
                    if asks.is_empty() {
                        a.x = LEFT;
                    } else {
                        x += STEP * 0.3;
                        a.x = x;
                        x += STEP * 0.7;
                    }
                    open_section = None;
                    asks.push(a);
                }
                Item::Say(seq, said, at) => {
                    says.push(Said {
                        seq,
                        x,
                        text: said.clone(),
                        at,
                    });
                    let name = section_name(&said);
                    if !name.is_empty() {
                        sections.push(Section {
                            seq,
                            text: name,
                            at: x - 10.0,
                            from: x,
                            to: x,
                        });
                        open_section = Some(sections.len() - 1);
                    }
                }
                Item::Start(id) => {
                    let l = line_for(&mut lines, &id);
                    lines[l].x0 = x - STEP * 0.5;
                    x += STEP * 0.2;
                }
                Item::Stop(id) => {
                    let l = line_for(&mut lines, &id);
                    lines[l].x1 = x - STEP * 0.5;
                    x += STEP * 0.2;
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

        // A run's span: from its start (or before its first Call) to its end (or past its last).
        for l in lines.iter_mut().filter(|l| l.kind == LineKind::Branch) {
            let xs: Vec<f64> = l
                .stations
                .iter()
                .filter_map(|q| at.get(q).map(|&i| stations[i].x))
                .collect();
            let (first, last) = (
                xs.iter().copied().fold(f64::MAX, f64::min),
                xs.iter().copied().fold(f64::MIN, f64::max),
            );
            if l.x0.is_nan() {
                l.x0 = first - STEP * 0.5;
            }
            if l.x1.is_nan() || l.x1 < last + CHAMFER {
                l.x1 = (last + STEP * 0.5).max(l.x0 + STEP);
            }
            if xs.is_empty() {
                l.x1 = l.x1.max(l.x0 + STEP);
            }
        }

        // Lanes: a run takes the first lane free since the last run in it ended, so runs one
        // after another share a lane and only overlapping runs need more.
        let mut ends: Vec<f64> = Vec::new();
        let mut order: Vec<usize> = (1..lines.len()).collect();
        order.sort_by(|a, b| lines[*a].x0.total_cmp(&lines[*b].x0));
        for l in order {
            let lane = match ends.iter().position(|&e| e + CHAMFER * 2.0 < lines[l].x0) {
                Some(i) => i,
                None => {
                    ends.push(f64::MIN);
                    ends.len() - 1
                }
            };
            ends[lane] = lines[l].x1;
            lines[l].y = MAIN_Y + LANE_GAP * (lane + 1) as f64;
        }
        for s in &mut stations {
            s.y = lines[s.line].y;
        }

        // Capsules: Calls of one subgraph in a row on one line.
        let mut capsules: Vec<Capsule> = Vec::new();
        for w in stations.windows(2) {
            let (a, b) = (&w[0], &w[1]);
            // An express station is its own stretch, never part of a capsule.
            if a.line == b.line
                && !a.folded()
                && !b.folded()
                && a.subgraph.is_some()
                && a.subgraph == b.subgraph
            {
                match capsules.last_mut() {
                    Some(c) if c.line == a.line && c.x1 == a.x => c.x1 = b.x,
                    _ => capsules.push(Capsule {
                        line: a.line,
                        x0: a.x,
                        x1: b.x,
                        y: a.y,
                    }),
                }
            }
        }

        let right = stations
            .iter()
            .map(|s| s.x)
            .chain(asks.iter().map(|a| a.x))
            .chain(lines.iter().skip(1).map(|l| l.x1))
            .fold(LEFT + FIRST, f64::max);
        let bottom = lines.iter().map(|l| l.y).fold(MAIN_Y, f64::max);
        Metro {
            lines,
            stations,
            asks,
            bands: Vec::new(),
            sections,
            capsules,
            last_word,
            says,
            width: right + 240.0,
            height: bottom + 110.0,
            at,
        }
    }

    /// A line's badge: 메 for the main agent, the subagent kind's initial for a run.
    pub fn badge(&self, line: usize) -> String {
        self.lines[line].badge.clone()
    }

    /// Stations whose name or target holds `query`, for the destination search.
    pub fn find(&self, query: &str) -> Vec<&Station> {
        let q = query.trim().to_lowercase();
        if q.is_empty() {
            return Vec::new();
        }
        self.stations
            .iter()
            .filter(|s| {
                s.name().to_lowercase().contains(&q)
                    || s.what.to_lowercase().contains(&q)
                    || s.tool.to_lowercase().contains(&q)
            })
            .take(8)
            .collect()
    }

    /// Where a session's route goes when nothing is picked: its first stop, or else the last
    /// thing it did, so the route always tells the session.
    pub fn default_dest(&self) -> Option<i64> {
        self.first_stop().or_else(|| {
            self.stations
                .iter()
                .max_by(|a, b| a.x.total_cmp(&b.x))
                .map(|s| s.seq)
        })
    }

    /// How many Calls the engine judged each way, counting every Call an express station
    /// stands for: all, denied, held for review, allowed, undecided.
    #[cfg(test)]
    pub fn verdicts(&self) -> Verdicts {
        let mut v = Verdicts::default();
        for s in &self.stations {
            let n = s.members.len();
            v.all += n;
            match s.decision.as_deref() {
                Some("DENY") => v.deny += n,
                Some("REVIEW") => v.review += n,
                Some(_) => v.allow += n,
                None => v.none += n,
            }
        }
        v
    }

    /// The first station that stopped: what a stopped session is opened on.
    pub fn first_stop(&self) -> Option<i64> {
        self.stations.iter().find(|s| s.stopped()).map(|s| s.seq)
    }

    /// The station a Call is drawn at: its own, or the folded one it is part of.
    pub fn station(&self, seq: i64) -> Option<&Station> {
        self.at.get(&seq).map(|&i| &self.stations[i])
    }

    /// The Calls on the engine's path to a Call: those in the subgraphs its trace names.
    fn on_path(&self, s: &Station) -> Vec<i64> {
        let mut out: Vec<&Station> = self
            .stations
            .iter()
            .filter(|o| o.seq != s.seq && o.x < s.x)
            .filter(|o| o.subgraph.is_some_and(|g| s.path.contains(&g)))
            .collect();
        out.sort_by(|a, b| a.x.total_cmp(&b.x));
        out.into_iter().map(|o| o.seq).collect()
    }

    /// The last Call before a Call in a SubGraph: the Call the engine would have had it follow
    /// had it chosen that SubGraph. None for a SubGraph with no Call before it, a request's own.
    pub fn last_in(&self, s: &Station, subgraph: i64) -> Option<i64> {
        self.stations
            .iter()
            .filter(|o| o.seq != s.seq && o.x < s.x && o.subgraph == Some(subgraph))
            .max_by(|a, b| a.x.total_cmp(&b.x))
            .map(|o| o.seq)
    }

    /// The Call the engine's path to a Call comes through last: the one whose result told it,
    /// else the last Call before it in the SubGraph the engine chose as its parent. None when
    /// that SubGraph is the request's own, or the engine chose none.
    pub fn parent_of(&self, s: &Station) -> Option<i64> {
        if let Some(by) = s.source.as_ref().map(|src| src.by)
            && by != s.seq
            && self.station(by).is_some()
        {
            return Some(by);
        }
        let parent = *s.path.first()?;
        self.stations
            .iter()
            .filter(|o| o.seq != s.seq && o.x < s.x && o.subgraph == Some(parent))
            .max_by(|a, b| a.x.total_cmp(&b.x))
            .map(|o| o.seq)
    }

    /// The main-line station a subagent run started from: the Call that handed it the work.
    fn delegated_at(&self, line: usize) -> Option<&Station> {
        let x0 = self.lines[line].x0;
        self.stations
            .iter()
            .filter(|s| s.line == 0 && s.action == Action::Delegate && s.x <= x0 + STEP)
            .max_by(|a, b| a.x.total_cmp(&b.x))
    }

    /// The way from what told the agent to a Call, as a route finder gives it, following the
    /// engine's path: from the turn's request through the Calls on the path, changing at the
    /// tool result whose words caused it, to the Call. None when the Call is not on the map.
    pub fn journey(&self, seq: i64) -> Option<Vec<Leg>> {
        let dest = self.station(seq)?;
        if dest.loose || dest.unsourced {
            return Some(vec![Leg {
                line: dest.line,
                board: Board::Nowhere,
                passed: Vec::new(),
                alight: dest.seq,
            }]);
        }
        let turn = self.asks.iter().rposition(|a| a.x <= dest.x).unwrap_or(0);
        let path = self.on_path(dest);
        let by = dest
            .source
            .as_ref()
            .and_then(|src| self.station(src.by))
            .filter(|b| b.seq != dest.seq);
        let mut legs = Vec::new();
        match by {
            // A cause: ride to the station whose result it was, change there.
            Some(b) => {
                legs.push(Leg {
                    line: b.line,
                    board: Board::Ask(turn),
                    passed: path
                        .iter()
                        .copied()
                        .filter(|&q| self.station(q).is_some_and(|s| s.x < b.x))
                        .collect(),
                    alight: b.seq,
                });
                legs.push(Leg {
                    line: dest.line,
                    board: Board::Station(b.seq),
                    passed: Vec::new(),
                    alight: dest.seq,
                });
            }
            // A subagent's Call: the main line to the hand-off, then the run's own line.
            None if dest.line != 0 => match self.delegated_at(dest.line) {
                Some(d) => {
                    legs.push(Leg {
                        line: 0,
                        board: Board::Ask(turn),
                        passed: Vec::new(),
                        alight: d.seq,
                    });
                    legs.push(Leg {
                        line: dest.line,
                        board: Board::Station(d.seq),
                        passed: path,
                        alight: dest.seq,
                    });
                }
                None => legs.push(Leg {
                    line: dest.line,
                    board: Board::Ask(turn),
                    passed: path,
                    alight: dest.seq,
                }),
            },
            None => legs.push(Leg {
                line: dest.line,
                board: Board::Ask(turn),
                passed: path,
                alight: dest.seq,
            }),
        }
        Some(legs)
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

    // The main agent rides one line, whatever told it to act; the README's hidden instruction
    // shows as an interchange at the README read, not as a line of its own.
    #[test]
    fn a_file_name_is_cut_in_its_middle_keeping_its_kind() {
        use super::clip_file;
        assert_eq!(clip_file("config.yaml", 22), "config.yaml");
        assert_eq!(
            clip_file("diagnostic_payload_backup.json", 22),
            "diagnostic_paylo….json"
        );
        assert_eq!(
            clip_file("a_very_long_name_without_a_kind", 10),
            "a_very_lo…"
        );
    }

    #[test]
    fn the_main_agent_rides_the_main_line() {
        let m = metro(&session("fa6576f7"));
        assert_eq!(m.lines.len(), 1);
        for seq in [5, 7, 11, 12, 18] {
            assert_eq!(line_of(&m, seq).kind, LineKind::Main, "{seq}");
        }
        assert!(m.lines[0].hot);
        // Columns follow the engine's order.
        let xs: Vec<f64> = [5, 7, 11, 12]
            .iter()
            .map(|&q| m.station(q).unwrap().x)
            .collect();
        assert!(xs.windows(2).all(|w| w[0] < w[1]));
    }

    // A subagent run is a branch of its own, off the main line for the stretch it ran; the
    // main line is faint while it is away.
    #[test]
    fn a_subagent_run_is_a_branch() {
        let m = metro(&session("b6380b32"));
        let explore = line_of(&m, 6);
        assert_eq!(explore.kind, LineKind::Branch);
        assert_eq!(explore.name, "Explore 에이전트");
        assert_eq!(explore.badge, "E");
        assert_eq!(explore.stations, [6, 8, 10]);
        assert!(explore.y > MAIN_Y);
        assert!(explore.x0 < m.station(6).unwrap().x && explore.x1 > m.station(10).unwrap().x);
        assert_eq!(line_of(&m, 14).kind, LineKind::Main);
        assert!(m.station(10).unwrap().failed);
        assert!(m.station(14).unwrap().irreversible);
    }

    // Runs one after another share a lane; only overlapping runs take another.
    #[test]
    fn runs_reuse_a_free_lane() {
        let m = metro(&session("7fcbd428"));
        let branches: Vec<&Line> = m
            .lines
            .iter()
            .filter(|l| l.kind == LineKind::Branch)
            .collect();
        assert!(!branches.is_empty());
        assert!(branches.iter().all(|l| l.y == MAIN_Y + LANE_GAP));
    }

    // Calls of one subgraph in a row sit in one capsule.
    #[test]
    fn a_subgraph_is_a_capsule() {
        let m = metro(&session("fa6576f7"));
        let (a, b) = (m.station(11).unwrap(), m.station(12).unwrap());
        assert!(m.capsules.iter().any(|c| c.x0 == a.x && c.x1 == b.x));
        let (r, t) = (m.station(5).unwrap(), m.station(7).unwrap());
        assert!(m.capsules.iter().any(|c| c.x0 == r.x && c.x1 == t.x));
    }

    // The denied Edit was told by the README's result: its cause link leaves the README's
    // station, and a Call the request asked for has none.
    #[test]
    fn a_cause_link_leaves_where_the_words_were_read() {
        let m = metro(&session("fa6576f7"));
        assert_eq!(m.first_stop(), Some(11));
        assert_eq!(m.badge(0), "메");
        assert_eq!(m.find("app.py")[0].seq, 12);
        assert!(m.find("  ").is_empty());
        // The way there: the request, a change at the README's result, the Edit.
        let legs = m.journey(12).unwrap();
        assert_eq!(legs.len(), 2);
        assert_eq!(legs[0].board, Board::Ask(0));
        assert_eq!(legs[0].alight, 5);
        assert_eq!(legs[1].board, Board::Station(5));
        assert_eq!(legs[1].alight, 12);
        // A Call the request asked for is one leg; one traced to nothing starts nowhere.
        assert_eq!(m.journey(7).unwrap().len(), 1);
        let loose = metro(&session("b6380b32"));
        assert_eq!(loose.journey(16).unwrap()[0].board, Board::Nowhere);
        let sub = loose.journey(8).unwrap();
        assert_eq!(sub.last().unwrap().line, loose.station(8).unwrap().line);
    }

    // A subagent's report recorded as a turn is not a person's request.
    #[test]
    fn a_report_opens_its_turn_in_the_agents_name() {
        let m = metro(&session("7fcbd428"));
        assert_eq!(m.asks.len(), 2);
        assert!(m.asks[0].agent.is_none());
        assert_eq!(m.asks[1].agent.as_deref(), Some("Explore"));
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

    // Plain calls in a row fold into one express station, unless opened; a stop or a hard undo
    // breaks the run and stays on its own.
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

    // The tabs count Calls, not stations: an express station counts every Call it stands for.
    #[test]
    fn verdicts_count_calls_not_stations() {
        let m = Metro::build(&made_up(&[
            ("Bash", "ALLOW", 0.0),
            ("Bash", "ALLOW", 0.0),
            ("Bash", "ALLOW", 0.0),
            ("Bash", "ALLOW", 0.0),
            ("Bash", "DENY", 0.0),
            ("Bash", "REVIEW", 0.0),
        ]));
        assert_eq!(m.stations.len(), 3);
        let v = m.verdicts();
        assert_eq!((v.all, v.deny, v.review, v.allow, v.none), (6, 1, 1, 4, 0));
        let fa = metro(&session("fa6576f7")).verdicts();
        assert_eq!((fa.all, fa.deny, fa.allow), (7, 2, 5));
    }

    #[test]
    fn stops_interchanges_and_hard_undos_never_fold() {
        let m = metro(&session("fa6576f7"));
        assert!(m.station(12).is_some_and(|s| !s.folded()));
        assert!(m.station(5).is_some_and(|s| !s.folded()));
    }

    // A Call's place depends only on what came before it: a live session that grows keeps
    // every station where it was.
    #[test]
    fn a_growing_session_never_moves_what_is_drawn() {
        let id = session("fa6576f7");
        let graph = Demo::load().graph(&id).unwrap();
        let whole = Metro::build(&Model::build(
            &serde_json::from_value(graph.clone()).unwrap(),
        ));
        // The session as it stood before its 14th node arrived.
        let mut early = graph;
        early["nodes"]
            .as_array_mut()
            .unwrap()
            .retain(|n| n["props"]["seq"].as_i64().is_some_and(|q| q < 14));
        let early = Metro::build(&Model::build(&serde_json::from_value(early).unwrap()));
        assert!(!early.stations.is_empty());
        for s in &early.stations {
            let later = whole.station(s.seq).unwrap();
            assert_eq!((s.x, s.y), (later.x, later.y), "{}", s.seq);
        }
    }

    #[test]
    fn stations_are_named_like_metro_stops() {
        let m = metro(&session("fa6576f7"));
        assert_eq!(m.station(12).unwrap().action, Action::Change);
        assert_eq!(m.station(7).unwrap().action, Action::Run);
        assert_eq!(m.station(5).unwrap().name(), "README.md 읽기");
        assert_eq!(m.station(5).unwrap().cause_name(), "README.md");
        assert_eq!(m.station(12).unwrap().name(), "app.py 수정");
        assert_eq!(m.station(11).unwrap().name(), "notes.md 작성");
        assert_eq!(m.station(7).unwrap().name(), "make test");
        assert_eq!(
            place_name("https://github.com/acme/payments/issues/42"),
            "issues/42"
        );
        assert_eq!(place_name("https://example.com"), "example.com");
        assert_eq!(place_name("https://example.com/docs?x=1"), "docs");
        assert_eq!(
            m.sections[0].text,
            "README를 먼저 읽고 테스트를 돌려 보겠습니다."
        );
    }

    #[test]
    fn a_quiet_session_goes_to_its_last_call() {
        let calm = Metro::build(&made_up(&[("Bash", "ALLOW", 0.0), ("Edit", "ALLOW", 0.9)]));
        assert!(calm.first_stop().is_none());
        assert_eq!(calm.default_dest(), calm.stations.last().map(|s| s.seq));
    }

    // A command is named by its program and a plain first word; quoted text never leaks in.
    #[test]
    fn the_telling_line_is_the_one_that_says_what_went_wrong() {
        assert_eq!(
            telling_line(
                "python -m compileall -q infuse\nmake: python: No such file or directory\nmake: *** [test] Error 1"
            ),
            Some(("make: python: No such file or directory".to_owned(), true))
        );
        assert_eq!(
            telling_line("Exit code 1\nUnknown JSON field: \"stars\"\nAvailable fields:"),
            Some(("Unknown JSON field: \"stars\"".to_owned(), true))
        );
        assert_eq!(
            telling_line("---\n'infuse'에 복제합니다..."),
            Some(("'infuse'에 복제합니다...".to_owned(), false))
        );
        assert_eq!(
            telling_line("{\n  \"name\": \"linear\","),
            Some(("\"name\": \"linear\",".to_owned(), false))
        );
        assert_eq!(
            telling_line(
                "/Library/Frameworks/Python.framework/Versions/3.12/bin/python3: No module named pytest"
            ),
            Some(("python3: No module named pytest".to_owned(), true))
        );
    }

    #[test]
    fn commands_are_named_by_their_program() {
        assert_eq!(command_name("make test"), "make test");
        assert_eq!(command_name("rm -rf data/backups"), "rm -rf backups");
        assert_eq!(
            command_name("python3 scripts/migrate.py --apply"),
            "migrate.py"
        );
        assert_eq!(command_name("bash ./deploy.sh prod"), "deploy.sh");
        assert_eq!(
            command_name("printf '__pycache__/\\n' > .gitignore && git add ."),
            "printf"
        );
        assert_eq!(command_name("CI=1 npm test"), "npm test");
        assert_eq!(command_name("cat src/app.py | head"), "cat app.py");
        assert_eq!(command_name("git status;"), "git status");
        assert_eq!(
            command_name("cat -n src/refund.py && echo ----"),
            "cat -n refund.py"
        );
        assert_eq!(command_name("git ls-files"), "git ls-files");
        // Setting up is passed over for the work it leads to.
        assert_eq!(
            command_label("tail -5 /r/batch_20261103_1506.jsonl"),
            "batch_….jsonl 내용 보기"
        );
        assert_eq!(
            command_label("wc -l /r/batch_20261103_1506.jsonl"),
            "batch_….jsonl 줄 세기"
        );
        assert_eq!(command_label("ps aux | grep infuse"), "실행 중인 작업 확인");
        assert_eq!(
            command_label("find results -name '*.json'"),
            "results에서 파일 찾기"
        );
        assert_eq!(
            command_label("grep -n \"overwrite\" /tmp/infuse/cli.py | head"),
            "cli.py에서 ‘overwrite’ 찾기"
        );
        assert_eq!(
            command_label("cd /tmp/infuse && python3 run_all.py --n 20"),
            "run_all.py 실행"
        );
        assert_eq!(command_label("pip install -e ."), "설치");
        assert_eq!(command_label("rm -rf data/backups"), "backups 삭제");
        assert_eq!(command_label("git status"), "git status");
        assert_eq!(command_label("make test"), "make test");
        assert_eq!(
            command_label("cd /tmp/x && python3 -m pip install -e \".[dev]\" -q 2>&1 | tail"),
            "설치"
        );
        assert_eq!(command_label("python3 -m pip install -e . -q"), "설치");
        assert_eq!(
            command_label("python3 -m pip install requests"),
            "requests 설치"
        );
        assert_eq!(
            command_label("cd /tmp/x && python3 -m pytest tests -q --co 2>&1 | head"),
            "테스트 실행"
        );
        assert_eq!(
            command_label("which pip3 && python3 -m pip install --upgrade pip -q"),
            "pip3 있는지 확인"
        );
        assert_eq!(
            command_label("cat results/run.log 2>/dev/null"),
            "run.log 내용 보기"
        );
        assert_eq!(command_label("ls -la out > list.txt"), "out 목록 보기");
        // A heading printed first is passed over; a loop is what it does, again.
        assert_eq!(
            command_label(
                "echo \"=== 요약 ===\" && wc -l /r/batch_20261103_1518.jsonl && echo \"\""
            ),
            "batch_….jsonl 줄 세기"
        );
        assert_eq!(
            command_label("echo hi > note.txt && cat note.txt"),
            "note.txt에 쓰기"
        );
        assert_eq!(
            command_label(
                "for i in 2 7; do echo \"== $i ==\" && cat /r/$i/a.manifest.json | grep -o x; done"
            ),
            "a.manifest.json 내용 보기 반복"
        );
        assert_eq!(
            command_label("grep -h \"\\\"attack_name\\\"\" /r/*/x.manifest.json | sort"),
            "x.manifest.json에서 ‘attack_name’ 찾기"
        );
        assert_eq!(
            command_name(
                "cd /tmp/infuse && export KEY=\"x\" && python3 /a/infuse_claude.py --condition y"
            ),
            "infuse_claude.py"
        );
        assert_eq!(command_name("# check the token\ncd ~/t\nls -la"), "ls -la");
        assert_eq!(command_name("cd infuse"), "cd infuse");
        // A script given inline is named by what it works on.
        assert_eq!(
            command_name(
                "python3 -c \"\nimport json\nt=json.load(open('/tmp/r/task_1_attack1.traj.json'))\nprint(t)\""
            ),
            "python · task_1_attack1.t….json"
        );
        assert_eq!(
            command_name("python3 -c \"import json,sys\nfor a in range(3):\n    print(a)\""),
            "python 스크립트 3줄"
        );
        assert_eq!(
            command_name("node -e 'console.log(process.version)'"),
            "node · console.log"
        );
        assert_eq!(
            command_name(
                "python3 - <<'EOF'\nfrom pathlib import Path\nPath('out/report.md').write_text('x')\nEOF"
            ),
            "python · report.md"
        );
    }

    /// The synthetic long sessions `mock/make_long.py` writes, alongside the recorded ones.
    fn long(prefix: &str) -> (Demo, String) {
        let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/mock/long.json"))
            .expect("mock/long.json: run python3 -I mock/make_long.py");
        let demo = Demo::parse(&text);
        let id = demo
            .sessions()
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|s| s["id"].as_str())
            .find(|id| id.starts_with(prefix))
            .unwrap()
            .to_owned();
        (demo, id)
    }

    fn long_metro(prefix: &str) -> (Metro, Model) {
        let (demo, id) = long(prefix);
        let model = Model::build(&serde_json::from_value(demo.graph(&id).unwrap()).unwrap());
        (Metro::build(&model), model)
    }

    // Every node of the long sessions has a detail the app can read: the quotes and the
    // inspector come from it.
    #[test]
    fn the_long_sessions_details_read() {
        let (demo, id) = long("c5204b3c");
        let graph = demo.graph(&id).unwrap();
        for n in graph["nodes"].as_array().unwrap() {
            let seq = n["props"]["seq"].as_i64().unwrap();
            let detail: crate::model::NodeDetail = serde_json::from_value(demo.node(&id, seq))
                .unwrap_or_else(|e| panic!("{seq}: {e}"));
            assert_eq!(detail.kind, n["kind"].as_str().unwrap());
        }
    }

    // A long ordinary session: two subagent runs at once take two lanes, a later run takes
    // the first again; every Call is counted and has a way back.
    #[test]
    fn a_long_session_lays_out_in_few_lanes() {
        let (m, model) = long_metro("c0001a7e");
        let lanes: HashSet<i64> = m.lines[1..].iter().map(|l| l.y as i64).collect();
        assert_eq!(m.lines.len(), 4, "main and three runs");
        assert_eq!(
            lanes.len(),
            2,
            "two runs at once need two lanes, the third reuses one"
        );
        let calls = model.nodes.iter().filter(|n| n.kind == "Call").count();
        assert_eq!(m.verdicts().all, calls);
        assert!(
            m.stations.len() < calls,
            "plain stretches fold into express stations"
        );
        assert!(m.stations.iter().any(|s| s.failed));
        assert_eq!(m.asks.len(), 3);
        // Two runs working at once still fold, each on its own line.
        for l in 1..m.lines.len() {
            assert!(
                m.stations.iter().any(|s| s.line == l && s.folded()),
                "line {l}"
            );
        }
    }

    // Runs one after another share one lane however many there are.
    #[test]
    fn runs_in_turn_share_one_lane() {
        let (m, _) = long_metro("c9000d4e");
        assert!(m.lines.len() > 3);
        assert!(m.lines[1..].iter().all(|l| l.y == MAIN_Y + LANE_GAP));
        // Hundreds of Calls read as a few dozen stations: the plain stretches go express, but
        // a Call a tool result told the agent to make never hides in one.
        assert!(m.verdicts().all > 200 && m.stations.len() < 40);
        assert_eq!(m.stations.iter().filter(|s| s.source.is_some()).count(), 2);
        assert!(
            m.stations
                .iter()
                .filter(|s| s.folded())
                .all(|s| s.source.is_none())
        );
    }

    // The issue read first carries the instruction the denied Calls near the end follow: the
    // cause link reaches all the way back.
    #[test]
    fn a_cause_can_reach_far_back() {
        let (m, _) = long_metro("c5204b3c");
        let issue = m.stations.iter().find(|s| s.tool == "WebFetch").unwrap();
        let denied: Vec<&Station> = m
            .stations
            .iter()
            .filter(|s| s.decision.as_deref() == Some("DENY"))
            .collect();
        assert_eq!(denied.len(), 2);
        for d in denied {
            assert_eq!(d.source.as_ref().map(|s| s.by), Some(issue.seq));
            // Far back in order; on the map, the express stretch between keeps it near.
            assert!(d.seq - issue.seq > 60, "far back");
            assert!(
                m.stations
                    .iter()
                    .any(|s| s.folded() && s.x > issue.x && s.x < d.x)
            );
        }
        assert_eq!(issue.cause_name(), "issues/42");
    }

    // A long live session that grows never moves what is drawn.
    #[test]
    fn a_long_session_grows_without_moving() {
        let (demo, id) = long("c0001a7e");
        let graph = demo.graph(&id).unwrap();
        let whole = Metro::build(&Model::build(
            &serde_json::from_value(graph.clone()).unwrap(),
        ));
        for cut in [40, 120, 200] {
            let mut early = graph.clone();
            early["nodes"]
                .as_array_mut()
                .unwrap()
                .retain(|n| n["props"]["seq"].as_i64().is_some_and(|q| q < cut));
            let early = Metro::build(&Model::build(&serde_json::from_value(early).unwrap()));
            for s in early.stations.iter().filter(|s| !s.folded()) {
                if let Some(later) = whole.station(s.seq).filter(|l| !l.folded()) {
                    assert_eq!((s.x, s.y), (later.x, later.y), "cut {cut}, {}", s.seq);
                }
            }
        }
    }

    #[test]
    fn every_demo_session_lays_out() {
        for s in Demo::load().sessions().as_array().unwrap() {
            let id = s["id"].as_str().unwrap();
            let m = metro(id);
            // Every Call is counted once, folded or not.
            let calls = model(id).nodes.iter().filter(|n| n.kind == "Call").count();
            let v = m.verdicts();
            assert_eq!(v.all, calls, "{id}");
            assert_eq!(v.deny + v.review + v.allow + v.none, calls, "{id}");
            assert!(m.width.is_finite() && m.height.is_finite());
            for st in &m.stations {
                assert!(st.x.is_finite() && st.y.is_finite());
            }
        }
    }
}
