//! The map column's drawing: the session's metro map of what told the agent what to do
//! (`crate::network`), with the way to the Call being looked at laid over it.

use std::collections::HashSet;
use std::sync::Arc;

use dioxus::prelude::*;

use super::metro::{
    Camera, Side, StationMark, map_names, name_width, place_sub, turn_at_call, turn_colour,
    turn_marks, turn_pick,
};
use super::{AppState, Selection};
use crate::layout::clip;
use crate::metro::{Ask, Metro, Station, place_label};
use crate::network::{Net, Through};

/// A line's dot in a transfer: where the line passes the station, and its colour.
type Dot = (f64, f64, &'static str);

/// The colour of a line of words that told the agent to do something.
pub(super) const TEXT_LINE: &str = "#9b5fb0";

/// The class of a station's mark: one no line reaches stands apart.
fn ring_of(s: &crate::network::NetStation) -> &'static str {
    if s.lines.is_empty() {
        "on-lost"
    } else {
        "on-main"
    }
}

/// A line's colour: its turn's, or the words' colour.
pub(super) fn colour(net: &Net, line: usize) -> &'static str {
    match net.lines[line].through {
        Through::Turn(n) => turn_colour(n),
        Through::Told(_) => TEXT_LINE,
    }
}

/// What a line is called: the turn and who asked, or where its words were read.
pub(super) fn line_name(net: &Net, asks: &[Ask], line: usize) -> (String, String) {
    match net.lines[line].through {
        Through::Turn(n) => {
            let ask = asks.get(n);
            let said = match ask.and_then(|a| a.agent.clone()) {
                Some(agent) => format!("{} 보고", clip(&agent, 14)),
                None => clip(ask.map_or("", |a| a.text.as_str()), 60),
            };
            (format!("턴 {}", n + 1), said)
        }
        Through::Told(_) => {
            let by = net
                .source_call(line)
                .map(|s| s.cause_name())
                .unwrap_or_default();
            ("글".to_owned(), format!("{by}에 적힌 글"))
        }
    }
}

