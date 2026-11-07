//! The session drawn as a metro map (see `crate::metro`): lines and stations in SVG, names in
//! HTML over them, a one-line summary on top. Read at a glance: the section names say what the
//! agent set out to do, the shapes what it did, the branches what told it, the red what stopped.
//! Panned by dragging or scrolling, zoomed with a pinch or ⌘-scroll; picking a station keeps
//! its way back to the request lit.

use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::Arc;

use dioxus::prelude::*;

use super::{AppState, Load, Selection};
use crate::layout::clip;
use crate::metro::{Board, LineKind, Metro, Station};

const MAX_ZOOM: f64 = 2.0;
pub(super) const MIN_ZOOM: f64 = 0.2;
/// The most a whole-map view enlarges a small session.
const FIT_ZOOM: f64 = 1.2;
/// The canvas above the map that the summary takes, and below it that the legend takes.
pub(super) const SUMMARY_H: f64 = 130.0;
pub(super) const LEGEND_H: f64 = 70.0;
/// The route panel on the left of the canvas, and the room the map leaves it.
const PANEL_ROOM: f64 = 0.0;
/// The room the main line's badge takes left of its first terminal.
const BADGE_ROOM: f64 = 30.0;
/// The width the zoom buttons and the turn rail take at the canvas's right edge.
const CONTROLS: f64 = 110.0;
/// The scale a map app keeps in its corner, in stations: a round count whose length on
/// screen is between 60 and 150 pixels.
fn scale(zoom: f64) -> (usize, f64) {
    let step = crate::metro::STEP * zoom;
    [1usize, 2, 5, 10, 20, 50, 100]
        .into_iter()
        .map(|n| (n, n as f64 * step))
        .find(|&(_, w)| w >= 60.0)
        .unwrap_or((100, 100.0 * step))
}

/// The room on screen the map keeps over the main line for the turn band and the names, and
/// under the lowest lane for its names: screen pixels, as those are drawn at their own size.
pub(super) const OVER: f64 = 112.0;
pub(super) const UNDER: f64 = 44.0;
/// A press that moves less than this is a click, not a drag.
pub(super) const CLICK_SLOP: f64 = 4.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Camera {
    pub(super) x: f64,
    pub(super) y: f64,
    pub(super) zoom: f64,
}

impl Camera {
    /// The whole network, centred in the room between the summary and the legend.
    fn fit_net(net: &crate::network::Net, width: f64, height: f64) -> Self {
        Camera::fit_box(net.width, net.top, net.bottom, width, height)
    }

    /// A map `map_w` wide whose lines run from `top` to `bottom`, centred in the room between
    /// the summary and the legend of a `width` × `height` canvas, with the names' room kept
    /// over and under the lines.
    pub(super) fn fit_box(map_w: f64, top: f64, bottom: f64, width: f64, height: f64) -> Self {
        let drawn = bottom - top;
        let room = (height - SUMMARY_H - LEGEND_H - OVER - UNDER).max(40.0);
        // The zoom buttons keep the right edge.
        let free = (width - PANEL_ROOM - CONTROLS).max(320.0);
        let zoom = ((free - 40.0 - BADGE_ROOM) / map_w)
            .min(if drawn > 0.0 {
                room / drawn
            } else {
                f64::INFINITY
            })
            .clamp(MIN_ZOOM, FIT_ZOOM);
        Camera {
            zoom,
            x: PANEL_ROOM + BADGE_ROOM + (free - BADGE_ROOM - map_w * zoom) / 2.0,
            y: SUMMARY_H + OVER + ((room - drawn * zoom) / 2.0).max(0.0) - top * zoom,
        }
    }

    /// How much larger than the map's own scale marks and names are drawn so they keep their
    /// size on screen when the map is shrunk to fit: 1 at full size and over.
    pub(super) fn grow(self) -> f64 {
        1.0 / self.zoom.min(1.0)
    }

    /// Whether a point of the map is in view, with a margin.
    fn shows(self, x: f64, y: f64, width: f64, height: f64) -> bool {
        let (sx, sy) = (x * self.zoom + self.x, y * self.zoom + self.y);
        (60.0..width - 60.0).contains(&sx) && (40.0..height - 40.0).contains(&sy)
    }

    /// Zooms by `factor` keeping the point under the pointer still.
    pub(super) fn zoom_at(self, factor: f64, px: f64, py: f64) -> Self {
        let zoom = (self.zoom * factor).clamp(MIN_ZOOM, MAX_ZOOM);
        let k = zoom / self.zoom;
        Camera {
            zoom,
            x: px - (px - self.x) * k,
            y: py - (py - self.y) * k,
        }
    }

