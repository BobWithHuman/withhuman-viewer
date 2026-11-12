//! The charts the security team's dashboards share, drawn as inline SVG or plain boxes: bars
//! stacked over time, a sparkline, a week's hours as a grid, a bar of shares with its legend,
//! and the layout of a treemap. Each mark says its numbers in a tooltip.

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

/// A tile of a treemap, in percent of the box it fills.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Tile {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

/// Lays out counts as tiles of a `w` by `h` box, each tile's area its count, in percent of the
/// box. The counts go in the order given, the largest first reading best, one at a time: each
/// takes the left part of what is left while that is well wider than tall, else its top part.
/// A big count then stands as a column beside the rest, which stack above each other, and small
/// counts stack instead of thinning into slivers.
pub(super) fn treemap(counts: &[usize], w: f64, h: f64) -> Vec<Tile> {
    let mut left = counts.iter().sum::<usize>() as f64;
    let (mut x, mut y, mut bw, mut bh) = (0.0, 0.0, w, h);
    let mut out = Vec::with_capacity(counts.len());
    for (i, &n) in counts.iter().enumerate() {
        let share = if left > 0.0 { n as f64 / left } else { 0.0 };
        let tile = if i + 1 == counts.len() {
            Tile { x, y, w: bw, h: bh }
        } else if bw > 1.8 * bh {
            let tw = bw * share;
            let t = Tile { x, y, w: tw, h: bh };
            x += tw;
            bw -= tw;
            t
        } else {
            let th = bh * share;
            let t = Tile { x, y, w: bw, h: th };
            y += th;
            bh -= th;
            t
        };
        out.push(Tile {
            x: tile.x / w * 100.0,
            y: tile.y / h * 100.0,
            w: tile.w / w * 100.0,
            h: tile.h / h * 100.0,
        });
        left -= n as f64;
    }
    out
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

#[cfg(test)]
mod tests {
    use super::*;

    fn area(t: &Tile) -> f64 {
        t.w * t.h
    }

    #[test]
    fn a_treemap_fills_its_box_with_tiles_as_big_as_their_counts() {
        let counts = [11, 9, 7, 3];
        let tiles = treemap(&counts, 300.0, 140.0);
        assert_eq!(tiles.len(), 4);
        let whole: f64 = tiles.iter().map(area).sum();
        assert!(
            (whole - 100.0 * 100.0).abs() < 1e-6,
            "the tiles fill the box"
        );
        for (t, n) in tiles.iter().zip(counts) {
            assert!(
                (area(t) / whole - n as f64 / 30.0).abs() < 1e-9,
                "a tile's area is its count"
            );
            assert!(t.x >= 0.0 && t.y >= 0.0);
            assert!(t.x + t.w <= 100.0 + 1e-9 && t.y + t.h <= 100.0 + 1e-9);
        }
        // The largest stands as a column down the left; the next lies across the top of the rest.
        assert_eq!((tiles[0].x, tiles[0].y, tiles[0].h), (0.0, 0.0, 100.0));
        assert!((tiles[1].x - tiles[0].w).abs() < 1e-9 && tiles[1].y == 0.0);
        assert!(
            tiles[1].w * 300.0 > tiles[1].h * 140.0,
            "the second is wider than tall"
        );
    }

    #[test]
    fn small_counts_stack_rather_than_thin_into_slivers() {
        let tiles = treemap(&[25, 2, 2, 1], 300.0, 140.0);
        // Beside the big column the rest is narrow, so each small count takes a full-width row.
        for t in &tiles[1..] {
            assert!(
                (t.w - tiles[1].w).abs() < 1e-9,
                "the small ones share a width"
            );
        }
        assert_eq!(
            treemap(&[5], 300.0, 140.0)[0],
            Tile {
                x: 0.0,
                y: 0.0,
                w: 100.0,
                h: 100.0
            }
        );
        assert!(treemap(&[], 300.0, 140.0).is_empty());
    }

    #[test]
    fn counts_close_together_stack_beside_the_largest_rather_than_stand_in_a_row() {
        let tiles = treemap(&[9, 8, 8], 300.0, 136.0);
        assert_eq!(tiles[0].h, 100.0, "the largest is a column");
        assert_eq!((tiles[1].x, tiles[2].x), (tiles[0].w, tiles[0].w));
        assert!(tiles[2].y > 0.0, "the other two lie one above the other");
    }
}