#[component]
pub(super) fn NetWorld(
    net: Arc<Net>,
    calls: Arc<Metro>,
    call: Option<i64>,
    cam: Camera,
    dragged: Signal<bool>,
    open: Signal<HashSet<i64>>,
    mut hovered: Signal<Option<i64>>,
) -> Element {
    let mut state = use_context::<AppState>();
    let n = &net;
    let k = cam.grow();
    let asks = calls.asks.clone();
    let dest = call.and_then(|c| n.station_of(c)).map(|s| s.rep.clone());
    let route = call.map(|c| n.route(c)).unwrap_or_default();
    let start = call.and_then(|c| n.start_of(c));
    let start_at = start.and_then(|t| {
        n.lines
            .iter()
            .find(|l| l.through == Through::Turn(t))
            .and_then(|l| l.points.first().copied())
    });
    // Transfers: stations more than one line runs through, each line's dot where it passes.
    let transfers: Vec<(usize, Vec<Dot>)> = n
        .stations
        .iter()
        .enumerate()
        .filter(|(_, s)| s.lines.len() > 1)
        .map(|(i, s)| {
            let dots = s
                .lines
                .iter()
                .filter_map(|&l| {
                    n.point_at(l, i)
                        .map(|p| (n.lines[l].points[p], colour(n, l)))
                })
                .map(|((x, y), c)| (x, y, c))
                .collect();
            (i, dots)
        })
        .collect();
    let forks: HashSet<i64> = transfers
        .iter()
        .map(|(i, _)| n.stations[*i].rep.seq)
        .collect();
    // How far a transfer's pill reaches over and under its station.
    let reach: std::collections::HashMap<i64, (f64, f64)> = transfers
        .iter()
        .map(|(i, dots)| {
            let y = n.stations[*i].y;
            let lo = dots.iter().map(|d| d.1).fold(y, f64::min);
            let hi = dots.iter().map(|d| d.1).fold(y, f64::max);
            (n.stations[*i].rep.seq, (y - lo + 2.0 * k, hi - y + 2.0 * k))
        })
        .collect();
    let reps: Vec<Station> = n.stations.iter().map(|s| s.rep.clone()).collect();
    let shown: Vec<i64> = reps
        .iter()
        .filter(|s| !place_label(s).is_empty())
        .map(|s| s.seq)
        .collect();
    // Room the names keep clear of: each line's name over its start, and the pins.
    let mut taken: Vec<(f64, f64, f64, f64)> = Vec::new();
    for (li, l) in n.lines.iter().enumerate() {
        let (tag, said) = line_name(n, &asks, li);
        if let (Through::Turn(t), Some(&(x, y))) = (l.through, l.points.first()) {
            let wide = name_width(&format!("{tag} {said}")) * k;
            let lead = if Some(t) == start { 30.0 } else { -6.0 } * k;
            taken.push((x + lead, x + lead + wide, y - 30.0 * k, y - 8.0 * k));
        }
    }
    if let Some((x, y)) = start_at {
        taken.push((x - 24.0 * k, x + 24.0 * k, y - 44.0 * k, y - 8.0 * k));
    }
    if let Some(d) = &dest {
        taken.push((
            d.x - 24.0 * k,
            d.x + 24.0 * k,
            d.y - 44.0 * k,
            d.y - 8.0 * k,
        ));
    }
    let names = map_names(
        &reps,
        dest.as_ref().map(|d| d.seq),
        &forks,
        k,
        &[],
        &shown,
        &HashSet::new(),
        &taken,
        &reach,
    );
    let height = n.bottom + 200.0;
    rsx! {
        div {
            class: "metro-world",
            style: "width: {n.width}px; height: {height}px; transform: translate({cam.x}px, {cam.y}px) scale({cam.zoom}); --k: {k}",
            svg { width: "{n.width}", height: "{height}",
                // The way to the Call being looked at, as a soft band under the lines it rides.
                for (li, a, b) in route.iter().copied() {
                    path { class: "route-glow", style: "stroke: {colour(n, li)}", d: "{n.stretch(li, a, b)}" }
                }
                for li in 0..n.lines.len() {
                    path { class: "casing", d: "{n.path(li)}" }
                }
                for li in 0..n.lines.len() {
                    path {
                        class: if matches!(n.lines[li].through, Through::Told(_)) { "track told-line" } else { "track main" },
                        style: "stroke: {colour(n, li)}",
                        d: "{n.path(li)}",
                    }
                }
                // Each turn's line sets out from its request: a terminal in the turn's colour.
                for (li, l) in n.lines.iter().enumerate() {
                    if let (Through::Turn(_), Some(&(x, y))) = (l.through, l.points.first()) {
                        circle { class: "terminus", style: "stroke: {colour(n, li)}", cx: "{x}", cy: "{y}", r: "{7.0 * k}" }
                    }
                    if let Some(&(x, y)) = l.points.last() {
                        path { class: "end-cap", style: "stroke: {colour(n, li)}", d: "M{x},{y - 7.0 * k} V{y + 7.0 * k}" }
                    }
                }
                // A transfer, as a metro map draws one: a pill of each line's dot.
                for (_, dots) in transfers.iter().cloned() {
                    {
                        let x = dots.iter().map(|d| d.0).sum::<f64>() / dots.len().max(1) as f64;
                        let y0 = dots.iter().map(|d| d.1).fold(f64::MAX, f64::min);
                        let y1 = dots.iter().map(|d| d.1).fold(f64::MIN, f64::max);
                        let r = 5.8 * k;
                        rsx! {
                            rect { class: "xchg-capsule", x: "{x - r}", y: "{y0 - r}", width: "{2.0 * r}", height: "{y1 - y0 + 2.0 * r}", rx: "{r}" }
                            for (dx, dy, c) in dots {
                                circle { class: "xchg-dot", style: "fill: {c}", cx: "{dx}", cy: "{dy}", r: "{3.0 * k}" }
                            }
                        }
                    }
                }
                for (i, s, ring) in n.stations.iter().enumerate().map(|(i, s)| (i, s, ring_of(s))) {
                    StationMark {
                        station: s.rep.clone(),
                        turn: s.lines.first().map_or("#9aa0a8", |&l| colour(n, l)),
                        ring,
                        fork: s.lines.len() > 1,
                        picked: dest.as_ref().is_some_and(|d| d.seq == s.rep.seq),
                        off: false,
                        capsule: s.lines.len() > 1 && transfers.iter().any(|(t, _)| *t == i),
                        k,
                        dragged,
                        open,
                        hovered,
                    }
                }
            }
            // Each line's name: a turn's at its terminal, the words' at their end.
            for (li, l) in n.lines.iter().enumerate() {
                {
                    let (tag, said) = line_name(n, &asks, li);
                    let c = colour(n, li);
                    let told = matches!(l.through, Through::Told(_));
                    let (x, y) = if told { l.points.last().copied().unwrap_or_default() } else { l.points.first().copied().unwrap_or_default() };
                    let ask_seq = match l.through { Through::Turn(t) => asks.get(t).map(|a| a.seq), Through::Told(_) => None };
                    let starts_here = matches!(l.through, Through::Turn(t) if Some(t) == start);
                    let place = if told { "translate(10px, -50%)" } else if starts_here { "translate(30px, calc(-100% - 12px))" } else { "translate(-8px, calc(-100% - 12px))" };
                    rsx! {
                        div {
                            class: if told { "line-tag told" } else { "line-tag" },
                            style: "left: {x}px; top: {y}px; --line: {c}; transform: scale({k}) {place}",
                            title: "{said}",
                            onclick: move |e| { e.stop_propagation(); if let Some(q) = ask_seq { state.selected.set(Selection::Node(q)); } },
                            b { "{tag}" }
                            span { "{said}" }
                        }
                    }
                }
            }
            for (s, side) in names.iter().cloned() {
                {
                    let sub = place_sub(&s);
                    let (up, down) = reach.get(&s.seq).map_or((0.0, 0.0), |r| (r.0 / k, r.1 / k));
                    let shift = match side {
                        Side::Under => format!("-50%, {}px", 11.0 + down),
                        Side::Over => format!("-50%, calc(-100% - {}px)", 11.0 + up),
                        Side::Right => "12px, -50%".to_owned(),
                        Side::Left => "calc(-100% - 12px), -50%".to_owned(),
                    };
                    let tone = match s.decision.as_deref() { Some("DENY") => " deny", Some("REVIEW") => " review", _ => "" };
                    let picked = if dest.as_ref().is_some_and(|d| d.seq == s.seq) { " picked" } else { "" };
                    let hub = if forks.contains(&s.seq) { " hub" } else { "" };
                    let seq = s.seq;
                    rsx! {
                        div {
                            class: "map-name{tone}{picked}{hub}",
                            style: "left: {s.x}px; top: {s.y}px; transform: scale({k}) translate({shift})",
                            title: "{s.what}",
                            onclick: move |e: MouseEvent| { e.stop_propagation(); state.selected.set(Selection::Node(seq)); },
                            span { class: "mn-main", "{place_label(&s)}" }
                            if !sub.is_empty() { span { class: "mn-sub", "{sub}" } }
                        }
                    }
                }
            }
            if let (Some(t), Some((x, y))) = (start, start_at) {
                div { class: "pin start", style: "left: {x}px; top: {y}px; --turn: {turn_colour(t)}; transform: scale({k}) translate(-50%, calc(-100% - 12px))", "출발" }
            }
            if let Some(d) = dest.clone() {
                {
                    let tone = match d.decision.as_deref() { Some("DENY") => "deny", Some("REVIEW") => "review", _ => "" };
                    rsx! {
                        div { class: "pin end {tone}", style: "left: {d.x}px; top: {d.y}px; transform: scale({k}) translate(-50%, calc(-100% - 15px))", "도착" }
                    }
                }
            }
            // The station under the pointer: where, how judged, how many Calls.
            if let Some(h) = hovered().and_then(|h| n.station_of(h)).filter(|s| dest.as_ref().is_none_or(|d| d.seq != s.rep.seq)).cloned() {
                {
                    let r = h.rep.clone();
                    let (tone, verdict) = match r.decision.as_deref() {
                        Some("DENY") => ("deny", "차단"),
                        Some("REVIEW") => ("review", "확인 요청"),
                        Some(_) => ("pass", "허용"),
                        None => ("quiet", "판정 없음"),
                    };
                    let dots: Vec<&'static str> = h.lines.iter().map(|&l| colour(n, l)).collect();
                    rsx! {
                        div { class: "balloon", style: "left: {r.x}px; top: {r.y}px; transform: scale({k}) translate(-50%, calc(-100% - 16px))",
                            div { class: "bl-head",
                                for c in dots { span { class: "bl-dot", style: "background: {c}" } }
                                b { "{place_label(&r)}" }
                                if h.calls.len() > 1 { span { class: "bl-n", "{h.calls.len()}번" } }
                                span { class: "chip {tone}", "{verdict}" }
                            }
                            div { class: "bl-what", "{r.tool} · {clip(&r.what, 60)}" }
                            div { class: "bl-foot", span { if h.lines.is_empty() { "시킨 것을 찾지 못함" } else { "눌러서 길찾기" } } }
                        }
                    }
                }
            }
        }
    }
}

