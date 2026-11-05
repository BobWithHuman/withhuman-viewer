//! The session drawn as a metro map (see `crate::metro`): lines and stations in SVG, names in
//! HTML over them, a one-line summary on top. Read at a glance: the section names say what the
//! agent set out to do, the shapes what it did, the branches what told it, the red what stopped.
//! Panned by dragging or scrolling, zoomed with a pinch or ⌘-scroll; picking a station keeps
//! its way back to the request lit.

use std::collections::HashSet;
use std::rc::Rc;
use std::sync::Arc;

use dioxus::prelude::*;

use super::{AppState, Load, Selection};
use crate::layout::clip;
use crate::metro::{Action, Band, LEFT, LineKind, MAIN_Y, Metro, Route, Station, file_name};

/// Below this zoom names get too small to read: the first view shows the whole map down to here.
const OVERVIEW: f64 = 0.65;
const MAX_ZOOM: f64 = 2.0;
const MIN_ZOOM: f64 = 0.2;
/// A press that moves less than this is a click, not a drag.
const CLICK_SLOP: f64 = 4.0;

#[derive(Clone, Copy, Debug, PartialEq)]
struct Camera {
    x: f64,
    y: f64,
    zoom: f64,
}

impl Camera {
    /// The whole session at once, which is what the map is for, down to a zoom labels still
    /// read at. A map longer than that opens at that zoom on the first stopped station, or on
    /// the start when nothing stopped.
    fn first_view(metro: &Metro, width: f64, height: f64) -> Self {
        let fit = ((width - 60.0) / metro.width)
            .min((height - 120.0) / metro.height)
            .min(1.0);
        if fit >= OVERVIEW {
            return Camera {
                zoom: fit,
                x: (width - metro.width * fit) / 2.0,
                y: 86.0,
            };
        }
        let zoom = OVERVIEW;
        let start_x = 30.0 - (LEFT - 20.0) * zoom;
        let top_y = 86.0;
        let Some(stop) = metro.stations.iter().find(|s| s.stopped()) else {
            return Camera {
                zoom,
                x: start_x,
                y: top_y,
            };
        };
        let x = (width * 0.62 - stop.x * zoom).min(start_x);
        let y = top_y.min(height - 110.0 - stop.y * zoom);
        Camera { zoom, x, y }
    }

    /// Zooms by `factor` keeping the point under the pointer still.
    fn zoom_at(self, factor: f64, px: f64, py: f64) -> Self {
        let zoom = (self.zoom * factor).clamp(MIN_ZOOM, MAX_ZOOM);
        let k = zoom / self.zoom;
        Camera {
            zoom,
            x: px - (px - self.x) * k,
            y: py - (py - self.y) * k,
        }
    }

    /// Brings a point of the map to the middle of the canvas.
    fn center_on(self, x: f64, y: f64, width: f64, height: f64) -> Self {
        let zoom = self.zoom.max(0.9);
        Camera {
            zoom,
            x: width / 2.0 - x * zoom,
            y: height / 2.0 - y * zoom,
        }
    }
}

