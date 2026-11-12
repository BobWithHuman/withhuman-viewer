//! 감사: whether the engine's decisions and people's answers can be shown, after the fact.
//!
//! - 위험 행동 점검: every Call stopped, held for review, or allowed though hard to undo, with
//!   why and what made it; a row opens the Call on the route map.
//! - 승인 기록: every Call held for review and what the person did (it ran, or it did not), how
//!   long they took, and the approvals given by habit: the same request again and again, and a
//!   risky one slipped in after them.
//! - 무결성·조작 기록: what needs the engine to report it, named.
//!
//! Both lists export to CSV for an audit.

use std::collections::{BTreeMap, HashMap};

use dioxus::prelude::*;

use super::main_page::{
    decision_key, decision_label, folder_of, kind_label, kind_of, machine_of_fact, now_ms, source_label, source_of,
};
use super::policy::{Grade, grade_of};
use crate::model::CallFact;
use crate::ui::home::big;
use crate::ui::{AppState, Screen};

const DAY: i64 = 86_400_000;
const MINUTE: i64 = 60_000;
/// How many rows a list shows before "더 보기".
const ROWS: usize = 60;
/// A request is a repeat when this many of the same kind came before it in the session...
const REPEATS: usize = 3;
/// ...within this long.
const REPEAT_WINDOW: i64 = 30 * MINUTE;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Tab {
    Risky,
    Approvals,
    /// Keys revoked, Calls refused for them, keys issued: who stopped whom.
    Kills,
    Integrity,
}

/// Which risky Calls the first list shows.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Show {
    All,
    Denied,
    Reviewed,
    /// Allowed though hard to undo.
    Undoable,
    /// Graded 3-1 by the draft rules.
    Irreversible,
}

impl Show {
    const ALL: [Show; 5] = [Show::All, Show::Denied, Show::Reviewed, Show::Undoable, Show::Irreversible];

    fn label(self) -> &'static str {
        match self {
            Show::All => "전체",
            Show::Denied => "차단",
            Show::Reviewed => "확인 요청",
            Show::Undoable => "되돌리기 어려운 허용",
            Show::Irreversible => "3-1 등급(초안 규칙)",
        }
    }

    fn keeps(self, f: &CallFact) -> bool {
        match self {
            Show::All => is_risky(f),
            Show::Denied => f.decision.as_deref() == Some("DENY"),
            Show::Reviewed => f.decision.as_deref() == Some("REVIEW"),
            Show::Undoable => allowed_undoable(f),
            Show::Irreversible => grade_of(f) == Grade::ThreeOne,
        }
    }
}

fn allowed_undoable(f: &CallFact) -> bool {
    f.decision.as_deref() == Some("ALLOW") && f.irreversible
}

/// A Call an audit looks at: stopped, held, allowed though hard to undo, or graded 3-1.
fn is_risky(f: &CallFact) -> bool {
    matches!(f.decision.as_deref(), Some("DENY" | "REVIEW")) || allowed_undoable(f) || grade_of(f) == Grade::ThreeOne
}

/// The kind of request a Call held for review was, for counting repeats: the tool and the
/// command's first word, as a person sees the same prompt again.
fn request_kind(f: &CallFact) -> String {
    let tool = f.tool.as_deref().unwrap_or("?");
    let head = f
        .target
        .as_deref()
        .and_then(|t| t.split_whitespace().next())
        .unwrap_or("");
    if tool == "Bash" && !head.is_empty() { format!("Bash · {head}") } else { tool.to_owned() }
}

/// A Call held for review, as the approvals list shows it.
#[derive(Clone, Debug, PartialEq)]
struct Asked {
    fact: CallFact,
    /// The person let it run.
    allowed: bool,
    /// From the request to its result (ms), when it ran.
    took: Option<i64>,
    /// At least [`REPEATS`] requests of its kind came just before it in the session.
    repeated: bool,
    /// A risky request after a run of repeats: what approval fatigue lets through.
    slipped: bool,
}

