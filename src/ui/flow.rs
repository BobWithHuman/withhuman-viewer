//! The route map after the Figma Make design (see `crate::flow`): two levels, the session's
//! network of tasks and one task's line, with the list beside it sliding between the two.

use std::collections::HashSet;
use std::sync::Arc;

use dioxus::prelude::*;

use super::{AppState, Load, Selection};
use crate::flow::{
    AMBER, Flow, GREEN, Item, Kind, LINE_COLOR, LINE_MUTED, Overview, RED, Stop, TEXT_COLOR,
    TEXT_MUTED, Turn,
};
use crate::layout::clip;
use crate::metro::Metro;

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
    let flow = use_memo(move || metro().map(|m| Arc::new(Flow::build(&m))));
    let mut level = use_signal(|| Level::Overview);
    let mut turn = use_signal(|| 0usize);
    let hover = use_signal(|| None::<usize>);
    let mut tab = use_signal(|| Tab::All);
    let mut zoom = use_signal(|| 1.0_f64);
    let mut detail = use_signal(|| true);
    let hovered = use_signal(|| None::<i64>);

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
        return rsx! { div { class: "routes fx" } };
    };
    let t = turn().min(f.turns.len() - 1);
    let task = f.task_of(t);
    let scope: Vec<usize> = f.task_turns(task).collect();
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
                zoom.set((zoom() + 0.15).min(2.0))
            }
        }
    };
    let zoom_out = move |_| {
        if level() == Level::Detail && zoom() <= 0.71 {
            level.set(Level::Overview)
        } else if level() == Level::Detail {
            zoom.set((zoom() - 0.15).max(0.7))
        }
    };
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
                                        div { class: "fx-where-sub", "턴 {a}–{b} · {scope.len()}턴" }
                                    }
                                }
                            }
                            Sidebar { flow: f.clone(), metro: m.clone(), scope: scope.clone(), visible: visible.clone(), turn, tab, detail }
                        }
                    }
                }
            }
            if detail() && picked.is_some() {
                super::metro::RoutePanel { metro: m.clone(), dest: picked, hovered, detail }
            }
            main { class: "fx-main",
                div { key: "{is_detail}", class: "{anim}",
                    if !is_detail {
                        OverviewMap { flow: f.clone(), turn, hover, level, zoom }
                    } else if !visible.is_empty() {
                        LineMap { flow: f.clone(), turns: visible.clone(), turn, zoom: zoom(), detail }
                    } else {
                        div { class: "fx-empty", "이 조건에 맞는 턴이 없습니다" }
                    }
                }
                div { class: "fx-zoom",
                    button { title: "확대", onclick: zoom_in, "+" }
                    button { title: "축소", disabled: !is_detail, onclick: zoom_out, "−" }
                    button {
                        class: if !is_detail { "fit on" } else { "fit" },
                        title: "전체 노선망",
                        onclick: move |_| level.set(Level::Overview),
                        "전체"
                    }
                }
                div { class: "fx-brand", "WITHHUMAN" }
            }
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
    let name = session_name(&state);
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
                div { class: "fx-session", "{name}" }
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