#[component]
pub fn MetroMap() -> Element {
    let mut state = use_context::<AppState>();
    // Folded runs the person opened, by their first Call; a new session starts folded.
    let mut open = use_signal(HashSet::<i64>::new);
    let metro = use_memo(move || match &*state.graph.read() {
        Load::Ready(model) => Some(Arc::new(Metro::build_with(model, &open.read()))),
        _ => None,
    });
    use_effect(move || {
        let _ = state.graph_of.read();
        open.set(HashSet::new());
    });
    // A Call picked elsewhere (the inspector, a notification) inside a folded run opens it.
    use_effect(move || {
        let Selection::Node(seq) = (state.selected)() else {
            return;
        };
        let Some(m) = metro.peek().clone() else {
            return;
        };
        if let Some(s) = m.station(seq).filter(|s| s.folded()) {
            open.write().insert(s.seq);
        }
    });
    let mut camera = use_signal(|| None::<Camera>);
    let mut size = use_signal(|| (0.0_f64, 0.0_f64));
    let mut mounted = use_signal(|| None::<Rc<MountedData>>);
    // A drag: where it started, the camera then, and whether it moved enough to count.
    let mut drag = use_signal(|| None::<(f64, f64, Camera, bool)>);
    let mut dragged = use_signal(|| false);
    let hovered = use_signal(|| None::<i64>);

    let measure = move || async move {
        let el = mounted.peek().clone();
        if let Some(el) = el
            && let Ok(rect) = el.get_client_rect().await
        {
            size.set((rect.width(), rect.height()));
        }
    };

    // A new session starts from its first view; a refresh of the same one keeps the camera.
    let drawn = use_signal(|| None::<Option<String>>);
    use_effect(move || {
        let Some(m) = metro() else { return };
        let session = state.graph_of.peek().clone();
        let mut drawn = drawn;
        let fresh = *drawn.peek() != Some(session.clone());
        drawn.set(Some(session));
        if fresh || camera.peek().is_none() {
            spawn(async move {
                measure().await;
                let (w, h) = *size.peek();
                camera.set(Some(Camera::first_view(&m, w, h)));
            });
        }
    });

    // "Fit" shows the whole map.
    use_effect(move || {
        if (state.fit)() == 0 {
            return;
        }
        let Some(m) = metro.peek().clone() else {
            return;
        };
        spawn(async move {
            measure().await;
            let (w, h) = *size.peek();
            let zoom = ((w - 60.0) / m.width)
                .min((h - 60.0) / m.height)
                .clamp(MIN_ZOOM, 1.0);
            camera.set(Some(Camera {
                zoom,
                x: (w - m.width * zoom) / 2.0,
                y: (h - m.height * zoom) / 2.0,
            }));
        });
    });

    // Focus waits for the open session's own map.
    use_effect(move || {
        let Some(seq) = (state.focus)() else { return };
        let Some(m) = metro() else { return };
        if *state.graph_of.read() != *state.current.read() {
            return;
        }
        state.focus.set(None);
        let Some(station) = m.station(seq).cloned() else {
            return;
        };
        spawn(async move {
            measure().await;
            let (w, h) = *size.peek();
            let current = *camera.peek();
            if let Some(c) = current {
                camera.set(Some(c.center_on(station.x, station.y, w, h)));
            }
        });
    });

    let Some(m) = metro() else {
        return rsx! { div { class: "metro", onmounted: move |e| mounted.set(Some(e.data())) } };
    };
    let cam = camera().unwrap_or(Camera {
        x: 0.0,
        y: 0.0,
        zoom: 1.0,
    });
    let picked = match (state.selected)() {
        Selection::Node(seq) => m.station(seq).map(|s| s.seq),
        _ => None,
    };
    let route = picked.map(|seq| m.route(seq));
    let forks: HashSet<i64> = m
        .lines
        .iter()
        .filter_map(|l| l.source.as_ref())
        .filter_map(|s| m.station(s.by).map(|st| st.seq))
        .collect();
    let notes = notes(&m);
    let hover = hovered().and_then(|seq| m.station(seq).cloned());

    rsx! {
        div {
            class: if drag().is_some_and(|d| d.3) { "metro dragging" } else { "metro" },
            onmounted: move |e| mounted.set(Some(e.data())),
            onwheel: move |e| {
                e.prevent_default();
                let current = *camera.peek();
                let Some(c) = current else { return };
                let at = e.element_coordinates();
                let delta = e.delta().strip_units();
                // A pinch arrives as a wheel with Ctrl; ⌘-scroll zooms too. Else it pans.
                if e.modifiers().ctrl() || e.modifiers().meta() {
                    camera.set(Some(c.zoom_at((-delta.y * 0.01).exp(), at.x, at.y)));
                } else {
                    camera.set(Some(Camera { x: c.x - delta.x, y: c.y - delta.y, ..c }));
                }
            },
            onmousedown: move |e| {
                let current = *camera.peek();
                if let Some(c) = current {
                    let p = e.client_coordinates();
                    drag.set(Some((p.x, p.y, c, false)));
                    dragged.set(false);
                }
            },
            onmousemove: move |e| {
                let current = *drag.peek();
                let Some((x0, y0, c, moved)) = current else { return };
                let p = e.client_coordinates();
                let (dx, dy) = (p.x - x0, p.y - y0);
                let moved = moved || dx.hypot(dy) > CLICK_SLOP;
                if moved {
                    camera.set(Some(Camera { x: c.x + dx, y: c.y + dy, ..c }));
                    drag.set(Some((x0, y0, c, true)));
                    dragged.set(true);
                }
            },
            onmouseup: move |_| drag.set(None),
            onmouseleave: move |_| drag.set(None),
            onclick: move |_| {
                if !dragged() {
                    state.selected.set(Selection::None);
                }
            },
            Summary { metro: m.clone() }
            div {
                class: "metro-world",
                style: "width: {m.width}px; height: {m.height}px; transform: translate({cam.x}px, {cam.y}px) scale({cam.zoom});",
                svg {
                    width: "{m.width}",
                    height: "{m.height}",
                    for b in m.bands.iter().cloned() {
                        BandMark { band: b.clone(), y: m.lines[b.line].y, lit: route.is_none() }
                    }
                    for sec in m.sections.iter().cloned() {
                        path {
                            class: if route.is_some() { "section-tick dim" } else { "section-tick" },
                            d: "M{sec.from - 10.0},{MAIN_Y - 26.0} V{MAIN_Y - 21.0} H{(sec.to + 10.0).max(sec.from + 24.0)} V{MAIN_Y - 26.0}",
                        }
                    }
                    for (x0, x1, agent) in m.main_segments() {
                        path {
                            class: if agent.is_some() { "track main agent" } else { "track main" },
                            d: "M{x0},{MAIN_Y} H{x1}",
                        }
                    }
                    for i in 1..m.lines.len() {
                        Track { metro: m.clone(), index: i, lit: route.as_ref().is_none_or(|r| r.lines.contains(&i)) }
                    }
                    for (n, a) in m.asks.iter().cloned().enumerate() {
                        circle {
                            class: format!(
                                "terminus{}{}",
                                if a.agent.is_some() { " agent" } else { "" },
                                if route.is_some() && n > 0 { " dim" } else { "" },
                            ),
                            cx: "{a.x}", cy: "{MAIN_Y}", r: "10",
                        }
                    }
                    for s in m.stations.iter().cloned() {
                        StationMark {
                            station: s.clone(),
                            ring: ring(&m, &s),
                            fork: forks.contains(&s.seq),
                            picked: picked == Some(s.seq),
                            lit: lit(&route, s.seq),
                            dragged,
                            open,
                            hovered,
                        }
                    }
                }
                // What the agent said it would do, over where it did it.
                for sec in m.sections.iter().cloned() {
                    div {
                        class: if route.is_some() { "section-name dim" } else { "section-name" },
                        style: "left: {sec.at}px; bottom: calc(100% - {MAIN_Y - 30.0}px); max-width: {crate::metro::NAME_WIDTH}px",
                        onclick: move |e| { e.stop_propagation(); state.selected.set(Selection::Node(sec.seq)); },
                        "{sec.text}"
                    }
                }
                // A branch is named by where its instruction came from.
                for (i, line) in m.lines.iter().enumerate() {
                    if line.kind == LineKind::Branch {
                        BranchName { metro: m.clone(), index: i, lit: route.as_ref().is_none_or(|r| r.lines.contains(&i)) }
                    }
                    if line.kind == LineKind::Loose {
                        div {
                            class: if route.as_ref().is_none_or(|r| r.lines.contains(&i)) { "line-name loose" } else { "line-name loose dim" },
                            style: "left: {m.station(line.stations[0]).map_or(0.0, |s| s.x) - 30.0}px; top: {line.y - 26.0}px",
                            "출처 없음"
                        }
                    }
                }
                for (n, a) in m.asks.iter().cloned().enumerate().skip(1) {
                    div {
                        class: format!("turn-name{}{}", if a.agent.is_some() { " agent" } else { "" }, if route.is_some() && n > 0 { " dim" } else { "" }),
                        style: "left: {a.x - 10.0}px; top: {MAIN_Y + 16.0}px",
                        onclick: move |e| { e.stop_propagation(); state.selected.set(Selection::Node(a.seq)); },
                        if let Some(agent) = a.agent.clone() {
                            b { "에이전트 보고" }
                            span { "{clip(&agent, 14)} · 사람이 아님" }
                        } else {
                            b { "다음 요청" }
                            span { "{clip(&a.text, 18)}" }
                        }
                    }
                }
                // Names only where something happened; any other on hover.
                for (s, lower) in notes {
                    StationNote { station: s.clone(), lower, lit: lit(&route, s.seq), dragged }
                }
                if let Some(s) = hover.filter(|s| !is_notable(s)) {
                    div {
                        class: "hover-name",
                        style: "left: {s.x - 10.0}px; top: {s.y - 34.0}px",
                        b { "{s.tool}" }
                        " {s.what}"
                    }
                }
            }
        }
    }
}

