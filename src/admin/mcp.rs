//! MCP 사용: what the gateway's allowlist lets the agents use, the requests developers send to
//! have more let through, and the calls the gateway refused.
//! - Three counts: the tools let through; the requests waiting; the refused calls by why.
//! - 도구: a board of the approved servers and their tools. A server is a row and the three
//!   levels (허용, 확인 요청, 차단) are the columns; a tool is a chip in the column of its level,
//!   drawn with what it does to the world (read, write, delete, send out), its calls over the
//!   period as a bar, and who called it. Dragged to another column, or switched in the panel
//!   that opens when it is picked, its level changes, and the change can be taken back. A tool
//!   unused for 30 days is drawn hollow.
//! - 승인 요청: what a developer asked for from their machine and why; the tools the server
//!   offers, each given a level, and whom they are for; approved or rejected with an answer.
//! - 막힌 요청: the refused calls in four kinds (not approved, waiting on a request, not for
//!   this person, set to deny), each told what it means and what to do.
//!
//! The allowlist is the gateway's. Until the gateway keeps one, only the demo has it.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::time::Duration;

use dioxus::prelude::*;

use super::main_page::{Card, DAY_MS, Empty, clock, decision_label, now_ms, period_label, stamp};
use crate::api::Backend;
use crate::model::{McpCatalog, McpChange, McpRefusal, McpRequest, McpTool, McpUse};
use crate::ui::AppState;
use crate::ui::home::when;

/// A tool unused this long is one to take back.
const IDLE: i64 = 30 * DAY_MS;
/// Refused this many times within an hour: someone trying again and again.
const REPEATED: i64 = 5;
const HOUR_MS: i64 = 60 * 60 * 1000;
/// The days the board draws a tool's use over.
const SPAN: usize = 30;
/// How long the page keeps saying a change was made.
const NOTE_STAYS: Duration = Duration::from_secs(8);

/// What a tool does to the world, in the order the board names them.
const NATURES: [&str; 4] = ["read", "write", "delete", "send"];
/// How far a tool is let through, in the order of the board's columns, and what each means.
const LEVELS: [(&str, &str); 3] = [
    ("allow", "그대로 통과"),
    ("review", "부를 때마다 사람이 확인"),
    ("deny", "게이트웨이가 막음"),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tab {
    Tools,
    Requests,
    Refused,
}

/// What the board lights; the rest dims.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Light {
    Nature(&'static str),
    Idle,
}

/// Whether a tool stays lit under the light and the search (lowercase).
fn lit(light: Option<Light>, q: &str, r: &Row) -> bool {
    let named = match light {
        None => true,
        Some(Light::Nature(n)) => r.tool.nature == n,
        Some(Light::Idle) => r.idle,
    };
    named
        && (q.is_empty()
            || r.tool.name.to_lowercase().contains(q)
            || r.server.to_lowercase().contains(q))
}

/// A change asked of the allowlist: what to say once it is made, and the change that takes it
/// back with what to say then.
#[derive(Clone, Debug, PartialEq)]
struct Ask {
    change: McpChange,
    done: String,
    undo: Option<(McpChange, String)>,
}

/// What the page says after a change, how to take the change back, and when it was said.
#[derive(Clone, Debug, PartialEq)]
struct Note {
    bad: bool,
    text: String,
    undo: Option<(McpChange, String)>,
    at: i64,
}

/// A tool picked up to be moved to another level.
#[derive(Clone, Debug, PartialEq)]
struct Held {
    server: String,
    tool: String,
    level: String,
}

/// A tool moved to another level; taken back, it returns to the one it had.
fn move_to(server: &str, tool: &str, from: &str, to: &str) -> Ask {
    let level = |level: &str| McpChange::Level {
        server: server.into(),
        tool: tool.into(),
        level: level.into(),
    };
    Ask {
        change: level(to),
        done: format!(
            "{server} · {tool} 판정을 {}에서 {}으로 바꿨습니다.",
            level_label(from),
            level_label(to)
        ),
        undo: Some((
            level(from),
            format!(
                "{server} · {tool} 판정을 {}으로 되돌렸습니다.",
                level_label(from)
            ),
        )),
    }
}

/// Why a call was refused: its key, its name, its colour, what it means, what to do about it.
struct Why {
    key: &'static str,
    label: &'static str,
    tone: &'static str,
    about: &'static str,
    todo: &'static str,
}

const WHYS: [Why; 4] = [
    Why {
        key: "unapproved",
        label: "승인 안 된 서버·도구",
        tone: "bad",
        about: "허용 목록에 없는 MCP를 불렀습니다.",
        todo: "개발자가 단말에서 승인 요청을 보내면 승인 요청 탭에 들어옵니다.",
    },
    Why {
        key: "pending",
        label: "승인 대기 중",
        tone: "warn",
        about: "승인 요청은 들어왔고 아직 검토 전입니다.",
        todo: "승인 요청 탭에서 승인하거나 거절하면 더 막히지 않습니다.",
    },
    Why {
        key: "scope",
        label: "허용 대상 아님",
        tone: "blue",
        about: "승인된 도구지만 이 사람·팀은 대상이 아닙니다.",
        todo: "필요하면 그 도구의 허용 대상을 넓힙니다.",
    },
    Why {
        key: "deny",
        label: "차단한 도구",
        tone: "grey",
        about: "일부러 막아 둔 도구를 불렀습니다.",
        todo: "같은 사람이 되풀이하면 이유를 물어봅니다.",
    },
];

/// The first tab, as VIEWER_TAB names it.
fn first_tab() -> Tab {
    match std::env::var("VIEWER_TAB").as_deref() {
        Ok("requests") => Tab::Requests,
        Ok("refused") => Tab::Refused,
        _ => Tab::Tools,
    }
}

fn nature_label(nature: &str) -> &'static str {
    match nature {
        "read" => "읽기",
        "write" => "쓰기",
        "delete" => "삭제",
        "send" => "외부 전송",
        _ => "모름",
    }
}

fn level_label(level: &str) -> &'static str {
    match level {
        "allow" => "허용",
        "review" => "확인 요청",
        "deny" => "차단",
        _ => "모름",
    }
}

fn kind_label(kind: &str) -> &'static str {
    match kind {
        "server" => "새 서버",
        "tool" => "새 도구",
        _ => "대상 넓히기",
    }
}

fn status_label(status: &str) -> (&'static str, &'static str) {
    match status {
        "approved" => ("ok", "승인함"),
        "rejected" => ("bad", "거절함"),
        _ => ("warn", "대기"),
    }
}

/// The part of an address before the `@`, as the lists name people.
fn short(user: &str) -> &str {
    user.split('@').next().unwrap_or(user)
}

/// A tool as the board shows it over the period.
#[derive(Clone, Debug, PartialEq)]
struct Row {
    server: String,
    tool: McpTool,
    /// Its calls in the period, and by whom.
    calls: usize,
    users: Vec<(String, String, usize)>,
    /// Its last call at all.
    last: Option<i64>,
    /// Let through, yet not called for [`IDLE`].
    idle: bool,
    /// The request its server was approved on.
    basis: Option<McpRequest>,
    /// Its calls day by day over the last [`SPAN`] days, the oldest first.
    days: Vec<usize>,
    /// The calls of it the gateway refused in the period.
    tried: i64,
}

