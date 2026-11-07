//! The route's detail column (see `crate::metro`): for the Call picked, who told the agent to
//! make it, drawn as a chain from the request through the words that took over to the Call; and
//! the viewer's verdict icons.

use std::sync::Arc;

use dioxus::prelude::*;

use super::{AppState, Load};
use crate::metro::{Board, LineKind, Metro, Station};

/// A press that moves less than this is a click, not a drag.
pub(super) const CLICK_SLOP: f64 = 4.0;

/// The colour of a leg that followed words read in a tool result rather than the request.
const TOLD: &str = "#8a909a";
/// How long a way took, as a route finder writes it.
fn took(ms: i64) -> String {
    let secs = ms.max(0) as f64 / 1000.0;
    if secs < 10.0 {
        format!("{secs:.1}초")
    } else if secs < 60.0 {
        format!("{}초", secs.round())
    } else {
        format!("{}분 {}초", (secs / 60.0).floor(), (secs % 60.0).round())
    }
}

/// What a route finder says about a way: its legs, each leg's span, colour and badge (and
/// whether it was walked), the Calls passed on each, how long it all took, and the changes.
struct RouteFacts {
    legs: Vec<crate::metro::Leg>,
    segs: Vec<(i64, &'static str, String, bool)>,
    passed: Vec<usize>,
    total: Option<i64>,
    changes: usize,
}

fn route_facts(m: &Metro, dest: &Station) -> RouteFacts {
    let legs = m.journey(dest.seq).unwrap_or_default();
    let by = dest.source.as_ref().map(|src| src.by);
    let board_x = |b: Board| match b {
        Board::Ask(i) => m.asks.get(i).map(|a| a.x),
        Board::Station(q) => m.station(q).map(|s| s.x),
        Board::Nowhere => None,
    };
    let mut segs = Vec::new();
    let mut passed = Vec::new();
    for leg in &legs {
        let from = match leg.board {
            Board::Ask(i) => m.asks.get(i).and_then(|a| a.at),
            Board::Station(b) => m.station(b).and_then(|s| s.at),
            Board::Nowhere => None,
        };
        let to = m.station(leg.alight).and_then(|s| s.at);
        let span = from.zip(to).map_or(0, |(a, b)| (b - a).max(0));
        let walked = matches!(leg.board, Board::Station(b) if Some(b) == by);
        let x1 = m.station(leg.alight).map_or(dest.x, |s| s.x);
        let colour = if walked {
            TOLD
        } else {
            line_colour(m, leg.line, x1)
        };
        segs.push((span, colour, m.badge(leg.line), walked));
        // The Calls on the stretch ridden, folded ones counted one by one.
        let count = board_x(leg.board).map_or(1, |x0| {
            m.stations
                .iter()
                .filter(|s| s.line == leg.line && s.x > x0.min(x1) && s.x <= x0.max(x1))
                .map(|s| s.members.len().max(1))
                .sum::<usize>()
        });
        passed.push(count);
    }
    let start = match legs.first().map(|l| l.board) {
        Some(Board::Ask(i)) => m.asks.get(i).and_then(|a| a.at),
        _ => None,
    };
    RouteFacts {
        changes: legs.len().saturating_sub(1),
        total: start.zip(dest.at).map(|(a, b)| b - a),
        legs,
        segs,
        passed,
    }
}

/// A subagent's report as a person reads it: its words, without the envelope the engine kept.
fn report_words(text: &str) -> String {
    let mut t = text.trim();
    if t.starts_with("<agent-message") {
        t = t.split_once('>').map_or(t, |(_, rest)| rest).trim();
    }
    if t.starts_with('[') {
        t = t.split_once(']').map_or(t, |(_, rest)| rest).trim();
    }
    t.trim_end_matches("</agent-message>").trim().to_owned()
}

/// A moment as the route finder's times column writes it: the hour, minute and second.
fn clock(ms: Option<i64>) -> String {
    use chrono::TimeZone;
    ms.and_then(|ms| chrono::Local.timestamp_millis_opt(ms).single())
        .map(|t| t.format("%H:%M:%S").to_string())
        .unwrap_or_default()
}

/// An icon of the viewer's own set, drawn as Lucide draws its icons (a 24 grid, a 2px round
/// stroke) and shaped like the signs a verdict means: a stop sign for a denial, a warning
/// triangle for a review, a ticked circle for an allowed Call, a dashed one for no verdict.
/// Shape carries the meaning before colour does.
#[component]
pub(super) fn Icon(name: &'static str, size: f64) -> Element {
    rsx! {
        svg {
            class: "icon {name}",
            width: "{size}", height: "{size}", view_box: "0 0 24 24",
            "aria-hidden": "true",
            match name {
                // An octagon, flat side up, with a cross.
                "deny" => rsx! {
                    path { class: "fill", d: "M8.1 2h7.8L22 8.1v7.8L15.9 22H8.1L2 15.9V8.1Z" }
                    path { class: "mark", d: "M9 9l6 6M15 9l-6 6" }
                },
                // A rounded triangle, point up, with an exclamation.
                "review" => rsx! {
                    path { class: "fill", d: "M10.3 3.9 1.9 18a2 2 0 0 0 1.7 3h16.8a2 2 0 0 0 1.7-3L13.7 3.9a2 2 0 0 0-3.4 0Z" }
                    path { class: "mark", d: "M12 9.5v4.5M12 17.5h.01" }
                },
                "allow" => rsx! {
                    circle { class: "line", cx: "12", cy: "12", r: "9.5" }
                    path { class: "mark-line", d: "m8 12.2 2.8 2.8L16.2 9.4" }
                },
                // Who told it: a person, words in a document, or no one found.
                "person" => rsx! {
                    circle { class: "line", cx: "12", cy: "8", r: "4" }
                    path { class: "line", d: "M4.5 21a7.5 7.5 0 0 1 15 0" }
                },
                "doc" => rsx! {
                    path { class: "line", d: "M14 2.5H6.5a2 2 0 0 0-2 2v15a2 2 0 0 0 2 2h11a2 2 0 0 0 2-2V8Z" }
                    path { class: "line", d: "M14 2.5V8h5.5M8.5 13h7M8.5 17h5" }
                },
                "unknown" => rsx! {
                    circle { class: "line dashed", cx: "12", cy: "12", r: "9.5" }
                    path { class: "line", d: "M9.3 9.3a2.8 2.8 0 0 1 5.4 1c0 1.9-2.7 2.4-2.7 4" }
                    path { class: "line", d: "M12 17.6h.01" }
                },
                "undecided" => rsx! {
                    circle { class: "line dashed", cx: "12", cy: "12", r: "9.5" }
                },
                // Hard to undo: an undo arrow, struck through.
                "undo" => rsx! {
                    path { class: "line", d: "M9 14 4 9l5-5" }
                    path { class: "line", d: "M4 9h10.5a5.5 5.5 0 0 1 4.3 8.9" }
                    path { class: "line", d: "M3 21 21 3" }
                },
                // The tool failed: a circle with a slash.
                "fail" => rsx! {
                    circle { class: "line", cx: "12", cy: "12", r: "9.5" }
                    path { class: "line", d: "m5.3 5.3 13.4 13.4" }
                },
                _ => rsx! {},
            }
        }
    }
}

/// Each turn's colour, in the order the turns came: the list, the detail and the map's turn
/// labels all paint a turn with it. Red and amber stay out: they mean a stop.
pub(super) const TURN_COLOURS: [&str; 5] = ["#3e5c76", "#4f8a87", "#81729f", "#7b8656", "#9c7a5e"];

/// The main agent's line: ink, ours rather than a metro's primary colours.
pub(super) const MAIN_INK: &str = "#2b2f3a";

pub(super) fn turn_colour(turn: usize) -> &'static str {
    TURN_COLOURS[turn % TURN_COLOURS.len()]
}