/// The Calls held for review, oldest first per session, each marked as a repeat when the same
/// kind came [`REPEATS`] times within [`REPEAT_WINDOW`] before it.
fn asked(facts: &[CallFact]) -> Vec<Asked> {
    let mut by_session: HashMap<&str, Vec<&CallFact>> = HashMap::new();
    for f in facts.iter().filter(|f| f.decision.as_deref() == Some("REVIEW")) {
        by_session.entry(f.session.as_str()).or_default().push(f);
    }
    let mut out = Vec::new();
    for list in by_session.values_mut() {
        list.sort_by_key(|f| (f.at, f.seq));
        for (i, f) in list.iter().enumerate() {
            let kind = request_kind(f);
            let at = f.at.unwrap_or(0);
            let before = list[..i]
                .iter()
                .filter(|p| at - p.at.unwrap_or(0) <= REPEAT_WINDOW)
                .collect::<Vec<_>>();
            let same = before.iter().filter(|p| request_kind(p) == kind).count();
            let run = before.len() >= REPEATS;
            let risky = matches!(grade_of(f), Grade::ThreeOne | Grade::TwoOne)
                || f.irreversible
                || kind_of(f).as_deref() == Some("exfiltration");
            out.push(Asked {
                fact: (*f).clone(),
                allowed: f.ran,
                took: f.ran_at.zip(f.at).map(|(r, a)| r - a).filter(|t| *t >= 0),
                repeated: same >= REPEATS,
                slipped: run && same < REPEATS && risky,
            });
        }
    }
    out.sort_by_key(|a| std::cmp::Reverse(a.fact.at));
    out
}

/// A duration as a person reads it: "12초", "3분", "1시간 5분".
fn span(ms: i64) -> String {
    let s = ms / 1000;
    match s {
        s if s < 60 => format!("{s}초"),
        s if s < 3600 => format!("{}분", s / 60),
        s => format!("{}시간 {}분", s / 3600, s % 3600 / 60),
    }
}

fn median(mut v: Vec<i64>) -> Option<i64> {
    if v.is_empty() {
        return None;
    }
    v.sort_unstable();
    Some(v[v.len() / 2])
}

/// When a Call happened, as an audit list writes it.
fn stamp(ms: Option<i64>) -> String {
    use chrono::{Local, TimeZone};
    ms.and_then(|ms| Local.timestamp_millis_opt(ms).single())
        .map_or(String::new(), |t| t.format("%Y-%m-%d %H:%M:%S").to_string())
}

/// When a Call happened, short enough for a list's column: "11-05 17:21".
fn short_stamp(ms: Option<i64>) -> String {
    use chrono::{Local, TimeZone};
    ms.and_then(|ms| Local.timestamp_millis_opt(ms).single())
        .map_or(String::new(), |t| t.format("%m-%d %H:%M").to_string())
}

/// One CSV field, quoted when it holds a comma, a quote or a line break.
fn field(text: &str) -> String {
    if text.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", text.replace('"', "\"\""))
    } else {
        text.to_owned()
    }
}

fn csv(head: &[&str], rows: impl Iterator<Item = Vec<String>>) -> String {
    // A BOM so a spreadsheet opens the Korean as UTF-8.
    let mut out = String::from("\u{feff}");
    out.push_str(&head.join(","));
    out.push('\n');
    for row in rows {
        out.push_str(&row.iter().map(|c| field(c)).collect::<Vec<_>>().join(","));
        out.push('\n');
    }
    out
}

/// Asks where to save `text` and writes it there.
fn export(name: String, text: String, mut note: Signal<Option<String>>) {
    spawn(async move {
        let Some(file) = rfd::AsyncFileDialog::new()
            .add_filter("CSV", &["csv"])
            .set_file_name(&name)
            .save_file()
            .await
        else {
            return;
        };
        let path = file.path().to_path_buf();
        note.set(Some(match std::fs::write(&path, text) {
            Ok(()) => format!("저장했습니다: {}", path.display()),
            Err(e) => format!("저장하지 못했습니다: {e}"),
        }));
    });
}

