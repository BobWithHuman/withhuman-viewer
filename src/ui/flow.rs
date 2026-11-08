//! The route map after the Figma Make design (see `crate::flow`): two levels, the session's
//! network of tasks and one task's line, with the list beside it sliding between the two.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use dioxus::prelude::*;

use super::{AppState, Load, Selection};
use crate::flow::{
    AMBER, Flow, GREEN, Item, Kind, LINE_COLOR, LINE_MUTED, Leg, Overview, RED, Stop, Summary,
    TEXT_COLOR, TEXT_MUTED, Glyph, Tone, Turn,
};
use crate::layout::clip;
use crate::metro::Metro;

/// The width of the card over the map, with its margin: kept in step with `.fx-main > .route-panel`.
const CARD_W: f64 = 328.0;
/// Whether picking a Call opens who told the agent as a card over the line. Off for now: it hid
/// too much of the line.
const SHOW_ROUTE_CARD: bool = false;
/// Moving and zooming the line map in the page (see the file).
const PAN_JS: &str = include_str!("pan.js");

#[derive(Clone, Copy, Debug, PartialEq)]
enum Level {
    Overview,
    Detail,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Tab {
    All,
    Blocked,
    Warn,
    Ok,
}

impl Tab {
    fn name(self) -> &'static str {
        match self {
            Tab::All => "전체",
            Tab::Blocked => "차단",
            Tab::Warn => "확인",
            Tab::Ok => "허용",
        }
    }

    fn holds(self, t: &Turn) -> bool {
        match self {
            Tab::All => true,
            Tab::Blocked => t.blocked() > 0,
            Tab::Warn => t.warn() > 0,
            Tab::Ok => t.blocked() + t.warn() == 0,
        }
    }
}

/// A moment as the list writes it: the hour and minute.
fn hm(ms: Option<i64>) -> String {
    use chrono::TimeZone;
    ms.and_then(|ms| chrono::Local.timestamp_millis_opt(ms).single())
        .map(|t| t.format("%H:%M").to_string())
        .unwrap_or_default()
}

/// Picks a turn; a Call picked in another turn is let go.
fn select_turn(mut turn: Signal<usize>, mut state: AppState, flow: &Flow, t: usize) {
    turn.set(t);
    let current = state.selected.peek().clone();
    if let Selection::Node(seq) = current
        && flow.turn_of(seq) != Some(t)
    {
        state.selected.set(Selection::None);
    }
}

#[component]
pub fn FlowMap() -> Element {
    let mut state = use_context::<AppState>();
    let metro = use_memo(move || match &*state.graph.read() {
        Load::Ready(model) => {
            let all: HashSet<i64> = model.nodes.iter().map(|n| n.seq()).collect();
            Some(Arc::new(Metro::build_with(model, &all)))
        }
        _ => None,
    });
    let flow = use_memo(move || {
        let m = metro()?;
        Some(Arc::new(Flow::build(&m)))
    });
    let mut level = use_signal(|| Level::Overview);
    let mut turn = use_signal(|| 0usize);
    let hover = use_signal(|| None::<usize>);
    let mut tab = use_signal(|| Tab::All);
    let mut zoom = use_signal(|| 1.0_f64);
    let mut detail = use_signal(|| true);
    let hovered = use_signal(|| None::<i64>);
    // The session list, opened from the session's name in either level.
    let mut picking = use_context_provider(|| Picking(Signal::new(false))).0;
    let opened = use_signal(|| None::<Option<String>>);
    use_effect(move || {
        // Another session opened from the list closes it.
        let now = (state.current)();
        let mut opened = opened;
        if opened.peek().as_ref().is_some_and(|was| was.is_some() && *was != now) {
            picking.set(false);
        }
        opened.set(Some(now));
    });
    use_effect(move || {
        if picking() {
            document::eval("requestAnimationFrame(() => document.getElementById('session-search')?.focus())");
        }
    });
    use_effect(|| {
        document::eval(
            "if (!window.__fxKeys) { window.__fxKeys = true; window.addEventListener('keydown', (e) => {\
               const t = e.target; if (t && (t.tagName === 'INPUT' || t.tagName === 'TEXTAREA' || t.tagName === 'SELECT' || t.isContentEditable)) return;\
               if (e.metaKey || e.ctrlKey || e.altKey) return;\
               const id = e.key === 'ArrowRight' ? 'fx-next' : e.key === 'ArrowLeft' ? 'fx-prev' : null;\
               if (!id) return; const b = document.getElementById(id); if (b && !b.disabled) { e.preventDefault(); b.click(); } }); }",
        );
    });
    let pick_list = rsx! {
        if picking() {
            div { class: "fx-pick-shade", onclick: move |_| picking.set(false) }
            div {
                class: "fx-pick-pop",
                onkeydown: move |e: KeyboardEvent| if e.key() == Key::Escape { picking.set(false) },
                div { class: "fx-pick-head",
                    b { "세션 고르기" }
                    button { class: "fx-pick-close", title: "닫기", onclick: move |_| picking.set(false), "✕" }
                }
                super::sidebar::Sidebar {}
            }
        }
    };

    // A new session opens on its first stop; one task opens on its line, more on the network.
    let drawn = use_signal(|| None::<Option<String>>);
    use_effect(move || {
        let (Some(m), Some(f)) = (metro(), flow()) else {
            return;
        };
        let session = state.graph_of.peek().clone();
        let mut drawn = drawn;
        if *drawn.peek() == Some(session.clone()) {
            return;
        }
        drawn.set(Some(session));
        detail.set(true);
        tab.set(Tab::All);
        zoom.set(1.0);
        level.set(if f.tasks.len() > 1 {
            Level::Overview
        } else {
            Level::Detail
        });
        if *state.selected.peek() == Selection::None
            && let Some(seq) = m.default_dest()
        {
            state.selected.set(Selection::Node(seq));
        }
        let t = match *state.selected.peek() {
            Selection::Node(seq) => f.turn_of(seq).unwrap_or(0),
            Selection::None => 0,
        };
        turn.set(t);
    });
    // A Call picked anywhere picks its turn.
    use_effect(move || {
        let Selection::Node(seq) = (state.selected)() else {
            return;
        };
        let Some(f) = flow.peek().clone() else { return };
        if let Some(t) = f.turn_of(seq)
            && *turn.peek() != t
        {
            turn.set(t);
        }
    });
    // A Call brought into view from elsewhere opens its line.
    use_effect(move || {
        let Some(seq) = (state.focus)() else { return };
        let Some(f) = flow() else { return };
        if *state.graph_of.read() != *state.current.read() {
            return;
        }
        state.focus.set(None);
        if let Some(t) = f.turn_of(seq) {
            turn.set(t);
            level.set(Level::Detail);
        }
    });
    use_effect(move || {
        if (state.fit)() > 0 {
            zoom.set(1.0);
        }
    });

    let (Some(m), Some(f)) = (metro(), flow()) else {
        return rsx! { div { class: "routes fx", {pick_list} } };
    };
    let t = turn().min(f.turns.len() - 1);
    let task = f.task_of(t);
    let task_turns: Vec<usize> = f.task_turns(task).collect();
    // The line runs through the whole session; a task picked is where it is brought to.
    let scope: Vec<usize> = (0..f.turns.len()).collect();
    let visible: Vec<usize> = scope
        .iter()
        .copied()
        .filter(|&i| tab().holds(&f.turns[i]))
        .collect();
    // A filter that hides the turn picked moves to the first it shows.
    if level() == Level::Detail && !visible.is_empty() && !visible.contains(&t) {
        let first = visible[0];
        let f2 = f.clone();
        spawn(async move { select_turn(turn, state, &f2, first) });
    }
    let picked = match (state.selected)() {
        Selection::Node(seq) => f.call(seq).map(|_| seq),
        Selection::None => None,
    };
    let is_detail = level() == Level::Detail;
    let slide = if is_detail {
        "translateX(-50%)"
    } else {
        "translateX(0)"
    };
    let f_focus = f.clone();
    let mut focus_turn = move |n: usize| {
        select_turn(turn, state, &f_focus, n);
        level.set(Level::Detail);
        zoom.set(1.0);
    };
    let f_zoom = f.clone();
    let zoom_in = {
        move |_| {
            if level() == Level::Overview {
                select_turn(turn, state, &f_zoom, t);
                level.set(Level::Detail);
                zoom.set(1.0);
            } else {
                document::eval(&format!("{PAN_JS}\nfxMap.zoomBy(1.25)"));
            }
        }
    };
    let zoom_out = move |_| {
        if level() == Level::Detail {
            document::eval(&format!("{PAN_JS}\nfxMap.zoomBy(0.8)"));
        }
    };
    // Only over the line: the network is read whole, and picking there opens the line.
    let card = SHOW_ROUTE_CARD && is_detail && detail() && picked.is_some();
    // Every Call of the session in the order made, to step through with ← and →.
    let order: Vec<i64> = f.turns.iter().flat_map(|t| t.calls.iter().map(|c| c.seq)).collect();
    let at = picked.and_then(|p| order.iter().position(|&q| q == p));
    let total = order.len();
    let step = {
        let order = order.clone();
        move |by: isize| {
            if order.is_empty() {
                return;
            }
            // From the Call picked now, not when this was drawn, so keys pressed fast still step.
            let now = match *state.selected.peek() {
                Selection::Node(q) => order.iter().position(|&o| o == q),
                Selection::None => None,
            };
            let next = match now {
                Some(i) => i.saturating_add_signed(by).min(order.len() - 1),
                None if by < 0 => order.len() - 1,
                None => 0,
            };
            let mut state = state;
            state.selected.set(Selection::Node(order[next]));
            level.set(Level::Detail);
        }
    };
    let (mut step_prev, mut step_next) = (step.clone(), step);
    let task_name = f.tasks[task].name.clone();
    let (a, b) = (f.tasks[task].from + 1, f.tasks[task].to + 1);
    let anim = if is_detail {
        "fx-stage anim-zoom-in"
    } else {
        "fx-stage anim-zoom-out"
    };

    rsx! {
        div { class: "routes fx",
            aside { class: "fx-aside",
                div { class: "fx-levels",
                    button {
                        class: if !is_detail { "on" } else { "" },
                        onclick: move |_| level.set(Level::Overview),
                        "전체"
                    }
                    button {
                        class: if is_detail { "on" } else { "" },
                        onclick: move |_| focus_turn(t),
                        "자세히"
                    }
                }
                div { class: "fx-slide-wrap",
                    div { class: "fx-slide", style: "transform: {slide}",
                        div { class: "fx-pane", aria_hidden: is_detail,
                            TaskPanel { flow: f.clone(), turn, hover, level, zoom }
                        }
                        div { class: "fx-pane", aria_hidden: !is_detail,
                            div { class: "fx-where",
                                button { class: "fx-back", onclick: move |_| level.set(Level::Overview), "‹ 전체" }
                                div { class: "fx-where-row",
                                    span { class: "fx-swatch", style: "background: {LINE_COLOR}" }
                                    div { class: "fx-where-text",
                                        div { class: "fx-where-name", "{task_name}" }
                                        div { class: "fx-where-sub", "턴 {a}–{b} · {task_turns.len()}턴" }
                                    }
                                }
                                {route_summary(&f.summary(task))}
                            }
                            Sidebar { flow: f.clone(), metro: m.clone(), scope: scope.clone(), visible: visible.clone(), turn, tab, detail }
                        }
                    }
                }
            }
            main { class: "fx-main",
                div { key: "{is_detail}", class: "{anim}",
                    if !is_detail {
                        OverviewMap { flow: f.clone(), turn, hover, level, zoom }
                    } else if !visible.is_empty() {
                        LineMap { flow: f.clone(), turns: visible.clone(), turn, detail, covered: if card { CARD_W } else { 0.0 } }
                    } else {
                        div { class: "fx-empty", "이 조건에 맞는 턴이 없습니다" }
                    }
                }
                // Stepping through the Calls in the order made: buttons, and ← → anywhere but
                // in a text field (the page clicks these for the keys).
                if total > 0 {
                    div { class: "fx-step",
                        button { id: "fx-prev", title: "이전 호출 (←)", disabled: at == Some(0), onclick: move |_| step_prev(-1), "‹" }
                        span { class: "n",
                            if let Some(i) = at { "{i + 1}" } else { "–" }
                            span { class: "of", " / {total}" }
                        }
                        button { id: "fx-next", title: "다음 호출 (→)", disabled: at == Some(total - 1), onclick: move |_| step_next(1), "›" }
                    }
                }
                div { class: "fx-zoom",
                    button { title: "확대", onclick: zoom_in, "+" }
                    button { title: "축소", disabled: !is_detail, onclick: zoom_out, "−" }
                    button {
                        class: "fit",
                        title: "노선 전체를 한 화면에",
                        disabled: !is_detail,
                        onclick: move |_| { document::eval(&format!("{PAN_JS}\nfxMap.fit()")); },
                        "맞춤"
                    }
                    button {
                        class: if !is_detail { "fit on" } else { "fit" },
                        title: "전체 노선망",
                        onclick: move |_| level.set(Level::Overview),
                        "노선망"
                    }
                }
                div { class: "fx-brand", "WITHHUMAN" }
                // Who told the agent to make the Call picked: a card over the map's right side.
                if card {
                    super::metro::RoutePanel { metro: m.clone(), dest: picked, hovered, detail }
                }
            }
            {pick_list}
        }
    }
}