/// The colours a subagent's line takes, one per kind of subagent, as a metro gives each line
/// its own: none of them red (a stop) or the turns' blues.
const BRANCH_COLOURS: [&str; 4] = ["#1f9e89", "#8c6bb1", "#4f7cac", "#5e8f3a"];

/// A line's colour where it runs at `x`: the main line in its turn's colour, a subagent's line
/// in its kind's.
pub(super) fn line_colour(m: &Metro, line: usize, x: f64) -> &'static str {
    if m.lines.get(line).is_some_and(|l| l.badge == "글") {
        return TOLD;
    }
    if m.lines.get(line).is_none_or(|l| l.kind == LineKind::Main) {
        // One line, one colour, as a metro map keeps it: the turns show at their markers.
        let _ = x;
        return MAIN_INK;
    }
    let mut kinds: Vec<&str> = Vec::new();
    for l in m.lines.iter().filter(|l| l.kind == LineKind::Branch) {
        if !kinds.contains(&l.name.as_str()) {
            kinds.push(&l.name);
        }
    }
    let at = kinds
        .iter()
        .position(|k| *k == m.lines[line].name)
        .unwrap_or(0);
    BRANCH_COLOURS[at % BRANCH_COLOURS.len()]
}

/// The colour a verdict wears in classes.
fn verdict_tone(decision: Option<&str>) -> &'static str {
    match decision {
        Some("DENY") => "deny",
        Some("REVIEW") => "review",
        Some(_) => "pass",
        None => "quiet",
    }
}

