//! 메인's 탐지 tab, as an endpoint console's detections page: the Calls worth a look (stopped,
//! held, or allowed though hard to undo), the worst first, found by a search and narrowed by
//! severity; a row opened beside the list says what the engine did and why, how long from the
//! decision to the run, and where the Call came from; a row unfolded shows its session's Calls
//! around it, as a console shows the process tree.

use chrono::{Local, TimeZone};
use dioxus::prelude::*;

use super::main_page::{
    Severity, clock, decision_key, detection_kind, folder_of, kind_label, machine_of_fact,
    mode_label, source_label, source_of, stamp,
};
use super::policy::grade_of;
use crate::model::CallFact;
use crate::ui::{AppState, Screen};

/// How many rows the list shows before "더 보기".
const PAGE: usize = 40;
/// How many of the session's Calls an unfolded row shows on each side of its own.
const AROUND: usize = 6;

#[derive(Clone, Copy, PartialEq)]
enum Sort {
    Worst,
    Newest,
}

/// What the engine did with the Call, and how it ended.
fn action(f: &CallFact) -> (&'static str, &'static str) {
    match (decision_key(f), f.ran) {
        ("DENY", _) => ("bad", "막음"),
        ("REVIEW", true) => ("warn", "확인 후 실행됨"),
        ("REVIEW", false) => ("warn", "확인 요청, 실행 안 됨"),
        ("ALLOW", true) => ("ok", "허용되어 실행됨"),
        ("ALLOW", false) => ("ok", "허용됨"),
        _ => ("", "판정 없음"),
    }
}

/// A span of time as a person reads it.
fn span(ms: i64) -> String {
    let ms = ms.max(0);
    if ms < 1_000 {
        format!("{:.3}초", ms as f64 / 1000.0)
    } else if ms < 60_000 {
        format!("{:.1}초", ms as f64 / 1000.0)
    } else if ms < 3_600_000 {
        format!("{}분 {}초", ms / 60_000, ms % 60_000 / 1000)
    } else {
        format!("{}시간 {}분", ms / 3_600_000, ms % 3_600_000 / 60_000)
    }
}

fn day(ms: Option<i64>) -> String {
    ms.and_then(|ms| Local.timestamp_millis_opt(ms).single())
        .map_or("시각 없음".into(), |t| t.format("%Y.%m.%d").to_string())
}

fn what(f: &CallFact) -> String {
    f.target
        .clone()
        .or_else(|| f.tool.clone())
        .unwrap_or_else(|| "(대상 없음)".into())
}

fn copy(text: &str) {
    let text = serde_json::to_string(text).unwrap_or_default();
    document::eval(&format!("navigator.clipboard.writeText({text})"));
}

/// Whether a Call matches what was typed: its target, reason, machine, tool or folder.
fn found(f: &CallFact, needle: &str) -> bool {
    needle.is_empty()
        || [
            f.target.as_deref(),
            f.reason.as_deref(),
            f.tool.as_deref(),
            f.cwd.as_deref(),
            f.client.as_deref(),
        ]
        .into_iter()
        .flatten()
        .any(|t| t.to_lowercase().contains(needle))
}

const SEVERITIES: [Severity; 4] = [
    Severity::Critical,
    Severity::High,
    Severity::Medium,
    Severity::After,
];