fn risky_rows(list: &[CallFact]) -> String {
    csv(
        &["시각", "단말", "세션", "판정", "등급(초안)", "도구", "대상", "이유", "위반 유형", "호출을 시킨 것", "되돌리기 어려움", "작업 폴더"],
        list.iter().map(|f| {
            vec![
                stamp(f.at),
                machine_of_fact(f),
                f.session.clone(),
                decision_label(decision_key(f)).to_owned(),
                grade_of(f).label().to_owned(),
                f.tool.clone().unwrap_or_default(),
                f.target.clone().unwrap_or_default(),
                f.reason.clone().unwrap_or_default(),
                kind_of(f).map(|k| kind_label(&k)).unwrap_or_default(),
                source_label(source_of(f)).to_owned(),
                if f.irreversible { "예" } else { "" }.to_owned(),
                folder_of(f.cwd.as_deref()),
            ]
        }),
    )
}

fn asked_rows(list: &[Asked]) -> String {
    csv(
        &["시각", "단말", "세션", "도구", "대상", "결과", "걸린 시간(초)", "반복된 요청", "반복 뒤 위험 요청"],
        list.iter().map(|a| {
            vec![
                stamp(a.fact.at),
                machine_of_fact(&a.fact),
                a.fact.session.clone(),
                a.fact.tool.clone().unwrap_or_default(),
                a.fact.target.clone().unwrap_or_default(),
                if a.allowed { "승인(실행됨)" } else { "거절 또는 실행 안 됨" }.to_owned(),
                a.took.map(|t| (t / 1000).to_string()).unwrap_or_default(),
                if a.repeated { "예" } else { "" }.to_owned(),
                if a.slipped { "예" } else { "" }.to_owned(),
            ]
        }),
    )
}

#[component]
pub fn Audit() -> Element {
    let state = use_context::<AppState>();
    let found = use_context::<super::Facts>().0;
    // Sent here from the main page: the approvals open, a Call's row marked.
    let mut sent = use_context::<super::ToApprovals>().0;
    let marked = use_signal(|| sent.peek().clone().flatten());
    let mut tab = use_signal(|| {
        if sent.peek().is_some() {
            return Tab::Approvals;
        }
        // VIEWER_TAB names another tab, to work on it without clicking there.
        match std::env::var("VIEWER_TAB").as_deref() {
            Ok("approvals") => Tab::Approvals,
            Ok("kills") => Tab::Kills,
            Ok("integrity") => Tab::Integrity,
            _ => Tab::Risky,
        }
    });
    use_effect(move || {
        if sent.peek().is_some() {
            sent.set(None);
        }
    });
    let mut days = use_signal(|| Some(7_i64));
    let mut machine = use_signal(|| None::<String>);
    let note = use_signal(|| None::<String>);
    let now = now_ms();
    let p = (state.public)();

    let (all, failed): (Vec<CallFact>, Option<String>) = match &*found.read() {
        Some(Ok(list)) => (list.clone(), None),
        Some(Err(e)) => (Vec::new(), Some(e.clone())),
        None => (Vec::new(), None),
    };
    let loading = found.read().is_none();
    let since = days().map(|d| now - d * DAY);
    let picked = machine();
    let facts: Vec<CallFact> = all
        .iter()
        .filter(|f| since.is_none_or(|t| f.at.is_some_and(|a| a >= t)))
        .filter(|f| picked.as_ref().is_none_or(|m| &machine_of_fact(f) == m))
        .cloned()
        .collect();
    let mut machines: Vec<String> = all.iter().map(machine_of_fact).collect();
    machines.sort();
    machines.dedup();
    let period = match days() {
        Some(1) => "최근 24시간".to_owned(),
        Some(d) => format!("최근 {d}일"),
        None => "전체 기간".to_owned(),
    };
    let source = p.store();
    let current = tab();

    rsx! {
        div { class: "home admin",
            section { class: "home-hero",
                div { class: "home-hero-in",
                    p { class: "home-chip",
                        span { class: if p.demo { "dot demo" } else { "dot" } }
                        "{source}"
                    }
                    h1 { class: "home-title", "감사" }
                    p { class: "home-lede", "엔진이 막거나 사람에게 물은 호출, 되돌리기 어려운데 허용된 호출, 그리고 사람이 어떻게 답했는지를 기간별로 점검하고 내보냅니다." }
                }
            }
            div { class: "admin-bar",
                nav { class: "admin-tabs",
                    for (t, label) in [(Tab::Risky, "위험 행동 점검"), (Tab::Approvals, "승인 기록"), (Tab::Kills, "킬스위치 기록"), (Tab::Integrity, "무결성·조작 기록")] {
                        button { key: "{label}", class: if current == t { "on" } else { "" }, onclick: move |_| tab.set(t), "{label}" }
                    }
                }
                div { class: "admin-filters",
                    label { class: "admin-pick-field",
                        span { "기간" }
                        select {
                            onchange: move |e| days.set(e.value().parse().ok()),
                            for (v, t) in [("1", "최근 24시간"), ("7", "최근 7일"), ("30", "최근 30일"), ("all", "전체 기간")] {
                                option { key: "{v}", value: "{v}", selected: days().map_or("all".to_owned(), |d| d.to_string()) == v, "{t}" }
                            }
                        }
                    }
                    label { class: if picked.is_some() { "admin-pick-field on" } else { "admin-pick-field" },
                        span { "단말" }
                        select {
                            onchange: move |e| machine.set(Some(e.value()).filter(|v| !v.is_empty())),
                            option { value: "", "모든 단말" }
                            for m in machines {
                                option { key: "{m}", value: "{m}", selected: picked.as_deref() == Some(m.as_str()), "{m}" }
                            }
                        }
                    }
                    if let Some(n) = note() {
                        span { class: "admin-note", "{n}" }
                    }
                }
            }
            if let Some(e) = failed {
                p { class: "admin-failed", "호출 기록을 불러오지 못했습니다: {e}" }
            }
            match current {
                Tab::Risky => rsx! { Risky { facts, loading, period, note } },
                Tab::Approvals => rsx! { Approvals { facts, loading, period, note, marked: marked() } },
                Tab::Kills => rsx! { Kills { since: days().map(|d| now - d * DAY), period, note } },
                Tab::Integrity => rsx! { Integrity {} },
            }
        }
    }
}