/// The icon a verdict wears.
pub(super) fn verdict_icon(decision: Option<&str>) -> &'static str {
    match decision {
        Some("DENY") => "deny",
        Some("REVIEW") => "review",
        Some(_) => "allow",
        None => "undecided",
    }
}

/// The engine's strongest reason for a verdict, by the probability it gave it.
pub(super) fn strongest_reason(model: &crate::layout::Model, seq: i64) -> Option<(String, String)> {
    let node = model.nodes.iter().find(|n| n.seq() == seq)?;
    let trace = crate::model::parse_json(node.props.get("trace_json"));
    let p = |r: &(String, String)| {
        r.1.split_whitespace()
            .last()
            .and_then(|n| n.parse::<f64>().ok())
            .unwrap_or(0.0)
    };
    super::trace::reasons(&trace)
        .into_iter()
        .max_by(|a, b| p(a).total_cmp(&p(b)))
}

/// What told the agent to make a Call, as the list groups it: the tool result whose words
/// caused it, nothing in the session, or the turn's request.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum Cause {
    Result(i64),
    Nothing,
    Turn(usize),
}

fn cause_of(m: &Metro, s: &Station) -> Cause {
    match &s.source {
        Some(src) => Cause::Result(src.result),
        None if s.loose || s.unsourced => Cause::Nothing,
        None => Cause::Turn(m.asks.iter().rposition(|a| a.x <= s.x).unwrap_or(0)),
    }
}

/// A cause as the list heads its group: "issues/42 결과의 지시", "출처 없음", the request.
fn cause_title(m: &Metro, cause: &Cause, example: &Station) -> (&'static str, String) {
    match cause {
        Cause::Result(_) => match example.source.as_ref().and_then(|src| m.station(src.by)) {
            Some(b) => ("used", format!("{}{}", b.cause_name(), source_kind(b))),
            None => ("used", "도구 결과".to_owned()),
        },
        Cause::Nothing => ("nobody", "시킨 것 없음".to_owned()),
        Cause::Turn(i) => {
            let ask = &m.asks[*i];
            let turn = if m.asks.len() > 1 {
                format!("턴 {} · ", i + 1)
            } else {
                String::new()
            };
            match &ask.agent {
                Some(agent) => ("person", format!("{turn}{agent} 보고")),
                None => ("person", format!("{turn}사람의 요청")),
            }
        }
    }
}

/// What told the agent, as the list's group heads and the detail's top both name it: a dot in
/// the cause's colour and the same words.
#[component]
fn CauseTag(tone: &'static str, text: String) -> Element {
    rsx! { span { class: "ctag {tone}", span { class: "dot" } span { class: "t", "{text}" } } }
}

/// The engine's reason for a stop, the same box wherever it shows.
#[component]
fn ReasonBox(tone: &'static str, text: String, p: String) -> Element {
    rsx! { div { class: "reason {tone}", span { "{text}" } if !p.is_empty() { span { class: "p", " {p}" } } } }
}

/// A Call's two lines, the same in the list and the detail: its verdict, name and time; its
/// tool, target and marks.
#[component]
fn CallLines(station: Station, title: String, full_time: bool) -> Element {
    let s = &station;
    let mark = match s.decision.as_deref() {
        Some("DENY") => "차단",
        Some("REVIEW") => "확인 요청",
        Some(_) => "허용",
        None => "판정 없음",
    };
    let time = clock(s.at);
    let time = if full_time {
        time
    } else {
        time.get(..5).unwrap_or_default().to_owned()
    };
    rsx! {
        span { class: "cr-mark", title: "{mark}", Icon { name: verdict_icon(s.decision.as_deref()), size: 16.0 } }
        span { class: "cr-name", "{title}" }
        span { class: "cr-time", "{time}" }
        span { class: "cr-meta",
            if !s.folded() { span { class: "cr-tool", "{s.tool}" } }
            span { class: "cr-target", title: "{s.what}", "{s.what}" }
            if s.irreversible { span { class: "cr-flag", title: "되돌리기 어려움", Icon { name: "undo", size: 13.0 } } }
            if s.failed { span { class: "cr-flag", title: "도구 실패", Icon { name: "fail", size: 13.0 } } }
            if let Some(agent) = s.agent.clone() { span { class: "cr-agent", title: "{agent}", "{agent}" } }
        }
    }
}