fn lit(route: &Option<Route>, seq: i64) -> bool {
    route.as_ref().is_none_or(|r| r.stations.contains(&seq))
}

/// The class of the line a station rides, for its ring: an allowed station wears its line's colour.
fn ring(m: &Metro, s: &Station) -> &'static str {
    match m.lines[s.line].kind {
        LineKind::Branch => "on-branch",
        LineKind::Loose => "on-loose",
        LineKind::Main if m.turn_agent(s.x).is_some() => "on-agent",
        LineKind::Main => "on-main",
    }
}

/// Something happened here worth naming without a hover.
fn is_notable(s: &Station) -> bool {
    s.stopped() || s.decision.is_none() || s.irreversible || s.failed || s.unsourced || s.folded()
}

/// The stations named on the map, each with whether it drops a line to clear the name before it.
fn notes(m: &Metro) -> Vec<(Station, bool)> {
    let mut last: std::collections::HashMap<usize, (f64, bool)> = std::collections::HashMap::new();
    let mut out = Vec::new();
    for s in m.stations.iter().filter(|s| is_notable(s)) {
        let lower = last
            .get(&s.line)
            .is_some_and(|(x, low)| s.x - x < 150.0 && !low);
        last.insert(s.line, (s.x, lower));
        out.push((s.clone(), lower));
    }
    out
}

