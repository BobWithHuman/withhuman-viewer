//! The session drawn as a metro map (see `crate::metro`): lines and stations in SVG, labels
//! and the quoted sources in HTML over them, panned by dragging or scrolling and zoomed with a
//! pinch or ⌘-scroll. A station is a Call; picking one keeps its way back to the request lit.

use std::collections::HashSet;
use std::rc::Rc;
use std::sync::Arc;

use dioxus::prelude::*;

use super::{AppState, Load, Selection};
use crate::api::Backend;
use crate::layout::clip;
use crate::metro::{Band, LEFT, LineKind, MAIN_Y, Metro, Route, Station};

/// Below this zoom labels get too small to read: a first view stops here.
const READABLE: f64 = 0.95;
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
    /// The whole map when it fits at a readable size. Otherwise a readable zoom that keeps the
    /// first stopped station in view, since that is what the map is opened for, and the start
    /// of the session when nothing stopped.
    fn first_view(metro: &Metro, width: f64, height: f64) -> Self {
        let fit = ((width - 60.0) / metro.width)
            .min((height - 60.0) / metro.height)
            .min(1.0);
        if fit >= READABLE {
            return Camera {
                zoom: fit,
                x: (width - metro.width * fit) / 2.0,
                y: (height - metro.height * fit) / 2.0,
            };
        }
        let zoom = ((width - 60.0) / metro.width).clamp(READABLE, 1.0);
        let start_x = 30.0 - (LEFT - 20.0) * zoom;
        let top_y = 30.0 - (MAIN_Y - 100.0) * zoom;
        let Some(stop) = metro.stations.iter().find(|s| s.stopped()) else {
            return Camera {
                zoom,
                x: start_x,
                y: top_y,
            };
        };
        // The stop sits right of the middle, with what led to it on its left; the view never
        // pans past the start or leaves the stop under the bottom edge.
        let x = (width * 0.62 - stop.x * zoom).min(start_x);
        let y = top_y.min(height - 150.0 - stop.y * zoom);
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
            div {
                class: "metro-world",
                style: "width: {m.width}px; height: {m.height}px; transform: translate({cam.x}px, {cam.y}px) scale({cam.zoom});",
                svg {
                    width: "{m.width}",
                    height: "{m.height}",
                    for b in m.bands.iter().cloned() {
                        BandMark { band: b.clone(), y: m.lines[b.line].y, lit: route.is_none() }
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
                    for a in m.asks.iter().cloned() {
                        circle {
                            class: match (route.is_some() && a.seq != m.asks[0].seq, a.agent.is_some()) {
                                (true, true) => "terminus agent dim",
                                (true, false) => "terminus dim",
                                (false, true) => "terminus agent",
                                (false, false) => "terminus",
                            },
                            cx: "{a.x}", cy: "{MAIN_Y}", r: "13",
                        }
                    }
                    for s in m.stations.iter().cloned() {
                        StationMark {
                            station: s.clone(),
                            ring: ring(&m, &s),
                            picked: picked == Some(s.seq),
                            lit: lit(&route, s.seq),
                            dragged,
                            open,
                        }
                    }
                }
                for (i, line) in m.lines.iter().enumerate() {
                    if line.kind == LineKind::Branch {
                        SourceQuote { metro: m.clone(), index: i, lit: route.as_ref().is_none_or(|r| r.lines.contains(&i)) }
                    }
                    if line.kind == LineKind::Loose {
                        div {
                            class: if route.as_ref().is_none_or(|r| r.lines.contains(&i)) { "loose-label" } else { "loose-label dim" },
                            style: "left: {m.station(line.stations[0]).map_or(0.0, |s| s.x) - 50.0}px; top: {line.y - 30.0}px",
                            "출처 없음 · 요청에도 이 세션의 도구 결과에도 이어지지 않음"
                        }
                    }
                }
                for (n, a) in m.asks.iter().cloned().enumerate() {
                    div {
                        class: if route.is_some() && n > 0 { "ask dim" } else { "ask" },
                        style: if n == 0 { format!("left: {}px; top: {}px; width: {}px", LEFT - 4.0, MAIN_Y + 26.0, crate::metro::FIRST - 40.0) } else { format!("left: {}px; top: {}px; width: 230px", a.x - 16.0, MAIN_Y + 26.0) },
                        onclick: move |e| { e.stop_propagation(); state.selected.set(Selection::Node(a.seq)); },
                        if let Some(agent) = a.agent.clone() {
                            div { class: "who agent", "에이전트 보고 · 사람이 쓴 요청이 아님" }
                            div { class: "say next agent", "{clip(&agent, 18)} 에이전트의 보고를 다음 턴으로 받음" }
                        } else {
                            div { class: "who", if n == 0 { "사용자 요청" } else { "다음 요청" } }
                            div { class: if n == 0 { "say" } else { "say next" }, "{a.text}" }
                        }
                        if let Some(intent) = a.intent.clone() {
                            div { class: "intent", "의도: {intent}" }
                        }
                    }
                }
                for b in m.bands.iter().cloned() {
                    div {
                        class: if route.is_some() { "band-label dim" } else { "band-label" },
                        style: if b.line == 0 { format!("left: {}px; top: {}px", b.x0 - 26.0, MAIN_Y + 26.0) } else { format!("left: {}px; top: {}px", b.x0 - 26.0, m.lines[b.line].y - 40.0) },
                        "{b.agent} 에이전트가 한 일"
                    }
                }
                for s in m.stations.iter().cloned() {
                    StationLabel { station: s.clone(), main: m.lines[s.line].kind == LineKind::Main, lit: lit(&route, s.seq), dragged }
                }
                if let Some((seq, word)) = m.last_word.clone() {
                    div {
                        class: if route.is_some() { "last-word dim" } else { "last-word" },
                        style: "left: {m.main_end() + 16.0}px; top: {MAIN_Y - 9.0}px",
                        onclick: move |e| { e.stop_propagation(); state.selected.set(Selection::Node(seq)); },
                        b { "끝" }
                        " · {clip(&word.replace(['*', '`', '#'], \"\"), 60)}"
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

/// The band behind a subagent's stations.
#[component]
fn BandMark(band: Band, y: f64, lit: bool) -> Element {
    let (x, w) = (band.x0 - 26.0, band.x1 - band.x0 + 52.0);
    rsx! {
        rect { class: if lit { "band" } else { "band dim" }, x: "{x}", y: "{y - 20.0}", width: "{w}", height: "40", rx: "20" }
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
    let end = line
        .stations
        .last()
        .and_then(|&s| metro.station(s))
        .map(|s| (s.x + 40.0, s.y));
    rsx! {
        g { class: if lit { "" } else { "dim" },
            path { class: "{class}", d: "{metro.path(index)}" }
            if let (LineKind::Branch, Some((x, y))) = (line.kind, end) {
                circle { class: "track-end", cx: "{x}", cy: "{y}", r: "4" }
            }
        }
    }
}

#[component]
fn StationMark(
    station: Station,
    ring: &'static str,
    picked: bool,
    lit: bool,
    dragged: Signal<bool>,
    open: Signal<HashSet<i64>>,
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
    let stop = match station.decision.as_deref() {
        Some("DENY") => Some("deny"),
        Some("REVIEW") => Some("review"),
        _ => None,
    };
    let undecided = station.decision.is_none();
    let count = station.members.len();
    rsx! {
        g { class: if lit { "station {ring}" } else { "station {ring} dim" }, onclick: select,
            // A larger, invisible target than the mark itself.
            circle { class: "hit", cx: "{cx}", cy: "{cy}", r: "20" }
            if let Some(kind) = stop {
                if picked { circle { class: "halo {kind}", cx: "{cx}", cy: "{cy}", r: "19" } }
                circle { class: "stop {kind}", cx: "{cx}", cy: "{cy}", r: "12" }
                if kind == "deny" {
                    path { class: "glyph", d: "M{cx - 4.0},{cy - 4.0} l8,8 M{cx + 4.0},{cy - 4.0} l-8,8" }
                } else {
                    path { class: "glyph", d: "M{cx},{cy - 5.0} v6 M{cx},{cy + 4.0} v0.5" }
                }
            } else if folded {
                rect { class: "pass fold", x: "{cx - 19.0}", y: "{cy - 9.0}", width: "38", height: "18", rx: "9" }
                text { class: "fold-count", x: "{cx}", y: "{cy + 4.0}", "×{count}" }
            } else {
                if picked { circle { class: "halo", cx: "{cx}", cy: "{cy}", r: "16" } }
                circle { class: if undecided { "pass undecided" } else { "pass" }, cx: "{cx}", cy: "{cy}", r: "8" }
                if station.irreversible {
                    circle { class: "undo", cx: "{cx}", cy: "{cy}", r: "15" }
                }
            }
            if station.failed {
                path { class: "fail", d: "M{cx - 7.0},{cy + 7.0} L{cx + 7.0},{cy - 7.0}" }
            }
        }
    }
}

#[component]
fn StationLabel(station: Station, main: bool, lit: bool, dragged: Signal<bool>) -> Element {
    let mut state = use_context::<AppState>();
    let seq = station.seq;
    let select = move |e: MouseEvent| {
        e.stop_propagation();
        if !dragged() {
            state.selected.set(Selection::Node(seq));
        }
    };
    let tone = match station.decision.as_deref() {
        Some("DENY") => "deny",
        Some("REVIEW") => "review",
        _ => "",
    };
    // One tag, by the same order as the marks: the decision, then how it went.
    let tag = match station.decision.as_deref() {
        Some("DENY") => Some("차단"),
        Some("REVIEW") => Some("확인 요청"),
        None => Some("판정 없음"),
        _ if station.irreversible => Some("되돌릴 수 없음 · 허용"),
        _ if station.failed => Some("실패"),
        _ if station.unsourced => Some("출처 결과를 찾지 못함"),
        _ => None,
    };
    let source = station.source.as_ref().map(|s| {
        let p = crate::layout::percent(s.probability);
        match s.paragraph {
            Some(i) => format!("¶{i} · 원인 {p}"),
            None => format!("원인 {p}"),
        }
    });
    let dim = if lit { "" } else { " dim" };
    // Main-line labels sit above the line, their verdict under the station, so the row stays
    // level; branch labels hang under their line.
    let (top, tag_top) = if main {
        (station.y - 52.0, station.y + 18.0)
    } else {
        (station.y + 20.0, 0.0)
    };
    rsx! {
        div { class: "label {tone}{dim}", style: "left: {station.x - 20.0}px; top: {top}px", onclick: select,
            div { class: "tool", if station.folded() { "{station.tool} ×{station.members.len()}" } else { "{station.tool}" } }
            div { class: "what", title: "{station.what}", "{station.what}" }
            if let Some(s) = source { div { class: "what source", "{s}" } }
            if !main { if let Some(t) = tag { div { class: "verdict", "{t}" } } }
        }
        if main {
            if let Some(t) = tag {
                div { class: match tag { Some("되돌릴 수 없음 · 허용") => format!("label undo{dim}"), Some("차단" | "확인 요청") => format!("label {tone}{dim}"), _ => format!("label quiet{dim}") },
                    style: "left: {station.x - 20.0}px; top: {tag_top}px",
                    div { class: "verdict", "{t}" }
                }
            }
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

/// A branch's source, quoted where it leaves its line: which result, which paragraph, and its
/// text, read from the result's own record.
#[component]
fn SourceQuote(metro: Arc<Metro>, index: usize, lit: bool) -> Element {
    let backend = use_context::<Arc<Backend>>();
    let mut state = use_context::<AppState>();
    let line = metro.lines[index].clone();
    let source = line.source.clone().expect("a branch has a source");
    let by = metro.station(source.by).cloned();
    let (result, paragraph) = (source.result, source.paragraph);
    let found = use_resource(move || {
        let backend = backend.clone();
        let session = state.current.peek().clone();
        async move {
            let detail = backend.node(&session?, result).await.ok().flatten()?;
            detail
                .paragraphs
                .iter()
                .find(|p| p.index == paragraph)
                .or_else(|| detail.paragraphs.first())
                .and_then(|p| p.text.clone())
        }
    });
    let text = match &*found.read() {
        Some(Some(t)) => excerpt(t),
        Some(None) => String::new(),
        None => "불러오는 중…".into(),
    };
    let x = by.as_ref().map_or(0.0, |s| s.x) + 30.0;
    let name = by.as_ref().map_or_else(String::new, |s| {
        if s.what.is_empty() {
            s.tool.clone()
        } else {
            s.what.clone()
        }
    });
    let mark = paragraph.map(|p| format!(" ¶{p}")).unwrap_or_default();
    let class = match (line.hot, lit) {
        (true, true) => "quote hot",
        (true, false) => "quote hot dim",
        (false, true) => "quote",
        (false, false) => "quote dim",
    };
    rsx! {
        div { class, style: "left: {x}px; top: {line.y - 84.0}px",
            onclick: move |e| { e.stop_propagation(); state.selected.set(Selection::Node(result)); },
            div { class: "src", "{name} 결과{mark}가 시킴" }
            if !text.is_empty() { div { class: "txt", "{text}" } }
        }
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
