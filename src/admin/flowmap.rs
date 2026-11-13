//! 기록 as a flow diagram, top to bottom: each turn's request and its Calls are cards, named at
//! their left, holding what they carry as chips, and joined by wires that say what each Call
//! followed.
//! - A blue wire runs from the request down through the Calls made for it.
//! - A pink wire leaves a Call whose result told the agent to make another, and enters that
//!   one, which stands to the right of the blue line: what a tool result pulled in.
//! - A Call nothing told stands on neither.
//! - A Call the engine stopped or held has a dark foot saying which of its three verdicts it
//!   gave, and a six-sided block beside it saying what it found and by which rule, wired in black.
//!
//! [`lay`] works out where everything stands; the component draws cards as HTML over one SVG of
//! wires. A card clicked is the app's selection.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use dioxus::prelude::*;

use super::main_page::{clock, kind_label};
use super::route::{Leg, legs};
use crate::flow::{Flow, Glyph};
use crate::model::parse_json;
use crate::ui::{AppState, Load, Selection};

const CARD_H: f64 = 46.0;
const FOOT_H: f64 = 32.0;
const GAP: f64 = 30.0;
const TURN_GAP: f64 = 58.0;
/// Where cards on the request's line start, where those a tool result pulled in start, and
/// how far right of its Call the engine's finding stands.
const MAIN_X: f64 = 250.0;
const SIDE_X: f64 = 410.0;
const FOUND_GAP: f64 = 104.0;
const PAD: f64 = 8.0;
const ICON_W: f64 = 38.0;
const CHIP_GAP: f64 = 6.0;

/// What a chip or a wire stands for, which is its colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Thread {
    /// The person's request.
    Asked,
    /// What a tool result said.
    Result,
    /// What the Call acted on, or the engine's finding.
    Plain,
    /// Nothing told the Call.
    Loose,
}