/// Every Call worth an audit, newest first; a row opens it on the route map.
#[component]
fn Risky(facts: Vec<CallFact>, loading: bool, period: String, note: Signal<Option<String>>) -> Element {
    let mut state = use_context::<AppState>();
    let mut show = use_signal(|| Show::All);
    let mut all = use_signal(|| false);
    let counts: Vec<(Show, usize)> = Show::ALL.iter().map(|s| (*s, facts.iter().filter(|f| s.keeps(f)).count())).collect();
    let mut list: Vec<CallFact> = facts.iter().filter(|f| show().keeps(f)).cloned().collect();
    list.sort_by_key(|f| std::cmp::Reverse(f.at));
    let total = list.len();
    let csv_text = risky_rows(&list);
    let file = format!("withhuman-위험행동-{}.csv", chrono::Local::now().format("%Y%m%d"));
    let shown: Vec<CallFact> = list.into_iter().take(if all() { usize::MAX } else { ROWS }).collect();
    rsx! {
        div { class: "admin-body",
            div { class: "admin-chips",
                for (s, n) in counts {
                    button {
                        key: "{s.label()}",
                        class: if show() == s { "admin-chip on" } else { "admin-chip" },
                        onclick: move |_| show.set(s),
                        "{s.label()}"
                        span { class: "n", "{big(n as i64)}" }
                    }
                }
                button {
                    class: "admin-export",
                    disabled: total == 0,
                    onclick: move |_| export(file.clone(), csv_text.clone(), note),
                    "CSV로 내보내기"
                }
            }
            p { class: "admin-about",
                "{period} 동안 차단·확인 요청된 호출, 되돌리기 어려운데 허용된 호출, 초안 규칙표로 3-1 등급인 호출입니다. 줄을 누르면 노선도에서 그 호출이 열립니다. 등급은 엔진이 아니라 정책 화면의 초안 규칙표로 매긴 값입니다."
            }
            div { class: "home-list admin-table-wrap",
                if shown.is_empty() {
                    p { class: "home-empty", if loading { "불러오는 중…" } else { "해당하는 호출이 없습니다." } }
                } else {
                    div { class: "admin-audit head",
                        span { "시각" }
                        span { "단말" }
                        span { "판정" }
                        span { "등급" }
                        span { "무엇을" }
                        span { "이유" }
                        span { "시킨 것" }
                    }
                }
                for f in shown {
                    {
                        let session = f.session.clone();
                        let seq = f.seq;
                        let decision = decision_key(&f);
                        let grade = grade_of(&f);
                        let why = kind_of(&f).map(|k| kind_label(&k)).or_else(|| f.reason.clone()).unwrap_or_else(|| {
                            if allowed_undoable(&f) { "허용됐지만 되돌리기 어려움".into() } else { String::new() }
                        });
                        let what = format!("{} {}", f.tool.clone().unwrap_or_default(), f.target.clone().unwrap_or_default());
                        rsx! {
                            button {
                                key: "{f.session}-{f.seq}",
                                class: "admin-audit",
                                title: "{what}",
                                onclick: move |_| {
                                    state.open_call(session.clone(), seq);
                                    state.screen.set(Screen::Work);
                                },
                                span { class: "when", title: "{stamp(f.at)}", "{short_stamp(f.at)}" }
                                span { class: "machine", "{machine_of_fact(&f)}" }
                                span { class: "decision {decision}", "{decision_label(decision)}" }
                                span { class: "grade g{grade.key()}", "{grade.short()}" }
                                span { class: "what", "{what}" }
                                span { class: "why", "{why}" }
                                span { class: "src", "{source_label(source_of(&f))}" }
                            }
                        }
                    }
                }
                if total > ROWS {
                    button { class: "home-more", onclick: move |_| all.toggle(),
                        if all() { "접기" } else { "{total - ROWS}개 더 보기" }
                    }
                }
            }
        }
    }
}