/// One line over the map saying what the session did.
#[component]
fn Summary(metro: Arc<Metro>) -> Element {
    let state = use_context::<AppState>();
    let calls: usize = metro.stations.iter().map(|s| s.members.len()).sum();
    let branches = metro
        .lines
        .iter()
        .filter(|l| l.kind == LineKind::Branch)
        .count();
    let loose: usize = metro
        .lines
        .iter()
        .filter(|l| l.kind == LineKind::Loose)
        .map(|l| l.stations.len())
        .sum();
    let stops = metro.stations.iter().filter(|s| s.stopped()).count();
    let undo = metro
        .stations
        .iter()
        .filter(|s| s.irreversible && !s.stopped())
        .count();
    let ask = metro
        .asks
        .first()
        .map(|a| a.text.clone())
        .unwrap_or_default();
    let _ = state;
    rsx! {
        div { class: "map-summary", onmousedown: move |e| e.stop_propagation(),
            div { class: "summary-ask", "{clip(&ask, 80)}" }
            div { class: "counts",
                span { b { "{calls}" } "번 도구 호출" }
                if branches > 0 { span { class: "x", b { "{branches}" } "곳의 도구 결과가 지시" } }
                if loose > 0 { span { b { "{loose}" } "번 출처 없음" } }
                span { class: if stops > 0 { "s" } else { "" }, b { "{stops}" } "번 멈춤" }
                if undo > 0 { span { class: "u", b { "{undo}" } "번 되돌릴 수 없는 작업 허용" } }
            }
        }
    }
}

/// The band behind a subagent's stations, and its name.
#[component]
fn BandMark(band: Band, y: f64, lit: bool) -> Element {
    let (x, w) = (band.x0 - 18.0, band.x1 - band.x0 + 36.0);
    rsx! {
        rect { class: if lit { "band" } else { "band dim" }, x: "{x}", y: "{y - 16.0}", width: "{w}", height: "32", rx: "16" }
    }
}