    /// Brings a point of the map to the middle of the room the panel and the signboard leave.
    pub(super) fn center_on(self, x: f64, y: f64, width: f64, height: f64) -> Self {
        let zoom = self.zoom;
        let free = (width - PANEL_ROOM).max(320.0);
        Camera {
            zoom,
            x: PANEL_ROOM + free / 2.0 - x * zoom,
            y: (height - LEGEND_H) / 2.0 - y * zoom,
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
    // The map draws the places the agents went to on the lines of what told them to.
    // Only the turn being looked at and its neighbours, however many turns the session has.
    let net = use_memo(move || {
        let Load::Ready(model) = &*state.graph.read() else {
            return None;
        };
        let cm = metro()?;
        let call = match (state.selected)() {
            Selection::Node(seq) => cm.station(seq).map(|_| seq),
            Selection::None => None,
        }
        .or_else(|| cm.default_dest());
        let t = turn_at_call(&cm, call);
        let last = cm.asks.len().saturating_sub(1);
        let window = (t.saturating_sub(WINDOW), (t + WINDOW).min(last));
        Some(Arc::new(crate::network::Net::window(model, Some(window))))
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
    // 지도 or 노선도: the land under the lines, or the lines alone.
    let plain = use_signal(|| true);

    let measure = move || async move {
        let el = mounted.peek().clone();
        if let Some(el) = el
            && let Ok(rect) = el.get_client_rect().await
        {
            size.set((rect.width(), rect.height()));
        }
    };

    // A new window of turns is a new map: fit it.
    let fitted = use_signal(|| None::<(usize, usize)>);
    use_effect(move || {
        let Some(n) = net() else { return };
        let mut fitted = fitted;
        if *fitted.peek() == Some(n.turns) {
            return;
        }
        let first = fitted.peek().is_none();
        fitted.set(Some(n.turns));
        if first {
            return;
        }
        spawn(async move {
            measure().await;
            let (w, h) = *size.peek();
            camera.set(Some(Camera::fit_net(&n, w, h)));
        });
    });

    // A new session starts from its first view; a refresh of the same one keeps the camera.
    // The detail column: open on a route, closed with its ✕ while the list stays.
    let mut detail = use_signal(|| true);
    let drawn = use_signal(|| None::<Option<String>>);
    use_effect(move || {
        let Some(m) = metro() else { return };
        let session = state.graph_of.peek().clone();
        let mut drawn = drawn;
        let fresh = *drawn.peek() != Some(session.clone());
        drawn.set(Some(session));
        if fresh {
            detail.set(true);
        }
        // A stopped session opens on its first stop, as the Call being looked at.
        if fresh
            && *state.selected.peek() == Selection::None
            && let Some(seq) = m.default_dest()
        {
            state.selected.set(Selection::Node(seq));
        }
        if fresh || camera.peek().is_none() {
            let Some(p) = net() else { return };
            spawn(async move {
                measure().await;
                let (w, h) = *size.peek();
                camera.set(Some(Camera::fit_net(&p, w, h)));
            });
        }
    });

    // A new destination off the screen is brought into view; one on it stays where it is.
    let framed = use_signal(|| None::<i64>);
    use_effect(move || {
        let selected = (state.selected)();
        let (Some(m), Some(cm)) = (net.peek().clone(), metro.peek().clone()) else {
            return;
        };
        let dest = match selected {
            Selection::Node(seq) => m.station_of(seq).map(|_| seq),
            Selection::None => None,
        }
        .or_else(|| cm.default_dest());
        let mut framed = framed;
        if dest.is_none() || *framed.peek() == dest {
            return;
        }
        framed.set(dest);
        let Some(dest) = dest else { return };
        spawn(async move {
            measure().await;
            let (w, h) = *size.peek();
            let current = *camera.peek();
            if let (Some(c), Some(st)) = (current, m.station_of(dest))
                && w > 0.0
                && !c.shows(st.x, st.y, w, h)
            {
                camera.set(Some(c.center_on(st.x, st.y, w, h)));
            }
        });
    });

    // "Fit" shows the whole map.
    use_effect(move || {
        if (state.fit)() == 0 {
            return;
        }
        let Some(m) = net.peek().clone() else {
            return;
        };
        spawn(async move {
            measure().await;
            let (w, h) = *size.peek();
            camera.set(Some(Camera::fit_net(&m, w, h)));
        });
    });

    // Focus waits for the open session's own map.
    use_effect(move || {
        let Some(seq) = (state.focus)() else { return };
        let Some(m) = net() else { return };
        if *state.graph_of.read() != *state.current.read() {
            return;
        }
        state.focus.set(None);
        let Some(station) = m.station_of(seq).cloned() else {
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

    let (Some(cm), Some(nm)) = (metro(), net()) else {
        return rsx! { div { class: "metro", onmounted: move |e| mounted.set(Some(e.data())) } };
    };
    let cam = camera().unwrap_or(Camera {
        x: 0.0,
        y: 0.0,
        zoom: 1.0,
    });
    let picked = match (state.selected)() {
        Selection::Node(seq) => cm.station(seq).map(|_| seq),
        _ => None,
    };
    // The Call being looked at (the one picked, else the first that stopped), and on the map
    // the station it was made at.
    let call = picked.or_else(|| cm.default_dest());
    let mut zoom_by = move |factor: f64| {
        let (w, h) = *size.peek();
        let current = *camera.peek();
        if let Some(c) = current {
            let cx = PANEL_ROOM + (w - PANEL_ROOM).max(320.0) / 2.0;
            camera.set(Some(c.zoom_at(factor, cx, (h - LEGEND_H) / 2.0)));
        }
    };

    rsx! {
        div { class: "routes",
        RouteList { metro: cm.clone(), dest: call, detail }
        if detail() && call.is_some() {
            RoutePanel { metro: cm.clone(), dest: call, hovered, detail }
        }
        div {
            class: format!(
                "metro{}{}{}",
                if drag().is_some_and(|d| d.3) { " dragging" } else { "" },
                "",
                if plain() { " plain" } else { " landed" },
            ),
            onmounted: move |e| mounted.set(Some(e.data())),
            // A window resized fits the whole session again in the room it now has.
            onresize: move |e| {
                let Ok(box_size) = e.get_border_box_size() else { return };
                let (w, h) = (box_size.width, box_size.height);
                let (w0, h0) = *size.peek();
                if (w - w0).abs() < 1.0 && (h - h0).abs() < 1.0 {
                    return;
                }
                size.set((w, h));
                let Some(m) = net.peek().clone() else { return };
                if w0 > 0.0 {
                    camera.set(Some(Camera::fit_net(&m, w, h)));
                }
            },
            // ← and → step the destination along its line, as the station sign's arrows do.
            tabindex: "0",
            onkeydown: move |e| {
                let step = match e.key() {
                    Key::ArrowLeft => 0,
                    Key::ArrowRight => 1,
                    _ => return,
                };
                let Some(m) = metro.peek().clone() else { return };
                let current = match *state.selected.peek() {
                    Selection::Node(seq) => m.station(seq).map(|s| s.seq),
                    Selection::None => None,
                }
                .or_else(|| m.default_dest());
                let Some(here) = current.and_then(|d| m.station(d)).cloned() else { return };
                let (prev, next) = neighbours(&m, &here);
                if let Some(seq) = if step == 0 { prev } else { next } {
                    e.prevent_default();
                    state.selected.set(Selection::Node(seq));
                }
            },
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
            super::net::NetWorld { net: nm.clone(), calls: cm.clone(), call, cam, dragged, open, hovered }
            super::net::NetLegend { net: nm.clone(), calls: cm.clone() }
            super::net::TurnRail { net: nm.clone(), calls: cm.clone(), call }
            {
                let (n, w) = scale(cam.zoom);
                rsx! {
                    div { class: "scalebar",
                        span { class: "sb-brand", "WITHHUMAN" }
                        span { class: "sb-n", "역 {n}개" }
                        span { class: "sb-bar", style: "width: {w}px" }
                    }
                }
            }
            div { class: "zoomctl", onmousedown: move |e| e.stop_propagation(),
                button { title: "확대", onclick: move |_| zoom_by(1.25), "+" }
                button { title: "축소", onclick: move |_| zoom_by(0.8), "−" }
                button { class: "fit", title: "전체 보기", onclick: move |_| state.fit += 1, "전체" }
            }
        }
        }
    }
}

/// The small line under a station's name, as Seoul's map writes it in English: where the file
/// is, or what ran.
pub(super) fn place_sub(s: &Station) -> String {
    use crate::metro::Action;
    // A station of several places names them all; one tool or folder would mislead.
    if crate::metro::place_label(s).contains(" · ") {
        return String::new();
    }
    match s.action {
        Action::Read | Action::Change if s.what.contains('/') && !s.what.contains("://") => {
            let dir = s
                .what
                .rsplit_once('/')
                .map_or("", |(d, _)| d)
                .trim_start_matches("./");
            let dir = dir
                .rsplit('/')
                .take(2)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect::<Vec<_>>()
                .join("/");
            clip(&dir, 22)
        }
        Action::Read if s.what.contains("://") => s
            .what
            .split("://")
            .nth(1)
            .and_then(|r| r.split('/').next())
            .unwrap_or("")
            .to_owned(),
        Action::Run => s.tool.clone(),
        _ => String::new(),
    }
}

/// Where a station's name stands beside it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Side {
    Under,
    Over,
    Right,
    Left,
}

/// How wide a name is on screen, at the map's 12px.
pub(super) fn name_width(text: &str) -> f64 {
    text.chars()
        .map(|c| if c.is_ascii() { 7.0 } else { 12.5 })
        .sum::<f64>()
        + 4.0
}

/// The stations named on the map, each with whether its name sits under the station rather
/// than over it. The most telling go first (the one looked at, then what stopped, then where a
/// cause was read); one whose name fits neither over nor under without running into a name
/// already placed, or into the turn band over the main line, is left to its hover card.
/// `grow` is how much larger than the map's own scale names are drawn.
#[allow(clippy::too_many_arguments)]
pub(super) fn map_names(
    stations: &[Station],
    dest: Option<i64>,
    forks: &HashSet<i64>,
    grow: f64,
    pins: &[(f64, f64)],
    shown: &[i64],
    ups: &HashSet<i64>,
    taken: &[(f64, f64, f64, f64)],
    reach: &HashMap<i64, (f64, f64)>,
) -> Vec<(Station, Side)> {
    let mut picked: Vec<&Station> = stations.iter().filter(|s| shown.contains(&s.seq)).collect();
    let rank = |s: &Station| {
        if Some(s.seq) == dest {
            0
        } else if s.decision.as_deref() == Some("DENY") {
            1
        } else if s.stopped() {
            2
        } else if forks.contains(&s.seq) {
            3
        } else if s.source.is_some() || s.loose || s.unsourced || s.line != 0 {
            // A branch's own stations are all the map shows of where it went.
            4
        } else {
            5
        }
    };
    picked.sort_by(|a, b| rank(a).cmp(&rank(b)).then(a.x.total_cmp(&b.x)));
    let (gap, tall) = (10.0 * grow, 28.0 * grow);
    // The pins stand over their stations,
    let mut boxes: Vec<(f64, f64, f64, f64)> = pins
        .iter()
        .map(|&(x, y)| {
            (
                x - 24.0 * grow,
                x + 24.0 * grow,
                y - 44.0 * grow,
                y - 8.0 * grow,
            )
        })
        // the turns' names over the main line, and every station's own mark, which no name may
        // cover.
        .chain(taken.iter().copied())
        .chain(stations.iter().map(|s| {
            (
                s.x - 8.0 * grow,
                s.x + 8.0 * grow,
                s.y - 8.0 * grow,
                s.y + 8.0 * grow,
            )
        }))
        .collect();
    let mut out = Vec::new();
    for s in picked {
        let wide = name_width(&crate::metro::place_label(s)) * grow + 6.0 * grow;
        let half = wide / 2.0;
        // Each side a name may take, as a box: under and over a station on the main line,
        // right and left of one on a branch, which runs across it.
        let spot = |side: Side| match side {
            // Clear of what the station's mark reaches over and under it, a transfer's pill.
            Side::Under => {
                let b = s.y + reach.get(&s.seq).map_or(0.0, |r| r.1);
                (s.x - half, s.x + half, b + gap, b + gap + tall)
            }
            Side::Over => {
                let t = s.y - reach.get(&s.seq).map_or(0.0, |r| r.0);
                (s.x - half, s.x + half, t - gap - tall, t - gap)
            }
            Side::Right => (
                s.x + 12.0 * grow,
                s.x + 12.0 * grow + wide,
                s.y - tall / 2.0,
                s.y + tall / 2.0,
            ),
            Side::Left => (
                s.x - 12.0 * grow - wide,
                s.x - 12.0 * grow,
                s.y - tall / 2.0,
                s.y + tall / 2.0,
            ),
        };
        let clear = |(x0, x1, y0, y1): (f64, f64, f64, f64)| {
            boxes
                .iter()
                .all(|&(a0, a1, b0, b1)| x1 <= a0 || x0 >= a1 || y1 <= b0 || y0 >= b1)
        };
        // Every name on the side away from the main line first (under it, over a branch that
        // goes up, right of a branch), else the other side; one that fits neither waits for
        // a hover.
        let tries = if s.line != 0 {
            [Side::Right, Side::Left]
        } else if ups.contains(&s.seq) {
            [Side::Over, Side::Under]
        } else {
            [Side::Under, Side::Over]
        };
        if let Some(&side) = tries.iter().find(|&&t| clear(spot(t))) {
            boxes.push(spot(side));
            out.push((s.clone(), side));
        }
    }
    out
}

#[component]
fn Track(metro: Arc<Metro>, index: usize, d: String) -> Element {
    let line = &metro.lines[index];
    // The words' line is dashed: not an agent going somewhere, but what it was told.
    let class = match line.kind {
        LineKind::Main => "track main",
        LineKind::Branch if line.badge == "글" => "track sub told",
        LineKind::Branch => "track sub",
    };
    rsx! {
        g {
            path { class: "casing", d: "{d}" }
            path { class: "{class}", style: "stroke: {line_colour(&metro, index, line.x0)}", d: "{d}" }
        }
    }
}

#[component]
#[allow(clippy::too_many_arguments)]
pub(super) fn StationMark(
    station: Station,
    turn: &'static str,
    ring: &'static str,
    fork: bool,
    picked: bool,
    off: bool,
    capsule: bool,
    k: f64,
    dragged: Signal<bool>,
    open: Signal<HashSet<i64>>,
    hovered: Signal<Option<i64>>,
) -> Element {
    let mut state = use_context::<AppState>();
    let (cx, cy, seq) = (station.x, station.y, station.seq);
    // A folded station opens; any other is picked.
    let select = move |e: MouseEvent| {
        e.stop_propagation();
        if dragged() {
            return;
        }
        // A station is a place: picking it picks the gravest Call made there.
        state.selected.set(Selection::Node(seq));
    };
    let stop = station.stopped();
    let r = if stop {
        6.5
    } else if fork {
        5.5
    } else {
        4.2
    };
    rsx! {
        g {
            class: format!("station {ring}{}{}", if fork { " xchg" } else { "" }, if off { " off" } else { "" }),
            style: "--turn: {turn}",
            // Drawn at its own size whatever the map's scale.
            transform: "translate({cx} {cy}) scale({k}) translate({-cx} {-cy})",
            onclick: select,
            onmouseenter: move |_| hovered.set(Some(seq)),
            onmouseleave: move |_| hovered.set(None),
            // A larger, invisible target than the mark itself.
            circle { class: "hit", cx: "{cx}", cy: "{cy}", r: "16" }
                match station.decision.as_deref() {
                    // A stop sign: an octagon, flat side up.
                    Some("DENY") => {
                        let k = r * 1.12;
                        let a = k * 0.4142;
                        rsx! { path { class: "mark deny", d: "M{cx - a},{cy - k} H{cx + a} L{cx + k},{cy - a} V{cy + a} L{cx + a},{cy + k} H{cx - a} L{cx - k},{cy + a} V{cy - a} Z" } }
                    }
                    // A warning sign: a triangle, point up.
                    Some("REVIEW") => {
                        let k = r * 1.35;
                        rsx! { path { class: "mark review", "stroke-linejoin": "round", d: "M{cx},{cy - k} L{cx + k * 0.95},{cy + k * 0.62} H{cx - k * 0.95} Z" } }
                    }
                    None => rsx! { circle { class: "mark undecided", cx: "{cx}", cy: "{cy}", r: "{r}" } },
                    // A station two lines share is its capsule, drawn under the lines' marks.
                    _ if capsule => rsx! {},
                    // A station: a white disc in its line's colour; an interchange in black.
                    // Where words were read and nothing else stops: a ring like a transfer's.
                    _ if fork => rsx! { circle { class: "mark xchg-ring", cx: "{cx}", cy: "{cy}", r: "{r}" } },
                    // A station, as Seoul's map draws one: a small white dot in the line.
                    _ => rsx! { circle { class: "mark dot", cx: "{cx}", cy: "{cy}", r: "1.6" } },
                }
                if station.decision.as_deref() == Some("DENY") {
                    path { class: "glyph", d: "M{cx - 3.2},{cy - 3.2} l6.4,6.4 M{cx + 3.2},{cy - 3.2} l-6.4,6.4" }
                }
                if station.decision.as_deref() == Some("REVIEW") {
                    path { class: "glyph", d: "M{cx},{cy - 4.2} v4.2 M{cx},{cy + 3.6} v0.2" }
                }
                if fork && stop { circle { class: "xchg-ring outer", cx: "{cx}", cy: "{cy}", r: "{r + 4.0}" } }
        }
    }
}

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

/// The map's key, as a metro map keeps one in its corner: each line by its colour, badge and
/// name (the main line in each turn's colour), then what the marks mean.
#[component]
fn MapLegend(metro: Arc<Metro>) -> Element {
    // Lines that share a name share a row: "E E2 E3 Explore 에이전트".
    let mut rows: Vec<(String, Vec<String>, &'static str)> = Vec::new();
    for (i, line) in metro.lines.iter().enumerate().skip(1) {
        let colour = line_colour(&metro, i, line.x0);
        match rows.iter_mut().find(|(name, ..)| *name == line.name) {
            Some((_, badges, _)) => badges.push(metro.badge(i)),
            None => rows.push((line.name.clone(), vec![metro.badge(i)], colour)),
        }
    }
    let turns: Vec<&'static str> = (0..metro.asks.len().max(1)).map(turn_colour).collect();
    rsx! {
        div { class: "map-legend", onmousedown: move |e| e.stop_propagation(),
            div { class: "lg-title", "노선 안내" }
            div { class: "lg-row",
                span { class: "lg-bar", span { style: "background: {MAIN_INK}" } }
                span { class: "line-badge small", style: "background: {MAIN_INK}", "{metro.badge(0)}" }
                span { "{metro.lines[0].name}" }
            }
            if turns.len() > 1 {
                div { class: "lg-row turns",
                    for (n, c) in turns.iter().enumerate() { span { class: "lg-turn", style: "background: {c}", "턴 {n + 1}" } }
                    span { class: "lg-note", "요청이 들어온 곳" }
                }
            }
            for (name, badges, colour) in rows.iter().cloned() {
                div { class: "lg-row",
                    span { class: "lg-bar", span { style: "background: {colour}" } }
                    for b in badges.iter().take(4) {
                        span { class: "line-badge small", style: "background: {colour}", "{b}" }
                    }
                    span { "{name}" }
                }
            }
            div { class: "lg-marks",
                span { class: "lg",
                    svg { width: "12", height: "20", view_box: "-6 -10 12 20",
                        rect { class: "lg-pill", x: "-5", y: "-9", width: "10", height: "18", rx: "5" }
                        circle { cx: "0", cy: "-3.5", r: "2.6", style: "fill: {MAIN_INK}" }
                        circle { cx: "0", cy: "3.5", r: "2.6", style: "fill: {BRANCH_COLOURS[0]}" }
                    }
                    "환승 · 여러 노선이 들른 곳"
                }
                span { class: "lg",
                    svg { width: "14", height: "14", view_box: "-7 -7 14 14", circle { class: "lg-xchg", r: "5" } }
                    "글을 읽은 곳"
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

/// A line's badge where the route rides it: the request's line opened by a subagent's report
/// reads as the report's.
fn badge_at(m: &Metro, line: usize, x: f64) -> String {
    let _ = x;
    m.badge(line)
}

/// The tone a line wears in the route card: its colour.
fn line_tone(m: &Metro, line: usize, x: f64) -> &'static str {
    let _ = x;
    match m.lines[line].kind {
        LineKind::Main => "main",
        LineKind::Branch => "agent",
    }
}

/// A moment as the route finder's times column writes it: the hour, minute and second.
fn clock(ms: Option<i64>) -> String {
    use chrono::TimeZone;
    ms.and_then(|ms| chrono::Local.timestamp_millis_opt(ms).single())
        .map(|t| t.format("%H:%M:%S").to_string())
        .unwrap_or_default()
}

/// Which routes the list shows, as a route finder's tabs split them by mode.
#[derive(Clone, Copy, PartialEq)]
enum Filter {
    All,
    Deny,
    Review,
    Allow,
    Undecided,
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

/// The turn a station belongs to: the last request before it.
/// How many turns either side of the one being looked at the map draws.
pub(super) const WINDOW: usize = 1;

/// The Call a turn is picked by: its gravest (denied, then held), else its first.
pub(super) fn turn_pick(m: &Metro, t: usize) -> Option<i64> {
    let mut on: Vec<&Station> = m.stations.iter().filter(|s| turn_of(m, s) == t).collect();
    on.sort_by(|a, b| a.x.total_cmp(&b.x));
    let weight = |s: &Station| match s.decision.as_deref() {
        Some("DENY") => 2,
        Some("REVIEW") => 1,
        _ => 0,
    };
    let best = on.iter().map(|s| weight(s)).max().unwrap_or(0);
    on.into_iter().find(|s| weight(s) == best).map(|s| s.seq)
}

/// Each turn's gravest verdict: 2 denied, 1 held, 0 neither.
pub(super) fn turn_marks(m: &Metro) -> Vec<u8> {
    let mut out = vec![0u8; m.asks.len().max(1)];
    for s in &m.stations {
        let w = match s.decision.as_deref() {
            Some("DENY") => 2,
            Some("REVIEW") => 1,
            _ => 0,
        };
        let t = turn_of(m, s).min(out.len() - 1);
        out[t] = out[t].max(w);
    }
    out
}

/// The turn the Call being looked at is in.
pub(super) fn turn_at_call(m: &Metro, call: Option<i64>) -> usize {
    call.and_then(|c| m.station(c)).map_or(0, |s| turn_of(m, s))
}

pub(super) fn turn_of(m: &Metro, s: &Station) -> usize {
    m.asks.iter().rposition(|a| a.x <= s.x).unwrap_or(0)
}

/// One entry of a turn in the list: a Call, or a run of allowed ones folded together.
#[derive(Clone, PartialEq)]
#[allow(clippy::large_enum_variant)]
enum Entry {
    Call(Station),
    Fold(usize, Vec<Station>),
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

/// The route finder's first column: the session to look in and what happened there, a search
/// for a station, tabs by verdict, then the Calls grouped by what told the agent to make them,
/// the groups that stopped first. The allowed Calls of a group stay folded until asked for.
#[component]
fn RouteList(metro: Arc<Metro>, dest: Option<i64>, mut detail: Signal<bool>) -> Element {
    // While the detail column is open it shows the reason; the list only marks the row.
    let detail_open = detail();
    let mut state = use_context::<AppState>();
    let mut query = use_signal(String::new);
    let mut searching = use_signal(|| false);
    let mut filter = use_signal(|| Filter::All);
    // Folded runs of allowed Calls the person opened, by their first Call.
    let mut unfolded = use_signal(HashSet::<i64>::new);
    // Turns the person opened or closed against the default, and runs of plain turns they
    // spread out, by their first turn.
    let mut toggled = use_signal(HashSet::<usize>::new);
    let mut spread = use_signal(HashSet::<usize>::new);
    let m = &metro;
    let found: Vec<Station> = if searching() {
        m.find(&query()).into_iter().cloned().collect()
    } else {
        Vec::new()
    };
    let first_found = found.first().map(|s| s.seq);
    let mut pick = move |seq: i64| {
        searching.set(false);
        query.set(String::new());
        state.selected.set(Selection::Node(seq));
        detail.set(true);
    };
    let reasons: HashMap<i64, (String, String)> = match &*state.graph.read() {
        Load::Ready(model) => m
            .stations
            .iter()
            .filter(|s| s.stopped())
            .filter_map(|s| Some((s.seq, strongest_reason(model, s.seq)?)))
            .collect(),
        _ => HashMap::new(),
    };
    // The tabs count Calls; the rows are stations, an express one standing for several.
    let counts = m.verdicts();
    let shown = |s: &Station| match filter() {
        Filter::All => true,
        Filter::Deny => s.decision.as_deref() == Some("DENY"),
        Filter::Review => s.decision.as_deref() == Some("REVIEW"),
        Filter::Allow => s.decision.is_some() && !s.stopped(),
        Filter::Undecided => s.decision.is_none(),
    };
    // The Calls by turn, in the order things happened; more than three allowed Calls in a row
    // fold into one entry until opened.
    let mut turns: Vec<(usize, Vec<Entry>)> = Vec::new();
    for s in m.stations.iter().filter(|s| shown(s)) {
        let t = turn_of(m, s);
        if turns.last().is_none_or(|(k, _)| *k != t) {
            turns.push((t, Vec::new()));
        }
        turns.last_mut().unwrap().1.push(Entry::Call(s.clone()));
    }
    let fold_after = if filter() == Filter::Allow {
        usize::MAX
    } else {
        2
    };
    for (_, entries) in turns.iter_mut() {
        let mut out: Vec<Entry> = Vec::new();
        let mut run: Vec<Station> = Vec::new();
        // Every allowed Call the request asked for folds, an express station with them; two or
        // more Calls in a row are worth one line.
        let flush = |run: &mut Vec<Station>, out: &mut Vec<Entry>| {
            let calls: usize = run.iter().map(|s| s.members.len()).sum();
            if calls >= fold_after && !unfolded().contains(&(run[0].seq)) {
                out.push(Entry::Fold(run[0].seq as usize, std::mem::take(run)));
            } else {
                out.extend(run.drain(..).map(Entry::Call));
            }
        };
        for e in entries.drain(..) {
            match e {
                // What is worth a row of its own: a stop, a Call a tool result told it to make, one
                // nothing told it to make, and the one being looked at.
                Entry::Call(s)
                    if !s.stopped()
                        && Some(s.seq) != dest
                        && (s.folded() || (s.source.is_none() && !s.loose && !s.unsourced)) =>
                {
                    run.push(s)
                }
                other => {
                    flush(&mut run, &mut out);
                    out.push(other);
                }
            }
        }
        flush(&mut run, &mut out);
        *entries = out;
    }
    // A turn shows its Calls when it is the one being looked at or something in it stopped, or
    // the person opened it; plain turns in a row read as one line until spread out.
    let here = turn_at_call(m, dest);
    let marks = turn_marks(m);
    let mark = |t: usize| marks.get(t).copied().unwrap_or(0);
    let open_by_default = |t: usize| t == here || mark(t) > 0;
    let is_open = |t: usize| open_by_default(t) ^ toggled().contains(&t);
    let mut rows: Vec<Row> = Vec::new();
    for (t, entries) in turns.iter().cloned() {
        let plain = filter() == Filter::All && !is_open(t) && mark(t) == 0;
        match rows.last_mut() {
            Some(Row::Plain(run)) if plain && !spread().contains(&run[0].0) => {
                run.push((t, entries))
            }
            _ if plain && !spread().contains(&t) => rows.push(Row::Plain(vec![(t, entries)])),
            _ => rows.push(Row::Turn(t, entries, is_open(t))),
        }
    }
    // A run of one plain turn is just that turn, folded.
    let rows: Vec<Row> = rows
        .into_iter()
        .map(|r| match r {
            Row::Plain(mut run) if run.len() == 1 => {
                let (t, e) = run.remove(0);
                Row::Turn(t, e, false)
            }
            other => other,
        })
        .collect();
    let picks: HashMap<usize, i64> = turns
        .iter()
        .filter_map(|(t, _)| Some((*t, turn_pick(m, *t)?)))
        .collect();
    // The region to search in: the session, by its folder and what was asked.
    let sessions: Vec<(String, String)> = match &*state.sessions.read() {
        Load::Ready(list) => list
            .iter()
            .map(|s| {
                let folder = s.folder();
                let label = format!(
                    "{}{}",
                    if folder.is_empty() {
                        String::new()
                    } else {
                        format!("{folder} · ")
                    },
                    clip(s.title(), 34)
                );
                (s.id.clone(), label)
            })
            .collect(),
        _ => Vec::new(),
    };
    let current = state.current.read().clone().unwrap_or_default();
    // 길찾기: the first station the field finds, or else the Call the map would open on.
    let finder = metro.clone();
    let m_find = move |q: &str| finder.find(q).first().map(|s| s.seq);
    let default = m.default_dest();
    let tab = |f: Filter, label: &str, n: usize| {
        rsx! {
            button {
                class: if filter() == f { "tab on" } else { "tab" },
                disabled: n == 0 && f != Filter::All,
                onclick: move |_| filter.set(f),
                "{label}"
                span { class: "n", "{n}" }
            }
        }
    };
    rsx! {
        div { class: "route-list", onmousedown: move |e| e.stop_propagation(), onclick: move |e| e.stop_propagation(),
            // The kinds of result, as a route finder's modes: all, then by verdict.
            div { class: "nv-modes",
                {tab(Filter::All, "전체", counts.all)}
                {tab(Filter::Deny, "차단", counts.deny)}
                {tab(Filter::Review, "확인", counts.review)}
                {tab(Filter::Allow, "허용", counts.allow)}
                if counts.none > 0 { {tab(Filter::Undecided, "판정 없음", counts.none)} }
            }
            // Where from and where to: the session as the region set out in, a station to arrive at.
            div { class: "nv-od",
                div { class: "od-row",
                    span { class: "od-dot start" }
                    select {
                        id: "session-search",
                        class: "region-pick",
                        title: "세션 바꾸기",
                        onchange: move |e| state.open_session(e.value()),
                        for (id, label) in sessions.iter().cloned() {
                            option { value: "{id}", selected: id == current, "{label}" }
                        }
                    }
                }
                div { class: "od-row find",
                    span { class: "od-dot end" }
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
                        div { class: "suggest",
                            if found.is_empty() { div { class: "none", "그런 역이 없습니다. 파일 이름이나 명령으로 찾아 보세요." } }
                            for s in found.iter().cloned() {
                                div {
                                    class: "sug",
                                    // Before the field's blur, which would close the list first.
                                    onmousedown: move |e| { e.prevent_default(); pick(s.seq); },
                                    span { class: "lbadge {line_tone(m, s.line, s.x)}", "{badge_at(m, s.line, s.x)}" }
                                    span { class: "nm", "{s.name()}" }
                                    span { class: "t", "{clock(s.at)}" }
                                }
                            }
                        }
                    }
                }
            }
            div { class: "nv-actions",
                button {
                    class: "nv-reset",
                    onclick: move |_| {
                        query.set(String::new());
                        filter.set(Filter::All);
                        state.selected.set(Selection::None);
                    },
                    "⟲ 다시입력"
                }
                button {
                    class: "nv-go",
                    onclick: move |_| {
                        let to = m_find(&query()).or(default);
                        if let Some(seq) = to { pick(seq) }
                    },
                    "길찾기 ›"
                }
            }
            // When the map was drawn, and whether it keeps up with the session.
            div { class: "nv-when",
                button {
                    class: if (state.live)() { "on-pill on" } else { "on-pill" },
                    title: "자동 갱신",
                    onclick: move |_| state.live.toggle(),
                    if (state.live)() { "ON" } else { "OFF" }
                }
                span { class: "nv-live", "자동 갱신" }
                span { class: "nv-asof", "{clock(m.stations.iter().filter_map(|s| s.at).max())} 기준" }
                button {
                    class: "nv-re",
                    title: "새로고침",
                    onclick: move |_| {
                        state.reload += 1;
                        state.refresh += 1;
                    },
                    "⟳ 새로고침"
                }
            }
            div { class: "groups",
                for row in rows.iter().cloned() {
                    match row {
                        Row::Plain(run) => {
                            let (a, b) = (run[0].0, run[run.len() - 1].0);
                            let calls: usize = run.iter().flat_map(|(_, e)| e.iter()).map(|e| match e {
                                Entry::Call(s) => s.members.len(),
                                Entry::Fold(_, v) => v.iter().map(|s| s.members.len()).sum(),
                            }).sum();
                            rsx! {
                                div {
                                    key: "plain-{a}",
                                    class: "turn plain-run",
                                    onclick: move |_| { spread.write().insert(a); },
                                    div { class: "t-head",
                                        div { class: "t-top",
                                            span { class: "t-no", "턴 {a + 1}–{b + 1}" }
                                            span { class: "t-who", "경고 없는 턴 {run.len()}개" }
                                            span { class: "g-counts", span { class: "c pass", Icon { name: "allow", size: 12.0 } "{calls}" } }
                                        }
                                        div { class: "t-said", "허용 {calls}개 · 눌러서 펼치기" }
                                    }
                                }
                            }
                        }
                        Row::Turn(t, entries, opened) => {
                        let ask = m.asks.get(t).cloned();
                        let colour = turn_colour(t);
                        let calls: Vec<&Station> = entries.iter().flat_map(|e| match e {
                            Entry::Call(s) => vec![s],
                            Entry::Fold(_, v) => v.iter().collect(),
                        }).collect();
                        let deny = calls.iter().filter(|s| s.decision.as_deref() == Some("DENY")).count();
                        let review = calls.iter().filter(|s| s.decision.as_deref() == Some("REVIEW")).count();
                        let allowed: usize = calls.iter().filter(|s| !s.stopped()).map(|s| s.members.len()).sum();
                        let who = match ask.as_ref().and_then(|a| a.agent.clone()) {
                            Some(agent) => format!("{agent} 보고"),
                            None => "사람의 요청".to_owned(),
                        };
                        let said = ask.as_ref().map(|a| report_words(&a.text)).unwrap_or_default();
                        let pick = picks.get(&t).copied();
                        let looked = t == here;
                        rsx! {
                            div { key: "turn-{t}", class: format!("turn{}{}", if opened { " open" } else { " shut" }, if looked { " here" } else { "" }), style: "--turn: {colour}",
                                // The head picks the turn; on the turn being looked at it opens and
                                // closes it.
                                div {
                                    class: "t-head",
                                    onclick: move |_| {
                                        if looked {
                                            let mut t2 = toggled.write();
                                            if !t2.remove(&t) { t2.insert(t); }
                                        } else if let Some(q) = pick {
                                            state.selected.set(Selection::Node(q));
                                        }
                                    },
                                    div { class: "t-top",
                                        span { class: "t-no", "턴 {t + 1}" }
                                        span { class: "t-who", "{who}" }
                                        span { class: "g-counts",
                                            if deny > 0 { span { class: "c deny", Icon { name: "deny", size: 12.0 } "{deny}" } }
                                            if review > 0 { span { class: "c review", Icon { name: "review", size: 12.0 } "{review}" } }
                                            if allowed > 0 { span { class: "c pass", Icon { name: "allow", size: 12.0 } "{allowed}" } }
                                        }
                                        span { class: "t-caret", if opened { "접기" } else { "펼치기" } }
                                    }
                                    div { class: "t-said", title: "{said}", "{said}" }
                                }
                                if opened {
                                for e in entries.iter().cloned() {
                                    match e {
                                        Entry::Call(s) => rsx! {
                                            CallRow {
                                                key: "{s.seq}",
                                                metro: metro.clone(),
                                                station: s.clone(),
                                                on: dest == Some(s.seq),
                                                reason: if detail_open { None } else { reasons.get(&s.seq).cloned() },
                                                detail,
                                            }
                                        },
                                        Entry::Fold(first, run) => {
                                            let n: usize = run.iter().map(|s| s.members.len()).sum();
                                            let first = first as i64;
                                            rsx! {
                                                div {
                                                    key: "fold-{first}",
                                                    class: "crow fold",
                                                    onclick: move |_| { unfolded.write().insert(first); },
                                                    span { class: "cr-mark", Icon { name: "allow", size: 16.0 } }
                                                    span { class: "cr-name", "허용 {n}개" }
                                                    span { class: "cr-time open", "펼치기" }
                                                    span { class: "cr-meta", span { class: "cr-target", "{tool_counts(&run)}" } }
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
                if turns.is_empty() {
                    p { class: "route-hint", "이 탭에 해당하는 호출이 없습니다." }
                }
            }
        }
    }
}

/// A row of the list: a turn (and whether it shows its Calls), or plain turns in a row.
#[derive(Clone, PartialEq)]
enum Row {
    Turn(usize, Vec<Entry>, bool),
    Plain(Vec<(usize, Vec<Entry>)>),
}

/// One Call in a group, on one line: its verdict, its name, what it acted on, when. The one
/// picked opens to the call itself and, when stopped, the engine's reason.
#[component]
fn CallRow(
    metro: Arc<Metro>,
    station: Station,
    on: bool,
    reason: Option<(String, String)>,
    mut detail: Signal<bool>,
) -> Element {
    let mut state = use_context::<AppState>();
    let s = &station;
    let seq = s.seq;
    let vtone = verdict_tone(s.decision.as_deref());
    let title = if s.folded() {
        format!("허용 {}개", s.members.len())
    } else {
        s.name()
    };
    rsx! {
        div {
            class: format!("crow {vtone}{}", if on { " on" } else { "" }),
            onclick: move |_| {
                state.selected.set(Selection::Node(seq));
                detail.set(true);
            },
            CallLines { station: s.clone(), title: title.clone(), full_time: false }
            if let Some(cause) = told_line(&metro, s) {
                span { class: "cr-cause", "↳ {cause}" }
            }
            if on {
                if let Some((text, p)) = reason.clone() { div { class: "cr-reason", ReasonBox { tone: vtone, text, p } } }
            }
        }
    }
}

/// What told the agent to make a Call, when it was not the turn's request: the result whose
/// words it followed, or that nothing in the session did.
fn told_line(m: &Metro, s: &Station) -> Option<String> {
    // An express station stands for many Calls; its first one's cause is not theirs.
    if s.folded() {
        return None;
    }
    if let Some(b) = s.source.as_ref().and_then(|src| m.station(src.by)) {
        return Some(format!("{}{}", b.cause_name(), source_kind(b)));
    }
    (s.loose || s.unsourced).then(|| "시킨 것 없음".to_owned())
}

/// The tools a set of stations ran, each with how many times: "Read 20 · Edit 7".
fn tool_counts(stations: &[Station]) -> String {
    let mut counts: Vec<(String, usize)> = Vec::new();
    for s in stations {
        if s.folded() {
            // An express station already says its tools: "Read 9 · Edit 6", or "15개".
            for part in s.what.split(" · ") {
                let mut words = part.rsplitn(2, ' ');
                let (n, t) = (
                    words.next().and_then(|n| n.parse::<usize>().ok()),
                    words.next(),
                );
                match (t, n) {
                    (Some(t), Some(n)) => add(&mut counts, t, n),
                    _ => add(&mut counts, &s.tool, s.members.len()),
                }
            }
        } else {
            add(&mut counts, &s.tool, 1);
        }
    }
    fn add(counts: &mut Vec<(String, usize)>, tool: &str, n: usize) {
        match counts.iter_mut().find(|(t, _)| t == tool) {
            Some((_, c)) => *c += n,
            None => counts.push((tool.to_owned(), n)),
        }
    }
    counts.sort_by_key(|c| std::cmp::Reverse(c.1));
    let name = |t: &str| match t {
        "SubagentHandback" => "보고".to_owned(),
        "Agent" | "Task" => "맡김".to_owned(),
        _ => t.to_owned(),
    };
    let mut parts: Vec<String> = counts
        .iter()
        .take(3)
        .map(|(t, c)| format!("{} {c}", name(t)))
        .collect();
    let rest: usize = counts.iter().skip(3).map(|(_, c)| c).sum();
    if rest > 0 {
        parts.push(format!("외 {rest}"));
    }
    parts.join(" · ")
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
fn RoutePanel(
    metro: Arc<Metro>,
    dest: Option<i64>,
    mut hovered: Signal<Option<i64>>,
    mut detail: Signal<bool>,
) -> Element {
    let mut state = use_context::<AppState>();
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
                                                                blockquote { key: "{q.result}-{q.paragraph:?}", class: if bs.action == crate::metro::Action::Run { "st-quote out" } else { "st-quote prose" },
                                                                    super::trace::Preview { seq: q.result, paragraph: q.paragraph, lines: true }
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

/// The stations either side of one on its line, as a platform sign shows them. Before a
/// branch's first station is the station it leaves.
fn neighbours(m: &Metro, s: &Station) -> (Option<i64>, Option<i64>) {
    let mut on_line: Vec<&Station> = m.stations.iter().filter(|o| o.line == s.line).collect();
    on_line.sort_by(|a, b| a.x.total_cmp(&b.x));
    let at = on_line.iter().position(|o| o.seq == s.seq).unwrap_or(0);
    let prev = at.checked_sub(1).map(|i| on_line[i].seq);
    (prev, on_line.get(at + 1).map(|o| o.seq))
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

    fn demo_net(prefix: &str) -> crate::network::Net {
        let graph = crate::demo::Demo::load();
        let id = graph
            .sessions()
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|s| s["id"].as_str())
            .find(|id| id.starts_with(prefix))
            .unwrap()
            .to_owned();
        crate::network::Net::build(&crate::layout::Model::build(
            &serde_json::from_value(graph.graph(&id).unwrap()).unwrap(),
        ))
    }

    // The whole-map view sits between the summary and the legend, with what is drawn inside.
    #[test]
    fn the_fit_keeps_the_map_between_summary_and_legend() {
        let m = demo_net("fa6576f7");
        let (w, h) = (840.0, 760.0);
        let c = Camera::fit_net(&m, w, h);
        let top = m.top * c.zoom + c.y;
        let low = m.bottom * c.zoom + c.y;
        assert!(
            top >= SUMMARY_H + OVER - 0.5 && low <= h - LEGEND_H - UNDER + 0.5,
            "{top}..{low}"
        );
        // The whole width in view.
        assert!(c.x >= 0.0 && c.x + m.width * c.zoom <= w + 0.5);
    }

    // Picking a turn picks a Call in that turn, its gravest: so the map's window and the list
    // follow whichever turn the minimap or a fold picked.
    #[test]
    fn a_turn_is_picked_by_its_gravest_call() {
        let demo = crate::demo::Demo::parse(
            &std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/mock/long.json"))
                .unwrap(),
        );
        let model = crate::layout::Model::build(
            &serde_json::from_value(demo.graph("c4000a1b-4444-4b5c-8d6e-000000000004").unwrap())
                .unwrap(),
        );
        let m = Metro::build(&model);
        let marks = turn_marks(&m);
        assert_eq!(marks.len(), 40);
        assert_eq!(marks.iter().filter(|&&w| w == 2).count(), 1);
        for (t, &mark) in marks.iter().enumerate() {
            let q = turn_pick(&m, t).expect("a Call");
            assert_eq!(turn_at_call(&m, Some(q)), t, "turn {t}");
            if mark == 2 {
                assert_eq!(m.station(q).unwrap().decision.as_deref(), Some("DENY"));
            }
        }
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