/// The map's key: each line by its colour and name, then what the marks mean.
#[component]
pub(super) fn NetLegend(net: Arc<Net>, calls: Arc<Metro>) -> Element {
    let n = &net;
    let rows: Vec<(&'static str, String, String)> = (0..n.lines.len())
        .map(|li| {
            let (tag, said) = line_name(n, &calls.asks, li);
            (colour(n, li), tag, said)
        })
        .collect();
    rsx! {
        div { class: "map-legend", onmousedown: move |e| e.stop_propagation(),
            div { class: "lg-title", "노선 안내 · 선은 에이전트에게 시킨 것" }
            for (c, tag, said) in rows {
                div { class: "lg-row",
                    span { class: "lg-bar", span { style: "background: {c}" } }
                    span { class: "lg-tag", style: "background: {c}", "{tag}" }
                    span { class: "lg-said", "{said}" }
                }
            }
            div { class: "lg-marks",
                span { class: "lg",
                    svg { width: "12", height: "20", view_box: "-6 -10 12 20",
                        rect { class: "lg-pill", x: "-5", y: "-9", width: "10", height: "18", rx: "5" }
                        circle { cx: "0", cy: "-3.5", r: "2.6", style: "fill: {turn_colour(0)}" }
                        circle { cx: "0", cy: "3.5", r: "2.6", style: "fill: {TEXT_LINE}" }
                    }
                    "환승 · 두 지시가 지나간 곳"
                }
                span { class: "lg",
                    svg { width: "14", height: "14", view_box: "-8 -8 16 16",
                        path { class: "lg-deny", d: "M-2.8,-6.7 H2.8 L6.7,-2.8 V2.8 L2.8,6.7 H-2.8 L-6.7,2.8 V-2.8 Z" }
                    }
                    "차단"
                }
                span { class: "lg",
                    svg { width: "14", height: "14", view_box: "-8 -8 16 16",
                        path { class: "lg-review", "stroke-linejoin": "round", d: "M0,-7 L7.2,5 H-7.2 Z" }
                    }
                    "확인 요청"
                }
                span { class: "lg",
                    svg { width: "20", height: "14", view_box: "-10 -7 20 14", path { class: "lg-glow", d: "M-8,0 H8" } }
                    "고른 행동까지의 길"
                }
            }
        }
    }
}

/// The turns the map leaves out, folded over and under it, and every turn of the session down
/// the right edge as a bar in its verdict's colour: red where something was denied, amber where
/// held, grey otherwise. A fold or a bar picks its turn.
#[component]
pub(super) fn TurnRail(net: Arc<Net>, calls: Arc<Metro>, call: Option<i64>) -> Element {
    let mut state = use_context::<AppState>();
    let (first, last) = net.turns;
    let count = net.turn_count;
    let before = first;
    let after = count.saturating_sub(last + 1);
    let here = turn_at_call(&calls, call);
    let marks = turn_marks(&calls);
    let pick_before = (before > 0).then(|| turn_pick(&calls, first - 1)).flatten();
    let pick_after = (after > 0).then(|| turn_pick(&calls, last + 1)).flatten();
    let bars: Vec<(usize, &'static str, bool, bool, Option<i64>)> = marks
        .iter()
        .enumerate()
        .map(|(t, &w)| {
            let tone = match w {
                2 => "deny",
                1 => "review",
                _ => "plain",
            };
            (
                t,
                tone,
                t >= first && t <= last,
                t == here,
                turn_pick(&calls, t),
            )
        })
        .collect();
    rsx! {
        if before > 0 {
            button {
                class: "turn-fold up",
                onmousedown: move |e| e.stop_propagation(),
                onclick: move |_| if let Some(q) = pick_before { state.selected.set(Selection::Node(q)); },
                "이전 {before}개 턴 ▲"
            }
        }
        if after > 0 {
            button {
                class: "turn-fold down",
                onmousedown: move |e| e.stop_propagation(),
                onclick: move |_| if let Some(q) = pick_after { state.selected.set(Selection::Node(q)); },
                "다음 {after}개 턴 ▼"
            }
        }
        if count > 1 {
            div { class: "turn-rail", onmousedown: move |e| e.stop_propagation(),
                for (t, tone, shown, on, pick) in bars {
                    button {
                        class: format!("tr-bar {tone}{}{}", if shown { " shown" } else { "" }, if on { " on" } else { "" }),
                        title: "턴 {t + 1}",
                        onclick: move |_| if let Some(q) = pick { state.selected.set(Selection::Node(q)); },
                    }
                }
            }
        }
    }
}