#[component]
fn Track(metro: Arc<Metro>, index: usize, lit: bool) -> Element {
    let line = &metro.lines[index];
    let class = match line.kind {
        LineKind::Main => "track main",
        LineKind::Branch => "track branch",
        LineKind::Loose => "track loose",
    };
    rsx! {
        g { class: if lit { "" } else { "dim" },
            path { class: "{class}", d: "{metro.path(index)}" }
        }
    }
}

/// The shape of an action, centred on (x, y): circle, square, triangle, diamond or hexagon.
fn shape(action: Action, x: f64, y: f64, r: f64) -> String {
    match action {
        Action::Read | Action::Other => format!(
            "M{},{y} a{r},{r} 0 1,0 {},0 a{r},{r} 0 1,0 {},0",
            x - r,
            2.0 * r,
            -2.0 * r
        ),
        Action::Change => {
            let h = r * 0.9;
            format!(
                "M{},{} h{} v{} h{} z",
                x - h,
                y - h,
                2.0 * h,
                2.0 * h,
                -2.0 * h
            )
        }
        Action::Run => {
            let k = r * 1.15;
            format!(
                "M{x},{} L{},{} L{},{} z",
                y - k,
                x + k,
                y + k * 0.75,
                x - k,
                y + k * 0.75
            )
        }
        Action::Search => {
            let k = r * 1.2;
            format!(
                "M{x},{} L{},{y} L{x},{} L{},{y} z",
                y - k,
                x + k,
                y + k,
                x - k
            )
        }
        Action::Delegate => {
            let k = r * 1.1;
            let pts: Vec<String> = (0..6)
                .map(|i| {
                    let a = std::f64::consts::PI / 3.0 * i as f64;
                    format!("{},{}", x + k * a.cos(), y + k * a.sin())
                })
                .collect();
            format!("M{} z", pts.join(" L"))
        }
    }
}

#[component]
#[allow(clippy::too_many_arguments)]
fn StationMark(
    station: Station,
    ring: &'static str,
    fork: bool,
    picked: bool,
    lit: bool,
    dragged: Signal<bool>,
    open: Signal<HashSet<i64>>,
    hovered: Signal<Option<i64>>,
) -> Element {
    let mut state = use_context::<AppState>();
    let (cx, cy, seq) = (station.x, station.y, station.seq);
    let folded = station.folded();
    // A folded station opens; any other is picked.
    let select = move |e: MouseEvent| {
        e.stop_propagation();
        if dragged() {
            return;
        }
        if folded {
            open.write().insert(seq);
        } else {
            state.selected.set(Selection::Node(seq));
        }
    };
    let fill = match station.decision.as_deref() {
        Some("DENY") => "mark deny",
        Some("REVIEW") => "mark review",
        None => "mark undecided",
        _ => "mark pass",
    };
    let stop = station.stopped();
    let r = if stop { 9.0 } else { 6.5 };
    let count = station.members.len();
    rsx! {
        g {
            class: if lit { "station {ring}" } else { "station {ring} dim" },
            onclick: select,
            onmouseenter: move |_| hovered.set(Some(seq)),
            onmouseleave: move |_| hovered.set(None),
            // A larger, invisible target than the mark itself.
            circle { class: "hit", cx: "{cx}", cy: "{cy}", r: "16" }
            if picked { circle { class: if stop { "halo stopped" } else { "halo" }, cx: "{cx}", cy: "{cy}", r: "{r + 7.0}" } }
            // Where a branch leaves: an interchange, ringed in the branch's colour.
            if fork && !folded { circle { class: "interchange", cx: "{cx}", cy: "{cy}", r: "{r + 4.5}" } }
            if folded {
                rect { class: "mark pass fold", x: "{cx - 17.0}", y: "{cy - 8.0}", width: "34", height: "16", rx: "8" }
                text { class: "fold-count", x: "{cx}", y: "{cy + 3.5}", "×{count}" }
            } else {
                path { class: "{fill}", d: "{shape(station.action, cx, cy, r)}" }
                if station.decision.as_deref() == Some("DENY") {
                    path { class: "glyph", d: "M{cx - 3.2},{cy - 3.2} l6.4,6.4 M{cx + 3.2},{cy - 3.2} l-6.4,6.4" }
                }
                if station.decision.as_deref() == Some("REVIEW") {
                    path { class: "glyph", d: "M{cx},{cy - 4.0} v4.5 M{cx},{cy + 3.4} v0.4" }
                }
                if station.irreversible {
                    circle { class: "undo", cx: "{cx}", cy: "{cy}", r: "{r + 5.0}" }
                }
            }
            if station.failed {
                path { class: "fail", d: "M{cx - 7.0},{cy + 7.0} L{cx + 7.0},{cy - 7.0}" }
            }
        }
    }
}