/// How people answered when the engine asked them.
#[component]
fn Approvals(
    facts: Vec<CallFact>,
    loading: bool,
    period: String,
    note: Signal<Option<String>>,
    /// A Call's row to mark and bring into view, sent from the main page.
    marked: Option<(String, i64)>,
) -> Element {
    let mut state = use_context::<AppState>();
    // Every row while one is marked, so it is among them.
    let mut all = use_signal(|| marked.is_some());
    let list = asked(&facts);
    let marked_id = marked.as_ref().map(|(s, q)| format!("ask-{s}-{q}"));
    // Again once the rows come in, when they were still on their way.
    let rows = list.len();
    use_effect(use_reactive!(|(marked_id, rows)| {
        if let (Some(id), true) = (marked_id, rows > 0) {
            document::eval(&format!(
                "requestAnimationFrame(() => document.getElementById('{id}')?.scrollIntoView({{block: 'center', behavior: 'smooth'}}))"
            ));
        }
    }));
    let total = list.len();
    let allowed = list.iter().filter(|a| a.allowed).count();
    let rate = |n: usize| if total == 0 { "–".to_owned() } else { format!("{:.0}%", n as f64 / total as f64 * 100.0) };
    let took = median(list.iter().filter_map(|a| a.took).collect());
    let slipped = list.iter().filter(|a| a.slipped).count();
    let kinds: Vec<(String, usize)> = {
        let mut by: BTreeMap<String, usize> = BTreeMap::new();
        for a in &list {
            *by.entry(request_kind(&a.fact)).or_default() += 1;
        }
        let mut v: Vec<(String, usize)> = by.into_iter().collect();
        v.sort_by_key(|k| std::cmp::Reverse(k.1));
        v.truncate(6);
        v
    };
    let kind_max = kinds.first().map_or(1, |k| k.1).max(1);
    let csv_text = asked_rows(&list);
    let file = format!("withhuman-승인기록-{}.csv", chrono::Local::now().format("%Y%m%d"));
    let shown: Vec<Asked> = list.into_iter().take(if all() { usize::MAX } else { ROWS }).collect();
    let tiles = [
        ("확인 요청", big(total as i64), "엔진이 사람에게 물은 호출", ""),
        ("승인", rate(allowed), "실행된 것(결과가 돌아온 것)", "good"),
        ("반려율", rate(total - allowed), "실행되지 않은 것", "warn"),
        ("승인까지 걸린 시간", took.map_or("–".to_owned(), span), "가운데 값", ""),
        ("반복 뒤 위험 요청", big(slipped as i64), "비슷한 요청이 이어진 뒤 온 위험한 요청", if slipped > 0 { "bad" } else { "" }),
    ];
    rsx! {
        div { class: "admin-body",
            div { class: "admin-tiles five",
                for (k, v, about, tone) in tiles {
                    div { key: "{k}", class: "admin-tile static {tone}",
                        span { class: "k", "{k}" }
                        span { class: "v", if loading { "…" } else { "{v}" } }
                        span { class: "about", "{about}" }
                    }
                }
            }
            p { class: "admin-about",
                "{period} 동안 엔진이 확인을 요청한 호출입니다. 결과가 돌아온 호출을 사람이 승인한 것으로 봅니다. 엔진이 확인 요청을 사람에게 묻지 않고 차단하도록 설정돼 있으면(WITHHUMAN_ASK=deny) 모두 실행 안 됨으로 잡힙니다. 승인 피로를 보려면 반려율이 지나치게 낮은지, 같은 요청이 반복되는지, 반복 뒤에 위험한 요청이 섞였는지 보면 됩니다."
            }
            div { class: "admin-grid two",
                div { class: "admin-card",
                    div { class: "admin-card-head",
                        h3 { "자주 반복된 확인 요청" }
                        span { class: "admin-card-note", "도구와 명령의 첫 단어 기준" }
                    }
                    if kinds.is_empty() {
                        p { class: "admin-none", if loading { "불러오는 중…" } else { "기간 안에 확인 요청이 없습니다." } }
                    }
                    div { class: "admin-hbars",
                        for (k, n) in kinds {
                            div { key: "{k}", class: "admin-hbar",
                                span { class: "name", "{k}" }
                                span { class: "track", span { class: "fill amber", style: "width:{n as f64 / kind_max as f64 * 100.0:.1}%" } }
                                span { class: "n", "{n}" }
                            }
                        }
                    }
                }
                div { class: "admin-card",
                    div { class: "admin-card-head", h3 { "내보내기" } }
                    p { class: "admin-none", "확인 요청마다 시각, 단말, 도구, 대상, 결과, 걸린 시간, 반복 여부를 CSV로 저장합니다." }
                    button {
                        class: "admin-export",
                        disabled: total == 0,
                        onclick: move |_| export(file.clone(), csv_text.clone(), note),
                        "CSV로 내보내기"
                    }
                }
            }
            div { class: "home-list admin-table-wrap",
                if shown.is_empty() {
                    p { class: "home-empty", if loading { "불러오는 중…" } else { "기간 안에 확인 요청이 없습니다." } }
                } else {
                    div { class: "admin-asked head",
                        span { "시각" }
                        span { "단말" }
                        span { "무엇을" }
                        span { "결과" }
                        span { "걸린 시간" }
                        span { "표시" }
                    }
                }
                for a in shown {
                    {
                        let session = a.fact.session.clone();
                        let seq = a.fact.seq;
                        let what = format!("{} {}", a.fact.tool.clone().unwrap_or_default(), a.fact.target.clone().unwrap_or_default());
                        let on = marked.as_ref().is_some_and(|(s, q)| *s == a.fact.session && *q == a.fact.seq);
                        rsx! {
                            button {
                                key: "{a.fact.session}-{a.fact.seq}",
                                id: "ask-{a.fact.session}-{a.fact.seq}",
                                class: if on { "admin-asked marked" } else { "admin-asked" },
                                title: "{what}",
                                onclick: move |_| {
                                    state.open_call(session.clone(), seq);
                                    state.screen.set(Screen::Work);
                                },
                                span { class: "when", title: "{stamp(a.fact.at)}", "{short_stamp(a.fact.at)}" }
                                span { class: "machine", "{machine_of_fact(&a.fact)}" }
                                span { class: "what", "{what}" }
                                span { class: if a.allowed { "result ok" } else { "result no" }, if a.allowed { "승인" } else { "실행 안 됨" } }
                                span { class: "took", "{a.took.map_or(String::new(), span)}" }
                                span { class: "flags",
                                    if a.slipped { span { class: "flag bad", "반복 뒤 위험" } }
                                    if a.repeated { span { class: "flag", "반복" } }
                                }
                            }
                        }
                    }
                }
                if total > ROWS {
                    button { class: "home-more", onclick: move |_| all.toggle(),
                        if all() { "접기" } else { "{total - ROWS}개 더 보기" }
                    }
                }
            }
        }
    }
}