/// Calls counted day by day over the last [`SPAN`] days, the oldest first. A day is 24 hours
/// back from now, so the last one is always whole.
fn daily(calls: &[McpUse], now: i64) -> Vec<usize> {
    let mut days = vec![0; SPAN];
    for u in calls {
        let age = (now - u.at).div_euclid(DAY_MS);
        if (0..SPAN as i64).contains(&age) {
            days[SPAN - 1 - age as usize] += 1;
        }
    }
    days
}

/// How many of the [`SPAN`] days the period covers, counted from today.
fn days_lit(days: Option<i64>) -> usize {
    days.map_or(SPAN, |d| (d.max(0) as usize).min(SPAN))
}

fn rows(c: &McpCatalog, since: Option<i64>, now: i64) -> Vec<Row> {
    let mut out = Vec::new();
    for s in &c.servers {
        let basis = s
            .request
            .as_ref()
            .and_then(|id| c.requests.iter().find(|r| &r.id == id))
            .cloned();
        let mut tools: Vec<Row> = s
            .tools
            .iter()
            .map(|t| {
                let in_period: Vec<_> = t
                    .calls
                    .iter()
                    .filter(|u| since.is_none_or(|since| u.at >= since))
                    .collect();
                let mut by: BTreeMap<(String, String), usize> = BTreeMap::new();
                for u in &in_period {
                    *by.entry((u.user.clone(), u.client.clone())).or_default() += 1;
                }
                let mut users: Vec<(String, String, usize)> =
                    by.into_iter().map(|((u, m), n)| (u, m, n)).collect();
                users.sort_by(|a, b| b.2.cmp(&a.2).then(a.0.cmp(&b.0)));
                let last = t.calls.iter().map(|u| u.at).max();
                let tried = c
                    .refused
                    .iter()
                    .filter(|x| x.server == s.name && x.tool.as_deref() == Some(t.name.as_str()))
                    .filter(|x| since.is_none_or(|since| x.last_at >= since))
                    .map(|x| x.count)
                    .sum();
                Row {
                    server: s.name.clone(),
                    tool: t.clone(),
                    calls: in_period.len(),
                    users,
                    last,
                    idle: t.level != "deny" && last.is_none_or(|at| at < now - IDLE),
                    basis: basis.clone(),
                    days: daily(&t.calls, now),
                    tried,
                }
            })
            .collect();
        // The most used first; tools set to deny last.
        tools.sort_by_key(|r| (r.tool.level == "deny", std::cmp::Reverse(r.calls)));
        out.extend(tools);
    }
    out
}

/// The letter a person goes by on the board: `dev.kim@…` is K.
fn initial(user: &str) -> String {
    short(user)
        .rsplit('.')
        .next()
        .and_then(|s| s.chars().next())
        .map(|c| c.to_uppercase().collect())
        .unwrap_or_default()
}

/// One of five tints for a person, the same wherever they show.
fn tint(user: &str) -> usize {
    short(user)
        .bytes()
        .fold(3usize, |a, b| a.wrapping_mul(37).wrapping_add(b as usize))
        % 5
}

/// What a tool does to the world, drawn: an eye reads, a pencil writes, a bin deletes, a paper
/// plane sends out.
fn glyph(nature: &str) -> Element {
    let d = match nature {
        "read" => {
            "M1.6 8c1.5-2.9 3.7-4.4 6.4-4.4s4.9 1.5 6.4 4.4c-1.5 2.9-3.7 4.4-6.4 4.4S3.1 10.9 1.6 8z M9.9 8a1.9 1.9 0 1 1-3.8 0 1.9 1.9 0 0 1 3.8 0z"
        }
        "write" => {
            "M3 13l.5-2.7 7.4-7.4a1.3 1.3 0 0 1 1.8 0l.4.4a1.3 1.3 0 0 1 0 1.8l-7.4 7.4L3 13z M9.8 4l2.2 2.2"
        }
        "delete" => {
            "M2.8 4.3h10.4 M6.2 4.3V2.9h3.6v1.4 M4 4.3l.7 8.9h6.6l.7-8.9 M6.7 6.8v4 M9.3 6.8v4"
        }
        "send" => "M14 2L2 6.6l4.9 2.5L9.4 14 14 2z M14 2L6.9 9.1",
        _ => "M8 3.5v5.5 M8 12v.5",
    };
    rsx! {
        svg { class: "mb-glyph", width: "14", height: "14", view_box: "0 0 16 16", fill: "none", stroke: "currentColor",
            stroke_width: "1.4", stroke_linecap: "round", stroke_linejoin: "round",
            path { d }
        }
    }
}

/// The last [`SPAN`] days of calls as bars, the oldest first, the last `lit` of them (the
/// period's) bright. A day without a call is a tick on the floor.
fn spark(days: &[usize], lit: usize, class: &'static str) -> Element {
    const H: f64 = 24.0;
    let most = days.iter().copied().max().unwrap_or(0).max(1) as f64;
    let n = days.len();
    let width = n * 4;
    rsx! {
        svg { class: "{class}", view_box: "0 0 {width} 24", preserve_aspect_ratio: "none",
            for (i, d) in days.iter().copied().enumerate() {
                {
                    let h = if d == 0 { 1.0 } else { (d as f64 / most * H).max(2.5) };
                    let (x, y) = (i * 4, H - h);
                    let tone = match (d, i + lit >= n) {
                        (0, true) => "zero on",
                        (0, false) => "zero",
                        (_, true) => "on",
                        _ => "",
                    };
                    rsx! { rect { key: "{i}", class: "{tone}", x: "{x}", y: "{y:.1}", width: "3", height: "{h:.1}" } }
                }
            }
        }
    }
}

/// Waiting first, then the newest.
fn requests_in_order(c: &McpCatalog) -> Vec<McpRequest> {
    let mut v = c.requests.clone();
    v.sort_by_key(|r| (r.status != "pending", std::cmp::Reverse(r.at)));
    v
}

/// Refused again and again within the hour.
fn repeated(r: &McpRefusal) -> bool {
    r.count >= REPEATED && r.last_at - r.first_at <= HOUR_MS
}

/// The teams the allowlist names, to open a tool to.
fn teams(c: &McpCatalog) -> Vec<String> {
    let people: BTreeSet<&str> = c.requests.iter().map(|r| short(&r.user)).collect();
    let mut t: BTreeSet<String> = BTreeSet::new();
    for s in &c.servers {
        for tool in &s.tools {
            t.extend(
                tool.scope
                    .iter()
                    .filter(|x| !people.contains(x.as_str()))
                    .cloned(),
            );
        }
    }
    t.into_iter().collect()
}

fn me() -> String {
    std::env::var("USER").unwrap_or_else(|_| "security-admin".into())
}

/// Brings the picked chip into view once the board has been drawn, for a jump from another tab.
const SHOW_PICKED_JS: &str = "setTimeout(() => document.querySelector('.mb-chip.on')?.scrollIntoView({ block: 'center' }), 120);";