#[component]
pub fn Detections(
    facts: Vec<CallFact>,
    all: Vec<CallFact>,
    picked: Signal<Option<(String, i64)>>,
    loading: bool,
) -> Element {
    let mut picked = picked;
    let mut query = use_signal(String::new);
    let mut only = use_signal(|| None::<Severity>);
    let mut sort = use_signal(|| Sort::Worst);
    let mut open = use_signal(|| None::<(String, i64)>);
    let mut shown = use_signal(|| PAGE);

    let needle = query().trim().to_lowercase();
    let worth: Vec<(Severity, CallFact)> = facts
        .iter()
        .filter_map(|f| Severity::of(f).map(|s| (s, f.clone())))
        .collect();
    let total = worth.len();
    let counts: Vec<(Severity, usize)> = SEVERITIES
        .iter()
        .map(|s| (*s, worth.iter().filter(|(w, _)| w == s).count()))
        .collect();
    let mut list: Vec<(Severity, CallFact)> = worth
        .into_iter()
        .filter(|(s, f)| only().is_none_or(|o| o == *s) && found(f, &needle))
        .collect();
    match sort() {
        Sort::Worst => list.sort_by_key(|(s, f)| std::cmp::Reverse((*s, f.at, f.seq))),
        Sort::Newest => list.sort_by_key(|(_, f)| std::cmp::Reverse((f.at, f.seq))),
    }
    let matched = list.len();
    // The row opened: the one picked if it is in the list, else the first.
    let current: Option<(Severity, CallFact)> = picked()
        .and_then(|(s, q)| {
            list.iter()
                .find(|(_, f)| f.session == s && f.seq == q)
                .cloned()
        })
        .or_else(|| list.first().cloned());
    list.truncate(shown());
    // The rows under their heading: the severity when the worst come first, else the day.
    let mut groups: Vec<(String, Vec<(Severity, CallFact)>)> = Vec::new();
    for (s, f) in list {
        let head = match sort() {
            Sort::Worst => s.tag().1.to_owned(),
            Sort::Newest => day(f.at),
        };
        match groups.last_mut() {
            Some((h, rows)) if *h == head => rows.push((s, f)),
            _ => groups.push((head, vec![(s, f)])),
        }
    }
    let narrowed = only().is_some() || !needle.is_empty();

    rsx! {
        div { class: "dt",
            div { class: "dt-head",
                h2 { "탐지" }
                span { class: "dt-count", "{matched}건" if narrowed { " (전체 {total}건)" } }
                div { class: "dt-tools",
                    input {
                        class: "dt-search",
                        r#type: "search",
                        placeholder: "명령, 이유, 단말, 폴더로 찾기",
                        value: "{query}",
                        oninput: move |e| {
                            query.set(e.value());
                            shown.set(PAGE);
                        },
                    }
                    for (s, n) in counts {
                        {
                            let (tone, label) = s.tag();
                            let on = only() == Some(s);
                            rsx! {
                                button {
                                    key: "{label}",
                                    class: if on { "dt-chip on" } else { "dt-chip" },
                                    onclick: move |_| {
                                        only.set(if only() == Some(s) { None } else { Some(s) });
                                        shown.set(PAGE);
                                    },
                                    Hex { tone }
                                    "{label}"
                                    span { class: "n", "{n}" }
                                }
                            }
                        }
                    }
                    if narrowed {
                        button {
                            class: "dt-clear",
                            onclick: move |_| {
                                only.set(None);
                                query.set(String::new());
                            },
                            "모두 지우기"
                        }
                    }
                    label { class: "dt-sort",
                        "정렬"
                        select {
                            onchange: move |e| sort.set(if e.value() == "new" { Sort::Newest } else { Sort::Worst }),
                            option { value: "worst", selected: sort() == Sort::Worst, "위험한 순서" }
                            option { value: "new", selected: sort() == Sort::Newest, "최신 순서" }
                        }
                    }
                }
            }
            div { class: "dt-split",
                div { class: "dt-list",
                    if groups.is_empty() {
                        p { class: "dt-none",
                            if loading { "불러오는 중…" } else if narrowed { "찾는 조건에 맞는 탐지가 없습니다." } else { "기간 안에 탐지가 없습니다." }
                        }
                    }
                    for (head, rows) in groups {
                        div { key: "{head}", class: "dt-group",
                            div { class: "dt-day", span { "{head}" } }
                            for (s, f) in rows {
                                {
                                    let id = (f.session.clone(), f.seq);
                                    let on = current.as_ref().is_some_and(|(_, c)| c.session == f.session && c.seq == f.seq);
                                    let unfolded = open().as_ref() == Some(&id);
                                    rsx! {
                                        Row { key: "{f.session}-{f.seq}", sev: s, f: f.clone(), on, unfolded,
                                            onpick: {
                                                let id = id.clone();
                                                move |_| picked.set(Some(id.clone()))
                                            },
                                            onfold: {
                                                let id = id.clone();
                                                move |_| open.set(if open().as_ref() == Some(&id) { None } else { Some(id.clone()) })
                                            },
                                        }
                                        if unfolded {
                                            Flow { f: f.clone(), all: all.clone() }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    if matched > shown() {
                        button { class: "dt-more", onclick: move |_| shown += PAGE, "{matched - shown()}건 더 보기" }
                    }
                }
                if let Some((s, f)) = current {
                    Detail { key: "{f.session}-{f.seq}", sev: s, f }
                }
            }
        }
    }
}

/// A severity's hexagon.
#[component]
fn Hex(tone: &'static str) -> Element {
    rsx! {
        svg { class: "hex {tone}", view_box: "0 0 16 16", width: "14", height: "14",
            polygon { points: "8,1 14.1,4.5 14.1,11.5 8,15 1.9,11.5 1.9,4.5" }
        }
    }
}

/// One detection as a row of labelled cells; it opens beside the list, and unfolds into its
/// session's Calls.
#[component]
fn Row(
    sev: Severity,
    f: CallFact,
    on: bool,
    unfolded: bool,
    onpick: EventHandler<()>,
    onfold: EventHandler<()>,
) -> Element {
    let mut state = use_context::<AppState>();
    let (tone, label) = sev.tag();
    let (act_tone, act) = action(&f);
    let kind = detection_kind(&f)
        .map(|k| kind_label(&k))
        .unwrap_or_default();
    let tool = f.tool.clone().unwrap_or_else(|| "도구 없음".into());
    let target = what(&f);
    let machine = machine_of_fact(&f);
    let folder = folder_of(f.cwd.as_deref());
    let session = f.session.clone();
    rsx! {
        div {
            class: if on { "dt-row {tone} on" } else { "dt-row {tone}" },
            onclick: move |_| onpick.call(()),
            div { class: "dt-cell sev", small { "심각도" } span { Hex { tone } "{label}" } }
            div { class: "dt-cell", small { "탐지 시각" } span { class: "num", "{clock(f.at)}" } }
            div { class: "dt-cell wide", small { "단말의 호출" } span { "{tool} · {machine}" } }
            div { class: "dt-cell", small { "관련 세션" }
                button {
                    class: "dt-link",
                    onclick: move |e: MouseEvent| {
                        e.stop_propagation();
                        state.open_session(session.clone());
                        state.mode.set(crate::ui::Mode::Graph);
                        state.screen.set(Screen::Work);
                    },
                    "세션 보기"
                }
            }
            div { class: "dt-cell kind", small { "유형" } span { title: "{kind}", "{kind}" } }
            div { class: "dt-cell wide target", small { "대상" } code { title: "{target}", "{target}" } }
            div { class: "dt-cell", small { "작업 폴더" } span { "{folder}" } }
            div { class: "dt-cell act {act_tone}", small { "조치" } span { "{act}" } }
            button {
                class: if unfolded { "dt-fold on" } else { "dt-fold" },
                title: if unfolded { "세션 흐름 접기" } else { "이 세션의 앞뒤 호출 보기" },
                onclick: move |e: MouseEvent| {
                    e.stop_propagation();
                    onfold.call(());
                },
                svg { view_box: "0 0 16 16", width: "14", height: "14",
                    path { d: "M4 6l4 4 4-4", fill: "none", stroke: "currentColor", stroke_width: "1.8", stroke_linecap: "round", stroke_linejoin: "round" }
                }
            }
        }
    }
}

/// The session's Calls around a detection, in order, the detection told in full.
#[component]
fn Flow(f: CallFact, all: Vec<CallFact>) -> Element {
    let mut calls: Vec<CallFact> = all.into_iter().filter(|c| c.session == f.session).collect();
    calls.sort_by_key(|c| c.seq);
    let at = calls.iter().position(|c| c.seq == f.seq).unwrap_or(0);
    let from = at.saturating_sub(AROUND);
    let to = (at + AROUND + 1).min(calls.len());
    let before = from;
    let after = calls.len() - to;
    let calls: Vec<CallFact> = calls[from..to].to_vec();
    rsx! {
        div { class: "dt-flow",
            if before > 0 { p { class: "dt-flow-more", "앞선 호출 {before}개" } }
            for c in calls {
                {
                    let me = c.seq == f.seq;
                    let tone = Severity::of(&c).map_or("plain", |s| s.tag().0);
                    let (act_tone, act) = action(&c);
                    let target = what(&c);
                    let kind = detection_kind(&c).map(|k| kind_label(&k));
                    rsx! {
                        div { key: "{c.seq}", class: if me { "dt-step me" } else { "dt-step" },
                            span { class: "t num", "{stamp(c.at)}" }
                            span { class: "dot {tone}" }
                            div { class: "body",
                                div { class: "line",
                                    strong { "{c.tool.clone().unwrap_or_default()}" }
                                    code { "{target}" }
                                    span { class: "act {act_tone}", "{act}" }
                                }
                                if me {
                                    div { class: "dt-me",
                                        if let Some(kind) = kind { h4 { "{kind}" } }
                                        if let Some(reason) = &c.reason { p { "{reason}" } }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            if after > 0 { p { class: "dt-flow-more", "뒤이은 호출 {after}개" } }
        }
    }
}

/// A detection told in full beside the list: what the engine did and why, from the decision to
/// the run, what was asked for, and where it came from.
#[component]
fn Detail(sev: Severity, f: CallFact) -> Element {
    let mut state = use_context::<AppState>();
    let (tone, label) = sev.tag();
    let (act_tone, act) = action(&f);
    let kind = detection_kind(&f)
        .map(|k| kind_label(&k))
        .unwrap_or_else(|| "-".into());
    let target = what(&f);
    let machine = machine_of_fact(&f);
    let tool = f.tool.clone().unwrap_or_else(|| "도구 없음".into());
    let grade = grade_of(&f).short();
    let (session, seq) = (f.session.clone(), f.seq);
    let open_map = {
        let session = session.clone();
        move |_| {
            state.open_call(session.clone(), seq);
            state.screen.set(Screen::Work);
        }
    };
    let took = f.ran_at.zip(f.at).map(|(r, a)| span(r - a));
    let copy_target = target.clone();
    let copy_session = session.clone();
    let cwd = f.cwd.clone().unwrap_or_default();
    let copy_cwd = cwd.clone();
    rsx! {
        aside { class: "dt-detail",
            div { class: "dt-d-time num", "{stamp(f.at)}" }
            div { class: "dt-d-title",
                Hex { tone }
                strong { title: "{target}", "{tool} · {machine}" }
                button { class: "dt-primary", onclick: open_map, "노선도에서 보기" }
            }
            div { class: "dt-d-acts",
                button {
                    onclick: move |_| {
                        state.open_session(session.clone());
                        state.mode.set(crate::ui::Mode::Graph);
                        state.screen.set(Screen::Work);
                    },
                    "세션 열기"
                }
                button { disabled: true, title: "엔진의 정지 기능이 생기면 이 세션의 모든 도구 호출을 막습니다.", "세션 정지" }
                button { onclick: move |_| state.screen.set(Screen::FeatureB), "정책으로" }
            }
            section { class: "dt-sec",
                h3 { "판정에서 실행까지" }
                div { class: "dt-run",
                    div { class: "ends",
                        span { class: "num", "{clock(f.at)}" }
                        span { class: "num", if let Some(r) = f.ran_at { "{clock(Some(r))}" } else { "실행 안 됨" } }
                    }
                    div { class: if f.ran { "track" } else { "track stopped" },
                        i { class: "start" }
                        i { class: "arrow" }
                        i { class: "end" }
                    }
                    div { class: "mid num", if let Some(t) = took { "{t}" } else if f.ran { "실행됨" } else { "판정 뒤 실행되지 않았습니다" } }
                }
                div { class: "dt-facts",
                    div { small { "심각도" } span { class: "sev {tone}", i {} "{label}" } }
                    div { small { "엔진이 한 일" } span { class: "act {act_tone}", "{act}" } }
                    div { small { "유형" } span { "{kind}" } }
                    div { small { "판정 방식" } span { "{mode_label(f.mode.as_deref())}" } }
                    div { small { "규칙" } span { "{f.rule.clone().unwrap_or_else(|| \"-\".into())}" } }
                    div { small { "등급(초안)" } span { "{grade}" } }
                }
            }
            section { class: "dt-sec",
                h3 { "판정 이유" }
                p { class: "dt-why", "{f.reason.clone().unwrap_or_else(|| \"엔진이 남긴 이유가 없습니다.\".into())}" }
            }
            section { class: "dt-sec",
                h3 { "실행하려던 것" }
                div { class: "dt-code",
                    code { "{target}" }
                    button { title: "복사", onclick: move |_| copy(&copy_target), {copy_icon()} }
                }
                if !cwd.is_empty() {
                    small { class: "dt-label", "작업 폴더" }
                    div { class: "dt-code",
                        code { "{cwd}" }
                        button { title: "복사", onclick: move |_| copy(&copy_cwd), {copy_icon()} }
                    }
                }
            }
            section { class: "dt-sec",
                h3 { "어디서 왔나" }
                div { class: "dt-facts",
                    div { small { "호출을 시킨 것" } span { "{source_label(source_of(&f))}" } }
                    div { small { "단말" } span { "{machine}" } }
                    div { small { "순번" } span { class: "num", "{seq}" } }
                }
                small { class: "dt-label", "세션" }
                div { class: "dt-code",
                    code { "{f.session}" }
                    button { title: "복사", onclick: move |_| copy(&copy_session), {copy_icon()} }
                }
            }
        }
    }
}

fn copy_icon() -> Element {
    rsx! {
        svg { view_box: "0 0 16 16", width: "14", height: "14",
            rect { x: "5", y: "5", width: "8.5", height: "8.5", rx: "1.5", fill: "none", stroke: "currentColor", stroke_width: "1.4" }
            path { d: "M10.5 3V2.5A1 1 0 0 0 9.5 1.5h-6a1 1 0 0 0-1 1v6a1 1 0 0 0 1 1H4", fill: "none", stroke: "currentColor", stroke_width: "1.4" }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spans_read_as_a_person_says_them() {
        assert_eq!(span(58), "0.058초");
        assert_eq!(span(5_915), "5.9초");
        assert_eq!(span(192_000), "3분 12초");
        assert_eq!(span(-5), "0.000초");
    }

    #[test]
    fn the_action_says_how_the_call_ended() {
        let f = CallFact {
            decision: Some("REVIEW".into()),
            ran: true,
            ..CallFact::default()
        };
        assert_eq!(action(&f).1, "확인 후 실행됨");
        let f = CallFact {
            decision: Some("DENY".into()),
            ..CallFact::default()
        };
        assert_eq!(action(&f).1, "막음");
    }

    #[test]
    fn the_search_looks_at_what_a_person_would_type() {
        let f = CallFact {
            target: Some("rm -rf data/backups".into()),
            client: Some("dev-mac-07".into()),
            ..CallFact::default()
        };
        assert!(found(&f, "backups"));
        assert!(found(&f, "mac-07"));
        assert!(!found(&f, "push"));
        assert!(found(&f, ""));
    }
}