// ── The list ──────────────────────────────────────────────────────────────────────────────

fn check_icon() -> Element {
    rsx! {
        svg { width: "15", height: "15", view_box: "0 0 16 16", fill: "none", stroke: "#8a8f98", stroke_width: "1.3",
            circle { cx: "8", cy: "8", r: "6.5" }
            path { d: "M5.2 8.2l2 2 3.6-4" }
        }
    }
}

fn warn_icon(s: f64) -> Element {
    rsx! {
        svg { width: "{s}", height: "{s}", view_box: "0 0 16 16",
            path { d: "M8 1.5L15 14H1z", fill: AMBER }
            rect { x: "7.3", y: "5.5", width: "1.4", height: "4.5", fill: "#fff" }
            rect { x: "7.3", y: "11", width: "1.4", height: "1.4", fill: "#fff" }
        }
    }
}

fn blocked_icon(s: f64) -> Element {
    rsx! {
        svg { width: "{s}", height: "{s}", view_box: "0 0 16 16",
            circle { cx: "8", cy: "8", r: "7", fill: "#dc2626" }
            path { d: "M5.3 5.3l5.4 5.4M10.7 5.3l-5.4 5.4", stroke: "#fff", stroke_width: "1.6", stroke_linecap: "round" }
        }
    }
}