impl Thread {
    fn class(self) -> &'static str {
        match self {
            Thread::Asked => "asked",
            Thread::Result => "result",
            Thread::Plain => "plain",
            Thread::Loose => "loose",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct Chip {
    thread: Thread,
    text: String,
    w: f64,
}

#[derive(Clone, Debug, PartialEq)]
struct Card {
    seq: Option<i64>,
    x: f64,
    y: f64,
    w: f64,
    name: String,
    sub: String,
    glyph: Option<Glyph>,
    chips: Vec<Chip>,
    /// The verdict the engine gave, when it stopped or held the Call.
    foot: Option<&'static str>,
    title: String,
}

impl Card {
    fn h(&self) -> f64 {
        CARD_H + if self.foot.is_some() { FOOT_H } else { 0.0 }
    }

    /// The middle of its chip `i`, along the card.
    fn chip_x(&self, i: usize) -> f64 {
        let before: f64 = self.chips[..i].iter().map(|c| c.w + CHIP_GAP).sum();
        self.x + PAD + ICON_W + before + self.chips[i].w / 2.0
    }
}

/// What the engine found in a Call it stopped or held, beside the Call.
#[derive(Clone, Debug, PartialEq)]
struct Found {
    x: f64,
    y: f64,
    w: f64,
    rule: String,
    chips: Vec<Chip>,
}

#[derive(Clone, Debug, PartialEq)]
struct Wire {
    thread: Thread,
    d: String,
    /// Its ends, each a port drawn on a card's edge.
    ports: Vec<(f64, f64)>,
    /// The Calls it joins, to draw it heavier when one is picked.
    ends: (Option<i64>, Option<i64>),
}

#[derive(Clone, Debug, Default, PartialEq)]
struct Diagram {
    cards: Vec<Card>,
    found: Vec<Found>,
    wires: Vec<Wire>,
    w: f64,
    h: f64,
}

/// About how wide a line of 12px text is.
fn wide(text: &str) -> f64 {
    text.chars()
        .map(|c| if c.is_ascii() { 6.8 } else { 11.8 })
        .sum()
}

fn cut(text: &str, most: usize) -> String {
    let line = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if line.chars().count() <= most {
        line
    } else {
        format!("{}…", line.chars().take(most).collect::<String>())
    }
}

fn chip(thread: Thread, text: String) -> Chip {
    let w = (wide(&text) + 22.0).clamp(52.0, 250.0);
    Chip { thread, text, w }
}

fn card(seq: Option<i64>, x: f64, y: f64, chips: Vec<Chip>) -> Card {
    let inner: f64 =
        chips.iter().map(|c| c.w).sum::<f64>() + CHIP_GAP * chips.len().saturating_sub(1) as f64;
    Card {
        seq,
        x,
        y,
        w: PAD + ICON_W + inner + PAD,
        name: String::new(),
        sub: String::new(),
        glyph: None,
        chips,
        foot: None,
        title: String::new(),
    }
}

/// A wire that leaves (x0, y0) downwards and enters (x1, y1) from above.
fn drop(x0: f64, y0: f64, x1: f64, y1: f64) -> String {
    if (x0 - x1).abs() < 0.5 {
        return format!("M{x0:.1},{y0:.1} L{x1:.1},{y1:.1}");
    }
    let bend = ((y1 - y0) / 2.0).clamp(18.0, 70.0);
    format!(
        "M{x0:.1},{y0:.1} C{x0:.1},{:.1} {x1:.1},{:.1} {x1:.1},{:.1} L{x1:.1},{y1:.1}",
        y0 + bend,
        y0 + bend * 0.35,
        y0 + bend * 1.35,
    )
}

/// The diagram of the turns given. `why` has, for each Call the engine stopped or held, what
/// it found and the rule that decided.
fn lay(legs: &[Leg], why: &HashMap<i64, (String, String)>) -> Diagram {
    let mut d = Diagram::default();
    // The Calls whose results told the agent to make others: they show that result as a chip.
    let tellers: HashSet<i64> = legs
        .iter()
        .flat_map(|l| l.calls.iter().filter_map(|c| c.by))
        .collect();
    let mut y = 34.0;
    for leg in legs {
        let mut ask = card(
            None,
            MAIN_X,
            y,
            vec![
                chip(Thread::Asked, format!("요청 {}", leg.n)),
                chip(Thread::Plain, cut(&leg.req, 44)),
            ],
        );
        ask.name = "요청".into();
        ask.sub = clock(leg.asked);
        ask.title = leg.req.clone();
        ask.glyph = Some(Glyph::Agent);
        // The request's line: where it left the last card on it.
        let mut line = (ask.chip_x(0), ask.y + CARD_H, None::<i64>);
        // Where each Call of this turn shows its result, and the last card a result's line reached.
        let mut out: HashMap<i64, (f64, f64)> = HashMap::new();
        let mut reached: Option<(i64, f64, f64, i64)> = None;
        y += CARD_H + GAP;
        d.cards.push(ask);
        for c in &leg.calls {
            let stopped = c.blocked || c.held;
            let thread = if c.by.is_some() {
                Thread::Result
            } else if c.loose {
                Thread::Loose
            } else {
                Thread::Asked
            };
            let first = match thread {
                Thread::Result => chip(thread, cut(c.via.as_deref().unwrap_or("도구 결과"), 20)),
                Thread::Loose => chip(thread, "시킨 것 없음".into()),
                _ => chip(thread, format!("요청 {}", leg.n)),
            };
            let mut chips = vec![
                first,
                chip(
                    Thread::Plain,
                    cut(if c.arg.is_empty() { &c.title } else { &c.arg }, 30),
                ),
            ];
            if tellers.contains(&c.seq) {
                chips.push(chip(Thread::Result, "읽은 내용".into()));
            }
            // A finding stands above and to the right of its Call, so the Call starts lower.
            let found_y = y;
            if stopped && why.contains_key(&c.seq) {
                y += 44.0;
            }
            let mut k = card(
                Some(c.seq),
                if thread == Thread::Result {
                    SIDE_X
                } else {
                    MAIN_X
                },
                y,
                chips,
            );
            k.name = c.tool.clone();
            k.sub = clock(c.at);
            k.glyph = Some(c.glyph);
            k.title = c.title.clone();
            k.foot = if c.blocked {
                Some("차단")
            } else if c.held {
                Some("확인 요청")
            } else {
                None
            };
            let port = k.chip_x(0);
            match thread {
                Thread::Asked => {
                    d.wires.push(Wire {
                        thread,
                        d: drop(line.0, line.1, port, k.y),
                        ports: vec![(line.0, line.1), (port, k.y)],
                        ends: (line.2, Some(c.seq)),
                    });
                    line = (port, k.y + k.h(), Some(c.seq));
                }
                Thread::Result => {
                    // From the last card on the same result's line, else from the result itself.
                    let from = reached
                        .filter(|r| Some(r.0) == c.by)
                        .map(|r| (r.1, r.2, Some(r.3)))
                        .or_else(|| c.by.and_then(|b| out.get(&b).map(|o| (o.0, o.1, Some(b)))));
                    if let Some((fx, fy, from_seq)) = from {
                        d.wires.push(Wire {
                            thread,
                            d: drop(fx, fy, port, k.y),
                            ports: vec![(fx, fy), (port, k.y)],
                            ends: (from_seq, Some(c.seq)),
                        });
                    }
                    reached = c.by.map(|b| (b, port, k.y + k.h(), c.seq));
                }
                _ => {}
            }
            if tellers.contains(&c.seq) {
                out.insert(c.seq, (k.chip_x(k.chips.len() - 1), k.y + k.h()));
            }
            if let (true, Some((what, rule))) = (stopped, why.get(&c.seq)) {
                let chips = vec![
                    chip(Thread::Plain, what.clone()),
                    chip(Thread::Plain, k.foot.unwrap_or("").to_owned()),
                ];
                let w = 30.0 + chips.iter().map(|c| c.w).sum::<f64>() + CHIP_GAP;
                // Far enough right of its Call that its name fits between them.
                let f = Found {
                    x: k.x + k.w + FOUND_GAP,
                    y: found_y,
                    w,
                    rule: rule.clone(),
                    chips,
                };
                // Out of the finding's foot, round to the top of the Call's right end.
                let (fx, fy) = (f.x + f.w * 0.62, f.y + 40.0);
                let (tx, ty) = (k.x + k.w - 26.0, k.y);
                d.wires.push(Wire {
                    thread: Thread::Plain,
                    d: format!(
                        "M{fx:.1},{fy:.1} C{fx:.1},{:.1} {tx:.1},{:.1} {tx:.1},{ty:.1}",
                        fy + 34.0,
                        ty - 40.0
                    ),
                    ports: vec![(fx, fy), (tx, ty)],
                    ends: (None, Some(c.seq)),
                });
                d.w = d.w.max(f.x + f.w);
                d.found.push(f);
            }
            d.w = d.w.max(k.x + k.w);
            y += k.h() + GAP;
            d.cards.push(k);
        }
        y += TURN_GAP - GAP;
    }
    d.w += 60.0;
    d.h = y;
    d
}

/// What the engine found in each Call it stopped or held, and the rule that decided: the kind
/// of deviation it scored highest, from the Call's trace.
fn findings(state: AppState, legs: &[Leg]) -> HashMap<i64, (String, String)> {
    let Load::Ready(model) = &*state.graph.read() else {
        return HashMap::new();
    };
    legs.iter()
        .flat_map(|l| &l.calls)
        .filter(|c| c.blocked || c.held)
        .filter_map(|c| {
            let node = model.node(c.seq)?;
            let rule = node
                .props
                .get("decision_rule")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let worst = parse_json(node.props.get("trace_json"))["deviation"]["worst"]
                .as_str()
                .filter(|w| !w.is_empty() && *w != "none")
                .map(str::to_owned);
            let what = if rule == "safeguard::change" {
                kind_label("safeguard")
            } else {
                kind_label(worst.as_deref().unwrap_or("other"))
            };
            // The kind's name without its gloss in brackets, which the list beside gives.
            let short = what.split('(').next().unwrap_or(&what).trim().to_owned();
            Some((c.seq, (cut(&short, 16), rule.to_owned())))
        })
        .collect()
}

fn icon(g: Glyph) -> Element {
    rsx! {
        svg { width: "17", height: "17", view_box: "0 0 16 16", fill: "none", stroke: "currentColor", stroke_width: "1.25", stroke_linecap: "round", stroke_linejoin: "round",
            match g {
                Glyph::Doc => rsx! { path { d: "M4 1.5h5.5L13 5v9.5H4z M9.5 1.5V5H13 M6.5 8.5h4 M6.5 11.5h4" } },
                Glyph::Search => rsx! { circle { cx: "7", cy: "7", r: "4.5" } path { d: "M10.5 10.5L14.5 14.5" } },
                Glyph::Edit => rsx! { path { d: "M2.5 13.5l1-3.8 7.8-7.8 2.8 2.8-7.8 7.8z M9.6 3.6l2.8 2.8" } },
                Glyph::Run => rsx! { path { d: "M2.5 4l4 4-4 4 M8.5 12.5h5" } },
                Glyph::Web => rsx! { circle { cx: "8", cy: "8", r: "6.2" } path { d: "M1.8 8h12.4 M8 1.8c-2.2 2.4-2.2 10 0 12.4 M8 1.8c2.2 2.4 2.2 10 0 12.4" } },
                Glyph::Plug => rsx! { path { d: "M5.5 1.5v3.5 M10.5 1.5v3.5 M3.5 5h9v3a4.5 4.5 0 0 1-9 0z M8 12.5v2.5" } },
                Glyph::Agent => rsx! { circle { cx: "8", cy: "5", r: "2.8" } path { d: "M2.5 14.5c.6-3 2.8-4.6 5.5-4.6s4.9 1.6 5.5 4.6" } },
                Glyph::Other => rsx! { path { d: "M4 8h.1 M8 8h.1 M12 8h.1" } },
            }
        }
    }
}

/// 기록's stage in the security team's app: the flow diagram, or the map of blocks seen at an
/// angle, switched at the top right.
#[component]
pub fn Record(flow: Arc<Flow>, turns: Vec<usize>, title: String, detail: Signal<bool>) -> Element {
    let mut blocks = use_signal(|| false);
    rsx! {
        if blocks() {
            super::route::RouteScene { flow, turns, title, detail }
        } else {
            FlowDiagram { flow, turns, detail }
        }
        div { class: "fd-switch",
            button { class: if blocks() { "" } else { "on" }, onclick: move |_| blocks.set(false), "흐름도" }
            button { class: if blocks() { "on" } else { "" }, onclick: move |_| blocks.set(true), "3D 지도" }
        }
    }
}

#[component]
fn FlowDiagram(flow: Arc<Flow>, turns: Vec<usize>, detail: Signal<bool>) -> Element {
    let mut state = use_context::<AppState>();
    let mut detail = detail;
    let legs = legs(&flow, &turns);
    let why = findings(state, &legs);
    let d = lay(&legs, &why);
    let picked = match (state.selected)() {
        Selection::Node(seq) => Some(seq),
        Selection::None => None,
    };
    // The card picked is brought into view, from wherever it was picked.
    use_effect(use_reactive!(|picked| {
        if picked.is_some() {
            document::eval(
                r#"document.querySelector(".fd-card.on")?.scrollIntoView({ block: "nearest", inline: "nearest", behavior: "smooth" });"#,
            );
        }
    }));
    rsx! {
        div { class: "fd-stage",
            div { class: "fd-canvas", style: "width:{d.w:.0}px;height:{d.h:.0}px",
                svg { class: "fd-wires", width: "{d.w:.0}", height: "{d.h:.0}", view_box: "0 0 {d.w:.0} {d.h:.0}",
                    for (i, w) in d.wires.iter().enumerate() {
                        {
                            let hot = picked.is_some() && (w.ends.0 == picked || w.ends.1 == picked);
                            rsx! {
                                path { key: "{i}", class: if hot { "{w.thread.class()} hot" } else { "{w.thread.class()}" }, d: "{w.d}" }
                            }
                        }
                    }
                    for f in d.found.iter() {
                        polygon { key: "{f.y}", class: "found",
                            points: "{f.x + 12.0:.1},{f.y:.1} {f.x + f.w - 12.0:.1},{f.y:.1} {f.x + f.w:.1},{f.y + 20.0:.1} {f.x + f.w - 12.0:.1},{f.y + 40.0:.1} {f.x + 12.0:.1},{f.y + 40.0:.1} {f.x:.1},{f.y + 20.0:.1}" }
                    }
                }
                for c in d.cards.iter().cloned() {
                    {
                        let on = c.seq.is_some() && c.seq == picked;
                        let seq = c.seq;
                        rsx! {
                            div { key: "n{c.y}", class: "fd-name", style: "width:{c.x - 14.0:.0}px;top:{c.y + 7.0:.0}px",
                                strong { "{c.name}" }
                                small { "{c.sub}" }
                            }
                            button {
                                class: if on { "fd-card on" } else if seq.is_none() { "fd-card ask" } else { "fd-card" },
                                style: "left:{c.x:.0}px;top:{c.y:.0}px;width:{c.w:.0}px;height:{c.h():.0}px",
                                title: "{c.title}",
                                onclick: move |_| if let Some(seq) = seq {
                                    state.selected.set(Selection::Node(seq));
                                    detail.set(true);
                                },
                                span { class: "row",
                                    span { class: "ico", if let Some(g) = c.glyph { {icon(g)} } }
                                    for (i, chip) in c.chips.iter().enumerate() {
                                        span { key: "{i}", class: "chip {chip.thread.class()}", style: "width:{chip.w:.0}px", "{chip.text}" }
                                    }
                                }
                                if let Some(foot) = c.foot {
                                    span { class: "foot",
                                        for v in ["허용", "확인 요청", "차단"] {
                                            span { key: "{v}", class: if v == foot { "on" } else { "" }, "{v}" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                for f in d.found.iter().cloned() {
                    div { key: "fn{f.y}", class: "fd-name", style: "width:{f.x - 14.0:.0}px;top:{f.y + 5.0:.0}px",
                        strong { "엔진 판정" }
                        small { "{f.rule}" }
                    }
                    div { class: "fd-found", style: "left:{f.x:.0}px;top:{f.y:.0}px;width:{f.w:.0}px",
                        for (i, chip) in f.chips.iter().enumerate() {
                            span { key: "{i}", class: "chip plain", style: "width:{chip.w:.0}px", "{chip.text}" }
                        }
                    }
                }
                // The ports are drawn over the cards' edges, above everything.
                svg { class: "fd-ports", width: "{d.w:.0}", height: "{d.h:.0}", view_box: "0 0 {d.w:.0} {d.h:.0}",
                    for (i, w) in d.wires.iter().enumerate() {
                        for (j, (x, y)) in w.ports.iter().enumerate() {
                            if w.thread == Thread::Plain {
                                rect { key: "{i}-{j}", class: "plain", x: "{x - 4.0:.1}", y: "{y - 4.0:.1}", width: "8", height: "8", transform: "rotate(45 {x:.1} {y:.1})" }
                            } else {
                                circle { key: "{i}-{j}", class: "{w.thread.class()}", cx: "{x:.1}", cy: "{y:.1}", r: "4.4" }
                            }
                        }
                    }
                }
            }
            div { class: "fd-keys",
                span { i { class: "asked" } "요청을 따라 한 호출" }
                span { i { class: "result" } "도구 결과가 시킨 호출" }
                span { i { class: "plain" } "엔진의 판정" }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::route::Step;
    use super::*;

    fn step(seq: i64, by: Option<i64>) -> Step {
        Step {
            seq,
            kind: "run",
            tool: "Bash".into(),
            title: format!("call {seq}"),
            arg: String::new(),
            via: by.map(|_| "README.md".into()),
            glyph: Glyph::Run,
            at: None,
            by,
            blocked: false,
            held: false,
            loose: false,
        }
    }

    #[test]
    fn calls_stand_on_the_line_of_what_told_them() {
        let mut stopped = step(3, Some(1));
        stopped.blocked = true;
        let mut loose = step(4, None);
        loose.loose = true;
        let leg = Leg {
            turn: 0,
            n: 1,
            req: "고쳐줘".into(),
            intent: None,
            asked: None,
            calls: vec![step(1, None), step(2, None), stopped, loose],
        };
        let why = HashMap::from([(3, ("목표 탈취".to_owned(), "intent::mismatch".to_owned()))]);
        let d = lay(&[leg], &why);
        let of = |seq: i64| d.cards.iter().find(|c| c.seq == Some(seq)).unwrap();
        assert_eq!(d.cards.len(), 5, "the request and its four Calls");
        assert_eq!((of(1).x, of(2).x, of(4).x), (MAIN_X, MAIN_X, MAIN_X));
        assert_eq!(of(3).x, SIDE_X, "a Call a result told stands to the right");
        assert_eq!(
            of(1).chips.last().unwrap().text,
            "읽은 내용",
            "the Call whose result told another shows it"
        );
        assert_eq!(of(3).foot, Some("차단"));
        assert_eq!(of(3).h(), CARD_H + FOOT_H);
        // Blue from the request to 1 and from 1 to 2; pink from 1 to 3; black from the finding to 3.
        let ends: Vec<_> = d.wires.iter().map(|w| (w.thread, w.ends)).collect();
        assert_eq!(
            ends,
            [
                (Thread::Asked, (None, Some(1))),
                (Thread::Asked, (Some(1), Some(2))),
                (Thread::Result, (Some(1), Some(3))),
                (Thread::Plain, (None, Some(3))),
            ]
        );
        assert_eq!(d.found.len(), 1);
        assert!(d.found[0].y < of(3).y, "the finding stands above its Call");
        assert!(
            d.cards.windows(2).all(|p| p[0].y + p[0].h() < p[1].y),
            "cards do not overlap"
        );
        // The pink wire leaves the first Call's result chip, at its foot.
        assert_eq!(d.wires[2].ports[0], (of(1).chip_x(2), of(1).y + CARD_H));
    }
}