/// A named station: what happened, then the tool and file, under the station.
#[component]
fn StationNote(station: Station, lower: bool, lit: bool, dragged: Signal<bool>) -> Element {
    let mut state = use_context::<AppState>();
    let seq = station.seq;
    let (tone, head) = match station.decision.as_deref() {
        Some("DENY") => ("deny", "차단"),
        Some("REVIEW") => ("review", "확인"),
        None => ("quiet", "판정 없음"),
        _ if station.irreversible => ("undo", "되돌릴 수 없음"),
        _ if station.failed => ("quiet", "실패"),
        _ if station.unsourced => ("quiet", "출처 결과 없음"),
        _ => ("quiet", ""),
    };
    let what = if station.folded() {
        format!(
            "{} ×{} · {}",
            station.tool,
            station.members.len(),
            station.what
        )
    } else {
        format!("{} {}", station.tool, file_name(&station.what))
    };
    let top = station.y + if lower { 38.0 } else { 16.0 };
    rsx! {
        div {
            class: if lit { format!("note {tone}") } else { format!("note {tone} dim") },
            style: "left: {station.x - 10.0}px; top: {top}px",
            onclick: move |e| {
                e.stop_propagation();
                if !dragged() {
                    state.selected.set(Selection::Node(seq));
                }
            },
            if !head.is_empty() { b { "{head}" } " · " }
            "{what}"
        }
    }
}

/// A branch's name where it turns into its row: the file or tool whose result told the agent.
#[component]
fn BranchName(metro: Arc<Metro>, index: usize, lit: bool) -> Element {
    let mut state = use_context::<AppState>();
    let line = metro.lines[index].clone();
    let source = line.source.clone().expect("a branch has a source");
    let x = metro.station(source.by).map_or(0.0, |s| s.x) + crate::metro::CHAMFER + 8.0;
    let result = source.result;
    let class = match (line.hot, lit) {
        (true, true) => "line-name branch hot",
        (true, false) => "line-name branch hot dim",
        (false, true) => "line-name branch",
        (false, false) => "line-name branch dim",
    };
    rsx! {
        div { class, style: "left: {x}px; top: {line.y - 27.0}px",
            onclick: move |e| { e.stop_propagation(); state.selected.set(Selection::Node(result)); },
            "{line.name}"
            if line.hot { " · 멈춤" }
        }
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

    #[test]
    fn a_first_view_keeps_the_first_stop_in_sight() {
        let graph = crate::demo::Demo::load();
        let id = graph
            .sessions()
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|s| s["id"].as_str())
            .find(|id| id.starts_with("b6380b32"))
            .unwrap()
            .to_owned();
        let model = crate::layout::Model::build(
            &serde_json::from_value(graph.graph(&id).unwrap()).unwrap(),
        );
        let m = Metro::build(&model);
        let (w, h) = (1000.0, 700.0);
        let c = Camera::first_view(&m, w, h);
        let stop = m.stations.iter().find(|s| s.stopped()).unwrap();
        let (sx, sy) = (stop.x * c.zoom + c.x, stop.y * c.zoom + c.y);
        assert!(
            (0.0..w).contains(&sx) && (0.0..h).contains(&sy),
            "stop at {sx},{sy}"
        );
    }

    #[test]
    fn zoom_keeps_the_point_under_the_pointer() {
        let c = Camera {
            x: 10.0,
            y: 20.0,
            zoom: 1.0,
        }
        .zoom_at(2.0, 110.0, 120.0);
        // The map point under (110, 120) was (100, 100) and stays there.
        assert_eq!(
            ((110.0 - c.x) / c.zoom, (120.0 - c.y) / c.zoom),
            (100.0, 100.0)
        );
    }
}
