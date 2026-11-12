//! The charts the security team's dashboards share, drawn as inline SVG or plain boxes: bars
//! stacked over time, a sparkline, a week's hours as a grid, a flow across three columns, and a
//! bar of shares with its legend. Each mark says its numbers in a tooltip.

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

/// A stop in a flow: its name and colour.
pub(super) type Stop = (&'static str, &'static str);

/// What went from where to where across three columns: each item names its stop in each column.
/// A stop is as tall as the items through it; a band between two stops is as thick as the items
/// they share. The first bands wear the middle stop's colour, the second the last stop's.
pub(super) fn flow(columns: [&[Stop]; 3], items: &[[usize; 3]], unit: &str) -> Element {
    let (w, h, pad, node, gap) = (760.0, 236.0, 132.0, 9.0, 12.0);
    let total = items.len();
    if total == 0 {
        return rsx! {};
    }
    let xs = [pad, (w - node) / 2.0, w - pad - node];
    let count = |col: usize, stop: usize| items.iter().filter(|it| it[col] == stop).count();
    // The stops that have any, top to bottom, each with where it starts and how tall it is.
    let most = (0..3)
        .map(|c| (0..columns[c].len()).filter(|s| count(c, *s) > 0).count())
        .max()
        .unwrap_or(1);
    let unit_h = (h - 8.0 - gap * (most.max(1) - 1) as f64) / total as f64;
    let place = |c: usize| -> Vec<(usize, f64, f64)> {
        let shown: Vec<usize> = (0..columns[c].len()).filter(|s| count(c, *s) > 0).collect();
        let used = total as f64 * unit_h + gap * (shown.len().max(1) - 1) as f64;
        let mut y = 4.0 + (h - 8.0 - used) / 2.0;
        shown
            .into_iter()
            .map(|s| {
                let tall = count(c, s) as f64 * unit_h;
                let at = (s, y, tall);
                y += tall + gap;
                at
            })
            .collect()
    };
    let placed = [place(0), place(1), place(2)];
    let mut bands: Vec<(String, &'static str, String)> = Vec::new();
    for c in 0..2 {
        // How far down each stop its bands have got, leaving on the right and arriving on the left.
        let mut out: Vec<f64> = placed[c].iter().map(|p| p.1).collect();
        let mut into: Vec<f64> = placed[c + 1].iter().map(|p| p.1).collect();
        for (i, (a, _, _)) in placed[c].iter().enumerate() {
            for (j, (b, _, _)) in placed[c + 1].iter().enumerate() {
                let n = items
                    .iter()
                    .filter(|it| it[c] == *a && it[c + 1] == *b)
                    .count();
                if n == 0 {
                    continue;
                }
                let thick = n as f64 * unit_h;
                let (x0, x1) = (xs[c] + node, xs[c + 1]);
                let mid = (x0 + x1) / 2.0;
                let (y0, y1) = (out[i], into[j]);
                out[i] += thick;
                into[j] += thick;
                let d = format!(
                    "M{x0:.1},{y0:.1} C{mid:.1},{y0:.1} {mid:.1},{y1:.1} {x1:.1},{y1:.1} L{x1:.1},{:.1} C{mid:.1},{:.1} {mid:.1},{:.1} {x0:.1},{:.1} Z",
                    y1 + thick,
                    y1 + thick,
                    y0 + thick,
                    y0 + thick
                );
                let title = format!("{} → {}: {n}{unit}", columns[c][*a].0, columns[c + 1][*b].0);
                bands.push((d, columns[c + 1][*b].1, title));
            }
        }
    }
    rsx! {
        svg { class: "cs-chart vz-flow", view_box: "0 0 {w} {h}",
            for (i, (d, color, title)) in bands.into_iter().enumerate() {
                path { key: "band-{i}", class: "vz-band", d: "{d}", fill: "{color}", title { "{title}" } }
            }
            for c in 0..3 {
                for (s, y, tall) in placed[c].iter().copied() {
                    g { key: "stop-{c}-{s}",
                        title { "{columns[c][s].0}: {count(c, s)}{unit}" }
                        rect { x: "{xs[c]}", y: "{y:.1}", width: "{node}", height: "{tall.max(1.5):.1}", rx: "2", fill: "{columns[c][s].1}" }
                        text {
                            x: if c == 2 { format!("{:.1}", xs[c] + node + 8.0) } else { format!("{:.1}", xs[c] - 8.0) },
                            y: "{y + tall / 2.0 + 3.5:.1}",
                            text_anchor: if c == 2 { "start" } else { "end" },
                            class: "name",
                            "{columns[c][s].0}"
                            tspan { class: "n", dx: "6", "{count(c, s)}" }
                        }
                    }
                }
            }
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