/// What kind of text a cause's result is, after its name: a command's output, a file's
/// contents, a page.
fn source_kind(b: &Station) -> &'static str {
    match (b.tool.as_str(), b.action) {
        ("WebFetch" | "WebSearch", _) => " 페이지",
        (_, crate::metro::Action::Run) => " 출력",
        (_, crate::metro::Action::Search) => " 검색 결과",
        (_, crate::metro::Action::Read | crate::metro::Action::Change) => " 내용",
        _ => " 결과",
    }
}

/// The detail column answers one question about the Call being looked at: did a person tell
/// the agent to do this, and if not, what did. It draws the answer as a chain read top to
/// bottom: the request, then (when something else took over) the words that did or a gap, then
/// the action. A solid link is "as asked"; a broken one is where the request stopped being
/// what the agent followed.
#[component]
pub(super) fn RoutePanel(
    metro: Arc<Metro>,
    dest: Option<i64>,
    mut hovered: Signal<Option<i64>>,
    mut detail: Signal<bool>,
) -> Element {
    let mut state = use_context::<AppState>();
    // The words a Call followed are folded to their first lines until opened, per Call.
    let mut quote_open = use_signal(|| None::<i64>);
    let m = &metro;
    let Some(dest) = dest.and_then(|d| m.station(d)).cloned() else {
        return rsx! {
            div { class: "route-panel detail", onmousedown: move |e| e.stop_propagation(), onclick: move |e| e.stop_propagation(),
                button { class: "dh-close", title: "닫기", onclick: move |_| detail.set(false), "✕" }
                p { class: "route-hint", "지도나 목록에서 호출을 고르면, 그 호출을 누가 시켰는지 보여 줍니다." }
            }
        };
    };
    let turn = m.asks.iter().rposition(|a| a.x <= dest.x).unwrap_or(0);
    let ask = m.asks.get(turn).cloned();
    let report = ask.as_ref().and_then(|a| a.agent.clone());
    let asker = if report.is_some() {
        "에이전트 보고"
    } else {
        "사람의 요청"
    };
    // A result followed and allowed is the agent using what it read; only a stop makes the
    // words a takeover.
    let hot = dest.stopped();
    // The same words and colour as this Call's group in the list.
    let (tone, kicker) = cause_title(m, &cause_of(m, &dest), &dest);
    let tone = if tone == "used" && hot { "text" } else { tone };
    let top_tone: &'static str = if tone == "text" { "text" } else { "turn" };
    let top_text = if kicker.starts_with("턴 ") {
        kicker.clone()
    } else {
        format!("턴 {} · {kicker}", turn + 1)
    };
    let vtone = verdict_tone(dest.decision.as_deref());
    let why: Option<(String, String)> = match &*state.graph.read() {
        Load::Ready(model) => strongest_reason(model, dest.seq),
        _ => None,
    };
    // Which of the engine's checks decided it, in a sentence.
    let branch: Option<String> = match &*state.graph.read() {
        Load::Ready(model) => model.node(dest.seq).and_then(|n| {
            let trace = crate::model::parse_json(n.props.get("trace_json"));
            let b = trace["check"]["branch"].as_str()?.to_owned();
            Some(match b.as_str() {
                "safeguard" => "에이전트 설정·훅을 바꾸는 호출이라 늘 막습니다".to_owned(),
                "run" => "실행하려는 스크립트의 코드를 읽고, 결과를 속이거나 시키지 않은 일을 하는지 물었습니다".to_owned(),
                "file" => "바뀌는 줄을 보고, 요청하지 않은 변경인지 물었습니다".to_owned(),
                "purpose" => "이 호출의 목적이 사용자의 요청에 반하는지 물었습니다".to_owned(),
                "clear" => "의도에서 벗어날 의심이 낮아 더 묻지 않고 허용했습니다".to_owned(),
                // A check this viewer does not know says nothing it could explain.
                _ => return None,
            })
        }),
        _ => None,
    };
    let mut focus = state.focus;
    let inspecting = (state.inspect)();
    let request = ask
        .as_ref()
        .map(|a| report_words(&a.text))
        .unwrap_or_default();
    let src = dest.source.clone();
    let facts = route_facts(m, &dest);
    let by_seq = dest.source.as_ref().map(|src| src.by);
    rsx! {
        div { class: "route-panel detail nv", style: "--turn: {turn_colour(turn)}", onmousedown: move |e| e.stop_propagation(), onclick: move |e| e.stop_propagation(),
            button { class: "dh-close", title: "닫기", onclick: move |_| detail.set(false), "✕" }
            div { class: "dp-top",
                CauseTag { tone: top_tone, text: top_text.clone() }
            }
            // The result, as a route finder heads one: how long, when it arrived, how many changes.
            div { class: "nv-head",
                b { class: "nv-time", if let Some(t) = facts.total { "{took(t)}" } else { "—" } }
                span { class: "nv-sep" }
                span { "{clock(dest.at)} 도착" }
                span { class: "nv-sep" }
                span { if facts.changes > 0 { "환승 {facts.changes}회" } else { "환승 없음" } }
            }
            div { class: "nv-bar",
                for (span, colour, badge, walked) in facts.segs.iter().cloned() {
                    div {
                        class: if walked { "nv-seg walk" } else { "nv-seg" },
                        style: "flex: {span.max(1) as f64} 1 0; --seg: {colour}",
                        if walked {
                            span { class: "nv-badge walk", Icon { name: "doc", size: 11.0 } }
                        } else {
                            span { class: "nv-badge", style: "background: {colour}", "{badge}" }
                        }
                        span { class: "nv-span", "{took(span)}" }
                    }
                }
            }
            // Each leg as a route finder lists it: the line on the left, where it was boarded on
            // the right; the words that told the agent where it changed.
            div { class: "nv-steps",
                for (n, leg) in facts.legs.iter().cloned().enumerate() {
                    {
                        let walked = matches!(leg.board, Board::Station(b) if Some(b) == by_seq);
                        let x = m.station(leg.alight).map_or(dest.x, |s| s.x);
                        let colour = if walked { TOLD } else { line_colour(m, leg.line, x) };
                        let line_name = if walked { "글을 따라".to_owned() } else { m.lines[leg.line].name.clone() };
                        let passed = facts.passed.get(n).copied().unwrap_or(0);
                        rsx! {
                            div { class: if walked { "nv-step walk" } else { "nv-step" }, style: "--line: {colour}",
                                div { class: "nv-line",
                                    if walked {
                                        span { class: "nv-badge walk", Icon { name: "doc", size: 11.0 } }
                                    } else {
                                        span { class: "nv-badge", style: "background: {colour}", "{m.badge(leg.line)}" }
                                    }
                                    span { class: "nv-line-name", "{line_name}" }
                                }
                                div { class: "nv-stop",
                                    match leg.board {
                                        Board::Ask(_) => rsx! {
                                            div { class: "nv-stop-head", b { "{asker}" } span { class: "nv-at", "{clock(ask.as_ref().and_then(|a| a.at))}" } }
                                            div { class: "nv-said", "{request}" }
                                            if let Some(intent) = ask.as_ref().and_then(|a| a.intent.clone()).filter(|i| !i.trim().is_empty()) {
                                                // What the agent wrote it would do this turn: what the verdict
                                                // measured the Call against.
                                                div { class: "nv-intent", span { class: "nv-intent-k", "에이전트가 적은 의도" } span { "{intent}" } }
                                            }
                                        },
                                        Board::Station(b) => {
                                            let bs = m.station(b).cloned();
                                            rsx! {
                                                if let Some(bs) = bs {
                                                    div {
                                                        class: "nv-stop-head link",
                                                        onclick: { let seq = bs.seq; move |_| focus.set(Some(seq)) },
                                                        onmouseenter: { let seq = bs.seq; move |_| hovered.set(Some(seq)) },
                                                        onmouseleave: move |_| hovered.set(None),
                                                        b { "{bs.cause_name()}" }
                                                        if walked { span { class: "nv-of", "{source_kind(&bs)}" } }
                                                        if walked && hot { span { class: "ln-flag", "여기서 바뀜" } }
                                                        span { class: "nv-at", "{clock(bs.at)}" }
                                                    }
                                                    if walked {
                                                        if let Some(q) = src.clone() {
                                                            // Keyed in a list, so another paragraph makes a new quote.
                                                            for q in [q] {
                                                                {
                                                                    let open = quote_open() == Some(dest.seq);
                                                                    let kind = if bs.action == crate::metro::Action::Run { "out" } else { "prose" };
                                                                    let cls = if open { format!("st-quote {kind}") } else { format!("st-quote {kind} folded") };
                                                                    let d = dest.seq;
                                                                    rsx! {
                                                                        blockquote { key: "{q.result}-{q.paragraph:?}", class: "{cls}",
                                                                            super::trace::Preview { seq: q.result, paragraph: q.paragraph, lines: true }
                                                                        }
                                                                        button {
                                                                            class: "nv-quote-toggle",
                                                                            onclick: move |_| quote_open.set(if open { None } else { Some(d) }),
                                                                            if open { "근거 글 접기" } else { "근거 글 펼치기" }
                                                                        }
                                                                    }
                                                                }
                                                                super::trace::ParagraphTags { seq: q.result, paragraph: q.paragraph }
                                                                div { class: "nv-meta",
                                                                    span { class: "cr-tool", "{bs.tool}" }
                                                                    span { class: "nv-src", title: "{bs.what}", "{bs.what}" }
                                                                }
                                                                div { class: "nv-meta faint",
                                                                    if let Some(p) = q.paragraph { "문단 {p} · " }
                                                                    "원인일 확률 {(q.probability * 100.0).round()}%"
                                                                }
                                                            }
                                                        }
                                                    } else {
                                                        div { class: "nv-meta faint", "여기서 {m.lines[leg.line].name}에게 맡김" }
                                                    }
                                                }
                                            }
                                        }
                                        Board::Nowhere => rsx! {
                                            div { class: "nv-stop-head", b { "시킨 것을 찾지 못함" } }
                                            div { class: "nv-meta faint", "세션 안의 요청에도 도구 결과에도 이어지지 않습니다. 세션 밖의 입력이거나 에이전트가 스스로 한 일일 수 있습니다." }
                                        },
                                    }
                                    if passed > 1 && !walked { div { class: "nv-pass", "{passed - 1}개 역 이동" } }
                                }
                            }
                        }
                    }
                }
                // Where it arrived: the Call, with the engine's verdict and its reason.
                div { class: "nv-step end {vtone}",
                    div { class: "nv-line",
                        span { class: "nv-end-dot {vtone}", Icon { name: verdict_icon(dest.decision.as_deref()), size: 14.0 } }
                        span { class: "nv-line-name", "도착" }
                    }
                    div { class: "nv-stop",
                        div { class: "crow big", CallLines { station: dest.clone(), title: dest.name(), full_time: true } }
                        if let Some(b) = branch.clone() {
                            div { class: "nv-branch", span { class: "nv-branch-k", "판정 방식" } span { "{b}" } }
                        }
                        if let Some((text, p)) = why.clone().filter(|_| dest.stopped()) {
                            div { class: "cr-reason", ReasonBox { tone: vtone, text, p } }
                        }
                    }
                }
            }
            div { class: "dp-foot",
                button {
                    class: if inspecting { "inspect-link on" } else { "inspect-link" },
                    onclick: move |_| state.inspect.toggle(),
                    if inspecting { "엔진 판정 닫기" } else { "엔진 판정 자세히 보기 ›" }
                }
            }
        }
    }
}

