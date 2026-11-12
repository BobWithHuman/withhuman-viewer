//! The charts the security team's dashboards share, drawn as inline SVG or plain boxes: bars
//! stacked over time, a sparkline, a week's hours as a grid, a ring of shares, and a bar of
//! shares with its legend. Each mark says its numbers in a tooltip.

use dioxus::prelude::*;

use super::main_page::scale;

/// One layer of a stacked chart: its name, its colour, and a value per bucket.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Series {
    pub label: &'static str,
    pub color: &'static str,
    pub values: Vec<usize>,
}

/// Bars over time, each stacked from the series given, the first at the bottom.
pub(super) fn stacked_bars(labels: &[String], series: &[Series], w: f64, h: f64) -> Element {
    let (left, right, top, bottom) = (26.0, 8.0, 10.0, 22.0);
    let plot_w = w - left - right;
    let plot_h = h - top - bottom;
    let n = labels.len().max(1);
    let at = |s: &Series, i: usize| s.values.get(i).copied().unwrap_or(0);
    let max = (0..labels.len())
        .map(|i| series.iter().map(|s| at(s, i)).sum::<usize>())
        .max()
        .unwrap_or(0);
    let (top_v, step) = scale(max);
    let y = move |v: usize| top + plot_h - v as f64 / top_v as f64 * plot_h;
    let slot = plot_w / n as f64;
    let bar = (slot * 0.62).clamp(2.0, 44.0);
    let every = n.div_ceil(8);
    let ticks: Vec<usize> = (0..=top_v).step_by(step).collect();
    let tick_every = ticks.len().div_ceil(5);
    rsx! {
        svg { class: "cs-chart", view_box: "0 0 {w} {h}",
            for (j, t) in ticks.into_iter().enumerate() {
                if j % tick_every == 0 {
                    g { key: "tick-{t}",
                        line { x1: "{left}", x2: "{w - right}", y1: "{y(t):.1}", y2: "{y(t):.1}", class: "grid" }
                        text { x: "{left - 6.0}", y: "{y(t) + 3.0:.1}", text_anchor: "end", class: "axis", "{t}" }
                    }
                }
            }
            for (i, label) in labels.iter().enumerate() {
                {
                    let x = left + slot * i as f64 + (slot - bar) / 2.0;
                    let mut under = 0;
                    let parts: Vec<(&'static str, f64, f64)> = series
                        .iter()
                        .filter_map(|s| {
                            let v = at(s, i);
                            if v == 0 {
                                return None;
                            }
                            let (py, ph) = (y(under + v), y(under) - y(under + v));
                            under += v;
                            // A hair of the card between layers, so they read as separate marks.
                            Some((s.color, py, if ph > 3.0 { ph - 1.5 } else { ph }))
                        })
                        .collect();
                    let title = std::iter::once(label.clone())
                        .chain(series.iter().filter(|s| at(s, i) > 0).map(|s| format!("{} {}", s.label, at(s, i))))
                        .collect::<Vec<_>>()
                        .join("\n");
                    rsx! {
                        g { key: "bar-{i}",
                            title { "{title}" }
                            rect { x: "{left + slot * i as f64:.1}", y: "{top}", width: "{slot:.1}", height: "{plot_h}", fill: "transparent" }
                            for (k, (color, py, ph)) in parts.into_iter().enumerate() {
                                rect { key: "{k}", x: "{x:.1}", y: "{py:.1}", width: "{bar:.1}", height: "{ph:.1}", rx: "1.5", fill: "{color}" }
                            }
                            if i % every == 0 || i + 1 == n {
                                text { x: "{x + bar / 2.0:.1}", y: "{h - 7.0}", text_anchor: "middle", class: "axis", "{label}" }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// A count over time as a small filled line, with no axes: the shape only.
pub(super) fn spark(values: &[usize], color: &str) -> Element {
    let (w, h) = (120.0, 34.0);
    let max = values.iter().copied().max().unwrap_or(0).max(1) as f64;
    let n = values.len().max(2);
    let x = |i: usize| 1.0 + (w - 2.0) * i as f64 / (n - 1) as f64;
    let y = |v: usize| h - 2.0 - (h - 7.0) * v as f64 / max;
    let points: Vec<String> = values
        .iter()
        .enumerate()
        .map(|(i, v)| format!("{:.1},{:.1}", x(i), y(*v)))
        .collect();
    if points.is_empty() {
        return rsx! { svg { class: "vz-spark", view_box: "0 0 {w} {h}" } };
    }
    let line = points.join(" ");
    let area = format!(
        "M{:.1},{h} L{} L{:.1},{h} Z",
        x(0),
        points.join(" L"),
        x(values.len() - 1)
    );
    rsx! {
        svg { class: "vz-spark", view_box: "0 0 {w} {h}", preserve_aspect_ratio: "none",
            path { d: "{area}", fill: "{color}", opacity: "0.16" }
            polyline { points: "{line}", fill: "none", stroke: "{color}", stroke_width: "1.6", stroke_linejoin: "round", "vector-effect": "non-scaling-stroke" }
        }
    }
}

const WEEKDAYS: [&str; 7] = ["월", "화", "수", "목", "금", "토", "일"];

/// A week's hours as a grid, Monday first: the more in an hour, the stronger its cell.
pub(super) fn heat(cells: &[[usize; 24]; 7], unit: &str) -> Element {
    let (left, top, step, size) = (22.0, 16.0, 17.0, 15.0);
    let (w, h) = (left + 24.0 * step, top + 7.0 * step);
    let max = cells.iter().flatten().copied().max().unwrap_or(0).max(1) as f64;
    rsx! {
        svg { class: "cs-chart vz-heat", view_box: "0 0 {w} {h}",
            for hour in [0, 6, 12, 18, 23] {
                text { key: "h{hour}", x: "{left + hour as f64 * step + size / 2.0}", y: "10", text_anchor: "middle", class: "axis", "{hour}시" }
            }
            for (d, row) in cells.iter().enumerate() {
                g { key: "d{d}",
                    text { x: "{left - 7.0}", y: "{top + d as f64 * step + 11.0}", text_anchor: "end", class: "axis", "{WEEKDAYS[d]}" }
                    for (hour, v) in row.iter().enumerate() {
                        rect { key: "{hour}",
                            x: "{left + hour as f64 * step}", y: "{top + d as f64 * step}", width: "{size}", height: "{size}", rx: "2",
                            class: if *v == 0 { "zero" } else { "on" },
                            opacity: if *v == 0 { "1".to_owned() } else { format!("{:.2}", 0.22 + 0.78 * *v as f64 / max) },
                            title { "{WEEKDAYS[d]}요일 {hour}시: {v}{unit}" }
                        }
                    }
                }
            }
        }
    }
}

/// Shares of a whole as a ring, the whole's count in its middle.
pub(super) fn donut(parts: &[(&'static str, &'static str, usize)], unit: &str) -> Element {
    let total: usize = parts.iter().map(|p| p.2).sum();
    if total == 0 {
        return rsx! {};
    }
    let (r, width) = (38.0, 15.0);
    let around = 2.0 * std::f64::consts::PI * r;
    let shown: Vec<(&'static str, &'static str, usize)> =
        parts.iter().copied().filter(|p| p.2 > 0).collect();
    // A hair of the card between parts, when there is more than one.
    let gap = if shown.len() > 1 { 1.6 } else { 0.0 };
    let mut start = 0.0;
    let arcs: Vec<(&'static str, &'static str, usize, f64, f64)> = shown
        .into_iter()
        .map(|(label, color, n)| {
            let long = n as f64 / total as f64 * around;
            let arc = (label, color, n, start, (long - gap).max(0.5));
            start += long;
            arc
        })
        .collect();
    rsx! {
        svg { class: "vz-ring", view_box: "0 0 100 100",
            for (label, color, n, from, long) in arcs {
                circle { key: "{label}", cx: "50", cy: "50", r: "{r}", fill: "none", stroke: "{color}", stroke_width: "{width}",
                    stroke_dasharray: "{long:.2} {around - long:.2}", stroke_dashoffset: "{-from:.2}", transform: "rotate(-90 50 50)",
                    title { "{label} {n}{unit} ({n as f64 / total as f64 * 100.0:.0}%)" }
                }
            }
            text { x: "50", y: "55", text_anchor: "middle", class: "total", "{total}" }
            text { x: "50", y: "66", text_anchor: "middle", class: "unit", "{unit}" }
        }
    }
}

/// Shares of a whole as one bar, with what each part is and its count beneath.
pub(super) fn mix(parts: &[(&'static str, &'static str, usize)]) -> Element {
    let shown: Vec<(&'static str, &'static str, usize)> =
        parts.iter().copied().filter(|p| p.2 > 0).collect();
    if shown.is_empty() {
        return rsx! {};
    }
    rsx! {
        div { class: "vz-mix",
            div { class: "bar",
                for (label, color, n) in shown.iter().copied() {
                    span { key: "{label}", title: "{label} {n}", style: "flex-grow:{n};background:{color}" }
                }
            }
            div { class: "keys",
                for (label, color, n) in shown {
                    span { key: "{label}", i { style: "background:{color}" } "{label}" strong { "{n}" } }
                }
            }
        }
    }
}