/// A chip carries its name when it is dragged, so the webview takes the drag for a move.
const DRAG_JS: &str = r#"
if (!window.mbDrag) {
  window.mbDrag = true;
  document.addEventListener('dragstart', (e) => {
    const chip = e.target instanceof Element ? e.target.closest('.mb-chip') : null;
    if (chip && e.dataTransfer) {
      e.dataTransfer.effectAllowed = 'move';
      e.dataTransfer.setData('text/plain', chip.querySelector('code')?.textContent ?? '');
    }
  });
}
"#;

#[component]
pub fn Mcp() -> Element {
    let state = use_context::<AppState>();
    let backend = use_context::<Arc<Backend>>();
    let mut bump = use_signal(|| 0u32);
    let found = use_resource({
        let backend = backend.clone();
        move || {
            let b = backend.clone();
            let _ = (state.reload)();
            let _ = (state.public)();
            let _ = bump();
            async move { (b.mcp_catalog().await, now_ms()) }
        }
    });
    let mut tab = use_signal(first_tab);
    let mut days = use_signal(|| Some(7_i64));
    let mut light = use_signal(|| None::<Light>);
    let mut query = use_signal(String::new);
    let mut picked = use_signal(|| None::<(String, String)>);
    let mut picked_request = use_signal(|| None::<String>);
    let mut reason = use_signal(|| None::<&'static str>);
    let mut note = use_signal(|| None::<Note>);
    // The tool being dragged, and the cell (server, level) it is over.
    let mut held = use_signal(|| None::<Held>);
    let mut over = use_signal(|| None::<(String, &'static str)>);
    let now = now_ms();
    let p = (state.public)();
    let (catalog, failed, loaded) = match &*found.read() {
        Some((Ok(c), at)) => (c.clone(), None, Some(*at)),
        Some((Err(e), at)) => (None, Some(e.clone()), Some(*at)),
        None => (None, None, None),
    };
    let loading = found.read().is_none();
    let at = clock(loaded);
    let since = days().map(|d| now - d * DAY_MS);
    let period = period_label(days());

    let change = use_callback({
        let backend = backend.clone();
        move |a: Ask| {
            let backend = backend.clone();
            spawn(async move {
                match backend.mcp_change(a.change).await {
                    Ok(()) => {
                        note.set(Some(Note {
                            bad: false,
                            text: a.done,
                            undo: a.undo,
                            at: now_ms(),
                        }));
                        bump += 1;
                    }
                    Err(e) => note.set(Some(Note {
                        bad: true,
                        text: e,
                        undo: None,
                        at: now_ms(),
                    })),
                }
            });
        }
    });
    // What went well leaves by itself; what went wrong stays until it is closed.
    use_effect(move || {
        let Some(shown) = note().filter(|n| !n.bad) else {
            return;
        };
        spawn(async move {
            tokio::time::sleep(NOTE_STAYS).await;
            if note.peek().as_ref().is_some_and(|n| n.at == shown.at) {
                note.set(None);
            }
        });
    });
    use_effect(|| {
        document::eval(DRAG_JS);
    });

    let source = match &catalog {
        Some(c) if p.demo => format!("데모 데이터 · 허용 목록 {}", c.version),
        Some(c) => format!("게이트웨이 · 허용 목록 {}", c.version),
        None if p.demo => "데모 데이터".to_owned(),
        None => p.store(),
    };
    let header = rsx! {
        header { class: "cs-head",
            div { class: "cs-crumb",
                strong { "MCP 사용" }
                span { class: "sep" }
                span { "게이트웨이가 허용한 도구" }
            }
            span { class: "cs-src",
                span { class: if p.demo { "dot demo" } else { "dot" } }
                "{source}"
            }
            if let Some(c) = &catalog {
                span { class: "mt-version", "{when(c.updated_at)} 바뀜" }
            }
            label { class: "admin-pick-field",
                span { "기간" }
                select {
                    onchange: move |e| days.set(e.value().parse().ok()),
                    for (v, l) in [("1", "최근 24시간"), ("7", "최근 7일"), ("30", "최근 30일"), ("all", "전체 기간")] {
                        option { key: "{v}", value: "{v}", selected: days().map_or("all".to_owned(), |d| d.to_string()) == v, "{l}" }
                    }
                }
            }
        }
    };

    let Some(c) = catalog else {
        return rsx! {
            div { class: "home admin mcp",
                {header}
                div { class: "cs-grid",
                    div { class: "cs-card inv-none",
                        if loading {
                            p { "불러오는 중…" }
                        } else if let Some(e) = failed {
                            p { class: "admin-failed", "MCP 허용 목록을 불러오지 못했습니다: {e}" }
                        } else {
                            h3 { "게이트웨이의 MCP 허용 목록이 아직 없습니다" }
                            p {
                                "엔진 게이트웨이가 MCP 통신을 중계하고 허용 목록을 갖게 되면, 여기에 허용한 도구와 개발자의 승인 요청, "
                                "막힌 요청이 나옵니다. 지금은 왼쪽 아래의 데이터 전환으로 데모를 볼 수 있습니다."
                            }
                        }
                    }
                }
            }
        };
    };

    let all_rows = rows(&c, since, now);
    let level_n = |l: &str| all_rows.iter().filter(|r| r.tool.level == l).count();
    let let_through = all_rows.len() - level_n("deny");
    let requests = requests_in_order(&c);
    let waiting: Vec<&McpRequest> = requests.iter().filter(|r| r.status == "pending").collect();
    let decided_in = |s: &str| {
        requests
            .iter()
            .filter(|r| r.status == s && since.is_none_or(|t| r.decided_at.is_some_and(|d| d >= t)))
            .count()
    };
    let refused: Vec<McpRefusal> = c
        .refused
        .iter()
        .filter(|r| since.is_none_or(|t| r.last_at >= t))
        .cloned()
        .collect();
    let refused_n: i64 = refused.iter().map(|r| r.count).sum();
    let count_of = |k: &str| {
        refused
            .iter()
            .filter(|r| r.reason == k)
            .map(|r| r.count)
            .sum::<i64>()
    };
    let people_of = |k: Option<&str>| {
        refused
            .iter()
            .filter(|r| k.is_none_or(|k| r.reason == k))
            .map(|r| r.user.as_str())
            .collect::<BTreeSet<_>>()
            .len()
    };
    let why_max = WHYS
        .iter()
        .map(|w| count_of(w.key))
        .max()
        .unwrap_or(0)
        .max(1);

    let q = query().trim().to_lowercase();
    let lights_on = light().is_some() || !q.is_empty();
    let is_lit = |r: &Row| lit(light(), &q, r);
    // The busiest tool of the period fills its chip; the rest are drawn against it.
    let most = all_rows.iter().map(|r| r.calls).max().unwrap_or(0).max(1);
    let span_lit = days_lit(days());
    let holding = held();
    let aim = over();
    let current: Option<Row> = picked().and_then(|(s, t)| {
        all_rows
            .iter()
            .find(|r| r.server == s && r.tool.name == t)
            .cloned()
    });
    let current_place = current
        .as_ref()
        .and_then(|r| c.servers.iter().find(|s| s.name == r.server))
        .map(|s| {
            format!(
                "{} · {}",
                if s.transport == "local" {
                    "로컬"
                } else {
                    "원격"
                },
                s.endpoint
            )
        })
        .unwrap_or_default();
    let current_request: Option<McpRequest> = picked_request()
        .and_then(|id| requests.iter().find(|r| r.id == id).cloned())
        .or_else(|| requests.first().cloned());
    let team_names = teams(&c);

    rsx! {
        div { class: "home admin mcp",
            {header}
            div { class: "cs-grid",
                div { class: "cs-row four mt-cards",
                    Card { title: "허용한 도구", note: format!("서버 {}개 · 허용 목록 {}", c.servers.len(), c.version), stamp: at.clone(),
                        button { class: "cs-figure", title: "도구 보드 보기",
                            onclick: move |_| { tab.set(Tab::Tools); light.set(None); },
                            span { class: "cs-big", "{let_through}" }
                        }
                        div { class: "mt-mix",
                            div { class: "bar",
                                for (l, _) in LEVELS {
                                    span { key: "{l}", class: "{l}", style: "flex-grow:{level_n(l)}" }
                                }
                            }
                            div { class: "legend",
                                for (l, _) in LEVELS {
                                    span { key: "{l}", i { class: "{l}" } "{level_label(l)} " strong { "{level_n(l)}" } }
                                }
                            }
                        }
                    }
                    Card { title: "승인 요청", note: "개발자가 단말에서 보낸 요청", stamp: at.clone(),
                        button { class: "cs-figure", title: "승인 요청 보기",
                            onclick: move |_| { tab.set(Tab::Requests); picked_request.set(None); },
                            span { class: if waiting.is_empty() { "cs-big" } else { "cs-big warn" }, "{waiting.len()}" }
                        }
                        div { class: "cs-sub",
                            if waiting.is_empty() {
                                span { "기다리는 요청이 없습니다" }
                            }
                            for r in waiting.iter().take(2) {
                                span { key: "{r.id}", class: "mt-wait",
                                    "{short(&r.user)} · "
                                    code { if r.kind == "server" { "{r.server}" } else { "{r.server} · {r.wanted.join(\", \")}" } }
                                    " {kind_label(&r.kind)}"
                                }
                            }
                            span { "{period} 처리 · 승인 " strong { class: "good", "{decided_in(\"approved\")}" } " · 거절 " strong { "{decided_in(\"rejected\")}" } }
                        }
                    }
                    Card { title: "막힌 요청", note: format!("{period} · 게이트웨이가 거절한 호출"), stamp: at.clone(),
                        div { class: "mt-refused-card",
                            button { class: "cs-figure", title: "막힌 요청 보기",
                                onclick: move |_| { tab.set(Tab::Refused); reason.set(None); },
                                span { class: if refused_n > 0 { "cs-big bad" } else { "cs-big" }, "{refused_n}" }
                                span { class: "mt-people", "막힌 사람 {people_of(None)}명" }
                            }
                            div { class: "mt-whys-mini",
                                for w in WHYS.iter() {
                                    {
                                        let n = count_of(w.key);
                                        let width = n as f64 / why_max as f64 * 100.0;
                                        let key = w.key;
                                        rsx! {
                                            button { key: "{w.key}", class: "mt-why-row {w.tone}", title: "{w.about}",
                                                onclick: move |_| { tab.set(Tab::Refused); reason.set(Some(key)); },
                                                i {}
                                                span { class: "label", "{w.label}" }
                                                span { class: "track", span { class: "fill", style: "width:{width:.1}%" } }
                                                span { class: "n", "{n}건" }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                div { class: "mb-bar",
                    nav { class: "inv-tabs",
                        for (t, label, n) in [(Tab::Tools, "도구", all_rows.len()), (Tab::Requests, "승인 요청", waiting.len()), (Tab::Refused, "막힌 요청", refused_n as usize)] {
                            button { key: "{label}", class: if tab() == t { "on" } else { "" }, onclick: move |_| tab.set(t),
                                "{label}" span { class: "n", "{n}" }
                            }
                        }
                    }
                    if tab() == Tab::Tools {
                        // What lights the board, and the key to the chips' marks at once.
                        div { class: "mb-lights",
                            for n in NATURES {
                                {
                                    let count = all_rows.iter().filter(|r| r.tool.nature == n).count();
                                    let on = light() == Some(Light::Nature(n));
                                    rsx! {
                                        button { key: "{n}", class: if on { "mb-light {n} on" } else { "mb-light {n}" }, aria_pressed: "{on}",
                                            title: "{nature_label(n)} 도구만 밝히기",
                                            onclick: move |_| light.set(if on { None } else { Some(Light::Nature(n)) }),
                                            {glyph(n)}
                                            "{nature_label(n)}"
                                            span { class: "n", "{count}" }
                                        }
                                    }
                                }
                            }
                            {
                                let count = all_rows.iter().filter(|r| r.idle).count();
                                let on = light() == Some(Light::Idle);
                                rsx! {
                                    button { class: if on { "mb-light idle on" } else { "mb-light idle" }, aria_pressed: "{on}",
                                        title: "허용했지만 30일 동안 아무도 안 쓴 도구만 밝히기",
                                        onclick: move |_| light.set(if on { None } else { Some(Light::Idle) }),
                                        i { class: "hollow" }
                                        "30일 안 씀"
                                        span { class: "n", "{count}" }
                                    }
                                }
                            }
                            input { class: "mt-search", r#type: "search", placeholder: "도구·서버 찾기", value: "{query}",
                                oninput: move |e| query.set(e.value()) }
                        }
                    }
                }
                match tab() {
                    Tab::Tools => rsx! {
                        div { class: if current.is_some() { "mb-wrap open" } else { "mb-wrap" },
                            onkeydown: move |e: KeyboardEvent| if e.key() == Key::Escape { picked.set(None) },
                            section { class: "cs-card mb",
                                div { class: "mb-grid",
                                    div { class: "mb-head corner",
                                        strong { "서버 {c.servers.len()} · 도구 {all_rows.len()}" }
                                        small { "칩을 누르면 자세히, 옆 칸으로 끌면 판정이 바뀝니다" }
                                    }
                                    for (l, about) in LEVELS {
                                        {
                                            let total = level_n(l);
                                            let shown = all_rows.iter().filter(|r| r.tool.level == l && is_lit(r)).count();
                                            let aimed = aim.as_ref().is_some_and(|(_, level)| *level == l);
                                            rsx! {
                                                div { key: "{l}", class: if aimed { "mb-head {l} aim" } else { "mb-head {l}" },
                                                    div { class: "name",
                                                        strong { "{level_label(l)}" }
                                                        if lights_on {
                                                            span { class: "n", "{shown}" span { class: "of", " / {total}" } }
                                                        } else {
                                                            span { class: "n", "{total}" }
                                                        }
                                                    }
                                                    small { "{about}" }
                                                }
                                            }
                                        }
                                    }
                                    for s in c.servers.iter() {
                                        {
                                            let tools: Vec<&Row> = all_rows.iter().filter(|r| r.server == s.name).collect();
                                            let calls: usize = tools.iter().map(|r| r.calls).sum();
                                            let people = tools.iter().flat_map(|r| r.users.iter().map(|u| u.0.as_str())).collect::<BTreeSet<_>>().len();
                                            let mut by_day = vec![0usize; SPAN];
                                            for r in &tools {
                                                for (day, n) in by_day.iter_mut().zip(&r.days) {
                                                    *day += n;
                                                }
                                            }
                                            let all_idle = tools.iter().filter(|r| r.tool.level != "deny").all(|r| r.idle);
                                            // Whom the whole server is for, when every tool of it is for the same.
                                            let only = tools.first().map(|r| r.tool.scope.clone()).filter(|sc| !sc.is_empty() && tools.iter().all(|r| &r.tool.scope == sc));
                                            let lane = match &holding {
                                                Some(h) if h.server == s.name => "mb-lane held",
                                                Some(_) => "mb-lane away",
                                                None if lights_on && !tools.iter().any(|r| is_lit(r)) => "mb-lane dark",
                                                None => "mb-lane",
                                            };
                                            let approved = format!("{} · 승인 {} {}", s.endpoint, when(s.approved_at), short(s.approved_by.as_deref().unwrap_or("")));
                                            rsx! {
                                                div { key: "{s.name}", class: "{lane}",
                                                    div { class: "mb-srv", title: "{approved}",
                                                        div { class: "name",
                                                            code { "{s.name}" }
                                                            span { class: "badge", if s.transport == "local" { "로컬" } else { "원격" } }
                                                        }
                                                        {spark(&by_day, span_lit, "mb-spark")}
                                                        div { class: "sum",
                                                            if all_idle {
                                                                span { class: "idle", "30일 안 씀" }
                                                            } else {
                                                                span { strong { "{calls}" } "회 · {people}명" }
                                                            }
                                                            if let Some(only) = &only {
                                                                span { class: "only", "{only.join(\", \")}만" }
                                                            }
                                                        }
                                                    }
                                                    for (l, _) in LEVELS {
                                                        {
                                                            let here: Vec<Row> = tools.iter().filter(|r| r.tool.level == l).map(|r| (*r).clone()).collect();
                                                            // A cell a held tool of this server can be dropped on.
                                                            let open = holding.as_ref().is_some_and(|h| h.server == s.name && h.level != l);
                                                            let aimed = open && aim.as_ref().is_some_and(|(server, level)| server == &s.name && *level == l);
                                                            let cell = match (open, aimed) {
                                                                (_, true) => format!("mb-cell {l} open aim"),
                                                                (true, _) => format!("mb-cell {l} open"),
                                                                _ => format!("mb-cell {l}"),
                                                            };
                                                            let (server_over, server_drop) = (s.name.clone(), s.name.clone());
                                                            rsx! {
                                                                div { key: "{l}", class: "{cell}",
                                                                    ondragover: move |e: DragEvent| {
                                                                        e.prevent_default();
                                                                        // Fires all the while: only a change of cell is told.
                                                                        let now_over = held.peek().clone().filter(|h| h.server == server_over && h.level != l).map(|h| (h.server, l));
                                                                        if *over.peek() != now_over {
                                                                            over.set(now_over);
                                                                        }
                                                                    },
                                                                    ondrop: move |e: DragEvent| {
                                                                        e.prevent_default();
                                                                        let dropped = held.peek().clone();
                                                                        held.set(None);
                                                                        over.set(None);
                                                                        if let Some(h) = dropped.filter(|h| h.server == server_drop && h.level != l) {
                                                                            change.call(move_to(&h.server, &h.tool, &h.level, l));
                                                                        }
                                                                    },
                                                                    for r in here {
                                                                        Chip {
                                                                            key: "{r.tool.name}",
                                                                            on: current.as_ref().is_some_and(|c| c.server == r.server && c.tool.name == r.tool.name),
                                                                            dim: lights_on && !is_lit(&r),
                                                                            lifted: holding.as_ref().is_some_and(|h| h.server == r.server && h.tool == r.tool.name),
                                                                            r,
                                                                            most,
                                                                            onpick: move |k: (String, String)| picked.set(if picked.peek().as_ref() == Some(&k) { None } else { Some(k) }),
                                                                            ongrab: move |h: Option<Held>| {
                                                                                if h.is_none() {
                                                                                    over.set(None);
                                                                                }
                                                                                held.set(h);
                                                                            },
                                                                        }
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                                p { class: "cs-stamp", "호출은 {period} · 마지막 갱신: {at}" }
                            }
                            if let Some(r) = current.clone() {
                                aside { class: "cs-card mb-side",
                                    ToolDetail {
                                        key: "{r.server}-{r.tool.name}",
                                        r,
                                        period: period.clone(),
                                        lit: span_lit,
                                        now,
                                        place: current_place.clone(),
                                        onchange: move |a: Ask| change.call(a),
                                        onrequest: move |id: String| { picked_request.set(Some(id)); tab.set(Tab::Requests); },
                                        onrefused: move |_| { tab.set(Tab::Refused); reason.set(None); },
                                        onclose: move |_| picked.set(None),
                                    }
                                }
                            }
                        }
                    },
                    Tab::Requests => rsx! {
                        div { class: "cs-row split mt-split",
                            Card { title: "승인 요청", note: "기다리는 것이 위에", stamp: at.clone(),
                                div { class: "mt-table",
                                    div { class: "mt-tr requests head",
                                        span { "상태" } span { "요청자" } span { "요청한 것" } span { "사유" } span { "막힘" } span { "요청" }
                                    }
                                    if requests.is_empty() {
                                        Empty { text: "받은 요청이 없습니다" }
                                    }
                                    div { class: "mt-tbody",
                                        for r in requests.iter().cloned() {
                                            RequestRow {
                                                key: "{r.id}",
                                                on: current_request.as_ref().is_some_and(|c| c.id == r.id),
                                                r,
                                                onpick: move |id: String| picked_request.set(Some(id)),
                                            }
                                        }
                                    }
                                }
                            }
                            Card {
                                title: "요청 자세히",
                                note: current_request.as_ref().map_or(String::new(), |r| format!("{} · {}", r.id, r.server)),
                                stamp: at.clone(),
                                if let Some(r) = current_request.clone() {
                                    RequestDetail {
                                        key: "{r.id}",
                                        r,
                                        teams: team_names.clone(),
                                        ondecide: move |(c, done): (McpChange, String)| change.call(Ask { change: c, done, undo: None }),
                                        onrefused: move |_| { tab.set(Tab::Refused); reason.set(None); },
                                    }
                                } else {
                                    Empty { text: "요청을 고르세요" }
                                }
                            }
                        }
                    },
                    Tab::Refused => rsx! {
                        div { class: "mt-whys",
                            for w in WHYS.iter() {
                                {
                                    let key = w.key;
                                    let on = reason() == Some(key);
                                    rsx! {
                                        button { key: "{w.key}", class: if on { "mt-why {w.tone} on" } else { "mt-why {w.tone}" },
                                            onclick: move |_| reason.set(if on { None } else { Some(key) }),
                                            span { class: "label", "{w.label}" }
                                            span { class: "n", "{count_of(w.key)}" span { class: "unit", "건 · {people_of(Some(w.key))}명" } }
                                            span { class: "about", "{w.about}" }
                                            span { class: "todo", "{w.todo}" }
                                        }
                                    }
                                }
                            }
                        }
                        Card { title: "막힌 요청", note: format!("{period} · 같은 사람·도구·이유는 한 줄로 · 칸을 누르면 그 이유만"), stamp: at.clone(),
                            div { class: "mt-table",
                                div { class: "mt-tr refused head",
                                    span { "마지막" } span { "사람" } span { "기기" } span { "서버 · 도구" } span { "횟수" } span { "" }
                                }
                                if refused.is_empty() {
                                    Empty { text: "기간 안에 막힌 요청이 없습니다" }
                                }
                                div { class: "mt-tbody",
                                    for w in WHYS.iter().filter(|w| reason().is_none_or(|k| k == w.key)) {
                                        {
                                            let mut list: Vec<McpRefusal> = refused.iter().filter(|r| r.reason == w.key).cloned().collect();
                                            list.sort_by_key(|r| std::cmp::Reverse(r.last_at));
                                            rsx! {
                                                if !list.is_empty() {
                                                    div { key: "why-{w.key}", class: "mt-why-head {w.tone}",
                                                        i {}
                                                        strong { "{w.label}" }
                                                        span { "{count_of(w.key)}건 · {w.about}" }
                                                    }
                                                }
                                                for r in list {
                                                    RefusedRow {
                                                        key: "{r.user}-{r.server}-{r.tool:?}-{r.reason}",
                                                        status: r.request.as_ref().and_then(|id| c.requests.iter().find(|x| &x.id == id)).map(|x| x.status.clone()),
                                                        known: c.servers.iter().any(|s| s.name == r.server && s.tools.iter().any(|t| Some(&t.name) == r.tool.as_ref())),
                                                        r,
                                                        onrequest: move |id: String| { picked_request.set(Some(id)); tab.set(Tab::Requests); },
                                                        onopen: move |k: (String, String)| {
                                                            picked.set(Some(k));
                                                            light.set(None);
                                                            query.set(String::new());
                                                            tab.set(Tab::Tools);
                                                            document::eval(SHOW_PICKED_JS);
                                                        },
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    },
                }
            }
            if let Some(n) = note() {
                div { class: "mb-toast-slot",
                    div { class: if n.bad { "mb-toast bad" } else { "mb-toast" }, role: "status",
                        span { "{n.text}" }
                        if let Some((back, said)) = n.undo.clone() {
                            button { class: "undo",
                                onclick: move |_| change.call(Ask { change: back.clone(), done: said.clone(), undo: None }),
                                "되돌리기"
                            }
                        }
                        button { class: "close", title: "닫기", aria_label: "닫기", onclick: move |_| note.set(None), "✕" }
                    }
                }
            }
        }
    }
}

/// A tool on the board: what it does to the world, its name, who called it, and how often,
/// drawn as a bar against the busiest tool. Picked, it opens the panel; dragged, it is held.
#[component]
fn Chip(
    r: Row,
    most: usize,
    on: bool,
    dim: bool,
    lifted: bool,
    onpick: EventHandler<(String, String)>,
    ongrab: EventHandler<Option<Held>>,
) -> Element {
    let t = &r.tool;
    let key = (r.server.clone(), t.name.clone());
    let key_too = key.clone();
    let grab = Held {
        server: r.server.clone(),
        tool: t.name.clone(),
        level: t.level.clone(),
    };
    let width = if r.calls == 0 {
        0.0
    } else {
        (r.calls as f64 / most.max(1) as f64 * 100.0).max(3.0)
    };
    let mut class = format!("mb-chip {} {}", t.nature, t.level);
    for (yes, name) in [
        (on, " on"),
        (dim, " dim"),
        (lifted, " lifted"),
        (r.idle, " idle"),
    ] {
        if yes {
            class.push_str(name);
        }
    }
    let mut faces: Vec<&str> = Vec::new();
    for (user, _, _) in &r.users {
        if !faces.contains(&user.as_str()) {
            faces.push(user);
        }
    }
    let more = faces.len().saturating_sub(3);
    rsx! {
        div { class: "{class}", role: "button", tabindex: "0", draggable: "true", aria_pressed: "{on}",
            title: "{t.name} · {nature_label(&t.nature)} · {t.description}",
            onclick: move |_| onpick.call(key.clone()),
            onkeydown: move |e: KeyboardEvent| {
                if e.key() == Key::Enter || e.key() == Key::Character(" ".into()) {
                    e.prevent_default();
                    onpick.call(key_too.clone());
                }
            },
            ondragstart: move |_| ongrab.call(Some(grab.clone())),
            ondragend: move |_| ongrab.call(None),
            span { class: "fill", style: "width:{width:.1}%" }
            {glyph(&t.nature)}
            code { "{t.name}" }
            if r.tried > 0 {
                span { class: "tried", title: "기간 안에 게이트웨이가 막은 호출", "막힘 {r.tried}" }
            }
            span { class: "faces",
                for user in faces.iter().take(3) {
                    i { key: "{user}", class: "av t{tint(user)}", title: "{short(user)}", "{initial(user)}" }
                }
                if more > 0 {
                    i { class: "av more", "+{more}" }
                }
            }
            if r.idle {
                span { class: "n idle", "30일 안 씀" }
            } else if r.calls > 0 || t.level != "deny" {
                span { class: "n", "{r.calls}" }
            }
        }
    }
}

/// A tool picked on the board: its level as a switch, its use over the last 30 days, who used
/// it, whom it is for and on what request it was let through, and its latest calls.
#[component]
fn ToolDetail(
    r: Row,
    period: String,
    lit: usize,
    now: i64,
    place: String,
    onchange: EventHandler<Ask>,
    onrequest: EventHandler<String>,
    onrefused: EventHandler<()>,
    onclose: EventHandler<()>,
) -> Element {
    let t = r.tool.clone();
    let (server, tool) = (r.server.clone(), t.name.clone());
    let to = {
        let (server, tool, from) = (server.clone(), tool.clone(), t.level.clone());
        move |level: &str| move_to(&server, &tool, &from, level)
    };
    let take_back = to("deny");
    let mut recent = t.calls.clone();
    recent.sort_by_key(|u| std::cmp::Reverse(u.at));
    recent.truncate(5);
    let month: usize = r.days.iter().sum();
    let top = r.users.first().map_or(1, |u| u.2).max(1);
    let first_day = when(Some(now - (SPAN as i64 - 1) * DAY_MS));
    rsx! {
        div { class: "mt-detail mb-detail",
            header { class: "mb-top",
                span { class: "mb-mark {t.nature}", {glyph(&t.nature)} }
                div { class: "what",
                    code { "{tool}" }
                    span { "{server} · {nature_label(&t.nature)}" }
                }
                button { class: "mb-close", title: "닫기", aria_label: "닫기", onclick: move |_| onclose.call(()), "✕" }
            }
            p { class: "mb-about", "{t.description}" }
            div { class: "mt-levels mb-switch", role: "group", aria_label: "판정",
                for (l, _) in LEVELS {
                    {
                        let ask = to(l);
                        rsx! {
                            button { key: "{l}", class: if t.level == l { "level {l} on" } else { "level {l}" },
                                disabled: t.level == l,
                                onclick: move |_| onchange.call(ask.clone()),
                                "{level_label(l)}"
                            }
                        }
                    }
                }
            }
            p { class: "mb-means",
                match t.level.as_str() {
                    "allow" => "부를 때마다 엔진이 판정하고, 문제없으면 그대로 갑니다.",
                    "review" => "부를 때마다 사람에게 확인을 받습니다.",
                    _ => "게이트웨이가 이 도구를 부르지 못하게 막습니다.",
                }
            }
            if r.idle {
                div { class: "mb-flag idle",
                    span { "30일 동안 아무도 쓰지 않았습니다." }
                    button { class: "inv-revoke", onclick: move |_| onchange.call(take_back.clone()), "허용 거두기" }
                }
            }
            if r.tried > 0 {
                div { class: "mb-flag tried",
                    span { "{period} 게이트웨이가 " strong { "{r.tried}번" } " 막았습니다." }
                    button { class: "link", onclick: move |_| onrefused.call(()), "막힌 요청 보기" }
                }
            }
            section { class: "mb-use",
                div { class: "head",
                    strong { "최근 30일" }
                    span { "{month}회 · 마지막 " if r.last.is_some() { "{when(r.last)}" } else { "없음" } }
                }
                {spark(&r.days, lit, "mb-spark tall")}
                div { class: "axis", span { "{first_day}" } span { "오늘" } }
            }
            h4 { "쓰는 사람" span { class: "n", "{period} · {r.users.len()}명" } }
            if r.users.is_empty() {
                p { class: "muted", "기간 안에 쓴 사람이 없습니다." }
            }
            for (user, machine, n) in r.users.iter().cloned() {
                {
                    let width = n as f64 / top as f64 * 100.0;
                    rsx! {
                        div { key: "{user}-{machine}", class: "mb-person", title: "{user} · {machine}",
                            i { class: "av t{tint(&user)}", "{initial(&user)}" }
                            span { class: "name", "{short(&user)}" small { "{machine}" } }
                            span { class: "track", span { class: "fill", style: "width:{width:.1}%" } }
                            span { class: "n", "{n}" }
                        }
                    }
                }
            }
            div { class: "mt-facts two",
                div { small { "허용 대상" } span { if t.scope.is_empty() { "모두" } else { "{t.scope.join(\", \")}" } } }
                div { small { "승인" } span { "{short(t.approved_by.as_deref().unwrap_or(\"-\"))} · {when(t.approved_at)}" } }
            }
            if let Some(b) = r.basis.clone() {
                div { class: "mt-basis",
                    div { class: "head",
                        strong { "승인 근거" }
                        button { class: "link", onclick: move |_| onrequest.call(b.id.clone()), "요청 {b.id} 보기" }
                    }
                    blockquote { title: "{b.message}", "{b.message}" }
                    span { class: "who", "{short(&b.user)} · {when(Some(b.at))} 요청" }
                }
            }
            h4 { "최근 호출" }
            if recent.is_empty() {
                p { class: "muted", "호출이 없습니다." }
            }
            for u in recent {
                div { key: "{u.at}", class: "mt-call",
                    span { class: "when", "{when(Some(u.at))}" }
                    span { class: "who", "{short(&u.user)}" }
                    code { title: "{u.target}", "{u.target}" }
                    span { class: "verdict {u.decision}",
                        match (u.decision.as_str(), u.ran) {
                            ("REVIEW", true) => "사용자 승인",
                            ("REVIEW", false) => "실행 안 됨",
                            (d, _) => decision_label(d),
                        }
                    }
                }
            }
            p { class: "mb-place", title: "{place}", "{place}" }
        }
    }
}

#[component]
fn RequestRow(r: McpRequest, on: bool, onpick: EventHandler<String>) -> Element {
    let id = r.id.clone();
    let (tone, label) = status_label(&r.status);
    let what = match r.kind.as_str() {
        "server" => r.server.clone(),
        _ => format!("{} · {}", r.server, r.wanted.join(", ")),
    };
    rsx! {
        button { class: if on { "mt-tr requests on" } else { "mt-tr requests" }, onclick: move |_| onpick.call(id.clone()),
            span { class: "status {tone}", "{label}" }
            span { "{short(&r.user)}" span { class: "muted", " · {r.client}" } }
            span { span { class: "badge", "{kind_label(&r.kind)}" } code { " {what}" } }
            span { class: "msg", title: "{r.message}", "{r.message}" }
            span { class: "num", "{r.attempts}" }
            span { class: "when", "{when(Some(r.at))}" }
        }
    }
}

/// A request told whole: who asked, from where, for what and why; the tools the server offers,
/// each given a level, and whom they are for; then approved or rejected with an answer.
#[component]
fn RequestDetail(
    r: McpRequest,
    teams: Vec<String>,
    ondecide: EventHandler<(McpChange, String)>,
    onrefused: EventHandler<()>,
) -> Element {
    let waiting = r.status == "pending";
    let mut levels = use_signal(|| {
        r.offered
            .iter()
            .map(|o| {
                let level = match (r.wanted.contains(&o.name), o.nature.as_str()) {
                    (false, _) => "deny",
                    (true, "read") => "allow",
                    (true, _) => "review",
                };
                (o.name.clone(), level.to_owned())
            })
            .collect::<Vec<(String, String)>>()
    });
    let me_only = short(&r.user).to_owned();
    let mut scope = use_signal(|| me_only.clone());
    let mut answer = use_signal(String::new);
    let (tone, label) = status_label(&r.status);
    let who = short(&r.user).to_owned();
    let decide = {
        let (id, server, who) = (r.id.clone(), r.server.clone(), who.clone());
        move |approve: bool| {
            let scope = match scope().as_str() {
                "모두" => Vec::new(),
                s => vec![s.to_owned()],
            };
            (
                McpChange::Decide {
                    id: id.clone(),
                    approve,
                    actor: me(),
                    answer: answer(),
                    levels: levels(),
                    scope,
                },
                if approve {
                    format!("{who}의 {server} 요청을 승인했습니다. 허용 목록에 반영됩니다.")
                } else {
                    format!("{who}의 {server} 요청을 거절했습니다. 답변이 개발자에게 갑니다.")
                },
            )
        }
    };
    let approve = decide.clone();
    let reject = decide;
    rsx! {
        div { class: "mt-detail",
            div { class: "mt-title",
                span { class: "badge kind", "{kind_label(&r.kind)}" }
                code { if r.kind == "server" { "{r.server}" } else { "{r.server} · {r.wanted.join(\", \")}" } }
                span { class: "status {tone}", "{label}" }
            }
            p { class: "mt-from",
                strong { "{who}" }
                " · {r.client} · {stamp(Some(r.at))}"
                if let Some(cwd) = &r.cwd { span { class: "muted", " · {cwd}" } }
            }
            div { class: "mt-message",
                small { "요청 사유" }
                blockquote { "{r.message}" }
            }
            if r.attempts > 0 {
                p { class: "mt-tried",
                    "요청하기 전에 게이트웨이에서 {r.attempts}번 막혔습니다. "
                    button { class: "link", onclick: move |_| onrefused.call(()), "막힌 요청 보기" }
                }
            }
            if let Some(e) = &r.endpoint {
                div { class: "mt-facts one",
                    div { small { if r.transport.as_deref() == Some("local") { "게이트웨이가 돌릴 명령" } else { "서버 주소" } } code { title: "{e}", "{e}" } }
                }
            }
            if r.kind == "scope" {
                div { class: "mt-facts one",
                    div { small { "넓힐 대상" } span { "{who}에게 {r.server} · {r.wanted.join(\", \")}을(를) 엽니다" } }
                }
            } else {
                h4 { "서버가 알린 도구" span { class: "n", "요청한 것 {r.wanted.len()}개 · 전체 {r.offered.len()}개" } }
                div { class: "mt-offered",
                    for o in r.offered.iter().cloned() {
                        {
                            let current = levels().iter().find(|(n, _)| n == &o.name).map(|(_, l)| l.clone()).unwrap_or_else(|| "deny".into());
                            let wanted = r.wanted.contains(&o.name);
                            rsx! {
                                div { key: "{o.name}", class: if wanted { "mt-offer wanted" } else { "mt-offer" },
                                    div { class: "what",
                                        code { "{o.name}" }
                                        span { class: "nature {o.nature}", "{nature_label(&o.nature)}" }
                                        if wanted { span { class: "badge", "요청함" } }
                                        p { "{o.description}" }
                                    }
                                    if waiting {
                                        div { class: "mt-levels small",
                                            for l in ["allow", "review", "deny"] {
                                                {
                                                    let name = o.name.clone();
                                                    rsx! {
                                                        button { key: "{l}", class: if current == l { "level {l} on" } else { "level {l}" },
                                                            onclick: move |_| {
                                                                let mut v = levels();
                                                                if let Some(x) = v.iter_mut().find(|(n, _)| *n == name) {
                                                                    x.1 = l.to_owned();
                                                                }
                                                                levels.set(v);
                                                            },
                                                            "{level_label(l)}"
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            if waiting {
                if r.kind != "scope" {
                    h4 { "허용 대상" }
                    div { class: "mt-scope",
                        for s in std::iter::once(me_only.clone()).chain(teams.iter().cloned()).chain(std::iter::once("모두".to_owned())) {
                            {
                                let label = if s == me_only { format!("요청자만 ({s})") } else { s.clone() };
                                let value = s.clone();
                                rsx! {
                                    button { key: "{s}", class: if scope() == s { "dt-chip on" } else { "dt-chip" },
                                        onclick: move |_| scope.set(value.clone()),
                                        "{label}"
                                    }
                                }
                            }
                        }
                    }
                }
                h4 { "개발자에게 보낼 답변" }
                textarea { class: "mt-answer", rows: "3", placeholder: "예: 읽기 도구만 엽니다. 쓰기가 필요하면 다시 요청해 주세요.",
                    value: "{answer}", oninput: move |e| answer.set(e.value()) }
                div { class: "mt-decide",
                    button { class: "mt-approve", onclick: move |_| ondecide.call(approve(true)), "승인" }
                    button { class: "inv-revoke", onclick: move |_| ondecide.call(reject(false)), "거절" }
                }
            } else {
                div { class: "mt-decided {tone}",
                    strong { "{label}" }
                    span { " · {short(r.decided_by.as_deref().unwrap_or(\"-\"))} · {when(r.decided_at)}" }
                    if let Some(a) = &r.answer { p { "{a}" } }
                }
            }
        }
    }
}

#[component]
fn RefusedRow(
    r: McpRefusal,
    status: Option<String>,
    known: bool,
    onrequest: EventHandler<String>,
    onopen: EventHandler<(String, String)>,
) -> Element {
    let tool = r.tool.clone().unwrap_or_default();
    let open = (r.server.clone(), tool.clone());
    let span_note = if r.count > 1 {
        format!("{} ~ {}", stamp(Some(r.first_at)), stamp(Some(r.last_at)))
    } else {
        String::new()
    };
    let request = r.request.clone();
    rsx! {
        div { class: "mt-tr refused",
            span { class: "when", title: "{stamp(Some(r.last_at))}", "{when(Some(r.last_at))}" }
            span { "{short(&r.user)}" }
            span { class: "muted", "{r.client}" }
            code { "{r.server}" if !tool.is_empty() { span { class: "muted", " · {tool}" } } }
            span { class: "num", title: "{span_note}",
                "{r.count}"
                if repeated(&r) { span { class: "badge warn", title: "한 시간 안에 {r.count}번", "반복" } }
            }
            span { class: "act",
                if let (Some(id), Some(s)) = (request, status) {
                    button { class: "mt-act", onclick: move |_| onrequest.call(id.clone()), "요청 보기 · {status_label(&s).1}" }
                } else if known {
                    button { class: "mt-act plain", onclick: move |_| onopen.call(open.clone()), "도구 보기" }
                } else {
                    span { class: "muted", "요청 없음" }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{McpServer, McpUse};

    const NOW: i64 = 1_800_000_000_000;

    fn tool(name: &str, level: &str, ages: &[i64]) -> McpTool {
        McpTool {
            name: name.into(),
            level: level.into(),
            nature: "read".into(),
            calls: ages
                .iter()
                .map(|a| McpUse {
                    at: NOW - a,
                    user: "a@x".into(),
                    client: "m".into(),
                    ..McpUse::default()
                })
                .collect(),
            ..McpTool::default()
        }
    }

    fn catalog(tools: Vec<McpTool>) -> McpCatalog {
        McpCatalog {
            servers: vec![McpServer {
                name: "s".into(),
                tools,
                ..McpServer::default()
            }],
            ..McpCatalog::default()
        }
    }

    #[test]
    fn a_tool_let_through_yet_unused_for_a_month_is_one_to_take_back() {
        let c = catalog(vec![
            tool("used", "allow", &[DAY_MS]),
            tool("stale", "allow", &[40 * DAY_MS]),
            tool("never", "review", &[]),
            tool("denied", "deny", &[]),
        ]);
        let r = rows(&c, Some(NOW - 7 * DAY_MS), NOW);
        let idle: Vec<&str> = r
            .iter()
            .filter(|r| r.idle)
            .map(|r| r.tool.name.as_str())
            .collect();
        assert_eq!(idle, ["stale", "never"]);
        assert_eq!(r.first().unwrap().tool.name, "used", "the most used first");
        assert_eq!(r.last().unwrap().tool.name, "denied", "denied last");
        assert_eq!(
            r.iter().find(|r| r.tool.name == "stale").unwrap().calls,
            0,
            "counted over the period"
        );
    }

    #[test]
    fn refusals_again_and_again_within_the_hour_are_marked() {
        let r = |count, span| McpRefusal {
            count,
            first_at: NOW - span,
            last_at: NOW,
            ..McpRefusal::default()
        };
        assert!(repeated(&r(9, 9 * 60_000)));
        assert!(!repeated(&r(9, 3 * HOUR_MS)));
        assert!(!repeated(&r(2, 60_000)));
    }

    #[test]
    fn waiting_requests_come_first() {
        let r = |id: &str, at, status: &str| McpRequest {
            id: id.into(),
            at,
            status: status.into(),
            ..McpRequest::default()
        };
        let c = McpCatalog {
            requests: vec![
                r("a", 3, "approved"),
                r("b", 1, "pending"),
                r("c", 2, "pending"),
                r("d", 4, "rejected"),
            ],
            ..McpCatalog::default()
        };
        let ids: Vec<String> = requests_in_order(&c).into_iter().map(|r| r.id).collect();
        assert_eq!(ids, ["c", "b", "d", "a"]);
    }

    #[test]
    fn every_reason_the_gateway_gives_is_told() {
        let keys: Vec<&str> = WHYS.iter().map(|w| w.key).collect();
        assert_eq!(keys, ["unapproved", "pending", "scope", "deny"]);
    }
}