fn session_name(state: &AppState) -> String {
    let current = state.current.read().clone().unwrap_or_default();
    match &*state.sessions.read() {
        Load::Ready(list) => list
            .iter()
            .find(|s| s.id == current)
            .map(|s| {
                let folder = s.folder();
                if folder.is_empty() {
                    clip(s.title(), 28)
                } else {
                    folder
                }
            })
            .unwrap_or_else(|| "세션".into()),
        _ => "세션".into(),
    }
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
    let sessions: Vec<(String, String)> = match &*state.sessions.read() {
        Load::Ready(list) => list
            .iter()
            .map(|s| {
                let folder = s.folder();
                let label = if folder.is_empty() {
                    clip(s.title(), 34)
                } else {
                    format!("{folder} · {}", clip(s.title(), 28))
                };
                (s.id.clone(), label)
            })
            .collect(),
        _ => Vec::new(),
    };
    let current = state.current.read().clone().unwrap_or_default();
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
                        select {
                            class: "fx-od-pick",
                            title: "세션 바꾸기",
                            onchange: move |e| state.open_session(e.value()),
                            for (id, label) in sessions {
                                option { value: "{id}", selected: id == current, "{label} · {f.turns.len()}턴" }
                            }
                        }
                        span { class: "fx-od-swap", "⇅" }
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
    let mut state = use_context::<AppState>();
    let mut groups = use_signal(HashSet::<usize>::new);
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
                    for (gi, item) in t.items.iter().enumerate() {
                        match item {
                            Item::Group { label, tools, .. } => {
                                let shown = groups().contains(&gi);
                                rsx! {
                                    div { key: "{gi}",
                                        div { class: "fx-item group",
                                            {check_icon()}
                                            "{label}"
                                            button {
                                                class: "fx-more",
                                                onclick: move |_| { let mut g = groups.write(); if !g.remove(&gi) { g.insert(gi); } },
                                                if shown { "접기" } else { "펼치기" }
                                            }
                                        }
                                        if shown { div { class: "fx-item-sub", "{Item::sub(tools)}" } }
                                    }
                                }
                            }
                            Item::Call(ci) => {
                                let c = &t.calls[*ci];
                                let seq = c.seq;
                                let cls = format!(
                                    "fx-item call{}{}",
                                    if c.blocked { " red" } else { "" },
                                    if picked == Some(seq) { " picked" } else { "" }
                                );
                                let icon = if c.blocked { blocked_icon(13.0) } else if c.warn { warn_icon(13.0) } else { check_icon() };
                                rsx! {
                                    div {
                                        key: "{gi}",
                                        class: "{cls}",
                                        title: "{c.arg}",
                                        onclick: move |_| state.selected.set(Selection::Node(seq)),
                                        {icon}
                                        span { class: "ttl", "{c.title}" }
                                        if c.via.is_some() { span { class: "fx-text-chip", "글" } }
                                        span { class: "tm", "{hm(c.at)}" }
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
                        let sw = if task == sel_task { "11" } else { "9" };
                        let op = if dim(task) { "0.15" } else { "1" };
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
                        let op = if dim(s.task) { "0.15" } else { "1" };
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
                                {overview_label(x, y, s.level, &p.name, label_fill(p), s.task == sel_task || p.blocked || p.warn)}
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
                                {overview_label(x.x, x.y, false, &p.name, label_fill(p), true)}
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
                        let from_t = hm(turns.first().and_then(|t| t.at()));
                        let to_t = hm(turns.last().and_then(|t| t.at()));
                        let first = f.tasks[r.start].from;
                        let fl = flow.clone();
                        let n_tasks = r.len();
                        rsx! {
                            button {
                                key: "c{c}",
                                class: if c == ch { "fx-chapter on" } else { "fx-chapter" },
                                onclick: move |_| select_turn(turn, state, &fl, first),
                                span { class: "top", "{c + 1}구간" span { class: "tm", "{from_t}–{to_t}" } }
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
            div { class: "fx-title",
                div { class: "big", "{ch + 1}구간 노선망" }
                p { "노선은 작업, 역은 파일·명령입니다. 두 노선이 만나는 역은 두 작업이 함께 건드린 파일입니다." }
            }
            div { class: "fx-legend",
                for task in rest_tasks {
                    {
                        let from = f.tasks[task].from;
                        let fl = flow.clone();
                        let cls = format!(
                            "fx-leg{}{}",
                            if task == sel_task { " on" } else { "" },
                            if dim(task) { " dim" } else { "" }
                        );
                        let col = color_of(task);
                        let n = num(task);
                        rsx! {
                            button {
                                key: "lg{task}",
                                class: "{cls}",
                                onmouseenter: move |_| hover.set(Some(task)),
                                onmouseleave: move |_| hover.set(None),
                                onclick: move |_| select_turn(turn, state, &fl, from),
                                span { class: "no", style: "background: {col}", "{n}" }
                                span { class: "nm", "{f.tasks[task].name}" }
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
    zoom: f64,
    detail: Signal<bool>,
) -> Element {
    let mut state = use_context::<AppState>();
    let mut detail = detail;
    let f = &flow;
    let line = f.line(&turns);
    let t = turn().min(f.turns.len() - 1);
    let mut open_id = use_signal(|| None::<String>);
    let mut size = use_signal(|| (0.0_f64, 0.0_f64));
    let mut scroll = use_signal(|| 0.0_f64);
    let mut drag = use_signal(|| None::<(f64, f64, bool)>);
    let mut dragged = use_signal(|| false);
    let picked = match (state.selected)() {
        Selection::Node(seq) => Some(seq),
        Selection::None => None,
    };
    let (w, h) = size();
    let (lw, lh) = (line.width * zoom, line.height * zoom);
    let max_scroll = (lw - w).max(0.0);
    let clamp = move |v: f64| v.clamp(0.0, max_scroll);

    // A turn picked is brought to the left fifth of the view.
    let mark_x = line.marks.iter().find(|m| m.turn == t).map(|m| m.x);
    let framed = use_signal(|| None::<(usize, u64, Option<i64>)>);
    let zkey = (zoom * 1000.0) as u64;
    // A Call picked further on than the view reaches is brought into it.
    let target_x = picked
        .and_then(|seq| {
            line.stops
                .iter()
                .find(|s| s.seq == Some(seq) && s.turn == t)
        })
        .map(|s| s.x);
    if let Some(mx) = mark_x
        && w > 0.0
        && *framed.peek() != Some((t, zkey, picked))
    {
        let mut framed = framed;
        framed.set(Some((t, zkey, picked)));
        let mut to = mx * zoom - w * 0.2;
        if let Some(tx) = target_x
            && tx * zoom - to > w - 160.0
        {
            to = tx * zoom - w * 0.6;
        }
        scroll.set(to.clamp(0.0, max_scroll));
    }

    let open_stop = open_id().and_then(|id| {
        line.stops
            .iter()
            .find(|s| s.id == id && s.turn == t)
            .cloned()
    });
    let target = picked.and_then(|seq| {
        line.stops
            .iter()
            .find(|s| s.seq == Some(seq) && s.turn == t && s.kind != Kind::Transfer)
            .or_else(|| {
                line.stops
                    .iter()
                    .find(|s| s.seq == Some(seq) && s.turn == t)
            })
            .cloned()
    });
    let col = move |n: usize| if n == t { LINE_COLOR } else { LINE_MUTED };
    let mut segs = line.segs.clone();
    segs.sort_by_key(|s| s.turn == t);
    let top = ((h - lh) / 2.0).max(0.0);
    let x0 = scroll().min(max_scroll);
    let moving = if drag().is_some_and(|d| d.2) {
        "fx-line dragging"
    } else {
        "fx-line"
    };

    rsx! {
        div {
            class: "{moving}",
            onresize: move |e| {
                if let Ok(b) = e.get_border_box_size() {
                    size.set((b.width, b.height));
                }
            },
            onwheel: move |e| {
                e.prevent_default();
                let d = e.delta().strip_units();
                let cur = *scroll.peek();
                scroll.set(clamp(cur + d.x + d.y));
            },
            onmousedown: move |e| {
                let p = e.client_coordinates();
                drag.set(Some((p.x, *scroll.peek(), false)));
                dragged.set(false);
            },
            onmousemove: move |e| {
                let Some((x, s0, moved)) = *drag.peek() else { return };
                let dx = e.client_coordinates().x - x;
                if moved || dx.abs() > super::metro::CLICK_SLOP {
                    drag.set(Some((x, s0, true)));
                    dragged.set(true);
                    scroll.set(clamp(s0 - dx));
                }
            },
            onmouseup: move |_| drag.set(None),
            onmouseleave: move |_| drag.set(None),
            svg {
                class: "fx-line-svg",
                width: "{lw}",
                height: "{lh}",
                view_box: "0 0 {line.width} {line.height}",
                style: "transform: translate({-x0}px, {top}px)",
                // The turn picked: a pale band under its stretch.
                for (i, s) in line.segs.iter().enumerate().filter(|(_, s)| s.turn == t) {
                    path {
                        key: "band{i}",
                        d: "{s.d}",
                        fill: "none",
                        stroke: "#eceef2",
                        stroke_width: if s.branch { "44" } else { "64" },
                        stroke_linecap: "round",
                        stroke_linejoin: "round",
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
                                onclick: move |_| if !dragged() { select_turn(turn, state, &fl, n) },
                            }
                        }
                    }
                }
                for (i, s) in segs.iter().enumerate() {
                    {
                        let c = if s.branch {
                            if s.turn == t { TEXT_COLOR } else { TEXT_MUTED }
                        } else {
                            col(s.turn)
                        };
                        rsx! {
                            g { key: "seg{i}", class: "fx-nohit",
                                path { d: "{s.d}", fill: "none", stroke: "{c}", stroke_width: "6", stroke_linecap: "round", stroke_linejoin: "round" }
                                if let Some((ex, ey)) = s.end {
                                    line { x1: "{ex}", x2: "{ex}", y1: "{ey - 12.0}", y2: "{ey + 12.0}", stroke: "{c}", stroke_width: "4" }
                                }
                            }
                        }
                    }
                }
                for s in line.stops.iter().cloned() {
                    {
                        let color = if s.branch {
                            if s.turn == t { TEXT_COLOR } else { "#c9c2dd" }
                        } else {
                            col(s.turn)
                        };
                        let sel = target.as_ref().is_some_and(|tg| tg.id == s.id);
                        let open = open_stop.as_ref().is_some_and(|o| o.id == s.id);
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
                                    if dragged() { return; }
                                    select_turn(turn, state, &fl, st.turn);
                                    if st.group.is_some() {
                                        open_id.set(if open { None } else { Some(st.id.clone()) });
                                    } else if let Some(seq) = st.seq {
                                        state.selected.set(Selection::Node(seq));
                                        detail.set(true);
                                        open_id.set(None);
                                    }
                                },
                            }
                        }
                    }
                }
                for m in line.marks.iter() {
                    {
                        let chars = ((m.avail - 70.0) / 14.0).floor().max(0.0) as usize;
                        let text = if chars < 3 {
                            String::new()
                        } else if m.text.chars().count() > chars {
                            format!("{}…", m.text.chars().take(chars - 1).collect::<String>())
                        } else {
                            m.text.clone()
                        };
                        let on = m.turn == t;
                        let n = m.turn;
                        let fl = flow.clone();
                        let (bg, edge, ink, ink2, fw) = if on {
                            (LINE_COLOR, LINE_COLOR, "#fff", "#1f2125", "600")
                        } else {
                            ("#fff", "#c9ccd2", "#6b7079", "#9a9ea6", "400")
                        };
                        rsx! {
                            g {
                                key: "mark{n}",
                                class: "fx-hit",
                                transform: "translate({m.x - 12.0} {m.y - 92.0})",
                                onclick: move |_| if !dragged() { select_turn(turn, state, &fl, n) },
                                rect { width: "46", height: "24", rx: "5", fill: "{bg}", stroke: "{edge}" }
                                text { x: "23", y: "16.5", text_anchor: "middle", font_size: "13", font_weight: "700", fill: "{ink}", "턴 {n + 1}" }
                                if !text.is_empty() {
                                    text { x: "54", y: "17", font_size: "14", font_weight: "{fw}", fill: "{ink2}", "{text}" }
                                }
                            }
                        }
                    }
                }
                for b in line.branch_labels.iter() {
                    {
                        let on = b.turn == t;
                        let (op, fill) = if on { ("1", TEXT_COLOR) } else { ("0.55", "#c9c2dd") };
                        rsx! {
                            g { key: "bl{b.turn}-{b.x}", transform: "translate({b.x} {b.y - 12.0})", opacity: "{op}",
                                rect { width: "26", height: "24", rx: "5", fill: "{fill}" }
                                text { x: "13", y: "16.5", text_anchor: "middle", font_size: "13", font_weight: "700", fill: "#fff", "글" }
                                text { x: "33", y: "17", font_size: "13.5", fill: TEXT_COLOR, "{b.text}" }
                            }
                        }
                    }
                }
                if let Some(os) = open_stop.as_ref() {
                    {group_card(f, os, line.width, open_id)}
                }
                if let Some(first) = line.stops.first() {
                    {pin(first.x, first.y - 14.0, "출발", GREEN)}
                }
                if let Some(tg) = target.as_ref().filter(|s| s.kind != Kind::Start) {
                    {pin(tg.x, tg.y - 16.0, "도착", if tg.kind == Kind::Blocked { RED } else { "#1f2125" })}
                }
            }
        }
    }
}

fn pin(x: f64, y: f64, label: &str, color: &str) -> Element {
    let (w, h) = (62.0, 36.0);
    let tip = format!("M{} {}l7 8 7-8z", x - 7.0, y - 8.0);
    rsx! {
        g { class: "fx-nohit",
            rect { x: "{x - w / 2.0}", y: "{y - h - 8.0}", width: "{w}", height: "{h}", rx: "7", fill: "{color}" }
            path { d: "{tip}", fill: "{color}" }
            text { x: "{x}", y: "{y - 8.0 - h / 2.0 + 7.0}", text_anchor: "middle", font_size: "19", font_weight: "700", fill: "#fff", "{label}" }
        }
    }
}

/// An opened group: the tools it ran, each with a bar of how often.
fn group_card(f: &Flow, s: &Stop, width: f64, mut open_id: Signal<Option<String>>) -> Element {
    let Some(Item::Group { tools, .. }) = s.group.and_then(|g| f.turns[s.turn].items.get(g)) else {
        return rsx! {};
    };
    let mut rows: Vec<(String, usize)> = tools.iter().take(4).cloned().collect();
    let rest: usize = tools.iter().skip(4).map(|t| t.1).sum();
    if rest > 0 {
        rows.push(("기타".into(), rest));
    }
    let max = rows.iter().map(|r| r.1).max().unwrap_or(1) as f64;
    let (w, h) = (240.0, 46.0 + rows.len() as f64 * 26.0);
    let below = s.up == Some(true);
    let x = (s.x - w / 2.0).max(20.0).min(width - w - 20.0);
    let y = if below { s.y + 22.0 } else { s.y - 22.0 - h };
    let tip = if below {
        format!("M{} {}l8 -10 8 10z", s.x - 8.0, y)
    } else {
        format!("M{} {}l8 10 8 -10z", s.x - 8.0, y + h)
    };
    rsx! {
        g { class: "fx-hit", onclick: move |_| open_id.set(None),
            path { d: "{tip}", fill: "#fff", stroke: "#d4d4d8" }
            rect { x: "{x}", y: "{y}", width: "{w}", height: "{h}", rx: "10", fill: "#fff", stroke: "#d4d4d8" }
            text { x: "{x + 14.0}", y: "{y + 24.0}", font_size: "13", font_weight: "700", fill: "#1f2125", "{s.label}" }
            text { x: "{x + w - 14.0}", y: "{y + 24.0}", text_anchor: "end", font_size: "11", fill: "#9a9ea6", "접기 −" }
            for (i, (name, n)) in rows.iter().enumerate() {
                g { key: "{name}", transform: "translate({x + 14.0} {y + 44.0 + i as f64 * 26.0})",
                    text { y: "4", font_size: "12.5", fill: "#3f434b", "{name}" }
                    rect { x: "70", y: "-6", width: "120", height: "10", rx: "3", fill: "#f1f2f4" }
                    rect { x: "70", y: "-6", width: "{120.0 * *n as f64 / max}", height: "10", rx: "3", fill: LINE_COLOR, opacity: "0.75" }
                    text { x: "{w - 28.0}", y: "4", text_anchor: "end", font_size: "12", font_weight: "600", fill: "#3f434b", "{n}" }
                }
            }
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
    let show = !s.label.is_empty() && (detail || important);
    let minor = s.kind == Kind::Tick;
    let up = s.up == Some(true);
    let gap = if s.kind == Kind::Transfer { 32.0 } else { 26.0 };
    let ly = if up {
        s.y - gap - 16.0
    } else {
        s.y + gap + 8.0
    };
    let sy = if up { s.y - gap } else { s.y + gap + 24.0 };
    let (x, y) = (s.x, s.y);
    let fill = match s.kind {
        Kind::Blocked => RED,
        Kind::Warn => "#a8790c",
        _ if minor => "#6b7079",
        _ => "#1f2125",
    };
    let (fs, fw) = if minor {
        ("14", "500")
    } else {
        ("15.5", "700")
    };
    let sub_text = if s.group.is_some() {
        if open {
            "접기 −".to_owned()
        } else {
            "펼치기 +".to_owned()
        }
    } else {
        s.sub.clone()
    };
    let sub_fill = if s.group.is_some() {
        "#3b82f6"
    } else {
        "#9a9ea6"
    };
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
            if sel { circle { cx: "{x}", cy: "{y}", r: "19", fill: "{color}", opacity: "0.14" } }
            match s.kind {
                Kind::Start => rsx! { circle { cx: "{x}", cy: "{y}", r: "10", fill: "#fff", stroke: "{color}", stroke_width: "4" } },
                Kind::Tick if open => rsx! { circle { cx: "{x}", cy: "{y}", r: "8", fill: "#fff", stroke: "{color}", stroke_width: "3.5" } },
                Kind::Tick => rsx! { circle { cx: "{x}", cy: "{y}", r: "3", fill: "#fff" } },
                Kind::Stop => rsx! { circle { cx: "{x}", cy: "{y}", r: "8", fill: "#fff", stroke: "{color}", stroke_width: "3.5" } },
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
            }
            if show {
                text { x: "{x}", y: "{ly}", text_anchor: "middle", font_size: "{fs}", font_weight: "{fw}", fill: "{fill}", "{s.label}" }
                if detail && !s.sub.is_empty() {
                    text { x: "{x}", y: "{sy}", text_anchor: "middle", font_size: "11.5", fill: "{sub_fill}", "{sub_text}" }
                }
            }
        }
    }
}
