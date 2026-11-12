//! The route map after the Figma Make design (see `crate::flow`): the session's line, turn by
//! turn, a legend over its top left to count, grey out and light kinds of station, and the
//! turns listed beside it.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use dioxus::prelude::*;

use super::{AppState, Load, Selection};
use crate::flow::{
    AMBER, Flow, GREEN, Glyph, Item, Kind, LINE_COLOR, LINE_MUTED, Leg, RED, Stop, Summary,
    TEXT_COLOR, TEXT_MUTED, Tone, Turn,
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
/// The way back from a Call to what told it, while it is shown.
const TRACE_COLOR: &str = "#7c3aed";
/// The rest of the line meanwhile: grey, out of the way.
const DIM_COLOR: &str = "#d4d6db";

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
        let mut f = Flow::build(&m);
        // Values a Call used that first came out of an earlier turn's tool result.
        if let Load::Ready(model) = &*state.graph.peek() {
            f.trace_values(model);
        }
        Some(Arc::new(f))
    });
    let mut turn = use_signal(|| 0usize);
    let mut tab = use_signal(|| Tab::All);
    let mut detail = use_signal(|| true);
    // The legend over the map's top left, opened and closed from the tools.
    let mut legend = use_signal(|| true);
    let hovered = use_signal(|| None::<i64>);
    // The session list, opened from the session's name in either level.
    let mut picking = use_context::<Picking>().0;
    // The turn whose prompt ← and → stopped at: its heading marked, nothing opened over the map.
    let mut head_open = use_context_provider(|| PromptPick(Signal::new(None))).0;
    let opened = use_signal(|| None::<Option<String>>);
    use_effect(move || {
        // Another session opened from the list closes it.
        let now = (state.current)();
        let mut opened = opened;
        if opened
            .peek()
            .as_ref()
            .is_some_and(|was| was.is_some() && *was != now)
        {
            picking.set(false);
            head_open.set(None);
        }
        opened.set(Some(now));
    });
    use_effect(move || {
        if picking() {
            document::eval(
                "requestAnimationFrame(() => document.getElementById('session-search')?.focus())",
            );
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

    // A new session opens on its first stop.
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
    // A Call picked anywhere picks its turn, and the prompt stepped to is let go.
    use_effect(move || {
        let Selection::Node(seq) = (state.selected)() else {
            return;
        };
        if head_open.peek().is_some() {
            head_open.set(None);
        }
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
    if !visible.is_empty() && !visible.contains(&t) {
        let first = visible[0];
        let f2 = f.clone();
        spawn(async move { select_turn(turn, state, &f2, first) });
    }
    let picked = match (state.selected)() {
        Selection::Node(seq) => f.call(seq).map(|_| seq),
        Selection::None => None,
    };
    let zoom_in = move |_| {
        document::eval(&format!("{PAN_JS}\nfxMap.zoomBy(1.25)"));
    };
    let zoom_out = move |_| {
        document::eval(&format!("{PAN_JS}\nfxMap.zoomBy(0.8)"));
    };
    let card = SHOW_ROUTE_CARD && detail() && picked.is_some();
    // Each turn's prompt and then its Calls, in the order made, to step through with ← and →.
    let order: Vec<Step> = f
        .turns
        .iter()
        .enumerate()
        .flat_map(|(n, t)| {
            std::iter::once(Step::Prompt(n)).chain(t.calls.iter().map(|c| Step::Call(c.seq)))
        })
        .collect();
    let now_at =
        move |order: &[Step], picked: Option<i64>, head: Option<usize>| match (picked, head) {
            (Some(p), _) => order.iter().position(|&s| s == Step::Call(p)),
            (None, Some(n)) => order.iter().position(|&s| s == Step::Prompt(n)),
            (None, None) => None,
        };
    let at = now_at(&order, picked, head_open());
    let total = order.len();
    let step = {
        let order = order.clone();
        let f = f.clone();
        move |by: isize| {
            if order.is_empty() {
                return;
            }
            // From what is picked now, not when this was drawn, so keys pressed fast still step.
            let picked = match *state.selected.peek() {
                Selection::Node(q) => Some(q),
                Selection::None => None,
            };
            let next = match now_at(&order, picked, *head_open.peek()) {
                Some(i) => i.saturating_add_signed(by).min(order.len() - 1),
                None if by < 0 => order.len() - 1,
                None => 0,
            };
            let mut state = state;
            match order[next] {
                Step::Prompt(n) => {
                    state.selected.set(Selection::None);
                    select_turn(turn, state, &f, n);
                    head_open.set(Some(n));
                }
                Step::Call(seq) => {
                    head_open.set(None);
                    state.selected.set(Selection::Node(seq));
                }
            }
        }
    };
    let (mut step_prev, mut step_next) = (step.clone(), step);
    let task_name = f.tasks[task].name.clone();
    let (a, b) = (f.tasks[task].from + 1, f.tasks[task].to + 1);

    rsx! {
        div { class: "routes fx",
            // The controls of the map, down its left as a design app keeps its tools: the
            // legend, zoom, fit, stepping through the Calls, reading live.
            nav { class: "fx-tools",
                button {
                    class: if legend() { "on" } else { "" },
                    title: if legend() { "범례 닫기" } else { "범례 열기 · 종류별로 보고 가리기" },
                    onclick: move |_| legend.toggle(),
                    {tool_icon("M4 5h16v14H4zM9 5v14M5.8 8.5h1.4M5.8 11.5h1.4M5.8 14.5h1.4")}
                }
                i { class: "sep" }
                button { title: "확대", onclick: zoom_in, {tool_icon("M10.5 4a6.5 6.5 0 1 0 0 13 6.5 6.5 0 0 0 0-13zM15.3 15.3L20 20M7.5 10.5h6M10.5 7.5v6")} }
                button { title: "축소", onclick: zoom_out, {tool_icon("M10.5 4a6.5 6.5 0 1 0 0 13 6.5 6.5 0 0 0 0-13zM15.3 15.3L20 20M7.5 10.5h6")} }
                button {
                    title: "노선 전체를 한 화면에",
                    onclick: move |_| { document::eval(&format!("{PAN_JS}\nfxMap.fit()")); },
                    {tool_icon("M4 9V4h5M15 4h5v5M20 15v5h-5M9 20H4v-5")}
                }
                i { class: "sep" }
                // Stepping through the Calls in the order made: buttons, and ← → anywhere but in
                // a text field (the page clicks these for the keys).
                button { id: "fx-prev", title: "이전 (←)", disabled: total == 0 || at == Some(0), onclick: move |_| step_prev(-1), {tool_icon("M14.5 6l-6 6 6 6")} }
                span { class: "n", title: "지금 몇 번째인지",
                    if let Some(i) = at { "{i + 1}" } else { "–" }
                    span { class: "of", "{total}" }
                }
                button { id: "fx-next", title: "다음 (→)", disabled: total == 0 || at == Some(total.saturating_sub(1)), onclick: move |_| step_next(1), {tool_icon("M9.5 6l6 6-6 6")} }
                div { class: "fill" }
                button {
                    class: if (state.live)() { "on live" } else { "" },
                    title: if (state.live)() { "자동 갱신 켜짐 · 3초마다 다시 읽기" } else { "자동 갱신 꺼짐" },
                    onclick: move |_| state.live.toggle(),
                    {tool_icon("M19 12a7 7 0 1 1-2.05-4.95M19 4.5V9h-4.5")}
                }
            }
            main { class: "fx-main",
                div { class: "fx-stage",
                    if !visible.is_empty() {
                        LineMap { flow: f.clone(), turns: visible.clone(), turn, detail, legend, covered: if card { CARD_W } else { 0.0 } }
                    } else {
                        div { class: "fx-empty", "이 조건에 맞는 턴이 없습니다" }
                    }
                }
                div { class: "fx-brand", "WITHHUMAN" }
                // Who told the agent to make the Call picked: a card over the map's right side.
                if card {
                    super::metro::RoutePanel { metro: m.clone(), dest: picked, hovered, detail }
                }
            }
            // What is looked at, in detail: the turns of the line.
            aside { class: "fx-aside",
                div { class: "fx-slide-wrap",
                    div { class: "fx-slide solo",
                        div { class: "fx-pane",
                            div { class: "fx-where",
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
            {pick_list}
        }
    }
}

/// A control's outline icon.
fn tool_icon(d: &'static str) -> Element {
    rsx! {
        svg { width: "18", height: "18", view_box: "0 0 24 24",
            path { d, fill: "none", stroke: "currentColor", stroke_width: "1.7", stroke_linecap: "round", stroke_linejoin: "round" }
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
pub struct Picking(pub Signal<bool>);

/// The turn whose prompt is the step ← and → are at, if any.
#[derive(Clone, Copy)]
struct PromptPick(Signal<Option<usize>>);

/// A stop on the way ← and → step along: a turn's prompt, then each of its Calls.
#[derive(Clone, Copy, PartialEq)]
enum Step {
    Prompt(usize),
    Call(i64),
}

/// The session's name, opening the session list to switch to another.
#[component]
pub fn SessionPick(class: &'static str) -> Element {
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
    let t = format!(
        "translate({} {}) scale({k})",
        x - size / 2.0,
        y - size / 2.0
    );
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
    a.title == b.title
        && a.blocked == b.blocked
        && a.warn == b.warn
        && a.via.is_some() == b.via.is_some()
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
fn call_row(
    c: &crate::flow::Call,
    key: String,
    picked: Option<i64>,
    mut state: AppState,
    inner: bool,
    times: usize,
) -> Element {
    let seq = c.seq;
    let cls = format!(
        "fx-item call{}{}{}",
        if c.blocked { " red" } else { "" },
        if picked == Some(seq) { " picked" } else { "" },
        if inner { " inner" } else { "" }
    );
    let icon = if c.blocked {
        blocked_icon(13.0)
    } else if c.warn {
        warn_icon(13.0)
    } else {
        check_icon()
    };
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
            // A value it used came out of an earlier turn's tool result.
            if let Some(o) = c.origin.as_ref() {
                span { class: "fx-text-chip cross", title: "‘{o.value}’: 턴 {o.turn + 1}의 도구 결과에서 처음 나온 값", "턴{o.turn + 1}" }
            }
            span { class: "tm", "{hm(c.at)}" }
        }
    }
}

// ── The legend ────────────────────────────────────────────────────────────────────────────

/// How wide the legend over the map is, with its margin: kept in step with `.fx-legend-panel`.
const LEGEND_W: f64 = 300.0;

/// What a Call's verdict is, as the legend counts it.
fn verdict(c: &crate::flow::Call) -> &'static str {
    if c.blocked {
        "blocked"
    } else if c.warn {
        "warn"
    } else if c.allowed {
        "allowed"
    } else {
        "none"
    }
}

/// Whether the legend hides a Call: its verdict or its kind of tool.
fn hides(hidden: &HashSet<&'static str>, c: &crate::flow::Call) -> bool {
    hidden.contains(verdict(c)) || hidden.contains(c.glyph.name())
}

/// The marks the legend can light.
const LIGHTS: [(&str, &str); 5] = [
    ("told", "도구 결과가 시킨 호출"),
    ("carried", "이전 턴의 값을 쓴 호출"),
    ("loose", "시킨 것을 못 찾은 호출"),
    ("outside", "밖과 주고받은 호출"),
    ("changed", "파일을 바꾼 호출"),
];

fn lights(key: &str, c: &crate::flow::Call) -> bool {
    match key {
        "told" => c.by.is_some() || c.cause.is_some(),
        "carried" => c.origin.is_some(),
        "loose" => c.loose,
        "outside" => matches!(c.glyph, Glyph::Web | Glyph::Plug),
        "changed" => c.glyph == Glyph::Edit,
        _ => false,
    }
}

/// The Calls a station stands for: its own and those folded into it, or its group's.
fn stop_calls<'a>(f: &'a Flow, s: &Stop) -> Vec<&'a crate::flow::Call> {
    let t = &f.turns[s.turn];
    if let Some(Item::Group { calls, .. }) = s.group.and_then(|g| t.items.get(g)) {
        return calls.iter().map(|&i| &t.calls[i]).collect();
    }
    s.seq
        .iter()
        .chain(s.also.iter())
        .filter_map(|q| t.calls.iter().find(|c| c.seq == *q))
        .collect()
}

#[derive(Clone, Debug, PartialEq)]
enum LegIcon {
    All,
    Swatch(&'static str),
    Glyph(Glyph),
    /// A verdict's mark: `blocked`, `warn`, `allowed` or `none`.
    Mark(&'static str),
    Light,
}

/// A Call (or a turn's request) a legend row unfolds into.
#[derive(Clone, Debug, PartialEq)]
struct LegPick {
    turn: usize,
    seq: Option<i64>,
    text: String,
    at: String,
}

struct LegRow {
    key: &'static str,
    label: String,
    n: usize,
    icon: LegIcon,
    picks: Vec<LegPick>,
}

fn pick_of(n: usize, c: &crate::flow::Call) -> LegPick {
    LegPick {
        turn: n,
        seq: Some(c.seq),
        text: c.title.clone(),
        at: hm(c.at),
    }
}

/// The legend's kinds of station over the turns shown, those there are, in the line's order:
/// every Call, the requests, the verdicts, the kinds of tool, the answers.
fn legend_rows(f: &Flow, turns: &[usize], stops: &[Stop]) -> Vec<LegRow> {
    let calls: Vec<(usize, &crate::flow::Call)> = turns
        .iter()
        .flat_map(|&n| f.turns[n].calls.iter().map(move |c| (n, c)))
        .collect();
    let mut rows = vec![LegRow {
        key: "all",
        label: "모든 호출".into(),
        n: calls.len(),
        icon: LegIcon::All,
        picks: Vec::new(),
    }];
    let asks: Vec<LegPick> = turns
        .iter()
        .map(|&n| LegPick {
            turn: n,
            seq: None,
            text: f.turns[n].req.clone(),
            at: hm(f.turns[n].asked),
        })
        .collect();
    if !asks.is_empty() {
        rows.push(LegRow {
            key: "ask",
            label: "사용자 요청".into(),
            n: asks.len(),
            icon: LegIcon::Swatch(Tone::Ask.color()),
            picks: asks,
        });
    }
    for (key, label) in [
        ("blocked", "차단"),
        ("warn", "확인 요청"),
        ("allowed", "허용"),
        ("none", "판정 없음"),
    ] {
        let picks: Vec<LegPick> = calls
            .iter()
            .filter(|(_, c)| verdict(c) == key)
            .map(|&(n, c)| pick_of(n, c))
            .collect();
        if !picks.is_empty() {
            rows.push(LegRow {
                key,
                label: label.into(),
                n: picks.len(),
                icon: LegIcon::Mark(key),
                picks,
            });
        }
    }
    for g in Glyph::ALL {
        let picks: Vec<LegPick> = calls
            .iter()
            .filter(|(_, c)| c.glyph == g)
            .map(|&(n, c)| pick_of(n, c))
            .collect();
        if !picks.is_empty() {
            rows.push(LegRow {
                key: g.name(),
                label: g.name().into(),
                n: picks.len(),
                icon: LegIcon::Glyph(g),
                picks,
            });
        }
    }
    let says = stops.iter().filter(|s| s.kind == Kind::Say).count();
    if says > 0 {
        rows.push(LegRow {
            key: "say",
            label: "에이전트 답변".into(),
            n: says,
            icon: LegIcon::Swatch(Tone::Say.color()),
            picks: Vec::new(),
        });
    }
    rows
}

/// The marks to light over the turns shown, each with the Calls it lights.
fn light_rows(f: &Flow, turns: &[usize]) -> Vec<LegRow> {
    LIGHTS
        .iter()
        .map(|&(key, label)| {
            let picks: Vec<LegPick> = turns
                .iter()
                .flat_map(|&n| {
                    f.turns[n]
                        .calls
                        .iter()
                        .filter(|c| lights(key, c))
                        .map(move |c| pick_of(n, c))
                })
                .collect();
            LegRow {
                key,
                label: label.into(),
                n: picks.len(),
                icon: LegIcon::Light,
                picks,
            }
        })
        .collect()
}

fn legend_icon(icon: &LegIcon) -> Element {
    match icon {
        LegIcon::All => rsx! {
            svg { width: "16", height: "16", view_box: "0 0 16 16",
                rect { x: "2", y: "2", width: "9", height: "9", rx: "1.5", fill: "none", stroke: "#c9c9cd", stroke_width: "1.4" }
                rect { x: "5", y: "5", width: "9", height: "9", rx: "1.5", fill: "#3a3a3f", stroke: "#c9c9cd", stroke_width: "1.4" }
            }
        },
        LegIcon::Swatch(c) => rsx! { i { class: "sw", style: "background: {c}" } },
        LegIcon::Glyph(g) => rsx! {
            span { class: "lg-glyph",
                svg { width: "14", height: "14", view_box: "0 0 16 16", {glyph(*g, 8.0, 8.0, 14.0, g.tone().color())} }
            }
        },
        LegIcon::Mark(k) => rsx! { span { class: "lg-mark {k}" } },
        LegIcon::Light => rsx! {
            svg { width: "16", height: "16", view_box: "0 0 16 16",
                circle { cx: "8", cy: "8", r: "6", fill: "none", stroke: "#b48cff", stroke_width: "1.6" }
                circle { cx: "8", cy: "8", r: "2.6", fill: "#b48cff" }
            }
        },
    }
}

fn eye_icon(off: bool) -> Element {
    rsx! {
        svg { width: "16", height: "16", view_box: "0 0 24 24",
            path { d: "M2.5 12s3.5-6.5 9.5-6.5S21.5 12 21.5 12s-3.5 6.5-9.5 6.5S2.5 12 2.5 12z", fill: "none", stroke: "currentColor", stroke_width: "1.7" }
            circle { cx: "12", cy: "12", r: "2.8", fill: "none", stroke: "currentColor", stroke_width: "1.7" }
            if off {
                path { d: "M4 4l16 16", stroke: "currentColor", stroke_width: "1.7", stroke_linecap: "round" }
            }
        }
    }
}

/// A legend row's Calls (or requests), one picked opening it on the line.
#[component]
fn LegendPicks(
    picks: Vec<LegPick>,
    flow: Arc<Flow>,
    turn: Signal<usize>,
    detail: Signal<bool>,
) -> Element {
    let mut state = use_context::<AppState>();
    let mut detail = detail;
    let picked = match (state.selected)() {
        Selection::Node(seq) => Some(seq),
        Selection::None => None,
    };
    rsx! {
        div { class: "lg-picks",
            for (i, p) in picks.into_iter().enumerate() {
                {
                    let fl = flow.clone();
                    let on = p.seq.is_some() && p.seq == picked;
                    rsx! {
                        button {
                            key: "{i}",
                            class: if on { "lg-pick on" } else { "lg-pick" },
                            title: "{p.text}",
                            onclick: move |_| {
                                select_turn(turn, state, &fl, p.turn);
                                if let Some(seq) = p.seq {
                                    state.selected.set(Selection::Node(seq));
                                    detail.set(true);
                                }
                            },
                            span { class: "t", "턴 {p.turn + 1}" }
                            span { class: "x", "{p.text}" }
                            span { class: "at", "{p.at}" }
                        }
                    }
                }
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
    /// The legend over the map's top left is open.
    legend: Signal<bool>,
    /// How much of the view's right side a card covers.
    covered: f64,
) -> Element {
    let mut state = use_context::<AppState>();
    let mut detail = detail;
    let mut legend = legend;
    // The legend: kinds of station hidden (greyed on the line), the mark lit (the rest greyed),
    // and the row unfolded into its Calls.
    let mut hidden = use_signal(HashSet::<&'static str>::new);
    let mut lit_by = use_signal(|| None::<&'static str>);
    let mut unfolded = use_signal(|| None::<&'static str>);
    // What the map brings into view stays clear of the legend.
    use_effect(move || {
        document::eval(&format!(
            "window.fxLeft = {};",
            if legend() { LEGEND_W } else { 0.0 }
        ));
    });
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
    // The turn whose prompt ← and → are at: its heading marked.
    let prompt_pick = use_context::<PromptPick>().0;
    // The turn whose answer is opened to what the agent said, whole.
    let mut say_open = use_signal(|| None::<usize>);
    // The branch whose cause is opened to the words read, whole.
    let mut cause_open = use_signal(|| None::<usize>);
    let picked = match (state.selected)() {
        Selection::Node(seq) => Some(seq),
        Selection::None => None,
    };
    // A stopped or held Call picked that something in a tool result told: the way back from it to what
    // told it, and from that to its own cause, up across turns, is shown in colour, the rest of
    // the line grey.
    let tracing = picked
        .and_then(|p| f.call(p))
        .is_some_and(|c| (c.blocked || c.warn) && (c.by.is_some() || c.origin.is_some()));
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
        .and_then(|seq| line.stops.iter().find(|s| s.is(seq) && s.turn == t))
        .or(grouped.as_ref())
        .map(|s| (s.x, s.y));
    // A turn or a Call picked is brought into view by the page's own mover (pan.js).
    let mark_at = line.marks.iter().find(|m| m.turn == t).map(|m| {
        let (x, y, w) = m
            .zones
            .first()
            .map_or((m.x, m.y - 206.0, 0.0), |z| (z.0, z.1, z.2));
        (x, y, w, m.rev)
    });
    let lkey = (line.width as u64) << 20 ^ line.height as u64;
    if let Some((mx, my, mw, rev)) = mark_at
        && h > 0.0
        && !tracing
        && *framed.peek() != Some((t, picked, lkey))
    {
        let mut framed = framed;
        framed.set(Some((t, picked, lkey)));
        let (tx, ty) = target_at.map_or(("null".to_owned(), "null".to_owned()), |(x, y)| {
            (x.to_string(), y.to_string())
        });
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
    let has_branch = !line.branch_labels.is_empty();
    // The way back, when it is on: (cause, follower) pairs from the Call picked up its causes.
    let mut trace: Vec<(i64, i64)> = Vec::new();
    if tracing {
        let mut cur = picked;
        let mut seen = HashSet::new();
        while let Some(q) = cur.filter(|q| seen.insert(*q)) {
            let call = f.call(q);
            let by = call.and_then(|c| c.by);
            // Across turns too: from the Call whose result first held a value it used.
            let from = call.and_then(|c| c.origin.as_ref()).map(|o| o.from);
            if let Some(b) = by {
                trace.push((b, q));
            }
            if let Some(o) = from {
                trace.push((o, q));
            }
            cur = by.or(from);
        }
    }
    // The Calls on the way back, the one picked with them: in colour while the rest is grey.
    let chain: HashSet<i64> = trace
        .iter()
        .flat_map(|&(c, f)| [c, f])
        .chain(picked.filter(|_| tracing))
        .collect();
    // A quote under a station on the way back, or about two of them.
    let on_chain = |b: &crate::flow::BranchLabel| {
        b.about.iter().any(|q| chain.contains(q))
            || (b.about.is_empty()
                && line.stops.iter().any(|s| {
                    (s.x - b.x).abs() < 0.5
                        && (s.y - b.y).abs() < 0.5
                        && s.seq.is_some_and(|q| chain.contains(&q))
                }))
    };
    // The whole way back brought into view, its far end and the Call picked both.
    let boxed = use_signal(|| None::<(Option<i64>, u64)>);
    if tracing && h > 0.0 && *boxed.peek() != Some((picked, lkey)) {
        let pts: Vec<(f64, f64)> = line
            .stops
            .iter()
            .filter(|s| s.seq.is_some_and(|q| chain.contains(&q)))
            .map(|s| (s.x, s.y))
            .collect();
        if !pts.is_empty() {
            let mut boxed = boxed;
            boxed.set(Some((picked, lkey)));
            let (x0, x1) = pts
                .iter()
                .fold((f64::MAX, f64::MIN), |(a, b), p| (a.min(p.0), b.max(p.0)));
            let (y0, y1) = pts
                .iter()
                .fold((f64::MAX, f64::MIN), |(a, b), p| (a.min(p.1), b.max(p.1)));
            // Room for the names over the stations and the quote boxes under them.
            let (x0, y0, x1, y1) = (x0 - 160.0, y0 - 110.0, x1 + 260.0, y1 + 170.0);
            document::eval(&format!(
                "{PAN_JS}\nrequestAnimationFrame(() => fxMap.box({x0}, {y0}, {x1}, {y1}));"
            ));
        }
    } else if !tracing && boxed.peek().is_some() {
        let mut boxed = boxed;
        boxed.set(None);
    }
    // The quotes shown (each over its stretch of the lower track; one tied to Calls only when
    // one of them is picked), each moved down under any shown before it that it would cover.
    let mut quote_y: HashMap<usize, f64> = HashMap::new();
    let mut placed: Vec<(f64, f64, f64)> = Vec::new();
    for (bi, b) in line.branch_labels.iter().enumerate() {
        let shown = if tracing {
            on_chain(b)
        } else {
            b.about.is_empty() || picked.is_some_and(|p| b.about.contains(&p))
        };
        if !shown {
            continue;
        }
        let mut y = b.qy;
        while placed
            .iter()
            .any(|&(x0, x1, py)| (py - y).abs() < 58.0 && x0 < b.qx + b.qw && x1 > b.qx)
        {
            y += 62.0;
        }
        placed.push((b.qx, b.qx + b.qw, y));
        quote_y.insert(bi, y);
    }
    let target = picked.and_then(|seq| {
        line.stops
            .iter()
            .find(|s| s.is(seq) && s.turn == t && s.kind != Kind::Transfer)
            .or_else(|| line.stops.iter().find(|s| s.is(seq) && s.turn == t))
            .cloned()
            .or(grouped.clone())
    });
    let rows = legend_rows(f, &turns, &line.stops);
    // The kinds there are, for the first row's eye: all hidden, or none.
    let present: Vec<&'static str> = rows.iter().map(|r| r.key).filter(|k| *k != "all").collect();
    let light_rows = light_rows(f, &turns);
    let hid_set = hidden();
    let light = lit_by();
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
                        let c = if tracing && s.link {
                            TRACE_COLOR
                        } else if tracing {
                            DIM_COLOR
                        } else if s.branch {
                            if s.turn == t { TEXT_COLOR } else { TEXT_MUTED }
                        } else {
                            LINE_MUTED
                        };
                        let lw = if tracing && s.link { "3.5" } else { "2.5" };
                        rsx! {
                            g { key: "seg{i}", class: "fx-nohit",
                                if s.link {
                                    // A passage from the station whose output was read to the
                                    // Call that followed it: thin, under the line, not ridden.
                                    path { d: "{s.d}", fill: "none", stroke: "{c}", stroke_width: "{lw}", stroke_dasharray: if s.unsure { "5 5" } else { "none" }, stroke_linecap: "round", stroke_linejoin: "round", opacity: "0.9" }
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
                        let op = if tracing || ts.turn == t { "1" } else { "0.28" };
                        let stroke = if tracing { DIM_COLOR } else { ts.tone.color() };
                        let d = ts.d.clone();
                        rsx! {
                            path { key: "tone{i}", class: "fx-nohit", d: "{d}", fill: "none", stroke: "{stroke}", stroke_width: "7", stroke_linecap: "round", opacity: "{op}" }
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
                        // With the way back shown, only its stations keep their colour and names.
                        let lit = s.seq.is_some_and(|q| chain.contains(&q));
                        // The legend's: a kind hidden, or another kind lit.
                        let here = stop_calls(f, &s);
                        let hid = match s.kind {
                            Kind::Start => hid_set.contains("ask"),
                            Kind::Say => hid_set.contains("say"),
                            _ => !here.is_empty() && here.iter().all(|c| hides(&hid_set, c)),
                        };
                        let shines = light.is_some_and(|k| here.iter().any(|c| lights(k, c)));
                        let dim = (tracing && !lit) || hid || (light.is_some() && !shines);
                        let color = if dim {
                            DIM_COLOR
                        } else if s.branch {
                            if s.turn == t || lit { TEXT_COLOR } else { "#c9c2dd" }
                        } else if s.turn == t || lit {
                            s.tone.color()
                        } else {
                            col(s.turn)
                        };
                        let sel = target.as_ref().is_some_and(|tg| tg.id == s.id);
                        let open = s.group.is_some_and(|gi| opened().contains(&(s.turn, gi)));
                        let fl = flow.clone();
                        let st = s.clone();
                        rsx! {
                            g { key: "{s.id}",
                            if shines && !hid {
                                circle { class: "fx-nohit fx-halo", cx: "{s.x}", cy: "{s.y}", r: "17" }
                            }
                            LineStop {
                                stop: s.clone(),
                                color: color.to_string(),
                                sel,
                                detail: if tracing { lit } else { s.turn == t },
                                dim,
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
                        let stepped = if prompt_pick() == Some(n) { " pick" } else { "" };
                        let cls = if on { format!("fx-turnhead {ask}{side} on{stepped}") } else { format!("fx-turnhead {ask}{side}{stepped}") };
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
                        let on = b.turn == t || tracing;
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
                                    let value = c.p < 0.0;
                                    let kind = if c.error { "오류" } else if value { "값" } else { "출력" };
                                    let cls = format!("fx-cause{}{}{}", if c.error { " err" } else { "" }, if value { " val" } else { "" }, if on { " on" } else { "" });
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
                                                    if !value { span { class: "p", "{p}%" } }
                                                }
                                                // The station whose output it was.
                                                if value {
                                                    div { class: "from", "← {b.text} 결과에서 처음 나온 값" }
                                                } else {
                                                    div { class: "from", "← {b.text}의 {kind}" }
                                                }
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
                            if let Some(intent) = tt.intent.as_deref().map(crate::flow::without_confidence) {
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
                        let kind = if c.error { "오류 출력" } else if c.p < 0.0 { "값이 처음 나온 결과" } else { "도구 출력" };
                        let p = (c.p * 100.0).round();
                        let reading = if c.p < 0.0 { b.text.clone() } else { format!("{} · 원인일 확률 {p}%", b.text) };
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
            // While the way back from a stopped Call is shown, what it is and how to leave it.
            if tracing {
                div { class: "fx-trace on", onmousedown: move |e| e.stop_propagation(),
                    span { class: "dot" }
                    {if picked.and_then(|p| f.call(p)).is_some_and(|c| c.blocked) { "차단된" } else { "확인 요청된" }}
                    " 호출의 원인 경로 · 다른 역을 누르면 돌아가요"
                }
            }
            // The legend, as a console's over its process tree: each kind of station with how
            // many there are, an eye to grey it out on the line, a row unfolded into its Calls
            // (one picked opens it); then marks to light, the rest greyed.
            if legend() {
                div { class: "fx-legend-panel", onmousedown: move |e| e.stop_propagation(), onwheel: move |e| e.stop_propagation(),
                    div { class: "lg-head",
                        strong { "범례" }
                        button { class: "lg-close", title: "범례 닫기", onclick: move |_| legend.set(false),
                            {tool_icon("M4 5h16v14H4zM9 5v14M16 9.5L13.5 12l2.5 2.5")}
                        }
                    }
                    for row in rows {
                        {
                            let key = row.key;
                            let off = if key == "all" { present.iter().all(|k| hid_set.contains(k)) } else { hid_set.contains(key) };
                            let open = unfolded() == Some(key);
                            let keys = present.clone();
                            rsx! {
                                div { key: "{key}", class: if off { "lg-row off" } else { "lg-row" },
                                    span { class: "lg-icon", {legend_icon(&row.icon)} }
                                    span { class: "lg-name", "{row.label}" }
                                    span { class: "lg-n", "{row.n}" }
                                    button {
                                        class: "lg-eye",
                                        title: if off { "다시 보기" } else { "노선에서 흐리게" },
                                        onclick: move |_| {
                                            let mut h = hidden.write();
                                            if key == "all" {
                                                if off { h.clear() } else { h.extend(keys.iter().copied()) }
                                            } else if !h.remove(key) {
                                                h.insert(key);
                                            }
                                        },
                                        {eye_icon(off)}
                                    }
                                    if row.picks.is_empty() {
                                        span { class: "lg-fold" }
                                    } else {
                                        button {
                                            class: if open { "lg-fold on" } else { "lg-fold" },
                                            title: if open { "접기" } else { "하나씩 보기" },
                                            onclick: move |_| unfolded.set(if open { None } else { Some(key) }),
                                            {tool_icon("M7 10l5 5 5-5")}
                                        }
                                    }
                                }
                                if open {
                                    LegendPicks { key: "{key}-picks", picks: row.picks.clone(), flow: flow.clone(), turn, detail }
                                }
                            }
                        }
                    }
                    div { class: "lg-head sub", strong { "강조해서 보기" } }
                    for row in light_rows {
                        {
                            let key = row.key;
                            let on = light == Some(key);
                            let open = unfolded() == Some(key);
                            rsx! {
                                div {
                                    key: "{key}",
                                    class: if on { "lg-row lit on" } else { "lg-row lit" },
                                    title: if on { "강조 끄기" } else { "이 호출만 강조하고 나머지는 흐리게" },
                                    onclick: move |_| lit_by.set(if on { None } else { Some(key) }),
                                    span { class: "lg-icon", {legend_icon(&row.icon)} }
                                    span { class: "lg-name", "{row.label}" }
                                    span { class: "lg-n", "{row.n}" }
                                    span {}
                                    if row.picks.is_empty() {
                                        span { class: "lg-fold" }
                                    } else {
                                        button {
                                            class: if open { "lg-fold on" } else { "lg-fold" },
                                            title: if open { "접기" } else { "하나씩 보기" },
                                            onclick: move |e: MouseEvent| {
                                                e.stop_propagation();
                                                unfolded.set(if open { None } else { Some(key) });
                                            },
                                            {tool_icon("M7 10l5 5 5-5")}
                                        }
                                    }
                                }
                                if open {
                                    LegendPicks { key: "{key}-picks", picks: row.picks.clone(), flow: flow.clone(), turn, detail }
                                }
                            }
                        }
                    }
                    if has_branch {
                        p { class: "lg-foot", i { class: "sw", style: "background: {TEXT_COLOR}" } "보라색 길은 도구 결과의 글을 따라간 길입니다." }
                    }
                }
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
    format!(
        "M{} {} L{} {} L{} {}z",
        x - 5.0,
        y + 7.0,
        x + 5.0,
        y + 7.0,
        x,
        y - 1.0
    )
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
    /// Off the way back being shown: grey, unnamed, faint.
    #[props(default)]
    dim: bool,
    open: bool,
    onsel: EventHandler<()>,
) -> Element {
    let s = &stop;
    let important = matches!(s.kind, Kind::Warn | Kind::Blocked | Kind::Transfer);
    // On a turn not picked only the stations that tell its story keep their names, faint:
    // what was stopped, interchanges, files changed, and how it ended.
    let key = important || s.xfer || s.tone == Tone::Change || s.kind == Kind::Say;
    let show = !s.label.is_empty() && (detail || (key && !dim));
    let faint = if detail { "1" } else { "0.55" };
    let mini = s.kind == Kind::Mini;
    let minor = s.kind == Kind::Tick || mini;
    let up = s.up == Some(true);
    let gap = if s.kind == Kind::Transfer {
        32.0
    } else if mini {
        14.0
    } else {
        26.0
    };
    // A small station's two lines sit closer: its tool, then what it did.
    let step = if mini { 14.0 } else { 16.0 };
    // A name moved a tier out stands that much further from the line.
    let lift = s.tier as f64 * 36.0;
    let ly = if up {
        s.y - gap - step - lift
    } else {
        s.y + gap + 8.0 + lift
    };
    let sy = if up {
        s.y - gap - lift
    } else {
        s.y + gap + 8.0 + step + lift
    };
    let (x, y) = (s.x, s.y);
    let fill = match s.kind {
        _ if dim => DIM_COLOR,
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
            opacity: if dim { "0.7" } else { "1" },
            onclick: move |e| { e.stop_propagation(); onsel.call(()); },
            title { "{tip}" }
            circle { cx: "{x}", cy: "{y}", r: "20", fill: "transparent" }
            // A stopped Call whose result also told another: the interchange's ring around it.
            if s.xfer && s.kind != Kind::Transfer && !dim {
                circle { cx: "{x}", cy: "{y}", r: "16", fill: "none", stroke: TEXT_COLOR, stroke_width: "2.5" }
            }
            // A value it used came from an earlier turn: a small tag with that turn over it.
            if let Some(n) = s.crossing.filter(|_| !dim) {
                g { class: "fx-nohit",
                    rect { x: "{x + 11.0}", y: "{y + 6.0}", width: "40", height: "16", rx: "8", fill: "#fff", stroke: "#c2410c", stroke_width: "1.5" }
                    text { x: "{x + 31.0}", y: "{y + 17.5}", text_anchor: "middle", font_size: "10", font_weight: "700", fill: "#c2410c", "↶턴{n + 1}" }
                }
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
                        rect { x: "{x - 9.0}", y: "{y - 15.0}", width: "18", height: "30", rx: "9", fill: "#fff", stroke: if dim { DIM_COLOR } else { "#1f2125" }, stroke_width: "3" }
                        circle { cx: "{x}", cy: "{y - 5.5}", r: "4", fill: "{color}" }
                        circle { cx: "{x}", cy: "{y + 5.5}", r: "4", fill: if dim { DIM_COLOR } else { TEXT_COLOR } }
                    }
                },
                Kind::Warn => rsx! {
                    g { transform: "translate({x - 12.0} {y - 12.0})",
                        path { d: "M12 1L24 22H0z", fill: if dim { DIM_COLOR } else { AMBER }, stroke: "#fff", stroke_width: "2", stroke_linejoin: "round" }
                        rect { x: "10.9", y: "7.5", width: "2.2", height: "7.5", rx: "1", fill: "#fff" }
                        circle { cx: "12", cy: "18", r: "1.3", fill: "#fff" }
                    }
                },
                Kind::Blocked => rsx! {
                    g {
                        circle { cx: "{x}", cy: "{y}", r: "11", fill: if dim { DIM_COLOR } else { RED }, stroke: "#fff", stroke_width: "2" }
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