/// The overview's list: the session's numbers, then its tasks by chapter, the one looked at
/// open on its turns.
#[component]
fn TaskPanel(
    flow: Arc<Flow>,
    turn: Signal<usize>,
    hover: Signal<Option<usize>>,
    level: Signal<Level>,
    zoom: Signal<f64>,
) -> Element {
    let state = use_context::<AppState>();
    let f = &flow;
    let t = turn().min(f.turns.len() - 1);
    let sel_task = f.task_of(t);
    let cur_ch = Flow::chapter_of(sel_task);
    let places: HashSet<&str> = f
        .turns
        .iter()
        .flat_map(|t| t.calls.iter().map(|c| c.key.as_str()))
        .collect();
    let blocked = f.turns.iter().filter(|t| t.blocked() > 0).count();
    let warn = f.turns.iter().filter(|t| t.warn() > 0).count();
    let stats = [
        ("노선", f.tasks.len(), ""),
        ("역", places.len(), ""),
        ("차단", blocked, "red"),
        ("확인", warn, "amber"),
    ];
    rsx! {
        div { class: "fx-tasks",
            div { class: "fx-tasks-head",
                div { class: "fx-kicker", "세션 노선망" }
                SessionPick { class: "fx-session-pick" }
                div { class: "fx-stats",
                    for (l, n, c) in stats {
                        div { class: "fx-stat",
                            div { class: "n {c}", "{n}" }
                            div { class: "l", "{l}" }
                        }
                    }
                }
            }
            div { class: "fx-kicker pad", "노선 · 작업" }
            div { class: "fx-task-list",
                for ch in 0..f.chapters() {
                    {
                        let range = f.chapter_tasks(ch);
                        let first_turn = f.tasks[range.start].from;
                        let fl = flow.clone();
                        rsx! {
                            button {
                                key: "ch{ch}",
                                class: if ch == cur_ch { "fx-ch on" } else { "fx-ch" },
                                onclick: move |_| select_turn(turn, state, &fl, first_turn),
                                span { "{ch + 1}구간" }
                                span { class: "rule" }
                                span { class: "st", if ch == cur_ch { "보는 중" } else { "펼치기" } }
                            }
                            if ch == cur_ch {
                                for ti in range {
                                    TaskRow { key: "task{ti}", flow: flow.clone(), task: ti, turn, hover, level, zoom }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn TaskRow(
    flow: Arc<Flow>,
    task: usize,
    turn: Signal<usize>,
    hover: Signal<Option<usize>>,
    level: Signal<Level>,
    zoom: Signal<f64>,
) -> Element {
    let state = use_context::<AppState>();
    let mut hover = hover;
    let f = &flow;
    let tk = &f.tasks[task];
    let t = turn().min(f.turns.len() - 1);
    let on = f.task_of(t) == task;
    let lit = on || hover() == Some(task);
    let turns: Vec<usize> = f.task_turns(task).collect();
    let b = turns.iter().filter(|&&i| f.turns[i].blocked() > 0).count();
    let w = turns.iter().filter(|&&i| f.turns[i].warn() > 0).count();
    let places: HashSet<&str> = turns
        .iter()
        .flat_map(|&i| f.turns[i].calls.iter().map(|c| c.key.as_str()))
        .collect();
    let swatch = if lit { LINE_COLOR } else { LINE_MUTED };
    let from = tk.from;
    let fl = flow.clone();
    let focus = move |n: usize, fl: &Flow| {
        let (mut level, mut zoom) = (level, zoom);
        select_turn(turn, state, fl, n);
        level.set(Level::Detail);
        zoom.set(1.0);
    };
    rsx! {
        div {
            class: if on { "fx-task on" } else { "fx-task" },
            onmouseenter: move |_| hover.set(Some(task)),
            onmouseleave: move |_| hover.set(None),
            button {
                class: "fx-task-btn",
                onclick: move |_| select_turn(turn, state, &fl, from),
                span { class: "fx-swatch", style: "background: {swatch}" }
                span { class: "fx-task-text",
                    span { class: "nm", "{tk.name}" }
                    span { class: "meta",
                        "역 {places.len()} · 턴 {tk.from + 1}–{tk.to + 1}"
                        if b > 0 { b { class: "red", " · 차단 {b}" } }
                        if w > 0 { b { class: "amber", " · 확인 {w}" } }
                    }
                    {legs(&f.summary(task))}
                }
            }
            if on {
                div { class: "fx-task-turns",
                    div { class: "fx-rail",
                        for i in turns.iter().copied() {
                            {
                                let tt = &f.turns[i];
                                let dot = if tt.blocked() > 0 { "dot red" } else if tt.warn() > 0 { "dot amber" } else { "dot" };
                                let fl = flow.clone();
                                rsx! {
                                    button {
                                        key: "{i}",
                                        class: if i == t { "fx-tturn on" } else { "fx-tturn" },
                                        onclick: move |_| focus(i, &fl),
                                        span { class: "{dot}" }
                                        span { class: "no", "턴 {i + 1}" }
                                        span { class: "rq", "{clip(&tt.req, 60)}" }
                                    }
                                }
                            }
                        }
                    }
                    {
                        let fl = flow.clone();
                        rsx! {
                            button { class: "fx-go", onclick: move |_| focus(t, &fl), "자세히 보기 ›" }
                        }
                    }
                }
            }
        }
    }
}

/// How long a task took, as a route finder heads a route: "1시간 27분", "12분", "40초".
fn span_text(ms: i64) -> String {
    let secs = ms.max(0) / 1000;
    if secs < 60 {
        format!("{secs}초")
    } else if secs < 3600 {
        format!("{}분", secs / 60)
    } else {
        format!("{}시간 {}분", secs / 3600, secs % 3600 / 60)
    }
}

/// A task's legs as a row of badges, as a route finder lists a route's rides.
fn legs(s: &Summary) -> Element {
    rsx! {
        span { class: "fx-legs",
            for (i, l) in s.legs.iter().enumerate() {
                if i > 0 { span { class: "sep", "›" } }
                match l {
                    Leg::Ask(n) => rsx! { span { class: "leg ask", "턴 {n}" } },
                    Leg::Text(via) => rsx! { span { class: "leg text", title: "{via} 결과의 글을 따름", "글 · {via}" } },
                    Leg::Stopped { blocked, name, times } => {
                        let (cls, mark) = if *blocked { ("leg deny", "✕") } else { ("leg hold", "▲") };
                        let n = if *times > 1 { format!(" ×{times}") } else { String::new() };
                        rsx! { span { class: "{cls}", "{mark} {name}{n}" } }
                    }
                    Leg::Allowed(n) => rsx! { span { class: "leg ok", "허용 {n}" } },
                }
            }
            if s.more > 0 { span { class: "leg more", "+{s.more}" } }
        }
    }
}

/// A task's summary over its list: how long it took in large type, its counts, its legs.
fn route_summary(s: &Summary) -> Element {
    let took = s.took.map(span_text).unwrap_or_else(|| "—".into());
    rsx! {
        div { class: "fx-sum",
            div { class: "fx-sum-head",
                b { class: "took", "{took}" }
                span { class: "facts",
                    "턴 {s.turns} · 호출 {s.calls}"
                    if s.blocked > 0 { span { class: "red", " · 차단 {s.blocked}" } }
                    if s.warn > 0 { span { class: "amber", " · 확인 {s.warn}" } }
                }
            }
            {legs(s)}
        }
    }
}

/// Whether the session list is open over the map.
#[derive(Clone, Copy)]
struct Picking(Signal<bool>);

/// The session's name, opening the session list to switch to another.
#[component]
fn SessionPick(class: &'static str) -> Element {
    let state = use_context::<AppState>();
    let mut picking = use_context::<Picking>().0;
    let current = state.current.read().clone().unwrap_or_default();
    let label = match &*state.sessions.read() {
        Load::Ready(list) => list
            .iter()
            .find(|s| s.id == current)
            .map(|s| {
                let folder = s.folder();
                let title = clip(s.title(), 40);
                let when = day_time(s.last_at.or(s.created_at));
                [folder, title, when]
                    .into_iter()
                    .filter(|t| !t.is_empty())
                    .collect::<Vec<_>>()
                    .join(" · ")
            })
            .unwrap_or_else(|| "세션 고르기".into()),
        _ => "세션 불러오는 중".into(),
    };
    rsx! {
        button {
            class: "{class} fx-pick-btn",
            title: "{label} · 다른 세션 고르기",
            onclick: move |_| picking.toggle(),
            span { class: "lb", "{label}" }
            span { class: "act", "바꾸기" span { class: "chev", "▾" } }
        }
    }
}

/// A moment as the session picker writes it: the day and the time.
fn day_time(ms: Option<i64>) -> String {
    use chrono::TimeZone;
    ms.and_then(|ms| chrono::Local.timestamp_millis_opt(ms).single())
        .map(|t| t.format("%m/%d %H:%M").to_string())
        .unwrap_or_default()
}

/// The line's list: the tabs, where from and where to, then the task's turns as cards.
#[component]
fn Sidebar(
    flow: Arc<Flow>,
    metro: Arc<Metro>,
    scope: Vec<usize>,
    visible: Vec<usize>,
    turn: Signal<usize>,
    tab: Signal<Tab>,
    detail: Signal<bool>,
) -> Element {
    let mut state = use_context::<AppState>();
    let mut tab = tab;
    let mut detail = detail;
    let f = &flow;
    let mut toggled = use_signal(std::collections::HashMap::<usize, bool>::new);
    let mut query = use_signal(String::new);
    let mut searching = use_signal(|| false);
    let t = turn().min(f.turns.len() - 1);
    let tabs: Vec<(Tab, usize)> = [Tab::All, Tab::Blocked, Tab::Warn, Tab::Ok]
        .into_iter()
        .map(|k| (k, scope.iter().filter(|&&i| k.holds(&f.turns[i])).count()))
        .collect();
    let found: Vec<(i64, String, String)> = if searching() {
        metro
            .find(&query())
            .into_iter()
            .map(|s| (s.seq, s.name(), hm(s.at)))
            .collect()
    } else {
        Vec::new()
    };
    let mut pick = move |seq: i64| {
        searching.set(false);
        query.set(String::new());
        state.selected.set(Selection::Node(seq));
        detail.set(true);
    };
    let first_found = found.first().map(|c| c.0);
    let live = (state.live)();
    // The picked turn's card stays in view in the list.
    use_effect(move || {
        let t = turn();
        document::eval(&format!(
            "const el=document.getElementById('fx-turn-{t}');const list=el&&el.closest('.fx-turns');\
             if(el&&list){{const top=el.offsetTop-list.offsetTop;\
             if(top<list.scrollTop||top+60>list.scrollTop+list.clientHeight)list.scrollTo({{top:top-40,behavior:'smooth'}});}}"
        ));
    });
    rsx! {
        div { class: "fx-side",
            div { class: "fx-side-top",
                div { class: "fx-tabs",
                    for (k, n) in tabs {
                        button {
                            class: if tab() == k { "on" } else { "" },
                            disabled: n == 0,
                            onclick: move |_| tab.set(k),
                            "{k.name()} "
                            span { class: "n", "{n}" }
                        }
                    }
                }
                div { class: "fx-od",
                    div { class: "fx-od-row",
                        span { class: "fx-od-dot start" }
                        SessionPick { class: "fx-od-pick" }
                    }
                    div { class: "fx-od-row find",
                        span { class: "fx-od-dot end" }
                        input {
                            value: "{query}",
                            placeholder: "도착 역 찾기 · 파일, 명령, 도구",
                            onfocus: move |_| searching.set(true),
                            onblur: move |_| searching.set(false),
                            oninput: move |e| query.set(e.value()),
                            onkeydown: move |e| match e.key() {
                                Key::Escape => { searching.set(false); query.set(String::new()); }
                                Key::Enter => if let Some(seq) = first_found { pick(seq) },
                                _ => {}
                            },
                        }
                        if searching() && !query().trim().is_empty() {
                            div { class: "fx-suggest",
                                if found.is_empty() { div { class: "none", "그런 역이 없습니다. 파일 이름이나 명령으로 찾아 보세요." } }
                                for (seq, name, at) in found.iter().cloned() {
                                    div {
                                        class: "sug",
                                        onmousedown: move |e| { e.prevent_default(); pick(seq); },
                                        span { class: "nm", "{name}" }
                                        span { class: "t", "{at}" }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            div { class: "fx-live",
                button {
                    class: if live { "pill on" } else { "pill" },
                    onclick: move |_| state.live.toggle(),
                    if live { "ON" } else { "OFF" }
                }
                span { class: "lbl", "자동 갱신" }
                span { class: "cnt", "{visible.len()}개 턴" }
            }
            div { class: "fx-turns",
                if visible.is_empty() { div { class: "fx-none", "이 조건에 맞는 턴이 없습니다" } }
                for i in visible.iter().copied() {
                    {
                        let open = toggled().get(&i).copied().unwrap_or(i == t);
                        let fl = flow.clone();
                        rsx! {
                            TurnCard {
                                key: "{i}",
                                flow: flow.clone(),
                                turn_index: i,
                                selected: i == t,
                                open,
                                onselect: move |_| select_turn(turn, state, &fl, i),
                                ontoggle: move |_| { toggled.write().insert(i, !open); },
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn TurnCard(
    flow: Arc<Flow>,
    turn_index: usize,
    selected: bool,
    open: bool,
    onselect: EventHandler<()>,
    ontoggle: EventHandler<()>,
) -> Element {
    let state = use_context::<AppState>();
    let t = &flow.turns[turn_index];
    let (blocked, warn, ok) = (t.blocked(), t.warn(), t.ok());
    let picked = match (state.selected)() {
        Selection::Node(seq) => Some(seq),
        Selection::None => None,
    };
    let badge = if selected { LINE_COLOR } else { LINE_MUTED };
    let edge = if selected { 4 } else { 0 };
    let caret = if open { "fx-caret open" } else { "fx-caret" };
    let req_class = if open { "fx-req open" } else { "fx-req" };
    rsx! {
        section {
            id: "fx-turn-{turn_index}",
            class: if selected { "fx-card on" } else { "fx-card" },
            style: "box-shadow: inset {edge}px 0 0 {LINE_COLOR}",
            div { class: "fx-card-head", onclick: move |_| onselect.call(()),
                button {
                    class: "{caret}",
                    onclick: move |e| { e.stop_propagation(); ontoggle.call(()); },
                    "▶"
                }
                div { class: "fx-card-main",
                    div { class: "fx-card-top",
                        b { class: "fx-tno", style: "background: {badge}", "턴 {t.n}" }
                        span { class: "fx-counts",
                            if blocked > 0 { span { class: "c red", {blocked_icon(11.0)} "{blocked}" } }
                            if warn > 0 { span { class: "c amber", {warn_icon(11.0)} "{warn}" } }
                            span { class: "c ok", {check_icon()} "{ok}" }
                        }
                    }
                    p { class: "{req_class}", "{t.req}" }
                }
            }
            if open {
                div { class: "fx-items",
                    for (gi, item, n) in item_runs(t) {
                        match item {
                            // A group is listed open, as the line draws it: its Calls one by one,
                            // the same in a row on one row (×n).
                            Item::Group { calls, .. } => rsx! {
                                for (ci, n) in runs(&t.calls, calls.iter().copied()) {
                                    {call_row(&t.calls[ci], format!("{gi}-{ci}"), picked, state, false, n)}
                                }
                            },
                            Item::Call(ci) => call_row(&t.calls[*ci], gi.to_string(), picked, state, false, n),
                        }
                    }
                    // What the agent answered, last, in its violet.
                    if let Some(last) = t.said.last() {
                        {
                            let words = crate::flow::plain_words(&last.text);
                            let head = crate::flow::outcome(&last.text, 40);
                            rsx! {
                                div { class: "fx-item say", title: "{words}",
                                    span { class: "tone" }
                                    span { class: "ttl", "답변 · {head}" }
                                    span { class: "tm", "{hm(last.at)}" }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// A tool's icon, `size` px square, centred at (x, y), drawn in strokes of `color`.
fn glyph(g: Glyph, x: f64, y: f64, size: f64, color: &str) -> Element {
    let k = size / 16.0;
    let t = format!("translate({} {}) scale({k})", x - size / 2.0, y - size / 2.0);
    let w = (1.9 / k).min(2.6);
    rsx! {
        g { transform: "{t}", fill: "none", stroke: "{color}", stroke_width: "{w}", stroke_linecap: "round", stroke_linejoin: "round", class: "fx-nohit",
            match g {
                // A page with its corner folded and two lines.
                Glyph::Doc => rsx! { path { d: "M4 1.5h5.5L13 5v9.5H4z M9.5 1.5V5H13 M6.5 8.5h4 M6.5 11.5h4" } },
                // A magnifier.
                Glyph::Search => rsx! { circle { cx: "7", cy: "7", r: "4.5" } path { d: "M10.5 10.5L14.5 14.5" } },
                // A pencil.
                Glyph::Edit => rsx! { path { d: "M2.5 13.5l1-3.8 7.8-7.8 2.8 2.8-7.8 7.8z M9.6 3.6l2.8 2.8" } },
                // A prompt: > and _.
                Glyph::Run => rsx! { path { d: "M2.5 4l4 4-4 4 M8.5 12.5h5" } },
                // A globe.
                Glyph::Web => rsx! { circle { cx: "8", cy: "8", r: "6.2" } path { d: "M1.8 8h12.4 M8 1.8c-2.2 2.4-2.2 10 0 12.4 M8 1.8c2.2 2.4 2.2 10 0 12.4" } },
                // A plug.
                Glyph::Plug => rsx! { path { d: "M5.5 1.5v3.5 M10.5 1.5v3.5 M3.5 5h9v3a4.5 4.5 0 0 1-9 0z M8 12.5v2.5" } },
                // A person.
                Glyph::Agent => rsx! { circle { cx: "8", cy: "5", r: "2.8" } path { d: "M2.5 14.5c.6-3 2.8-4.6 5.5-4.6s4.9 1.6 5.5 4.6" } },
                // Three dots.
                Glyph::Other => rsx! { path { d: "M4 8h.1 M8 8h.1 M12 8h.1" } },
            }
        }
    }
}

/// Whether two Calls in a row read as the same: the same name, verdict, and words followed.
fn same_call(a: &crate::flow::Call, b: &crate::flow::Call) -> bool {
    a.title == b.title && a.blocked == b.blocked && a.warn == b.warn && a.via.is_some() == b.via.is_some()
}

/// Calls in a row that read the same, as one: each first one and how many it stands for.
fn runs(calls: &[crate::flow::Call], idx: impl Iterator<Item = usize>) -> Vec<(usize, usize)> {
    let mut out: Vec<(usize, usize)> = Vec::new();
    for i in idx {
        match out.last_mut() {
            Some((f, n)) if same_call(&calls[*f], &calls[i]) => *n += 1,
            _ => out.push((i, 1)),
        }
    }
    out
}

/// A turn's items with the same Call in a row as one: index, item, and how many it stands for.
fn item_runs(t: &Turn) -> Vec<(usize, &Item, usize)> {
    let mut out: Vec<(usize, &Item, usize)> = Vec::new();
    for (gi, item) in t.items.iter().enumerate() {
        if let (Item::Call(ci), Some((_, Item::Call(prev), n))) = (item, out.last_mut())
            && same_call(&t.calls[*prev], &t.calls[*ci])
        {
            *n += 1;
            continue;
        }
        out.push((gi, item, 1));
    }
    out
}

/// A Call's row in a turn's card: its verdict, name (with the tool for one folded in a
/// group), a "글" chip when it followed words read, and its time. Picking it picks the Call.
fn call_row(c: &crate::flow::Call, key: String, picked: Option<i64>, mut state: AppState, inner: bool, times: usize) -> Element {
    let seq = c.seq;
    let cls = format!(
        "fx-item call{}{}{}",
        if c.blocked { " red" } else { "" },
        if picked == Some(seq) { " picked" } else { "" },
        if inner { " inner" } else { "" }
    );
    let icon = if c.blocked { blocked_icon(13.0) } else if c.warn { warn_icon(13.0) } else { check_icon() };
    rsx! {
        div {
            key: "{key}",
            class: "{cls}",
            title: "{c.arg}",
            onclick: move |_| state.selected.set(Selection::Node(seq)),
            {icon}
            span { class: "glyph", title: "{c.glyph.name()}",
                svg { width: "14", height: "14", view_box: "0 0 14 14", {glyph(c.glyph, 7.0, 7.0, 13.0, c.tone.color())} }
            }
            span { class: "ttl", "{c.title}" }
            if times > 1 { span { class: "times", "×{times}" } }
            if inner { span { class: "tool", "{c.tool}" } }
            if c.via.is_some() { span { class: "fx-text-chip", "글" } }
            span { class: "tm", "{hm(c.at)}" }
        }
    }
}

// ── The overview ──────────────────────────────────────────────────────────────────────────

#[component]
fn OverviewMap(
    flow: Arc<Flow>,
    turn: Signal<usize>,
    hover: Signal<Option<usize>>,
    level: Signal<Level>,
    zoom: Signal<f64>,
) -> Element {
    let state = use_context::<AppState>();
    let mut hover = hover;
    let mut size = use_signal(|| (1000.0_f64, 700.0_f64));
    let f = &flow;
    let t = turn().min(f.turns.len() - 1);
    let sel_task = f.task_of(t);
    let ch = Flow::chapter_of(sel_task);
    let (w, h) = size();
    let o: Overview = f.overview(ch, w, h);
    let lit = hover().unwrap_or(sel_task);
    let dim = move |task: usize| lit != task;
    let num = |task: usize| format!("{:02}", task + 1);
    let color_of = |task: usize| {
        o.lines
            .iter()
            .find(|l| l.task == task)
            .map_or(LINE_COLOR, |l| l.color)
    };
    let focus = move |n: usize, fl: &Flow| {
        let (mut level, mut zoom) = (level, zoom);
        select_turn(turn, state, fl, n);
        level.set(Level::Detail);
        zoom.set(1.0);
    };
    let mut order: Vec<usize> = (0..o.lines.len()).collect();
    order.sort_by_key(|&i| o.lines[i].task == lit);
    let range = f.chapter_tasks(ch);
    let rest_tasks: Vec<usize> = range.clone().collect();

    rsx! {
        div {
            class: "fx-overview",
            onresize: move |e| {
                if let Ok(b) = e.get_border_box_size() {
                    let (w0, h0) = *size.peek();
                    if (b.width - w0).abs() > 1.0 || (b.height - h0).abs() > 1.0 {
                        size.set((b.width, b.height));
                    }
                }
            },
            svg { width: "{w}", height: "{h}", view_box: "0 0 {w} {h}", class: "fx-svg",
                for i in order.iter().copied() {
                    {
                        let l = &o.lines[i];
                        let task = l.task;
                        let from = f.tasks[task].from;
                        let sw = if task == sel_task { "11" } else { "7" };
                        // The other tasks step back as a route finder's other routes do: a faint
                        // trace, their stations unnamed, until hovered.
                        let op = if dim(task) { "0.13" } else { "1" };
                        let fl = flow.clone();
                        rsx! {
                            path {
                                key: "line{task}",
                                d: "{l.d}",
                                fill: "none",
                                stroke: "{l.color}",
                                stroke_width: "{sw}",
                                stroke_linecap: "round",
                                stroke_linejoin: "round",
                                opacity: "{op}",
                                class: "fx-hit",
                                onmouseenter: move |_| hover.set(Some(task)),
                                onmouseleave: move |_| hover.set(None),
                                onclick: move |_| select_turn(turn, state, &fl, from),
                            }
                        }
                    }
                }
                for l in o.lines.iter() {
                    for (j, (e, bpt)) in l.ends.iter().enumerate() {
                        {
                            let task = l.task;
                            let from = f.tasks[task].from;
                            let op = if dim(task) { "0.2" } else { "1" };
                            let fl = flow.clone();
                            let n = num(task);
                            rsx! {
                                g {
                                    key: "end{task}-{j}",
                                    opacity: "{op}",
                                    class: "fx-hit",
                                    onclick: move |_| select_turn(turn, state, &fl, from),
                                    onmouseenter: move |_| hover.set(Some(task)),
                                    onmouseleave: move |_| hover.set(None),
                                    circle { cx: "{e.0}", cy: "{e.1}", r: "10", fill: "#fff", stroke: "{l.color}", stroke_width: "5" }
                                    circle { cx: "{bpt.0}", cy: "{bpt.1}", r: "13", fill: "{l.color}" }
                                    text { x: "{bpt.0}", y: "{bpt.1 + 4.5}", text_anchor: "middle", font_size: "11.5", font_weight: "700", fill: "#fff", "{n}" }
                                }
                            }
                        }
                    }
                }
                for (si, s) in o.stops.iter().enumerate() {
                    {
                        let p = &o.places[s.place];
                        let faint = dim(s.task);
                        let op = if faint { "0.22" } else { "1" };
                        let turn_n = s.turn;
                        let fl = flow.clone();
                        let fl2 = flow.clone();
                        let tip = format!("{} · 턴 {} · 두 번 누르면 자세히", p.name, turn_n + 1);
                        let (x, y) = (s.x, s.y);
                        rsx! {
                            g {
                                key: "st{si}",
                                opacity: "{op}",
                                class: "fx-hit",
                                onclick: move |_| select_turn(turn, state, &fl, turn_n),
                                ondoubleclick: move |_| focus(turn_n, &fl2),
                                title { "{tip}" }
                                circle { cx: "{x}", cy: "{y}", r: "11", fill: "transparent" }
                                {overview_mark(x, y, p.blocked, p.warn)}
                                if !faint {
                                    {overview_label(x, y, s.level, &p.name, label_fill(p), s.task == sel_task || p.blocked || p.warn)}
                                }
                            }
                        }
                    }
                }
                for (xi, x) in o.xfers.iter().enumerate() {
                    {
                        let p = &o.places[x.place];
                        let faded = !x.tasks.contains(&lit);
                        let op = if faded { "0.2" } else { "1" };
                        let owner = if x.tasks.contains(&sel_task) { sel_task } else { x.tasks[0] };
                        let turn_n = o.turn_for(x.place, owner, f);
                        let fill = if p.blocked { RED } else if p.warn { AMBER } else { "#fff" };
                        let tip = format!("{} · 환승: 노선 {} · {} · 두 번 누르면 자세히", p.name, num(x.tasks[0]), num(x.tasks[1]));
                        let fl = flow.clone();
                        let fl2 = flow.clone();
                        rsx! {
                            g {
                                key: "x{xi}",
                                opacity: "{op}",
                                class: "fx-hit",
                                onclick: move |_| select_turn(turn, state, &fl, turn_n),
                                ondoubleclick: move |_| focus(turn_n, &fl2),
                                title { "{tip}" }
                                circle { cx: "{x.x}", cy: "{x.y}", r: "8", fill: "{fill}", stroke: "#2b2e34", stroke_width: "2.5" }
                                if !faded {
                                    {overview_label(x.x, x.y, false, &p.name, label_fill(p), true)}
                                }
                            }
                        }
                    }
                }
            }
            // The chapters: picking one draws its network.
            div { class: "fx-chapters",
                for c in 0..f.chapters() {
                    {
                        let r = f.chapter_tasks(c);
                        let turns: Vec<&Turn> = r.clone().flat_map(|ti| f.task_turns(ti)).map(|i| &f.turns[i]).collect();
                        let b = turns.iter().filter(|t| t.blocked() > 0).count();
                        let wn = turns.iter().filter(|t| t.warn() > 0).count();
                        let from_t = hm(turns.iter().find_map(|t| t.at()));
                        let to_t = hm(turns.iter().rev().find_map(|t| t.at()));
                        let span = match (from_t.is_empty(), from_t == to_t) {
                            (true, _) => String::new(),
                            (false, true) => from_t,
                            (false, false) => format!("{from_t}–{to_t}"),
                        };
                        let first = f.tasks[r.start].from;
                        let fl = flow.clone();
                        let n_tasks = r.len();
                        rsx! {
                            button {
                                key: "c{c}",
                                class: if c == ch { "fx-chapter on" } else { "fx-chapter" },
                                onclick: move |_| select_turn(turn, state, &fl, first),
                                span { class: "top", "{c + 1}구간" if !span.is_empty() { span { class: "tm", "{span}" } } }
                                span { class: "bot",
                                    "작업 {n_tasks}"
                                    if b > 0 { span { class: "red", i { class: "d" } "차단 {b}" } }
                                    if wn > 0 { span { class: "amber", i { class: "d dia" } "확인 {wn}" } }
                                }
                            }
                        }
                    }
                }
            }
            if o.lines.is_empty() {
                div { class: "fx-ov-empty", "이 구간의 작업은 대화만 하고 파일이나 명령을 쓰지 않았습니다." }
            }
            div { class: "fx-title",
                div { class: "big", "{ch + 1}구간 노선망" }
                p { "노선은 작업, 역은 파일·명령입니다. 두 노선이 만나는 역은 두 작업이 함께 건드린 파일입니다." }
            }
            div { class: "fx-legend",
                for task in rest_tasks {
                    {
                        let from = f.tasks[task].from;
                        let fl = flow.clone();
                        let bare = o.bare.contains(&task);
                        let cls = format!(
                            "fx-leg{}{}{}",
                            if task == sel_task { " on" } else { "" },
                            if dim(task) { " dim" } else { "" },
                            if bare { " bare" } else { "" }
                        );
                        let col = if bare { "transparent" } else { color_of(task) };
                        let n = num(task);
                        let tip = if bare { "역 없음: 대화만 한 작업이라 지도에 노선을 그리지 않았습니다" } else { "" };
                        rsx! {
                            button {
                                key: "lg{task}",
                                class: "{cls}",
                                title: "{tip}",
                                onmouseenter: move |_| hover.set(Some(task)),
                                onmouseleave: move |_| hover.set(None),
                                onclick: move |_| select_turn(turn, state, &fl, from),
                                span { class: "no", style: "background: {col}", "{n}" }
                                span { class: "nm", "{f.tasks[task].name}" }
                                if bare { span { class: "none", "역 없음" } }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn label_fill(p: &crate::flow::Place) -> &'static str {
    if p.blocked {
        RED
    } else if p.warn {
        "#a8790c"
    } else if p.text {
        TEXT_COLOR
    } else {
        "#3a3d44"
    }
}

fn overview_mark(x: f64, y: f64, blocked: bool, warn: bool) -> Element {
    if blocked {
        let d = format!(
            "M{} {}l5.2 5.2M{} {}l-5.2 5.2",
            x - 2.6,
            y - 2.6,
            x + 2.6,
            y - 2.6
        );
        rsx! {
            circle { cx: "{x}", cy: "{y}", r: "7", fill: RED, stroke: "#fff", stroke_width: "2" }
            path { d: "{d}", stroke: "#fff", stroke_width: "1.7", stroke_linecap: "round" }
        }
    } else if warn {
        rsx! {
            g { transform: "translate({x - 8.0} {y - 8.0})",
                path { d: "M8 1L16 14.5H0z", fill: AMBER, stroke: "#fff", stroke_width: "1.6", stroke_linejoin: "round" }
                rect { x: "7.25", y: "5", width: "1.5", height: "4.6", rx: "0.75", fill: "#fff" }
                circle { cx: "8", cy: "11.8", r: "0.85", fill: "#fff" }
            }
        }
    } else {
        rsx! { circle { cx: "{x}", cy: "{y}", r: "4", fill: "#fff" } }
    }
}

fn overview_label(
    x: f64,
    y: f64,
    level: bool,
    name: &str,
    fill: &'static str,
    bold: bool,
) -> Element {
    let fw = if bold { "700" } else { "500" };
    if level {
        rsx! {
            text {
                transform: "translate({x + 3.0} {y - 11.0}) rotate(-38)",
                font_size: "12", font_weight: "{fw}", fill: "{fill}",
                stroke: "#fff", stroke_width: "3", paint_order: "stroke",
                "{name}"
            }
        }
    } else {
        rsx! {
            text {
                x: "{x + 13.0}", y: "{y + 4.0}",
                font_size: "12", font_weight: "{fw}", fill: "{fill}",
                stroke: "#fff", stroke_width: "3", paint_order: "stroke",
                "{name}"
            }
        }
    }
}

// ── One task's line ───────────────────────────────────────────────────────────────────────

#[component]
fn LineMap(
    flow: Arc<Flow>,
    turns: Vec<usize>,
    turn: Signal<usize>,
    detail: Signal<bool>,
    /// How much of the view's right side a card covers.
    covered: f64,
) -> Element {
    let mut state = use_context::<AppState>();
    let mut detail = detail;
    let f = &flow;
    // The groups opened by hand into their Calls, by (turn, item).
    let mut opened = use_signal(HashSet::<(usize, usize)>::new);
    let mut size = use_signal(|| (0.0_f64, 0.0_f64));
    // The line folds into rows as wide as the view, so it is read down rather than across.
    let wrap = (size().0 - covered - 180.0).max(520.0);
    let line = f.line_open(&turns, &opened(), wrap);
    let t = turn().min(f.turns.len() - 1);
    // The turn whose heading is opened to its whole intent and the person's own words.
    let mut head_open = use_signal(|| None::<usize>);
    // The turn whose answer is opened to what the agent said, whole.
    let mut say_open = use_signal(|| None::<usize>);
    // The branch whose cause is opened to the words read, whole.
    let mut cause_open = use_signal(|| None::<usize>);
    // Whether the way back from the Call picked to what told it is drawn: the passages from it to
    // the station whose output it followed, from that one to its own, and so on up.
    let mut trace_on = use_signal(|| false);
    let picked = match (state.selected)() {
        Selection::Node(seq) => Some(seq),
        Selection::None => None,
    };
    let (w_all, h) = size();
    let framed = use_signal(|| None::<(usize, Option<i64>, u64)>);
    // A Call folded into a group is found at the group.
    let in_group = |s: &Stop, seq: i64| {
        matches!(
            s.group.and_then(|g| f.turns[s.turn].items.get(g)),
            Some(Item::Group { calls, .. }) if calls.iter().any(|&i| f.turns[s.turn].calls[i].seq == seq)
        )
    };
    let grouped = picked.and_then(|seq| {
        line.stops
            .iter()
            .find(|s| s.turn == t && in_group(s, seq))
            .cloned()
    });
    let target_at = picked
        .and_then(|seq| {
            line.stops
                .iter()
                .find(|s| s.is(seq) && s.turn == t)
        })
        .or(grouped.as_ref())
        .map(|s| (s.x, s.y));
    // A turn or a Call picked is brought into view by the page's own mover (pan.js).
    let mark_at = line.marks.iter().find(|m| m.turn == t).map(|m| {
        let (x, y, w) = m.zones.first().map_or((m.x, m.y - 206.0, 0.0), |z| (z.0, z.1, z.2));
        (x, y, w, m.rev)
    });
    let lkey = (line.width as u64) << 20 ^ line.height as u64;
    if let Some((mx, my, mw, rev)) = mark_at
        && h > 0.0
        && *framed.peek() != Some((t, picked, lkey))
    {
        let mut framed = framed;
        framed.set(Some((t, picked, lkey)));
        let (tx, ty) = target_at.map_or(("null".to_owned(), "null".to_owned()), |(x, y)| (x.to_string(), y.to_string()));
        document::eval(&format!(
            "{PAN_JS}\nrequestAnimationFrame(() => fxMap.frame({mx}, {my}, {tx}, {ty}, {rev}, {mw}));"
        ));
    }
    let _ = w_all;

    // The icons this line uses, for its legend; the request, answers, the words' branch and
    // the verdicts around them.
    let mut glyphs: Vec<Glyph> = Vec::new();
    for s in &line.stops {
        if let Some(g) = s.glyph
            && !glyphs.contains(&g)
        {
            glyphs.push(g);
        }
    }
    glyphs.sort_by_key(|g| Glyph::ALL.iter().position(|a| a == g).unwrap_or(9));
    let has_ask = line.stops.iter().any(|s| s.tone == Tone::Ask);
    let has_say = line.stops.iter().any(|s| s.kind == Kind::Say);
    let has_branch = !line.branch_labels.is_empty();
    // The way back, when it is on: (cause, follower) pairs from the Call picked up its causes.
    let mut trace: Vec<(i64, i64)> = Vec::new();
    if trace_on() {
        let mut cur = picked;
        let mut seen = HashSet::new();
        while let Some(q) = cur.filter(|q| seen.insert(*q)) {
            let by = f.call(q).and_then(|c| c.by);
            if let Some(b) = by {
                trace.push((b, q));
            }
            cur = by;
        }
    }
    // The quotes shown (each over its stretch of the lower track; one tied to Calls only when
    // one of them is picked), each moved down under any shown before it that it would cover.
    let mut quote_y: HashMap<usize, f64> = HashMap::new();
    let mut placed: Vec<(f64, f64, f64)> = Vec::new();
    for (bi, b) in line.branch_labels.iter().enumerate() {
        if !(b.about.is_empty() || picked.is_some_and(|p| b.about.contains(&p))) {
            continue;
        }
        let mut y = b.qy;
        while placed.iter().any(|&(x0, x1, py)| (py - y).abs() < 58.0 && x0 < b.qx + b.qw && x1 > b.qx) {
            y += 62.0;
        }
        placed.push((b.qx, b.qx + b.qw, y));
        quote_y.insert(bi, y);
    }
    let has_block = line.stops.iter().any(|s| s.kind == Kind::Blocked);
    let has_warn = line.stops.iter().any(|s| s.kind == Kind::Warn);
    let target = picked.and_then(|seq| {
        line.stops
            .iter()
            .find(|s| s.is(seq) && s.turn == t && s.kind != Kind::Transfer)
            .or_else(|| {
                line.stops
                    .iter()
                    .find(|s| s.is(seq) && s.turn == t)
            })
            .cloned()
            .or(grouped.clone())
    });
    let col = move |n: usize| if n == t { LINE_COLOR } else { LINE_MUTED };
    let mut segs = line.segs.clone();
    segs.sort_by_key(|s| s.turn == t);

    rsx! {
        div {
            id: "fx-line",
            class: "fx-line",
            onresize: move |e| {
                if let Ok(b) = e.get_border_box_size() {
                    size.set((b.width, b.height));
                }
            },
            onmounted: move |_| {
                document::eval(&format!("{PAN_JS}\nfxMap.attach();"));
            },
            // The map and what opens over it, moved and scaled together by the page (pan.js);
            // nothing here sets its transform, so drawing it again does not undo a move.
            div { id: "fx-layer", class: "fx-layer",
            svg {
                class: "fx-line-svg",
                width: "{line.width}",
                height: "{line.height}",
                view_box: "0 0 {line.width} {line.height}",
                // Each turn a zone of its own: the picked one tinted under a bar in the line's
                // colour, the others alternately faint, a dashed line between them.
                for (i, m) in line.marks.iter().enumerate() {
                    {
                        let on = m.turn == t;
                        // Only the turn picked is tinted; the others are outlines.
                        let fill = if on { "#f3f4f6" } else { "#ffffff" };
                        let _ = i;
                        let n = m.turn;
                        rsx! {
                            g { key: "zone{n}",
                                for (zi, &(zx, zy, zw, zh)) in m.zones.iter().enumerate() {
                                    {
                                        let fl = flow.clone();
                                        rsx! {
                                            rect {
                                                key: "{zi}",
                                                x: "{zx}", y: "{zy}", width: "{zw}", height: "{zh}", rx: "14",
                                                fill: "{fill}",
                                                stroke: if on { "#e4e4e7" } else { "#f0f0f2" },
                                                class: "fx-hit",
                                                onclick: move |_| select_turn(turn, state, &fl, n),
                                            }
                                        }
                                    }
                                }
                                if on && let Some(&(zx, zy, zw, _)) = m.zones.first() {
                                    rect { x: "{zx}", y: "{zy}", width: "{zw}", height: "4", rx: "2", fill: LINE_COLOR, class: "fx-nohit" }
                                }
                            }
                        }
                    }
                }
                if let Some(tg) = target.as_ref().filter(|s| !s.route.is_empty()) {
                    g { fill: "none", stroke_linecap: "round", stroke_linejoin: "round",
                        path { d: "{tg.route}", stroke: "#e4e1ea", stroke_width: "24" }
                        path { d: "{tg.route}", stroke: "#fff", stroke_width: "19" }
                    }
                }
                for (i, s) in line.segs.iter().enumerate() {
                    {
                        let n = s.turn;
                        let fl = flow.clone();
                        rsx! {
                            path {
                                key: "hit{i}",
                                d: "{s.d}",
                                fill: "none",
                                stroke: "transparent",
                                stroke_width: "60",
                                style: "pointer-events: stroke",
                                class: "fx-hit",
                                onclick: move |_| select_turn(turn, state, &fl, n),
                            }
                        }
                    }
                }
                // A passage shows only for the Call picked: the one into it, or those out of it.
                for (i, s) in segs.iter().enumerate().filter(|(_, s)| !s.link || trace.iter().any(|&(c, f)| s.about.contains(&c) && s.about.contains(&f))) {
                    {
                        // Under the coloured stretches the line is grey: where it runs on past a
                        // turn's last station nothing was done.
                        let c = if s.branch {
                            if s.turn == t { TEXT_COLOR } else { TEXT_MUTED }
                        } else {
                            LINE_MUTED
                        };
                        rsx! {
                            g { key: "seg{i}", class: "fx-nohit",
                                if s.link {
                                    // A passage from the station whose output was read to the
                                    // Call that followed it: thin, under the line, not ridden.
                                    path { d: "{s.d}", fill: "none", stroke: "{c}", stroke_width: "2.5", stroke_dasharray: if s.unsure { "5 5" } else { "none" }, stroke_linecap: "round", stroke_linejoin: "round", opacity: "0.9" }
                                    path { d: "{arrow_head(&s.d)}", fill: "{c}" }
                                } else if s.ret {
                                    // The way back to the next row: faint and dashed, not a stretch ridden.
                                    path { d: "{s.d}", fill: "none", stroke: "#c9ccd2", stroke_width: "3", stroke_dasharray: "2 9", stroke_linecap: "round", stroke_linejoin: "round" }
                                } else if s.unsure {
                                    // A branch the engine was less than even sure of.
                                    path { d: "{s.d}", fill: "none", stroke: "{c}", stroke_width: "6", stroke_dasharray: "10 7", stroke_linecap: "butt", stroke_linejoin: "round" }
                                } else {
                                    path { d: "{s.d}", fill: "none", stroke: "{c}", stroke_width: "6", stroke_linecap: "round", stroke_linejoin: "round" }
                                }
                                if let Some((ex, ey)) = s.end {
                                    line { x1: "{ex}", x2: "{ex}", y1: "{ey - 12.0}", y2: "{ey + 12.0}", stroke: "{c}", stroke_width: "4" }
                                }
                            }
                        }
                    }
                }
                // The main line in the colours of what was done along it: each stretch the
                // colour of the station it reaches. Other turns keep them, faint.
                for (i, ts) in line.tones.iter().enumerate() {
                    {
                        let op = if ts.turn == t { "1" } else { "0.28" };
                        let d = ts.d.clone();
                        rsx! {
                            path { key: "tone{i}", class: "fx-nohit", d: "{d}", fill: "none", stroke: ts.tone.color(), stroke_width: "7", stroke_linecap: "round", opacity: "{op}" }
                        }
                    }
                }
                // Which way the line is read: small arrows on it.
                for (i, &(ax, ay, deg)) in line.arrows.iter().enumerate() {
                    path {
                        key: "arrow{i}",
                        class: "fx-nohit",
                        d: "M-1.8 -2.8 L1.6 0 L-1.8 2.8",
                        transform: "translate({ax} {ay}) rotate({deg})",
                        fill: "none",
                        stroke: "#fff",
                        stroke_width: "1.5",
                        stroke_linecap: "round",
                        stroke_linejoin: "round",
                        opacity: "0.95",
                    }
                }
                for s in line.stops.iter().cloned() {
                    {
                        let color = if s.branch {
                            if s.turn == t { TEXT_COLOR } else { "#c9c2dd" }
                        } else if s.turn == t {
                            s.tone.color()
                        } else {
                            col(s.turn)
                        };
                        let sel = target.as_ref().is_some_and(|tg| tg.id == s.id);
                        let open = s.group.is_some_and(|gi| opened().contains(&(s.turn, gi)));
                        let fl = flow.clone();
                        let st = s.clone();
                        rsx! {
                            LineStop {
                                key: "{s.id}",
                                stop: s.clone(),
                                color: color.to_string(),
                                sel,
                                detail: s.turn == t,
                                open,
                                onsel: move |_| {
                                    select_turn(turn, state, &fl, st.turn);
                                    if st.kind == Kind::Say {
                                        say_open.set(if say_open() == Some(st.turn) { None } else { Some(st.turn) });
                                    } else if let Some(gi) = st.group {
                                        // A group opens into its Calls on the line itself, and
                                        // closes again from its head.
                                        let mut o = opened.write();
                                        if !o.remove(&(st.turn, gi)) {
                                            o.insert((st.turn, gi));
                                        }
                                    } else if let Some(seq) = st.seq {
                                        state.selected.set(Selection::Node(seq));
                                        detail.set(true);
                                    }
                                },
                            }
                        }
                    }
                }
                // A zone's heading: the turn and what it was for, the intent the agent wrote (else
                // the request) cut to its gist; the stations under it are what the agent did.
                // Clicked, it opens to the intent whole and the person's own words.
                for m in line.marks.iter() {
                    {
                        let on = m.turn == t;
                        let n = m.turn;
                        let fl = flow.clone();
                        let (hx, hy, hw, _) = m.zones.first().copied().unwrap_or((m.x - 70.0, m.y - 206.0, 200.0, 0.0));
                        let w = (hw - 28.0).max(60.0);
                        let tt = &f.turns[n];
                        let who = match (tt.agent.is_some(), tt.intent.is_some()) {
                            (true, _) => "에이전트 보고",
                            (false, true) => "요청 의도",
                            (false, false) => "요청",
                        };
                        let heading = m.head.clone();
                        let opened = head_open() == Some(n);
                        let ask = if f.turns[n].agent.is_some() { "report" } else { "ask" };
                        let side = if m.rev { " rev" } else { "" };
                        let cls = if on { format!("fx-turnhead {ask}{side} on") } else { format!("fx-turnhead {ask}{side}") };
                        rsx! {
                            foreignObject {
                                key: "head{n}",
                                x: "{hx + 14.0}", y: "{hy + 14.0}", width: "{w}", height: "128",
                                div {
                                    class: "{cls}",
                                    title: "눌러서 사용자 프롬프트 보기",
                                    onclick: move |_| {
                                        select_turn(turn, state, &fl, n);
                                        head_open.set(if opened { None } else { Some(n) });
                                    },
                                    div { class: "top",
                                        span { class: "no", "턴 {n + 1}" }
                                        span { class: "who", "{who}" }
                                        span { class: "more", if opened { "닫기 ▴" } else { "원문 ▾" } }
                                    }
                                    div { class: "rq", "{heading}" }
                                    if let Some(res) = m.result.as_ref() {
                                        div { class: "res", "→ {res}" }
                                    }
                                }
                            }
                        }
                    }
                }
                // A quote over each stretch on the lower track; one tied to Calls only when picked.
                for (bi, b) in line.branch_labels.iter().enumerate().filter(|(i, _)| quote_y.contains_key(i)) {
                    {
                        let qy = quote_y[&bi];
                        let on = b.turn == t;
                        let (op, fill) = if on { ("1", TEXT_COLOR) } else { ("0.55", "#c9c2dd") };
                        let opened = cause_open() == Some(bi);
                        rsx! {
                            if b.end {
                                g { key: "bl{b.turn}-{b.x}", transform: "translate({b.x} {b.y - 12.0})", opacity: "{op}",
                                    // On a row running left the badge and words read leftwards.
                                    rect { x: if b.rev { "-26" } else { "0" }, width: "26", height: "24", rx: "5", fill: "{fill}" }
                                    text { x: if b.rev { "-13" } else { "13" }, y: "16.5", text_anchor: "middle", font_size: "13", font_weight: "700", fill: "#fff", "글" }
                                    text { x: if b.rev { "-33" } else { "33" }, y: "17", text_anchor: if b.rev { "end" } else { "start" }, font_size: "13.5", fill: TEXT_COLOR, "{b.text}" }
                                }
                            }
                            // What told the branch, where it bends away: the error or output read,
                            // and how sure the engine was. Clicked, it opens the words whole.
                            if let Some(c) = b.cause.as_ref() {
                                {
                                    let kind = if c.error { "오류" } else { "출력" };
                                    let cls = format!("fx-cause{}{}", if c.error { " err" } else { "" }, if on { " on" } else { "" });
                                    let quote = c.quote.clone().unwrap_or_else(|| "결과 보기".into());
                                    let p = (c.p * 100.0).round();
                                    rsx! {
                                        foreignObject { key: "cq{bi}", x: "{b.qx}", y: "{qy}", width: "{b.qw}", height: "60",
                                            div {
                                                class: "{cls}",
                                                title: "눌러서 원인 글 전체 보기",
                                                onclick: move |_| cause_open.set(if opened { None } else { Some(bi) }),
                                                div { class: "row",
                                                    span { class: "k", "{kind}" }
                                                    span { class: "q", "“{quote}”" }
                                                    span { class: "p", "{p}%" }
                                                }
                                                // The station whose output it was.
                                                div { class: "from", "← {b.text}의 {kind}" }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                // Before the first station, pointing at it, clear of the names over the line.
                if let Some(first) = line.stops.first() {
                    {pin_left(first.x - 14.0, first.y, "출발", GREEN)}
                }
                if let Some(tg) = target.as_ref().filter(|s| s.kind != Kind::Start) {
                    {
                        // Under the line when the station's name is written above it, not on it.
                        let color = if tg.kind == Kind::Blocked { RED } else { "#1f2125" };
                        if tg.up == Some(true) && !tg.label.is_empty() {
                            pin_below(tg.x, tg.y + 16.0, "도착", color)
                        } else {
                            pin(tg.x, tg.y - 16.0, "도착", color)
                        }
                    }
                }
            }
            // A heading opened: the intent whole and the person's prompt as they typed it.
            if let Some(m) = head_open().and_then(|n| line.marks.iter().find(|m| m.turn == n)) {
                {
                    let tt = &f.turns[m.turn];
                    // Kept in the part of the view no card covers.
                    let (hx, hy) = m.zones.first().map_or((m.x, m.y), |z| (z.0, z.1));
                    // In the map's own coordinates: it moves and scales with the map; on a row
                    // running left it opens from the heading's right edge.
                    let left = if m.rev {
                        m.zones.first().map_or(hx, |z| z.0 + z.2) - 354.0
                    } else {
                        hx + 14.0
                    };
                    let y = hy + 144.0;
                    let n = m.turn;
                    let asker = if tt.agent.is_some() { "에이전트 보고" } else { "사용자 프롬프트" };
                    rsx! {
                        div {
                            class: "fx-prompt",
                            style: "left: {left.max(8.0)}px; top: {y}px",
                            onmousedown: move |e| e.stop_propagation(),
                            onwheel: move |e| e.stop_propagation(),
                            div { class: "hd",
                                span { class: "no", "턴 {n + 1}" }
                                button { class: "x", title: "닫기", onclick: move |_| head_open.set(None), "✕" }
                            }
                            if let Some(intent) = tt.intent.clone() {
                                div { class: "k", "에이전트가 적은 의도" }
                                div { class: "intent", "{intent}" }
                            }
                            div { class: "k", "{asker}" }
                            div { class: "said", "{tt.req}" }
                        }
                    }
                }
            }
            // An answer opened: the agent's last message whole, and before it what else it said.
            if let Some(st) = say_open().and_then(|n| line.stops.iter().find(|s| s.turn == n && s.kind == Kind::Say)) {
                {
                    let tt = &f.turns[st.turn];
                    let left = st.x - 40.0;
                    let y = st.y + 44.0;
                    let n = st.turn;
                    let last = tt.said.last().map(|m| crate::flow::plain_words(&m.text)).unwrap_or_default();
                    let earlier: Vec<String> = tt.said.iter().rev().skip(1).rev().map(|m| crate::flow::plain_words(&m.text)).collect();
                    let when = hm(tt.said.last().and_then(|m| m.at));
                    rsx! {
                        div {
                            class: "fx-prompt say",
                            style: "left: {left.max(8.0)}px; top: {y}px",
                            onmousedown: move |e| e.stop_propagation(),
                            onwheel: move |e| e.stop_propagation(),
                            div { class: "hd",
                                span { class: "no", "턴 {n + 1} · 답변" }
                                span { class: "tm", "{when}" }
                                button { class: "x", title: "닫기", onclick: move |_| say_open.set(None), "✕" }
                            }
                            div { class: "said", "{last}" }
                            if !earlier.is_empty() {
                                div { class: "k", "그 전에 한 말 {earlier.len()}개" }
                                for (i, e) in earlier.iter().enumerate() {
                                    div { key: "{i}", class: "said faint", "{e}" }
                                }
                            }
                        }
                    }
                }
            }
            // A cause opened: the result read, whole, with the paragraph that told the Call.
            if let Some(b) = cause_open().and_then(|i| line.branch_labels.get(i)) {
                if let Some(c) = b.cause.clone() {
                    {
                        let kind = if c.error { "오류 출력" } else { "도구 출력" };
                        let p = (c.p * 100.0).round();
                        let reading = format!("{} · 원인일 확률 {p}%", b.text);
                        rsx! {
                            div {
                                class: "fx-prompt cause",
                                style: "left: {b.qx}px; top: {quote_y.get(&cause_open().unwrap_or(usize::MAX)).copied().unwrap_or(b.qy) + 58.0}px",
                                onmousedown: move |e| e.stop_propagation(),
                                onwheel: move |e| e.stop_propagation(),
                                div { class: "hd",
                                    span { class: if c.error { "no err" } else { "no" }, "{kind}" }
                                    span { class: "tm", "{reading}" }
                                    button { class: "x", title: "닫기", onclick: move |_| cause_open.set(None), "✕" }
                                }
                                blockquote { class: "said quote",
                                    super::trace::Preview { seq: c.result, paragraph: c.paragraph, lines: true }
                                }
                                if c.p < 0.5 {
                                    div { class: "k", "엔진이 반 넘게 확신하지 못한 원인이에요. 사용자 요청 때문이었을 수도 있어요." }
                                }
                            }
                        }
                    }
                }
            }
            }
            // Showing the way back from the Call picked to what told it.
            button {
                class: if trace_on() { "fx-trace on" } else { "fx-trace" },
                title: "고른 호출이 어떤 출력을 보고 한 일인지 거슬러 올라가는 길을 점선으로 보여 줍니다",
                onmousedown: move |e| e.stop_propagation(),
                onclick: move |_| trace_on.toggle(),
                span { class: "dot" }
                if trace_on() { "원인 경로 켜짐" } else { "원인 경로" }
            }
            // What the colours mean.
            div { class: "fx-tones", onmousedown: move |e| e.stop_propagation(),
                if has_ask { span { class: "it", i { class: "sw", style: "background: {Tone::Ask.color()}" } "사용자 요청" } }
                for g in glyphs.iter().copied() {
                    span { key: "{g:?}", class: "it",
                        svg { width: "16", height: "16", view_box: "0 0 16 16", {glyph(g, 8.0, 8.0, 14.0, g.tone().color())} }
                        "{g.name()}"
                    }
                }
                if has_say { span { class: "it", i { class: "sw", style: "background: {Tone::Say.color()}" } "에이전트 답변" } }
                if has_branch { span { class: "it", i { class: "sw", style: "background: {TEXT_COLOR}" } "글을 따라감" } }
                if has_block { span { class: "it red", "✕ 차단" } }
                if has_warn { span { class: "it amber", "▲ 확인 요청" } }
            }
        }
    }
}

fn pin(x: f64, y: f64, label: &str, color: &str) -> Element {
    let (w, h) = (44.0, 23.0);
    let tip = format!("M{} {}l5 6 5-6z", x - 5.0, y - 6.0);
    rsx! {
        g { class: "fx-nohit",
            rect { x: "{x - w / 2.0}", y: "{y - h - 6.0}", width: "{w}", height: "{h}", rx: "5", fill: "{color}" }
            path { d: "{tip}", fill: "{color}" }
            text { x: "{x}", y: "{y - 6.0 - h / 2.0 + 4.8}", text_anchor: "middle", font_size: "13", font_weight: "700", fill: "#fff", "{label}" }
        }
    }
}

/// A small head at the end of a passage, pointing up into the Call it leads to: from the path's
/// last point.
fn arrow_head(d: &str) -> String {
    let nums: Vec<f64> = d
        .split(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-'))
        .filter_map(|t| t.parse().ok())
        .collect();
    let (Some(&y), Some(&x)) = (nums.last(), nums.get(nums.len().saturating_sub(2))) else {
        return String::new();
    };
    format!("M{} {} L{} {} L{} {}z", x - 5.0, y + 7.0, x + 5.0, y + 7.0, x, y - 1.0)
}

/// A pin left of its point, its tip pointing right at it.
fn pin_left(x: f64, y: f64, label: &str, color: &str) -> Element {
    let (w, h) = (44.0, 23.0);
    let tip = format!("M{} {}l6 5-6 5z", x - 6.0, y - 5.0);
    rsx! {
        g { class: "fx-nohit",
            rect { x: "{x - 6.0 - w}", y: "{y - h / 2.0}", width: "{w}", height: "{h}", rx: "5", fill: "{color}" }
            path { d: "{tip}", fill: "{color}" }
            text { x: "{x - 6.0 - w / 2.0}", y: "{y + 4.8}", text_anchor: "middle", font_size: "13", font_weight: "700", fill: "#fff", "{label}" }
        }
    }
}

/// A pin under its point, its tip up.
fn pin_below(x: f64, y: f64, label: &str, color: &str) -> Element {
    let (w, h) = (44.0, 23.0);
    let tip = format!("M{} {}l5 -6 5 6z", x - 5.0, y + 6.0);
    rsx! {
        g { class: "fx-nohit",
            rect { x: "{x - w / 2.0}", y: "{y + 6.0}", width: "{w}", height: "{h}", rx: "5", fill: "{color}" }
            path { d: "{tip}", fill: "{color}" }
            text { x: "{x}", y: "{y + 6.0 + h / 2.0 + 4.8}", text_anchor: "middle", font_size: "13", font_weight: "700", fill: "#fff", "{label}" }
        }
    }
}

/// A station of the line. On the turn picked every name shows; elsewhere only the stops and
/// interchanges keep theirs, so the line's shape reads first.
#[component]
fn LineStop(
    stop: Stop,
    color: String,
    sel: bool,
    detail: bool,
    open: bool,
    onsel: EventHandler<()>,
) -> Element {
    let s = &stop;
    let important = matches!(s.kind, Kind::Warn | Kind::Blocked | Kind::Transfer);
    // On a turn not picked only the stations that tell its story keep their names, faint:
    // what was stopped, interchanges, files changed, and how it ended.
    let key = important || s.xfer || s.tone == Tone::Change || s.kind == Kind::Say;
    let show = !s.label.is_empty() && (detail || key);
    let faint = if detail { "1" } else { "0.55" };
    let mini = s.kind == Kind::Mini;
    let minor = s.kind == Kind::Tick || mini;
    let up = s.up == Some(true);
    let gap = if s.kind == Kind::Transfer { 32.0 } else if mini { 14.0 } else { 26.0 };
    // A small station's two lines sit closer: its tool, then what it did.
    let step = if mini { 14.0 } else { 16.0 };
    // A name moved a tier out stands that much further from the line.
    let lift = s.tier as f64 * 36.0;
    let ly = if up {
        s.y - gap - step - lift
    } else {
        s.y + gap + 8.0 + lift
    };
    let sy = if up { s.y - gap - lift } else { s.y + gap + 8.0 + step + lift };
    let (x, y) = (s.x, s.y);
    let fill = match s.kind {
        Kind::Mini => color.as_str(),
        Kind::Blocked => RED,
        Kind::Warn => "#a8790c",
        Kind::Say => "#6d28d9",
        _ if minor => "#6b7079",
        _ => "#1f2125",
    };
    let (fs, fw) = if mini {
        ("12", "600")
    } else if minor {
        ("14", "500")
    } else {
        ("15.5", "700")
    };
    // A group says which tools it ran; the sign after them opens it.
    let (sub_text, sign) = if s.group.is_some() {
        (s.sub.clone(), if open { " −" } else { " +" })
    } else {
        (s.sub.clone(), "")
    };
    let sub_fill = "#8a8f98";
    let tip = [s.label.as_str(), s.sub.as_str()]
        .iter()
        .filter(|t| !t.is_empty())
        .copied()
        .collect::<Vec<_>>()
        .join(" · ");
    let x_d = format!("M{} {}l9 9M{} {}l-9 9", x - 4.5, y - 4.5, x + 4.5, y - 4.5);
    rsx! {
        g {
            class: "fx-hit",
            onclick: move |e| { e.stop_propagation(); onsel.call(()); },
            title { "{tip}" }
            circle { cx: "{x}", cy: "{y}", r: "20", fill: "transparent" }
            // A stopped Call whose result also told another: the interchange's ring around it.
            if s.xfer && s.kind != Kind::Transfer {
                circle { cx: "{x}", cy: "{y}", r: "16", fill: "none", stroke: TEXT_COLOR, stroke_width: "2.5" }
            }
            if sel {
                circle { cx: "{x}", cy: "{y}", r: "19", fill: "{color}", opacity: "0.14" }
                // A ring that spreads once from the station just picked, to say where it is.
                circle { class: "fx-pulse", cx: "{x}", cy: "{y}", r: "16", fill: "none", stroke: "{color}", stroke_width: "3" }
            }
            match s.kind {
                Kind::Start => rsx! { circle { cx: "{x}", cy: "{y}", r: "10", fill: "#fff", stroke: "{color}", stroke_width: "4" } },
                Kind::Tick if open => rsx! { circle { cx: "{x}", cy: "{y}", r: "8", fill: "#fff", stroke: "{color}", stroke_width: "3.5" } },
                Kind::Tick => rsx! { circle { cx: "{x}", cy: "{y}", r: "3", fill: "#fff" } },
                Kind::Stop => rsx! {
                    g {
                        circle { cx: "{x}", cy: "{y}", r: "12", fill: "#fff", stroke: "{color}", stroke_width: "3" }
                        if let Some(g) = s.glyph { {glyph(g, x, y, 13.0, &color)} }
                    }
                },
                Kind::Transfer => rsx! {
                    g {
                        rect { x: "{x - 9.0}", y: "{y - 15.0}", width: "18", height: "30", rx: "9", fill: "#fff", stroke: "#1f2125", stroke_width: "3" }
                        circle { cx: "{x}", cy: "{y - 5.5}", r: "4", fill: "{color}" }
                        circle { cx: "{x}", cy: "{y + 5.5}", r: "4", fill: TEXT_COLOR }
                    }
                },
                Kind::Warn => rsx! {
                    g { transform: "translate({x - 12.0} {y - 12.0})",
                        path { d: "M12 1L24 22H0z", fill: AMBER, stroke: "#fff", stroke_width: "2", stroke_linejoin: "round" }
                        rect { x: "10.9", y: "7.5", width: "2.2", height: "7.5", rx: "1", fill: "#fff" }
                        circle { cx: "12", cy: "18", r: "1.3", fill: "#fff" }
                    }
                },
                Kind::Blocked => rsx! {
                    g {
                        circle { cx: "{x}", cy: "{y}", r: "11", fill: RED, stroke: "#fff", stroke_width: "2" }
                        path { d: "{x_d}", stroke: "#fff", stroke_width: "2.4", stroke_linecap: "round" }
                    }
                },
                // A Call let through in an opened group: a small ring in its tool's colour.
                // A small station: its tool's icon in a ring of its colour.
                Kind::Mini => rsx! {
                    g {
                        circle { cx: "{x}", cy: "{y}", r: "9", fill: "#fff", stroke: "{color}", stroke_width: "2.2" }
                        if let Some(g) = s.glyph { {glyph(g, x, y, 10.0, &color)} }
                    }
                },
                // A speech bubble in the answer's violet, three dots in it.
                Kind::Say => rsx! {
                    g {
                        rect { x: "{x - 12.0}", y: "{y - 10.0}", width: "24", height: "18", rx: "6", fill: "{color}", stroke: "#fff", stroke_width: "2" }
                        path { d: "M{x - 4.0} {y + 7.0}l-3 6 8-5z", fill: "{color}" }
                        circle { cx: "{x - 5.5}", cy: "{y - 1.0}", r: "1.6", fill: "#fff" }
                        circle { cx: "{x}", cy: "{y - 1.0}", r: "1.6", fill: "#fff" }
                        circle { cx: "{x + 5.5}", cy: "{y - 1.0}", r: "1.6", fill: "#fff" }
                    }
                },
            }
            if show {
                text { x: "{x}", y: "{ly}", text_anchor: "middle", font_size: "{fs}", font_weight: "{fw}", fill: "{fill}", opacity: "{faint}", "{s.label}" }
                // The line under a name only where it says what the name cannot: a verdict, an
                // interchange, a group's tools. The rest is in the station's tooltip.
                if detail && !s.sub.is_empty() && (important || s.xfer || s.group.is_some()) {
                    text { x: "{x}", y: "{sy}", text_anchor: "middle", font_size: if mini { "10.5" } else { "11.5" }, fill: "{sub_fill}",
                        "{sub_text}"
                        if !sign.is_empty() { tspan { fill: "#3b82f6", font_weight: "700", "{sign}" } }
                    }
                }
            }
        }
    }
}