/// What this tab will hold once the engine reports it.
#[component]
fn Kills(since: Option<i64>, period: String, note: Signal<Option<String>>) -> Element {
    let state = use_context::<AppState>();
    let backend = use_context::<std::sync::Arc<crate::api::Backend>>();
    let found = use_resource(move || {
        let b = backend.clone();
        let _ = (state.reload)();
        let _ = (state.public)();
        async move { b.inventory().await }
    });
    let mut show = use_signal(|| None::<&'static str>);
    let (events, failed, loading) = match &*found.read() {
        Some(Ok(Some(inv))) => (inv.events.clone(), None, false),
        Some(Ok(None)) => (Vec::new(), Some("이 엔진은 아직 키와 킬스위치 기록을 남기지 않습니다(엔진의 인벤토리 기능이 필요합니다).".to_owned()), false),
        Some(Err(e)) => (Vec::new(), Some(format!("기록을 불러오지 못했습니다: {e}")), false),
        None => (Vec::new(), None, true),
    };
    let in_period: Vec<crate::model::KeyEvent> = events.into_iter().filter(|e| since.is_none_or(|t| e.at.is_some_and(|a| a >= t))).collect();
    let count = |k: &str| in_period.iter().filter(|e| e.kind == k).count();
    let chips = [(None, "전체", in_period.len()), (Some("revoked"), "회수", count("revoked")), (Some("denied"), "거부", count("denied")), (Some("issued"), "발급", count("issued"))];
    let shown: Vec<crate::model::KeyEvent> = in_period.iter().filter(|e| show().is_none_or(|k| e.kind == k)).cloned().collect();
    let text = |v: &Option<String>| v.clone().unwrap_or_default();
    let csv_text = csv(
        &["시각", "종류", "누가", "사용자", "키", "사유", "기기", "세션"],
        shown.iter().map(|e| vec![
            stamp(e.at), super::inventory::event_label(&e.kind).to_owned(), text(&e.actor), text(&e.user_email),
            text(&e.key_id), text(&e.reason), text(&e.client_id), text(&e.session_id),
        ]),
    );
    let file = format!("withhuman-킬스위치기록-{}.csv", chrono::Local::now().format("%Y%m%d"));
    let empty = shown.is_empty();
    rsx! {
        div { class: "admin-body",
            section { class: "home-group",
                div { class: "home-group-head",
                    h2 { "킬스위치 기록" }
                    span { class: "n", "{in_period.len()}" }
                }
                p { class: "admin-about", "{period} 동안 누가 어떤 사용자의 키를 회수했는지, 회수된 키로 온 도구 호출을 엔진이 언제 막았는지, 키를 누구에게 발급했는지입니다. 회수와 발급은 인벤토리에서 합니다." }
                div { class: "admin-chips",
                    for (k, label, n) in chips {
                        button { key: "{label}", class: if show() == k { "admin-chip on" } else { "admin-chip" }, onclick: move |_| show.set(k),
                            "{label}" span { class: "n", "{n}" }
                        }
                    }
                    button { class: "admin-export", disabled: empty, onclick: move |_| export(file.clone(), csv_text.clone(), note), "CSV로 내보내기" }
                }
                div { class: "home-list admin-table-wrap",
                    if let Some(f) = failed {
                        p { class: "home-empty", "{f}" }
                    } else if empty {
                        p { class: "home-empty", if loading { "불러오는 중…" } else { "기간 안에 기록이 없습니다." } }
                    } else {
                        div { class: "admin-kill head",
                            span { "시각" } span { "종류" } span { "누가" } span { "사용자" } span { "키" } span { "사유" } span { "기기 · 세션" }
                        }
                    }
                    for (i, e) in shown.into_iter().enumerate() {
                        div { key: "{i}", class: "admin-kill",
                            span { class: "when", title: "{stamp(e.at)}", "{short_stamp(e.at)}" }
                            span { class: "kind {e.kind}", "{super::inventory::event_label(&e.kind)}" }
                            span { "{e.actor.clone().unwrap_or_else(|| \"엔진\".into())}" }
                            span { class: "machine", "{text(&e.user_email)}" }
                            span { class: "what", "{text(&e.key_id)}" }
                            span { class: "why", title: "{text(&e.reason)}", "{text(&e.reason)}" }
                            span { class: "why", "{text(&e.client_id)} {e.session_id.as_deref().map(|s| s.chars().take(8).collect::<String>()).unwrap_or_default()}" }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn Integrity() -> Element {
    rsx! {
        div { class: "admin-body",
            section { class: "home-group",
                div { class: "home-group-head", h2 { "엔진이 알려 줘야 보이는 것" } }
                p { class: "admin-about", "아래는 지금 기록만으로는 확인할 수 없어, 엔진에 해당 기능이 생긴 뒤 이 탭에 나옵니다." }
                div { class: "home-list admin-later",
                    p { strong { "기록 무결성" } span { "기록마다 앞 기록의 해시를 이어 붙여, 기록이 고쳐졌는지 검증합니다. 내보낸 보고서에는 서명을 붙입니다." } }
                    p { strong { "관리자 조작 기록" } span { "세션 정지·해제, 정책 변경, 개발자 요청 처리를 누가 언제 했는지 남깁니다." } }
                    p { strong { "판정에 쓴 정책 버전" } span { "판정마다 그때의 기준선, 규칙표, 사전 승인 정책 버전을 함께 남겨, 왜 그렇게 판정했는지 나중에 재현합니다." } }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn review(session: &str, seq: i64, at: i64, tool: &str, target: &str, ran: bool) -> CallFact {
        CallFact {
            session: session.into(),
            seq,
            at: Some(at),
            decision: Some("REVIEW".into()),
            tool: Some(tool.into()),
            target: Some(target.into()),
            ran,
            ran_at: ran.then_some(at + 5_000),
            ..CallFact::default()
        }
    }

    #[test]
    fn a_risky_request_after_a_run_of_repeats_is_marked() {
        let mut facts: Vec<CallFact> = (0..4).map(|i| review("s", i, i * MINUTE, "Bash", "npm test", true)).collect();
        facts.push(review("s", 9, 5 * MINUTE, "Bash", "git push --force origin main", true));
        let list = asked(&facts);
        let last = list.iter().find(|a| a.fact.seq == 9).unwrap();
        assert!(last.slipped, "a force push after four test runs");
        assert!(!last.repeated);
        let fourth = list.iter().find(|a| a.fact.seq == 3).unwrap();
        assert!(fourth.repeated, "the fourth npm test follows three of its kind");
        assert!(!fourth.slipped, "a repeat itself is not the risky one");
        assert_eq!(last.took, Some(5_000));
    }

    #[test]
    fn requests_far_apart_are_not_repeats() {
        let facts: Vec<CallFact> = (0..4).map(|i| review("s", i, i * 2 * REPEAT_WINDOW, "Bash", "npm test", false)).collect();
        assert!(asked(&facts).iter().all(|a| !a.repeated && !a.allowed));
    }

    #[test]
    fn csv_quotes_what_needs_it() {
        assert_eq!(field("plain"), "plain");
        assert_eq!(field("a,b"), "\"a,b\"");
        assert_eq!(field("say \"hi\""), "\"say \"\"hi\"\"\"");
    }

    #[test]
    fn spans_read_as_people_say_them() {
        assert_eq!(span(12_000), "12초");
        assert_eq!(span(180_000), "3분");
        assert_eq!(span(3_900_000), "1시간 5분");
        assert_eq!(median(vec![5, 1, 9]), Some(5));
        assert_eq!(median(vec![]), None);
    }
}