/// The part of a text worth quoting with its lines kept: from a hidden comment when there is
/// one, without blank lines or rules of dashes, cut to a few hundred characters.
pub(super) fn excerpt_lines(text: &str) -> String {
    let from = text.find("<!--").map_or(text, |i| &text[i..]);
    let kept: Vec<&str> = from
        .lines()
        .map(str::trim_end)
        .filter(|l| !l.trim().is_empty())
        .filter(|l| !l.trim().chars().all(|c| "-=_*~#".contains(c)))
        .collect();
    let joined = kept.join("\n");
    // Cut by characters, not by words, so the lines stay.
    if joined.chars().count() > 400 {
        format!("{}…", joined.chars().take(400).collect::<String>())
    } else {
        joined
    }
}

/// The part of a paragraph worth quoting: a hidden comment when there is one, else its start.
pub(super) fn excerpt(text: &str) -> String {
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    match flat.find("<!--") {
        Some(i) => flat[i..].to_owned(),
        None => flat,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quotes_start_at_a_hidden_comment() {
        assert_eq!(
            excerpt("# 제목\n안내\n<!-- AI 도우미에게:\n하세요 -->"),
            "<!-- AI 도우미에게: 하세요 -->"
        );
        assert_eq!(
            excerpt("tests/a.py:41:  AssertionError"),
            "tests/a.py:41: AssertionError"
        );
    }
}
