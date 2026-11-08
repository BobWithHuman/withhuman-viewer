//! The main page, in a notes app's light: a short opening over a soft sky with where the data
//! comes from and the engine's numbers, then the sessions worth a look (those with a Call
//! stopped, then those with one held for review), each opening on the route map. The app's
//! features live in the sidebar, so the page keeps to the sessions.

use dioxus::prelude::*;

use super::{AppState, Load, Mode, Screen};
use crate::model::Session;
use crate::settings::Engine;

/// A number as the page writes it large: 1,234 → "1.2k".
fn big(n: i64) -> String {
    match n {
        n if n >= 10_000 => format!("{}k", n / 1000),
        n if n >= 1000 => format!("{:.1}k", n as f64 / 1000.0),
        n => n.to_string(),
    }
}

/// When a session was last active, as a list writes it: today's time, else the date.
fn when(ms: Option<i64>) -> String {
    use chrono::{Local, TimeZone};
    let Some(t) = ms.and_then(|ms| Local.timestamp_millis_opt(ms).single()) else {
        return String::new();
    };
    if t.date_naive() == Local::now().date_naive() {
        t.format("오늘 %H:%M").to_string()
    } else {
        t.format("%-m월 %-d일").to_string()
    }
}

/// How many sessions of each kind the page lists before "더 보기".
const SHOWN: usize = 8;

#[component]
pub fn Home() -> Element {
    let mut state = use_context::<AppState>();
    let p = (state.public)();
    let sessions: Vec<Session> = match &*state.sessions.read() {
        Load::Ready(list) => list.clone(),
        _ => Vec::new(),
    };
    let loading = matches!(*state.sessions.read(), Load::Loading);
    let calls: i64 = sessions.iter().map(|s| s.calls).sum();
    let denied: i64 = sessions.iter().map(|s| s.denied).sum();
    let reviewed: i64 = sessions.iter().map(|s| s.reviewed).sum();
    let source = if p.demo {
        "데모 데이터".to_owned()
    } else {
        match p.engine {
            Engine::Local => format!("이 맥의 엔진 · {}", p.engine_dir.clone().unwrap_or_default()),
            Engine::Server => format!("서버 · {}", p.neo4j_url),
            Engine::Unset => format!("Neo4j · {}", p.neo4j_url),
        }
    };
    let numbers = [
        ("기록된 세션", sessions.len() as i64),
        ("도구 호출", calls),
        ("차단된 호출", denied),
        ("확인을 요청한 호출", reviewed),
    ];
    // The list comes newest first; the sessions worth a look keep that order.
    let stopped: Vec<Session> = sessions.iter().filter(|s| s.denied > 0).cloned().collect();
    let held: Vec<Session> = sessions.iter().filter(|s| s.denied == 0 && s.reviewed > 0).cloned().collect();
    let mut more_stopped = use_signal(|| false);
    let mut more_held = use_signal(|| false);
    let (empty_stopped, empty_held) = if loading {
        ("세션을 불러오는 중…", "세션을 불러오는 중…")
    } else {
        ("차단된 호출이 있었던 세션이 없습니다.", "확인을 요청한 호출이 있었던 세션이 없습니다.")
    };

    rsx! {
        div { class: "home",
            section { class: "home-hero",
                div { class: "home-sky" }
                div { class: "home-hero-in",
                    p { class: "home-chip",
                        span { class: if p.demo { "dot demo" } else { "dot" } }
                        "{source}"
                    }
                    h1 { class: "home-title", "주목할 세션" }
                    p { class: "home-lede", "차단되거나 확인을 요청한 도구 호출이 있었던 세션을 모았습니다. 누르면 그 세션의 노선도가 열리고, 차단된 호출을 누르면 원인 경로가 보입니다." }
                    div { class: "home-stats",
                        for (label, n) in numbers {
                            div { key: "{label}", class: "home-stat",
                                span { class: "v", if loading { "…" } else { "{big(n)}" } }
                                span { class: "k", "{label}" }
                            }
                        }
                    }
                }
            }
            SessionGroup {
                title: "차단이 있었던 세션",
                empty: empty_stopped,
                sessions: stopped.clone(),
                all: more_stopped(),
                onmore: move |_| more_stopped.toggle(),
            }
            SessionGroup {
                title: "확인을 요청한 세션",
                empty: empty_held,
                sessions: held.clone(),
                all: more_held(),
                onmore: move |_| more_held.toggle(),
            }
            footer { class: "home-foot",
                div {
                    strong { "연결" }
                    span { "{source}" }
                }
                div {
                    strong { "Jev 키" }
                    span { if p.jev_saved { "등록됨 · 키체인" } else { "등록되지 않음" } }
                }
                div { class: "acts",
                    button { class: "home-soft small", onclick: move |_| state.settings_open.set(true), "연결 설정" }
                    button { class: "home-soft small", onclick: move |_| super::setup::shrink(state), "처음 설정 다시 하기" }
                }
            }
        }
    }
}

/// A kind of session worth a look, as a list on a white card: the first few, then all.
#[component]
fn SessionGroup(title: &'static str, empty: &'static str, sessions: Vec<Session>, all: bool, onmore: EventHandler<MouseEvent>) -> Element {
    let mut state = use_context::<AppState>();
    let total = sessions.len();
    let shown: Vec<Session> = sessions.into_iter().take(if all { usize::MAX } else { SHOWN }).collect();
    rsx! {
        section { class: "home-group",
            div { class: "home-group-head",
                h2 { "{title}" }
                span { class: "n", "{total}" }
            }
            div { class: "home-list",
                if shown.is_empty() {
                    p { class: "home-empty", "{empty}" }
                }
                for s in shown {
                    {
                        let id = s.id.clone();
                        let folder = s.folder();
                        let tone = if s.denied > 0 { "red" } else { "amber" };
                        rsx! {
                            button {
                                key: "{id}",
                                class: "home-row",
                                title: "{s.title()}",
                                onclick: move |_| {
                                    state.open_session(id.clone());
                                    state.mode.set(Mode::Graph);
                                    state.screen.set(Screen::Work);
                                },
                                span { class: "doc {tone}" }
                                span { class: "t", "{s.title()}" }
                                span { class: "chips",
                                    if !folder.is_empty() { span { class: "chip", "{folder}" } }
                                    span { class: "chip", "호출 {s.calls}" }
                                    if s.denied > 0 { span { class: "chip red", "차단 {s.denied}" } }
                                    if s.reviewed > 0 { span { class: "chip amber", "확인 {s.reviewed}" } }
                                }
                                span { class: "when", "{when(s.last_at.or(s.created_at))}" }
                            }
                        }
                    }
                }
                if total > SHOWN {
                    button { class: "home-more", onclick: move |e| onmore.call(e),
                        if all { "접기" } else { "{total - SHOWN}개 더 보기" }
                    }
                }
            }
        }
    }
}
