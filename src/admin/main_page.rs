//! The security team's main page, laid out as an endpoint console's activity dashboard: a thin
//! header, tabs, a filter bar, then cards on a dark grid, each saying when it was read.
//!
//! - 개요: machines blocking out of those active, new requests for a person's approval, the
//!   prompts whose turns were stopped most, the Calls people approved (hardest to undo first); the Calls worth a look over time beside the latest of them; and those Calls by
//!   kind over time. A figure, a bar or a legend clicked narrows the filter, as a console's
//!   widgets do.
//! - 단말: every machine, the riskiest first, found by name.
//! - 위험 세션: the sessions worth a look in four kinds.
//! - 엔진 상태: how the engine enforced on each machine; what it cannot tell yet is named.
//!
//! The filter (period, machine, decision, what was broken, tool, what made the Call, folder)
//! applies to every tab.

use std::collections::{BTreeMap, HashSet};
use std::time::{SystemTime, UNIX_EPOCH};

use chrono::{Duration, Local, NaiveDate, TimeZone, Timelike};
use dioxus::prelude::*;

use super::policy::{Grade, grade_of};
use crate::model::{CallFact, Session};
use crate::ui::home::{big, when};
use crate::ui::{AppState, Load, Mode, Screen};

/// How many sessions of each kind the page lists before "더 보기".
const SHOWN: usize = 8;
/// How many machines the list shows before "더 보기".
const MACHINES_SHOWN: usize = 10;
const HOUR: i64 = 3_600_000;
const DAY: i64 = 24 * HOUR;
pub(super) const NO_NAME: &str = "이름 없는 단말";

pub(super) const DAY_MS: i64 = DAY;

pub(super) fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64)
}

/// The machine a session came from: the client ID its hooks sent.
pub(super) fn machine_of(s: &Session) -> String {
    s.client.clone().unwrap_or_else(|| NO_NAME.into())
}

pub(super) fn machine_of_fact(f: &CallFact) -> String {
    f.client.clone().unwrap_or_else(|| NO_NAME.into())
}

/// The last part of a working folder, whether it uses / or \.
pub(super) fn folder_of(cwd: Option<&str>) -> String {
    cwd.unwrap_or("")
        .split(['/', '\\'])
        .rfind(|s| !s.is_empty())
        .unwrap_or("")
        .to_owned()
}

/// What a stopped or held Call broke most, as the filter names it; none for an allowed Call or
/// one with no decision. A Call the safeguard rule stopped broke the agent's own safeguards.
pub(super) fn kind_of(f: &CallFact) -> Option<String> {
    if !matches!(f.decision.as_deref(), Some("DENY" | "REVIEW")) {
        return None;
    }
    if f.rule.as_deref() == Some("safeguard::change") {
        return Some("safeguard".into());
    }
    Some(
        f.worst
            .clone()
            .filter(|w| w != "none" && !w.is_empty())
            .unwrap_or_else(|| "other".into()),
    )
}

pub(super) fn kind_label(kind: &str) -> String {
    match kind {
        "exfiltration" => "데이터·비밀값 외부 전송",
        "hijacked_goal" => "목표 탈취(도구 결과가 끌어온 목표)",
        "unconfirmed_irreversible" => "묻지 않은 되돌릴 수 없는 행위",
        "security_change" => "보안 설정 변경",
        "unrelated_change" => "요청과 무관한 변경",
        "out_of_scope_access" => "요청 범위 밖 접근",
        "explicit" => "사용자가 말한 것 위반",
        "safeguard" => "에이전트 안전장치 변경",
        "other" => "그 밖의 이유",
        ALLOWED_IRREVERSIBLE => "되돌리기 어려운 허용",
        other => return other.to_owned(),
    }
    .to_owned()
}

/// What made the agent take a Call, as the filter names it.
pub(super) fn source_of(f: &CallFact) -> &'static str {
    match f.link.as_deref() {
        Some("request") => "request",
        Some("segment") => "segment",
        Some("none") => "none",
        _ => "untraced",
    }
}

pub(super) fn source_label(source: &str) -> &'static str {
    match source {
        "request" => "사용자 요청",
        "segment" => "도구 결과",
        "none" => "찾지 못함",
        _ => "추적 없음",
    }
}

pub(super) fn decision_key(f: &CallFact) -> &'static str {
    match f.decision.as_deref() {
        Some("ALLOW") => "ALLOW",
        Some("REVIEW") => "REVIEW",
        Some("DENY") => "DENY",
        _ => "NONE",
    }
}

pub(super) fn decision_label(decision: &str) -> &'static str {
    match decision {
        "ALLOW" => "허용",
        "REVIEW" => "확인 요청",
        "DENY" => "차단",
        _ => "판정 없음",
    }
}

/// What the page looks at; every tab follows it.
#[derive(Clone, Debug, PartialEq)]
struct Filter {
    /// The last so many days; none for all.
    days: Option<i64>,
    machine: Option<String>,
    /// `ALLOW`, `REVIEW`, `DENY` or `NONE`.
    decision: Option<String>,
    /// What a stopped or held Call broke ([`kind_of`]).
    kind: Option<String>,
    tool: Option<String>,
    /// What made the Call ([`source_of`]).
    source: Option<String>,
    folder: Option<String>,
}

impl Default for Filter {
    fn default() -> Self {
        Self {
            days: Some(7),
            machine: None,
            decision: None,
            kind: None,
            tool: None,
            source: None,
            folder: None,
        }
    }
}

impl Filter {
    fn since(&self, now: i64) -> Option<i64> {
        self.days.map(|d| now - d * DAY)
    }

    /// Whether it narrows the Calls, not just the period and the machine.
    fn narrows_calls(&self) -> bool {
        self.decision.is_some()
            || self.kind.is_some()
            || self.tool.is_some()
            || self.source.is_some()
            || self.folder.is_some()
    }

    /// The period and the machine only: what coverage is counted over.
    fn base(&self) -> Self {
        Self {
            days: self.days,
            machine: self.machine.clone(),
            ..Self {
                days: None,
                ..Self::default()
            }
        }
    }

    fn keeps(&self, f: &CallFact, now: i64) -> bool {
        let eq = |want: &Option<String>, have: &str| want.as_deref().is_none_or(|w| w == have);
        self.since(now)
            .is_none_or(|t| f.at.is_some_and(|at| at >= t))
            && eq(&self.machine, &machine_of_fact(f))
            && eq(&self.decision, decision_key(f))
            && self
                .kind
                .as_ref()
                .is_none_or(|k| kind_of(f).as_ref() == Some(k))
            && eq(&self.tool, f.tool.as_deref().unwrap_or(""))
            && eq(&self.source, source_of(f))
            && eq(&self.folder, &folder_of(f.cwd.as_deref()))
    }

    fn keeps_session(&self, s: &Session, now: i64, sessions_with_calls: &HashSet<&str>) -> bool {
        self.since(now)
            .is_none_or(|t| s.last_at.or(s.created_at).is_some_and(|at| at >= t))
            && self.machine.as_ref().is_none_or(|m| &machine_of(s) == m)
            && (!self.narrows_calls() || sessions_with_calls.contains(s.id.as_str()))
    }
}

/// How risky a session looks, from its counts alone: a rule a reader can check, not a score.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Risk {
    Low,
    /// Held for review, decided by nothing, or made by a tool result.
    Watch,
    /// A Call stopped, or allowed though hard to undo.
    High,
}

impl Risk {
    pub(super) fn of(s: &Session) -> Self {
        if s.denied > 0 || s.risky > 0 {
            Risk::High
        } else if s.reviewed > 0 || s.undecided > 0 || s.from_result > 0 {
            Risk::Watch
        } else {
            Risk::Low
        }
    }

    pub(super) fn badge(self) -> Option<(&'static str, &'static str)> {
        match self {
            Risk::High => Some(("high", "높음")),
            Risk::Watch => Some(("watch", "주의")),
            Risk::Low => None,
        }
    }
}

/// What the engine does with its decisions, from the mode of the latest one.
fn mode_line(mode: Option<&str>) -> (&'static str, &'static str, &'static str) {
    match mode {
        Some("enforce") => (
            "on",
            "차단 중",
            "엔진이 판정대로 도구 호출을 막고 있습니다.",
        ),
        Some("shadow") => (
            "warn",
            "시범 운영 중",
            "판정만 남기고 막지는 않습니다. 차단으로 표시된 호출도 실제로는 실행됐습니다.",
        ),
        Some("record") => (
            "warn",
            "기록만 하는 중",
            "호출을 막지 않고, 판정은 실행 뒤에 계산합니다.",
        ),
        _ => (
            "off",
            "판정 기록 없음",
            "아직 엔진이 판정한 호출이 없습니다.",
        ),
    }
}

pub(super) fn mode_label(mode: Option<&str>) -> &'static str {
    match mode {
        Some("enforce") => "차단",
        Some("shadow") => "시범 운영",
        Some("record") => "기록만",
        _ => "판정 없음",
    }
}

/// A machine the sessions came from, as the hooks named it.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Machine {
    name: String,
    sessions: usize,
    denied: i64,
    risky: i64,
    undecided: i64,
    last_at: Option<i64>,
}

impl Machine {
    /// The dot a machine wears: a Call stopped or allowed though hard to undo, or one with no
    /// decision.
    fn risk(&self) -> &'static str {
        if self.denied + self.risky > 0 {
            "high"
        } else if self.undecided > 0 {
            "watch"
        } else {
            ""
        }
    }
}

pub(super) fn machines(sessions: &[Session]) -> Vec<Machine> {
    let mut by: BTreeMap<String, Machine> = BTreeMap::new();
    for s in sessions {
        let name = machine_of(s);
        let m = by.entry(name.clone()).or_insert(Machine {
            name,
            sessions: 0,
            denied: 0,
            risky: 0,
            undecided: 0,
            last_at: None,
        });
        m.sessions += 1;
        m.denied += s.denied;
        m.risky += s.risky;
        m.undecided += s.undecided;
        m.last_at = m.last_at.max(s.last_at.or(s.created_at));
    }
    let mut list: Vec<Machine> = by.into_values().collect();
    // The machines with the most to look at first (stopped or hard to undo, then undecided),
    // then the most recently active.
    list.sort_by(|a, b| {
        (b.denied + b.risky, b.undecided, b.last_at).cmp(&(
            a.denied + a.risky,
            a.undecided,
            a.last_at,
        ))
    });
    list
}

/// How well the machines are covered over the Calls given: those active, those whose latest
/// decision blocked, those whose latest only watched, and those with a Call nothing decided.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Coverage {
    active: usize,
    blocking: usize,
    watching: usize,
    undecided: usize,
}

fn coverage(facts: &[CallFact]) -> Coverage {
    // Per machine: the latest decision's mode, and whether any Call went undecided.
    let mut by: BTreeMap<String, (Option<(i64, String)>, bool)> = BTreeMap::new();
    for f in facts {
        let entry = by.entry(machine_of_fact(f)).or_default();
        if let (Some(mode), Some(at)) = (&f.mode, f.at)
            && entry.0.as_ref().is_none_or(|(t, _)| at >= *t)
        {
            entry.0 = Some((at, mode.clone()));
        }
        if f.decision.is_none() {
            entry.1 = true;
        }
    }
    let mut c = Coverage {
        active: by.len(),
        ..Coverage::default()
    };
    for (latest, undecided) in by.values() {
        match latest.as_ref().map(|(_, m)| m.as_str()) {
            Some("enforce") => c.blocking += 1,
            Some(_) => c.watching += 1,
            None => {}
        }
        if *undecided {
            c.undecided += 1;
        }
    }
    c
}

/// One bar of the decisions over time.
#[derive(Clone, Debug, PartialEq)]
struct Bucket {
    label: String,
    denied: usize,
    reviewed: usize,
    risky: usize,
}

/// The period's time axis: by the hour for a day, else by the day (the last thirty for all
/// time), ending now.
pub(super) struct Axis {
    hourly: bool,
    n: usize,
    today: NaiveDate,
    this_hour: i64,
}

impl Axis {
    pub(super) fn new(days: Option<i64>, now: i64) -> Self {
        let hourly = days == Some(1);
        let today = Local
            .timestamp_millis_opt(now)
            .single()
            .map_or_else(|| Local::now().date_naive(), |t| t.date_naive());
        Self {
            hourly,
            n: if hourly {
                24
            } else {
                days.unwrap_or(30).clamp(1, 30) as usize
            },
            today,
            this_hour: now - now.rem_euclid(HOUR),
        }
    }

    pub(super) fn labels(&self) -> Vec<String> {
        let first_day: NaiveDate = self.today - Duration::days(self.n as i64 - 1);
        (0..self.n)
            .map(|i| {
                if self.hourly {
                    let start = self.this_hour - (self.n - 1 - i) as i64 * HOUR;
                    Local
                        .timestamp_millis_opt(start)
                        .single()
                        .map_or(String::new(), |t| format!("{}시", t.hour()))
                } else {
                    (first_day + Duration::days(i as i64))
                        .format("%-m/%-d")
                        .to_string()
                }
            })
            .collect()
    }

    /// The bucket a moment falls in; none outside the axis.
    pub(super) fn index(&self, at: i64) -> Option<usize> {
        let back = if self.hourly {
            (self.this_hour - (at - at.rem_euclid(HOUR))) / HOUR
        } else {
            let day = Local.timestamp_millis_opt(at).single()?.date_naive();
            (self.today - day).num_days()
        };
        (0..self.n as i64)
            .contains(&back)
            .then(|| self.n - 1 - back as usize)
    }
}

/// The decisions over the period, one bucket per hour or day.
fn trend(facts: &[CallFact], days: Option<i64>, now: i64) -> Vec<Bucket> {
    let axis = Axis::new(days, now);
    let mut buckets: Vec<Bucket> = axis
        .labels()
        .into_iter()
        .map(|label| Bucket {
            label,
            denied: 0,
            reviewed: 0,
            risky: 0,
        })
        .collect();
    for f in facts {
        let Some(i) = f.at.and_then(|at| axis.index(at)) else {
            continue;
        };
        let b = &mut buckets[i];
        match f.decision.as_deref() {
            Some("DENY") => b.denied += 1,
            Some("REVIEW") => b.reviewed += 1,
            Some("ALLOW") if f.irreversible => b.risky += 1,
            _ => {}
        }
    }
    buckets
}

/// What a Call worth a look counts under by kind: what a stopped or held one broke, or that it
/// was allowed though hard to undo.
pub(super) const ALLOWED_IRREVERSIBLE: &str = "allowed_irreversible";

pub(super) fn detection_kind(f: &CallFact) -> Option<String> {
    kind_of(f).or_else(|| {
        (f.decision.as_deref() == Some("ALLOW") && f.irreversible)
            .then(|| ALLOWED_IRREVERSIBLE.into())
    })
}

/// Each kind's colour, the same wherever it is drawn.
pub(super) fn kind_color(kind: &str) -> &'static str {
    match kind {
        "exfiltration" => "#7fd6f2",
        "hijacked_goal" => "#8f8cf6",
        "unconfirmed_irreversible" => "#c48cf2",
        "security_change" => "#f6c4c4",
        "unrelated_change" => "#ef9fca",
        "out_of_scope_access" => "#f2a46b",
        "explicit" => "#9fe0a6",
        "safeguard" => "#f2d36b",
        ALLOWED_IRREVERSIBLE => "#c9c9c9",
        _ => "#b8909f",
    }
}

/// The Calls worth a look by kind over the period: the axis's labels, each bucket's counts by
/// kind, and the kinds seen, the most first.
type ByKind = (
    Vec<String>,
    Vec<BTreeMap<String, usize>>,
    Vec<(String, usize)>,
);

fn by_kind(facts: &[CallFact], days: Option<i64>, now: i64) -> ByKind {
    let axis = Axis::new(days, now);
    let labels = axis.labels();
    let mut buckets = vec![BTreeMap::new(); labels.len()];
    let mut kept = Vec::new();
    for f in facts {
        let (Some(kind), Some(i)) = (detection_kind(f), f.at.and_then(|at| axis.index(at))) else {
            continue;
        };
        *buckets[i].entry(kind.clone()).or_default() += 1;
        kept.push(kind);
    }
    (labels, buckets, counted(kept.into_iter()))
}

/// How bad a Call worth a look is, for the list of the latest ones.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Severity {
    /// Allowed though hard to undo: to look at after the fact.
    After,
    /// Held for a person to confirm.
    Medium,
    /// Stopped.
    High,
    /// Cannot be undone by the draft grade rules, or stopped for sending data out or a hijacked goal.
    Critical,
}

impl Severity {
    pub(super) fn of(f: &CallFact) -> Option<Self> {
        let s = match decision_key(f) {
            "DENY" => Severity::High,
            "REVIEW" => Severity::Medium,
            "ALLOW" if f.irreversible => Severity::After,
            _ => return None,
        };
        let worst = matches!(
            kind_of(f).as_deref(),
            Some("exfiltration" | "hijacked_goal")
        );
        if grade_of(f) == Grade::ThreeOne || (s == Severity::High && worst) {
            return Some(Severity::Critical);
        }
        Some(s)
    }

    pub(super) fn tag(self) -> (&'static str, &'static str) {
        match self {
            Severity::Critical => ("crit", "치명"),
            Severity::High => ("high", "높음"),
            Severity::Medium => ("mid", "중간"),
            Severity::After => ("after", "사후 확인"),
        }
    }
}

/// Counts by a key, the most first, as (key, count).
fn counted<'a>(keys: impl Iterator<Item = String> + 'a) -> Vec<(String, usize)> {
    let mut by: BTreeMap<String, usize> = BTreeMap::new();
    for k in keys {
        *by.entry(k).or_default() += 1;
    }
    let mut list: Vec<(String, usize)> = by.into_iter().collect();
    list.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    list
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Tab {
    Overview,
    Detections,
    Machines,
    Sessions,
    Engine,
}

impl Tab {
    const ALL: [Tab; 5] = [
        Tab::Overview,
        Tab::Detections,
        Tab::Machines,
        Tab::Sessions,
        Tab::Engine,
    ];

    fn label(self) -> &'static str {
        match self {
            Tab::Overview => "개요",
            Tab::Detections => "탐지",
            Tab::Machines => "단말",
            Tab::Sessions => "위험 세션",
            Tab::Engine => "엔진 상태",
        }
    }
}

/// The tab the page opens on: `VIEWER_TAB` (overview, detections, machines, sessions, engine)
/// names another, to work on it without clicking there.
fn first_tab() -> Tab {
    match std::env::var("VIEWER_TAB").as_deref() {
        Ok("detections") => Tab::Detections,
        Ok("machines") => Tab::Machines,
        Ok("sessions") => Tab::Sessions,
        Ok("engine") => Tab::Engine,
        _ => Tab::Overview,
    }
}

#[component]
pub fn Main() -> Element {
    let mut state = use_context::<AppState>();
    let p = (state.public)();
    let super::Facts(found, refreshed) = use_context::<super::Facts>();
    let mut tab = use_signal(first_tab);
    let mut filter = use_signal(Filter::default);
    // The Call worth a look opened on the 탐지 tab.
    let picked = use_signal(|| None::<(String, i64)>);
    let now = now_ms();
    let f = filter();

    let every: Vec<Session> = match &*state.sessions.read() {
        Load::Ready(list) => list.clone(),
        _ => Vec::new(),
    };
    let loading = matches!(*state.sessions.read(), Load::Loading) || found.read().is_none();
    let (all_facts, failed): (Vec<CallFact>, Option<String>) = match &*found.read() {
        Some(Ok(list)) => (list.clone(), None),
        Some(Err(e)) => (Vec::new(), Some(e.clone())),
        None => (Vec::new(), None),
    };
    let base_filter = f.base();
    let base: Vec<CallFact> = all_facts
        .iter()
        .filter(|x| base_filter.keeps(x, now))
        .cloned()
        .collect();
    let facts: Vec<CallFact> = base.iter().filter(|x| f.keeps(x, now)).cloned().collect();
    let with_calls: HashSet<&str> = facts.iter().map(|x| x.session.as_str()).collect();
    let sessions: Vec<Session> = every
        .iter()
        .filter(|s| f.keeps_session(s, now, &with_calls))
        .cloned()
        .collect();
    // The machines over the period alone, so the list keeps every machine to pick from.
    let period = Filter {
        days: f.days,
        ..Filter {
            days: None,
            ..Filter::default()
        }
    };
    let in_period: Vec<Session> = every
        .iter()
        .filter(|s| period.keeps_session(s, now, &HashSet::new()))
        .cloned()
        .collect();
    let all_machines = machines(&every);

    // The newest decision in view tells the engine's mode.
    let mode = base
        .iter()
        .filter(|x| x.mode.is_some())
        .max_by_key(|x| x.at)
        .and_then(|x| x.mode.clone());
    let (tone, mode_name, mode_text) = mode_line(mode.as_deref());
    let source = if p.demo {
        "데모 데이터".to_owned()
    } else {
        p.store()
    };
    let lede = match &f.machine {
        Some(m) => format!("{m} 단말의 에이전트가 한 도구 호출과 엔진의 판정입니다."),
        None => "모든 개발자 단말의 에이전트가 한 도구 호출과 엔진의 판정입니다.".to_owned(),
    };

    // The filter's choices, from the period's Calls.
    let tools: Vec<String> = counted(base.iter().filter_map(|x| x.tool.clone()))
        .into_iter()
        .map(|(k, _)| k)
        .collect();
    let kinds: Vec<String> = counted(base.iter().filter_map(kind_of))
        .into_iter()
        .map(|(k, _)| k)
        .collect();
    let folders: Vec<String> = counted(
        base.iter()
            .map(|x| folder_of(x.cwd.as_deref()))
            .filter(|x| !x.is_empty()),
    )
    .into_iter()
    .map(|(k, _)| k)
    .collect();
    let narrowed = f != Filter::default();
    let current = tab();

    rsx! {
        div { class: "home admin",
            header { class: "cs-head",
                div { class: "cs-crumb",
                    strong { "보안 현황" }
                    span { class: "sep" }
                    span { "{current.label()}" }
                }
                span { class: "cs-src", title: "{lede}",
                    span { class: if p.demo { "dot demo" } else { "dot" } }
                    "{source}"
                }
                span { class: "cs-mode {tone}", title: "{mode_text}",
                    span { class: "cs-mode-dot" }
                    "엔진: {mode_name}"
                }
            }
            div { class: "admin-bar",
                nav { class: "admin-tabs",
                    for t in Tab::ALL {
                        button {
                            key: "{t.label()}",
                            class: if current == t { "on" } else { "" },
                            onclick: move |_| tab.set(t),
                            "{t.label()}"
                        }
                    }
                }
                div { class: "admin-filters",
                    Pick {
                        label: "기간",
                        value: f.days.map_or("all".to_owned(), |d| d.to_string()),
                        choices: vec![
                            ("1".into(), "최근 24시간".into()),
                            ("7".into(), "최근 7일".into()),
                            ("30".into(), "최근 30일".into()),
                            ("all".into(), "전체 기간".into()),
                        ],
                        all: None,
                        onpick: move |v: String| filter.write().days = v.parse().ok(),
                    }
                    Pick {
                        label: "단말",
                        value: f.machine.clone().unwrap_or_default(),
                        choices: all_machines.iter().map(|m| (m.name.clone(), m.name.clone())).collect(),
                        all: Some("모든 단말"),
                        onpick: move |v: String| filter.write().machine = Some(v).filter(|v| !v.is_empty()),
                    }
                    Pick {
                        label: "판정",
                        value: f.decision.clone().unwrap_or_default(),
                        choices: ["DENY", "REVIEW", "ALLOW", "NONE"].iter().map(|k| (k.to_string(), decision_label(k).to_owned())).collect(),
                        all: Some("모든 판정"),
                        onpick: move |v: String| filter.write().decision = Some(v).filter(|v| !v.is_empty()),
                    }
                    Pick {
                        label: "위반 유형",
                        value: f.kind.clone().unwrap_or_default(),
                        choices: kinds.iter().map(|k| (k.clone(), kind_label(k))).collect(),
                        all: Some("모든 유형"),
                        onpick: move |v: String| filter.write().kind = Some(v).filter(|v| !v.is_empty()),
                    }
                    Pick {
                        label: "도구",
                        value: f.tool.clone().unwrap_or_default(),
                        choices: tools.iter().map(|k| (k.clone(), k.clone())).collect(),
                        all: Some("모든 도구"),
                        onpick: move |v: String| filter.write().tool = Some(v).filter(|v| !v.is_empty()),
                    }
                    Pick {
                        label: "호출을 시킨 것",
                        value: f.source.clone().unwrap_or_default(),
                        choices: ["request", "segment", "none", "untraced"].iter().map(|k| (k.to_string(), source_label(k).to_owned())).collect(),
                        all: Some("모두"),
                        onpick: move |v: String| filter.write().source = Some(v).filter(|v| !v.is_empty()),
                    }
                    Pick {
                        label: "작업 폴더",
                        value: f.folder.clone().unwrap_or_default(),
                        choices: folders.iter().map(|k| (k.clone(), k.clone())).collect(),
                        all: Some("모든 폴더"),
                        onpick: move |v: String| filter.write().folder = Some(v).filter(|v| !v.is_empty()),
                    }
                    if narrowed {
                        button { class: "admin-clear", onclick: move |_| filter.set(Filter::default()), "필터 지우기" }
                    }
                }
            }
            if let Some(e) = failed {
                p { class: "admin-failed", "호출 통계를 불러오지 못했습니다: {e}" }
            }
            match current {
                Tab::Overview => rsx! {
                    Overview { facts, base, days: f.days, now, loading, filter, tab, picked, refreshed: refreshed() }
                },
                Tab::Detections => rsx! {
                    super::detections::Detections { facts, all: all_facts.clone(), picked, loading }
                },
                Tab::Machines => rsx! {
                    Machines {
                        machines: machines(&in_period),
                        current: f.machine.clone(),
                        loading,
                        onpick: move |name: String| {
                            filter.write().machine = Some(name);
                            tab.set(Tab::Overview);
                            document::eval("document.querySelector('.home.admin')?.scrollTo({top: 0, behavior: 'smooth'})");
                        },
                    }
                },
                Tab::Sessions => rsx! { Sessions { sessions, loading } },
                Tab::Engine => rsx! { Engine { base, loading } },
            }
            footer { class: "home-foot",
                div {
                    strong { "연결" }
                    span { "{source}" }
                }
                div { class: "acts",
                    button { class: "home-soft small", onclick: move |_| state.settings_open.set(true), "연결 설정" }
                }
            }
        }
    }
}

/// One of the filter bar's choices: a labelled drop-down, "all" first when it has one.
#[component]
fn Pick(
    label: &'static str,
    value: String,
    choices: Vec<(String, String)>,
    all: Option<&'static str>,
    onpick: EventHandler<String>,
) -> Element {
    let on = !value.is_empty() && all.is_some();
    rsx! {
        label { class: if on { "admin-pick-field on" } else { "admin-pick-field" },
            span { "{label}" }
            select { value: "{value}", onchange: move |e| onpick.call(e.value()),
                if let Some(all) = all {
                    option { value: "", "{all}" }
                }
                for (key, text) in choices {
                    option { key: "{key}", value: "{key}", selected: key == value, "{text}" }
                }
            }
        }
    }
}

/// When a moment was, to the second.
pub(super) fn clock(ms: Option<i64>) -> String {
    ms.and_then(|ms| Local.timestamp_millis_opt(ms).single())
        .map_or(String::new(), |t| t.format("%H:%M:%S").to_string())
}

pub(super) fn stamp(ms: Option<i64>) -> String {
    ms.and_then(|ms| Local.timestamp_millis_opt(ms).single())
        .map_or(String::new(), |t| t.format("%Y.%m.%d %H:%M:%S").to_string())
}

pub(super) fn period_label(days: Option<i64>) -> String {
    match days {
        Some(1) => "최근 24시간".into(),
        Some(d) => format!("최근 {d}일"),
        None => "전체 기간".into(),
    }
}

/// A prompt as a card shows it, on one line: the query cuts it at its first quote and leaves
/// JSON's escapes, so those are undone here.
fn prompt_line(p: &str) -> String {
    let p = p
        .trim_end_matches('\\')
        .replace("\\n", " ")
        .replace("\\t", " ");
    p.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The prompts whose turns had Calls stopped, the most first: the prompt, how many it had
/// stopped, and its latest stopped Call, to open.
fn stopping_prompts(facts: &[CallFact]) -> Vec<(String, usize, (String, i64))> {
    /// A prompt's stops so far, and the latest of them.
    struct Stops<'a> {
        n: usize,
        latest: &'a CallFact,
    }
    let mut by: BTreeMap<String, Stops> = BTreeMap::new();
    for f in facts.iter().filter(|f| decision_key(f) == "DENY") {
        let text = f
            .prompt
            .as_deref()
            .map(prompt_line)
            .filter(|p| !p.is_empty())
            .unwrap_or_else(|| "(프롬프트를 찾지 못함)".into());
        let e = by.entry(text).or_insert(Stops { n: 0, latest: f });
        e.n += 1;
        if (f.at, f.seq) >= (e.latest.at, e.latest.seq) {
            e.latest = f;
        }
    }
    let mut list: Vec<(String, usize, (String, i64))> = by
        .into_iter()
        .map(|(text, s)| (text, s.n, (s.latest.session.clone(), s.latest.seq)))
        .collect();
    list.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    list
}

/// Hard to undo: the engine said so, or the draft grade rules put it at 2-1 or 3-1.
fn hard_to_undo(f: &CallFact) -> bool {
    f.irreversible || matches!(grade_of(f), Grade::TwoOne | Grade::ThreeOne)
}

/// The Calls a person let run after the engine asked, the hardest to undo first, then the newest.
fn approved_calls(facts: &[CallFact]) -> Vec<CallFact> {
    let mut list: Vec<CallFact> = facts
        .iter()
        .filter(|f| decision_key(f) == "REVIEW" && f.ran)
        .cloned()
        .collect();
    list.sort_by_key(|f| std::cmp::Reverse((hard_to_undo(f), f.at, f.seq)));
    list
}

/// A chart's top and the step between its grid lines: a round step, about ten lines at most.
pub(super) fn scale(max: usize) -> (usize, usize) {
    let max = max.max(1);
    let mut step = 1;
    'find: for power in [1, 10, 100, 1_000, 10_000, 100_000] {
        for m in [1, 2, 5] {
            if max.div_ceil(m * power) <= 10 {
                step = m * power;
                break 'find;
            }
        }
    }
    (max.div_ceil(step) * step, step)
}

/// The overview, laid out as an endpoint console's activity dashboard: four numbers and lists
/// across the top, then the decisions over time beside the latest Calls worth a look, then
/// those Calls by kind over time.
#[component]
fn Overview(
    facts: Vec<CallFact>,
    base: Vec<CallFact>,
    days: Option<i64>,
    now: i64,
    loading: bool,
    filter: Signal<Filter>,
    tab: Signal<Tab>,
    picked: Signal<Option<(String, i64)>>,
    refreshed: Option<i64>,
) -> Element {
    let mut picked = picked;
    let mut state = use_context::<AppState>();
    let c = coverage(&base);
    let period = period_label(days);
    let at = clock(refreshed);
    let count = |d: &str| facts.iter().filter(|f| decision_key(f) == d).count();
    let (denied, reviewed) = (count("DENY"), count("REVIEW"));
    let approved_n = facts
        .iter()
        .filter(|f| decision_key(f) == "REVIEW" && f.ran)
        .count();
    let prompts = stopping_prompts(&facts);
    let prompt_total: usize = prompts.iter().map(|p| p.1).sum();
    let approved = approved_calls(&facts);
    let mut to_approvals = use_context::<super::ToApprovals>().0;
    let buckets = trend(&facts, days, now);
    let (labels, kind_buckets, kinds) = by_kind(&facts, days, now);
    let mut recent: Vec<(Severity, CallFact)> = facts
        .iter()
        .filter_map(|f| Severity::of(f).map(|s| (s, f.clone())))
        .collect();
    recent.sort_by_key(|(_, f)| std::cmp::Reverse((f.at, f.seq)));
    let recent_total = recent.len();
    recent.truncate(RECENT);
    let picked_kind = filter().kind;
    let none = if loading {
        "불러오는 중…"
    } else {
        "데이터 없음"
    };

    rsx! {
        div { class: "cs-grid",
            div { class: "cs-row four",
                Card { title: "차단 모드로 보호 중", stamp: at.clone(),
                    button { class: "cs-figure", title: "단말별 판정 방식 보기", onclick: move |_| tab.set(Tab::Engine),
                        span { class: "cs-big", if loading { "…" } else { "{c.blocking}" } }
                        span { class: "cs-of", "/ {c.active}" }
                    }
                    div { class: "cs-sub",
                        span { "막지 않는 모드 " strong { class: "warn", "{c.watching}" } "대" }
                        span { "판정 없는 호출이 있는 단말 " strong { "{c.undecided}" } "대" }
                    }
                }
                Card { title: "새 승인 요청", note: period.clone(), stamp: at.clone(),
                    button {
                        class: "cs-figure",
                        title: "감사의 승인 기록 보기",
                        onclick: move |_| {
                            to_approvals.set(Some(None));
                            state.screen.set(Screen::FeatureA);
                        },
                        span { class: "cs-big", if loading { "…" } else { "{big(reviewed as i64)}" } }
                    }
                    div { class: "cs-sub",
                        span { title: "확인 요청 뒤 실행된 것", "사용자 승인 " strong { class: "good", "{approved_n}" } "건" }
                        span { title: "확인 요청 뒤 실행되지 않은 것", "반려 " strong { "{reviewed - approved_n}" } "건" }
                        span { "같은 기간 차단 " strong { class: "bad", "{denied}" } "건" }
                    }
                }
                Card { title: "자주 차단을 일으킨 프롬프트", note: "누르면 그 프롬프트로 일어난 일", stamp: at.clone(),
                    if prompts.is_empty() {
                        Empty { text: none }
                    } else {
                        div { class: "cs-list",
                            for (text, n, (session, seq)) in prompts {
                                button {
                                    key: "{text}",
                                    class: "cs-list-row pick",
                                    title: "{text}",
                                    onclick: move |_| {
                                        state.open_call(session.clone(), seq);
                                        state.screen.set(Screen::Work);
                                    },
                                    span { class: "prompt", "“{text}”" }
                                    span { class: "n", "{n}" }
                                }
                            }
                        }
                        div { class: "cs-total",
                            span { "차단 합계" }
                            span { class: "n", "{prompt_total}" }
                        }
                    }
                }
                Card { title: "사용자가 승인한 것", note: "되돌리기 어려운 것부터", stamp: at.clone(),
                    if approved.is_empty() {
                        Empty { text: none }
                    } else {
                        div { class: "cs-list",
                            for f in approved {
                                {
                                    let hard = hard_to_undo(&f);
                                    let id = (f.session.clone(), f.seq);
                                    let what = f.target.clone().or_else(|| f.tool.clone()).unwrap_or_default();
                                    rsx! {
                                        button {
                                            key: "{f.session}-{f.seq}",
                                            class: "cs-list-row pick approved",
                                            title: "감사의 승인 기록에서 보기",
                                            onclick: move |_| {
                                                to_approvals.set(Some(Some(id.clone())));
                                                state.screen.set(Screen::FeatureA);
                                            },
                                            span { class: if hard { "mark hard" } else { "mark" } }
                                            code { "{what}" }
                                            span { class: "meta", "{machine_of_fact(&f)} · {clock(f.at)}" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            div { class: "cs-row split",
                Card { title: "판정 추이", note: "차단·확인 요청·되돌리기 어려운 허용", stamp: at.clone(),
                    Line { buckets }
                }
                Card { title: "최근 탐지", note: "{recent_total}건 중 최근 {recent.len()}건", stamp: at.clone(),
                    div { class: "cs-table",
                        div { class: "cs-tr head",
                            span { "심각도" }
                            span { "유형과 대상" }
                            span {}
                            span { "시각" }
                            span { "단말" }
                            span { "링크" }
                        }
                        if recent.is_empty() {
                            Empty { text: none }
                        }
                        div { class: "cs-tbody",
                            for (sev, f) in recent {
                                {
                                    let (tone, label) = sev.tag();
                                    let kind = detection_kind(&f).map(|k| kind_label(&k)).unwrap_or_default();
                                    let what = f.target.clone().or_else(|| f.tool.clone()).unwrap_or_default();
                                    let why = f.reason.clone().unwrap_or_default();
                                    let info = format!("{what}\n\n{why}");
                                    let session = f.session.clone();
                                    let seq = f.seq;
                                    let open = (f.session.clone(), f.seq);
                                    rsx! {
                                        div {
                                            key: "{f.session}-{f.seq}",
                                            class: "cs-tr pick",
                                            title: "탐지 탭에서 자세히 보기",
                                            onclick: move |_| {
                                                picked.set(Some(open.clone()));
                                                tab.set(Tab::Detections);
                                            },
                                            span { class: "sev {tone}",
                                                svg { view_box: "0 0 16 16", width: "15", height: "15",
                                                    polygon { points: "8,1 14.1,4.5 14.1,11.5 8,15 1.9,11.5 1.9,4.5" }
                                                }
                                                "{label}"
                                            }
                                            span { class: "what",
                                                span { class: "kind", "{kind}" }
                                                code { "{what}" }
                                            }
                                            span { class: "info", title: "{info}", "i" }
                                            span { class: "when", "{stamp(f.at)}" }
                                            span { class: "host", "{machine_of_fact(&f)}" }
                                            button {
                                                class: "link",
                                                onclick: move |e: MouseEvent| {
                                                    e.stop_propagation();
                                                    state.open_call(session.clone(), seq);
                                                    state.screen.set(Screen::Work);
                                                },
                                                "노선도에서 보기"
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            div { class: "cs-row",
                Card { title: "유형별 탐지", note: "{period}", stamp: at.clone(),
                    Stack { labels, buckets: kind_buckets, kinds: kinds.clone(), picked: picked_kind.clone(), filter }
                    div { class: "cs-legend",
                        for (k, _) in kinds {
                            {
                                let on = picked_kind.as_deref() == Some(k.as_str());
                                let dim = picked_kind.is_some() && !on;
                                let key = k.clone();
                                rsx! {
                                    button {
                                        key: "{k}",
                                        class: if dim { "dim" } else { "" },
                                        disabled: k == ALLOWED_IRREVERSIBLE,
                                        onclick: move |_| pick_kind(filter, &key),
                                        i { style: "background:{kind_color(&k)}" }
                                        "{kind_label(&k)}"
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

/// How many of the latest Calls worth a look the overview lists.
const RECENT: usize = 50;

/// Narrows the filter to a kind, or lets it go when it was the one.
fn pick_kind(mut filter: Signal<Filter>, kind: &str) {
    if kind == ALLOWED_IRREVERSIBLE {
        return;
    }
    let mut w = filter.write();
    w.kind = if w.kind.as_deref() == Some(kind) {
        None
    } else {
        Some(kind.to_owned())
    };
}

/// A dashboard card: its title, what it counts over, what it shows, and when it was read.
#[component]
pub(super) fn Card(
    title: &'static str,
    note: Option<String>,
    stamp: String,
    children: Element,
) -> Element {
    rsx! {
        section { class: "cs-card",
            div { class: "cs-card-head",
                h3 { "{title}" }
                if let Some(note) = note { span { class: "cs-note", "{note}" } }
            }
            div { class: "cs-card-body", {children} }
            p { class: "cs-stamp", if !stamp.is_empty() { "마지막 갱신: {stamp}" } }
        }
    }
}

/// A card with nothing to show.
#[component]
pub(super) fn Empty(text: &'static str) -> Element {
    rsx! {
        div { class: "cs-empty",
            svg { view_box: "0 0 20 20", width: "18", height: "18",
                rect { x: "3", y: "3", width: "14", height: "4", rx: "1" }
                rect { x: "3", y: "9", width: "14", height: "3", rx: "1" }
                rect { x: "3", y: "14", width: "14", height: "3", rx: "1" }
            }
            strong { "{text}" }
        }
    }
}

/// The Calls worth a look over time, as one line: stopped, held, and allowed though hard to undo.
#[component]
fn Line(buckets: Vec<Bucket>) -> Element {
    let (w, h, left, right, top, bottom) = (480.0, 330.0, 26.0, 18.0, 10.0, 24.0);
    let plot_w = w - left - right;
    let plot_h = h - top - bottom;
    let values: Vec<usize> = buckets
        .iter()
        .map(|b| b.denied + b.reviewed + b.risky)
        .collect();
    let n = values.len().max(1);
    let (top_v, step) = scale(values.iter().copied().max().unwrap_or(0));
    let x = move |i: usize| {
        if n == 1 {
            left + plot_w / 2.0
        } else {
            left + plot_w * i as f64 / (n - 1) as f64
        }
    };
    let y = move |v: usize| top + plot_h - v as f64 / top_v as f64 * plot_h;
    let points: String = values
        .iter()
        .enumerate()
        .map(|(i, v)| format!("{:.1},{:.1}", x(i), y(*v)))
        .collect::<Vec<_>>()
        .join(" ");
    let every = n.div_ceil(8);
    let ticks: Vec<usize> = (0..=top_v).step_by(step).collect();
    rsx! {
        svg { class: "cs-chart", view_box: "0 0 {w} {h}",
            for t in ticks {
                g { key: "tick-{t}",
                    line { x1: "{left}", x2: "{w - right}", y1: "{y(t):.1}", y2: "{y(t):.1}", class: "grid" }
                    text { x: "{left - 6.0}", y: "{y(t) + 3.0:.1}", text_anchor: "end", class: "axis", "{t}" }
                }
            }
            polyline { class: "cs-line", points: "{points}" }
            for (i, b) in buckets.iter().enumerate() {
                g { key: "pt-{i}",
                    title { "{b.label}: 차단 {b.denied}, 확인 요청 {b.reviewed}, 되돌리기 어려운 허용 {b.risky}" }
                    circle { cx: "{x(i):.1}", cy: "{y(values[i]):.1}", r: "2.6", class: "cs-dot" }
                    if i % every == 0 || i + 1 == n {
                        text { x: "{x(i):.1}", y: "{h - 8.0}", text_anchor: "middle", class: "axis", "{b.label}" }
                    }
                }
            }
        }
    }
}

/// The Calls worth a look by kind over time, stacked; a part clicked narrows the filter to its kind.
#[component]
fn Stack(
    labels: Vec<String>,
    buckets: Vec<BTreeMap<String, usize>>,
    kinds: Vec<(String, usize)>,
    picked: Option<String>,
    filter: Signal<Filter>,
) -> Element {
    let (w, h, left, right, top, bottom) = (1200.0, 260.0, 30.0, 10.0, 10.0, 26.0);
    let plot_w = w - left - right;
    let plot_h = h - top - bottom;
    let n = labels.len().max(1);
    let max = buckets
        .iter()
        .map(|b| b.values().sum::<usize>())
        .max()
        .unwrap_or(0);
    let (top_v, step) = scale(max);
    let y = move |v: usize| top + plot_h - v as f64 / top_v as f64 * plot_h;
    let slot = plot_w / n as f64;
    let bar = (slot * 0.6).max(2.0);
    let every = n.div_ceil(10);
    let ticks: Vec<usize> = (0..=top_v).step_by(step).collect();
    rsx! {
        svg { class: "cs-chart wide", view_box: "0 0 {w} {h}",
            for t in ticks {
                g { key: "tick-{t}",
                    line { x1: "{left}", x2: "{w - right}", y1: "{y(t):.1}", y2: "{y(t):.1}", class: "grid" }
                    text { x: "{left - 6.0}", y: "{y(t) + 3.0:.1}", text_anchor: "end", class: "axis", "{t}" }
                }
            }
            for (i, b) in buckets.iter().enumerate() {
                {
                    let x = left + slot * i as f64 + (slot - bar) / 2.0;
                    let mut under = 0;
                    let parts: Vec<(String, f64, f64)> = kinds
                        .iter()
                        .filter_map(|(k, _)| {
                            let v = *b.get(k)?;
                            let part = (k.clone(), y(under + v), y(under) - y(under + v));
                            under += v;
                            Some(part)
                        })
                        .collect();
                    let title = std::iter::once(labels[i].clone())
                        .chain(kinds.iter().filter_map(|(k, _)| b.get(k).map(|v| format!("{} {v}", kind_label(k)))))
                        .collect::<Vec<_>>()
                        .join("\n");
                    rsx! {
                        g { key: "bar-{i}",
                            title { "{title}" }
                            for (k, py, ph) in parts {
                                {
                                    let dim = picked.is_some() && picked.as_deref() != Some(k.as_str());
                                    let key = k.clone();
                                    rsx! {
                                        rect {
                                            key: "{k}",
                                            class: "cs-part",
                                            x: "{x:.1}", y: "{py:.1}", width: "{bar:.1}", height: "{ph:.1}",
                                            fill: "{kind_color(&k)}",
                                            opacity: if dim { "0.25" } else { "1" },
                                            onclick: move |_| pick_kind(filter, &key),
                                        }
                                    }
                                }
                            }
                            if i % every == 0 || i + 1 == n {
                                text { x: "{x + bar / 2.0:.1}", y: "{h - 8.0}", text_anchor: "middle", class: "axis", "{labels[i]}" }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// The sessions worth a look, in four kinds.
#[component]
fn Sessions(sessions: Vec<Session>, loading: bool) -> Element {
    let pick = |f: fn(&Session) -> bool| {
        sessions
            .iter()
            .filter(|s| f(s))
            .cloned()
            .collect::<Vec<_>>()
    };
    let groups = [
        (
            "차단이 있었던 세션",
            "엔진이 막은 호출이 있습니다. 누르면 그 세션의 노선도가 열리고, 차단된 호출을 누르면 원인 경로가 보입니다.",
            pick(|s| s.denied > 0),
        ),
        (
            "되돌리기 어려운데 허용된 호출이 있는 세션",
            "사용자 의도와 맞아 허용됐지만 되돌리기 어려운 호출이 있습니다. 실제로 의도한 일이었는지 사후 감사가 필요합니다.",
            pick(|s| s.risky > 0),
        ),
        (
            "판정 없이 실행된 호출이 있는 세션",
            "판정이 남지 않은 호출이 있습니다. 엔진이 관찰만 했거나, 판정에 실패했거나, 훅이 우회됐을 수 있습니다.",
            pick(|s| s.undecided > 0),
        ),
        (
            "확인을 요청한 세션",
            "엔진이 사람에게 확인을 넘긴 호출이 있습니다.",
            pick(|s| s.reviewed > 0),
        ),
    ];
    rsx! {
        div { class: "admin-body",
            for (title, about, list) in groups {
                Group { key: "{title}", title, about, sessions: list, loading }
            }
        }
    }
}

/// How the engine enforced, machine by machine; and what this page cannot tell until the
/// engine reports it.
#[component]
fn Engine(base: Vec<CallFact>, loading: bool) -> Element {
    /// A machine as the engine tab lists it.
    #[derive(Default)]
    struct Row {
        name: String,
        /// The latest decision's time and mode.
        latest: Option<(i64, String)>,
        calls: usize,
        undecided: usize,
    }
    let mut by: BTreeMap<String, Row> = BTreeMap::new();
    for f in &base {
        let name = machine_of_fact(f);
        let e = by.entry(name.clone()).or_insert_with(|| Row {
            name,
            ..Row::default()
        });
        e.calls += 1;
        if f.decision.is_none() {
            e.undecided += 1;
        }
        if let (Some(mode), Some(at)) = (&f.mode, f.at)
            && e.latest.as_ref().is_none_or(|(t, _)| at >= *t)
        {
            e.latest = Some((at, mode.clone()));
        }
    }
    let mut rows: Vec<Row> = by.into_values().collect();
    // Machines not blocking first, then those with undecided Calls.
    rows.sort_by_key(|r| {
        (
            r.latest.as_ref().is_some_and(|(_, m)| m == "enforce"),
            std::cmp::Reverse(r.undecided),
            r.name.clone(),
        )
    });
    rsx! {
        div { class: "admin-body",
            section { class: "home-group",
                div { class: "home-group-head",
                    h2 { "단말별 판정 방식" }
                    span { class: "n", "{rows.len()}" }
                }
                p { class: "admin-about", "단말마다 기간 안 가장 최근 판정이 어떤 모드였는지입니다. 막지 않는 단말과 판정 없는 호출이 있는 단말이 위에 옵니다." }
                div { class: "home-list",
                    if rows.is_empty() {
                        p { class: "home-empty", if loading { "불러오는 중…" } else { "기간 안에 호출이 없습니다." } }
                    } else {
                        div { class: "admin-engine head",
                            span { "단말" }
                            span { "판정 방식" }
                            span { "호출" }
                            span { "판정 없음" }
                            span { "마지막 판정" }
                        }
                    }
                    for Row { name, latest, calls, undecided } in rows {
                        {
                            let mode = latest.as_ref().map(|(_, m)| m.as_str());
                            let tone = match mode {
                                Some("enforce") => "good",
                                Some(_) => "warn",
                                None => "grey",
                            };
                            let label = mode_label(mode);
                            let at = latest.as_ref().map(|(t, _)| *t);
                            rsx! {
                                div { key: "{name}", class: "admin-engine",
                                    span { class: "name", "{name}" }
                                    span { class: "mode {tone}", "{label}" }
                                    span { "{calls}" }
                                    span { class: if undecided > 0 { "grey strong" } else { "" }, "{undecided}" }
                                    span { class: "when", "{when(at)}" }
                                }
                            }
                        }
                    }
                }
            }
            section { class: "home-group",
                div { class: "home-group-head", h2 { "엔진이 알려 줘야 보이는 것" } }
                p { class: "admin-about", "아래는 지금 기록만으로는 알 수 없어, 엔진이 상태를 알려 주게 된 뒤에 이 탭에 나옵니다." }
                div { class: "home-list admin-later",
                    p { strong { "판정 실패 시 동작" } span { "판정에 실패하거나 늦을 때 막는지 통과시키는지" } }
                    p { strong { "강등 모드" } span { "Jev 장애로 규칙만으로 판정 중인지" } }
                    p { strong { "구성요소 상태" } span { "PostgreSQL, Valkey, Jev가 응답하는지" } }
                    p { strong { "보안 점검" } span { "게이트웨이가 모든 네트워크에 열려 있는지, 훅 요청에 인증이 있는지" } }
                }
            }
        }
    }
}

/// Every machine, the riskiest first, found by name; a row looks at that machine alone.
#[component]
fn Machines(
    machines: Vec<Machine>,
    current: Option<String>,
    loading: bool,
    onpick: EventHandler<String>,
) -> Element {
    let mut query = use_signal(String::new);
    let mut all = use_signal(|| false);
    let needle = query().trim().to_lowercase();
    let found: Vec<Machine> = machines
        .iter()
        .filter(|m| needle.is_empty() || m.name.to_lowercase().contains(&needle))
        .cloned()
        .collect();
    let total = found.len();
    let shown: Vec<Machine> = found
        .into_iter()
        .take(if all() { usize::MAX } else { MACHINES_SHOWN })
        .collect();
    rsx! {
        div { class: "admin-body",
            section { class: "home-group",
                div { class: "home-group-head",
                    h2 { "단말" }
                    span { class: "n", "{machines.len()}" }
                    input {
                        class: "admin-search",
                        r#type: "search",
                        placeholder: "단말 이름으로 찾기",
                        value: "{query}",
                        oninput: move |e| query.set(e.value()),
                    }
                }
                p { class: "admin-about", "기간 안에 세션이 있었던 개발자 단말입니다. 훅이 보낸 단말 이름(x-withhuman-client) 기준이고, 위험한 단말이 위에 옵니다. 줄을 누르면 그 단말만 봅니다." }
                div { class: "home-list",
                    if machines.is_empty() {
                        p { class: "home-empty", if loading { "세션을 불러오는 중…" } else { "기간 안에 세션이 있었던 단말이 없습니다." } }
                    } else if shown.is_empty() {
                        p { class: "home-empty", "\"{query}\"에 맞는 단말이 없습니다." }
                    } else {
                        div { class: "admin-machine head",
                            span { "단말" }
                            span { "세션" }
                            span { "차단" }
                            span { "되돌리기 어려운 허용" }
                            span { "판정 없음" }
                            span { "마지막 활동" }
                        }
                    }
                    for m in shown {
                        button {
                            key: "{m.name}",
                            class: if current.as_deref() == Some(m.name.as_str()) { "admin-machine on" } else { "admin-machine" },
                            onclick: {
                                let name = m.name.clone();
                                move |_| onpick.call(name.clone())
                            },
                            span { class: "name",
                                if !m.risk().is_empty() { span { class: "admin-pick-dot {m.risk()}" } }
                                "{m.name}"
                            }
                            span { "{m.sessions}" }
                            span { class: if m.denied > 0 { "red" } else { "" }, "{m.denied}" }
                            span { class: if m.risky > 0 { "orange" } else { "" }, "{m.risky}" }
                            span { class: if m.undecided > 0 { "grey" } else { "" }, "{m.undecided}" }
                            span { class: "when", "{when(m.last_at)}" }
                        }
                    }
                    if total > MACHINES_SHOWN {
                        button { class: "home-more", onclick: move |_| all.toggle(),
                            if all() { "접기" } else { "{total - MACHINES_SHOWN}개 더 보기" }
                        }
                    }
                }
            }
        }
    }
}

/// A kind of session worth a look: what it means, then the first few rows and all on demand.
#[component]
fn Group(
    title: &'static str,
    about: &'static str,
    sessions: Vec<Session>,
    loading: bool,
) -> Element {
    let mut all = use_signal(|| false);
    let total = sessions.len();
    let shown: Vec<Session> = sessions
        .into_iter()
        .take(if all() { usize::MAX } else { SHOWN })
        .collect();
    rsx! {
        section { class: "home-group",
            div { class: "home-group-head",
                h2 { "{title}" }
                span { class: "n", "{total}" }
            }
            p { class: "admin-about", "{about}" }
            div { class: "home-list",
                if shown.is_empty() {
                    p { class: "home-empty", if loading { "세션을 불러오는 중…" } else { "해당하는 세션이 없습니다." } }
                }
                for s in shown {
                    SessionRow { key: "{s.id}", s }
                }
                if total > SHOWN {
                    button { class: "home-more", onclick: move |_| all.toggle(),
                        if all() { "접기" } else { "{total - SHOWN}개 더 보기" }
                    }
                }
            }
        }
    }
}

/// A session as a row: how risky it looks, what was asked, where and what happened; it opens the
/// session's route map.
#[component]
pub(super) fn SessionRow(s: Session) -> Element {
    let mut state = use_context::<AppState>();
    let id = s.id.clone();
    let folder = s.folder();
    let risk = Risk::of(&s).badge();
    rsx! {
        div { class: "admin-row",
        button {
            class: "home-row",
            title: "{s.title()}",
            onclick: move |_| {
                state.open_session(id.clone());
                state.mode.set(Mode::Graph);
                state.screen.set(Screen::Work);
            },
            if let Some((tone, label)) = risk {
                span { class: "admin-risk {tone}", "{label}" }
            } else {
                span { class: "admin-risk" }
            }
            span { class: "t", "{s.title()}" }
            span { class: "chips",
                if let Some(client) = &s.client { span { class: "chip machine", "{client}" } }
                if !folder.is_empty() { span { class: "chip", "{folder}" } }
                if s.denied > 0 { span { class: "chip red", "차단 {s.denied}" } }
                if s.risky > 0 { span { class: "chip orange", "되돌리기 어려움 {s.risky}" } }
                if s.undecided > 0 { span { class: "chip", "판정 없음 {s.undecided}" } }
                if s.reviewed > 0 { span { class: "chip amber", "확인 {s.reviewed}" } }
                if s.from_result > 0 { span { class: "chip", "도구 결과가 시킴 {s.from_result}" } }
            }
            span { class: "when", "{when(s.last_at.or(s.created_at))}" }
        }
        // The kill switch's place: it stops the session once the engine can.
        button {
            class: "admin-stop",
            disabled: true,
            title: "엔진의 정지 기능이 생기면 이 세션의 모든 도구 호출을 막습니다.",
            "정지"
        }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session(
        client: Option<&str>,
        denied: i64,
        risky: i64,
        undecided: i64,
        last_at: i64,
    ) -> Session {
        Session {
            id: format!("{client:?}-{last_at}"),
            client: client.map(str::to_owned),
            denied,
            risky,
            undecided,
            last_at: Some(last_at),
            ..Session::default()
        }
    }

    fn fact(client: &str, at: i64, decision: Option<&str>, mode: Option<&str>) -> CallFact {
        CallFact {
            session: format!("{client}-s"),
            client: Some(client.into()),
            at: Some(at),
            decision: decision.map(str::to_owned),
            mode: mode.map(str::to_owned),
            ..CallFact::default()
        }
    }

    #[test]
    fn risk_follows_the_counts() {
        assert_eq!(Risk::of(&session(None, 1, 0, 0, 0)), Risk::High);
        assert_eq!(Risk::of(&session(None, 0, 1, 0, 0)), Risk::High);
        assert_eq!(Risk::of(&session(None, 0, 0, 2, 0)), Risk::Watch);
        assert_eq!(
            Risk::of(&Session {
                from_result: 1,
                ..Session::default()
            }),
            Risk::Watch
        );
        assert_eq!(Risk::of(&Session::default()), Risk::Low);
    }

    #[test]
    fn machines_put_the_riskiest_first() {
        let list = machines(&[
            session(Some("a"), 0, 0, 1, 30),
            session(Some("b"), 1, 1, 0, 10),
            session(Some("b"), 0, 0, 0, 20),
            session(None, 0, 0, 0, 5),
            session(Some("c"), 0, 0, 0, 40),
        ]);
        let names: Vec<&str> = list.iter().map(|m| m.name.as_str()).collect();
        assert_eq!(names, ["b", "a", "c", NO_NAME]);
        assert_eq!(list[0].sessions, 2);
        assert_eq!(list[0].last_at, Some(20));
    }

    #[test]
    fn coverage_counts_machines_by_their_latest_mode() {
        let c = coverage(&[
            fact("a", 10, Some("DENY"), Some("shadow")),
            fact("a", 20, Some("ALLOW"), Some("enforce")),
            fact("b", 30, Some("ALLOW"), Some("shadow")),
            fact("c", 40, None, None),
            fact("c", 50, Some("ALLOW"), Some("enforce")),
        ]);
        assert_eq!(
            c,
            Coverage {
                active: 3,
                blocking: 2,
                watching: 1,
                undecided: 1
            }
        );
    }

    #[test]
    fn the_filter_narrows_calls_and_sessions() {
        let now = 100 * DAY;
        let mut f = Filter::default();
        let old = fact("a", now - 8 * DAY, Some("DENY"), None);
        let new = fact("a", now - DAY, Some("DENY"), None);
        assert!(!f.keeps(&old, now), "older than the seven days");
        assert!(f.keeps(&new, now));
        f.days = None;
        assert!(f.keeps(&old, now));
        f.decision = Some("NONE".into());
        assert!(!f.keeps(&new, now));
        assert!(f.keeps(&fact("a", now, None, None), now));
        f.decision = None;
        f.machine = Some("b".into());
        assert!(!f.keeps(&new, now));
        // A session counts only when one of its Calls is kept, once the Calls are narrowed.
        let mut f = Filter {
            days: None,
            ..Filter::default()
        };
        let s = Session {
            id: "x".into(),
            last_at: Some(now),
            ..Session::default()
        };
        assert!(f.keeps_session(&s, now, &HashSet::new()));
        f.tool = Some("Bash".into());
        assert!(!f.keeps_session(&s, now, &HashSet::new()));
        assert!(f.keeps_session(&s, now, &HashSet::from(["x"])));
    }

    #[test]
    fn kinds_name_what_stopped_calls_broke() {
        let mut f = fact("a", 0, Some("DENY"), None);
        f.worst = Some("exfiltration".into());
        assert_eq!(kind_of(&f).as_deref(), Some("exfiltration"));
        f.rule = Some("safeguard::change".into());
        assert_eq!(kind_of(&f).as_deref(), Some("safeguard"));
        f.rule = None;
        f.worst = Some("none".into());
        assert_eq!(kind_of(&f).as_deref(), Some("other"));
        f.decision = Some("ALLOW".into());
        assert_eq!(
            kind_of(&f),
            None,
            "an allowed Call broke nothing worth a bar"
        );
    }

    #[test]
    fn the_trend_puts_each_decision_on_its_day() {
        let now = Local
            .with_ymd_and_hms(2026, 11, 10, 12, 0, 0)
            .unwrap()
            .timestamp_millis();
        let mut risky = fact("a", now - DAY, Some("ALLOW"), None);
        risky.irreversible = true;
        let facts = [
            fact("a", now, Some("DENY"), None),
            fact("a", now - DAY, Some("REVIEW"), None),
            risky,
            fact("a", now - DAY, Some("ALLOW"), None),
            fact("a", now - 9 * DAY, Some("DENY"), None),
        ];
        let b = trend(&facts, Some(7), now);
        assert_eq!(b.len(), 7);
        assert_eq!(b[6].label, "11/10");
        assert_eq!((b[6].denied, b[6].reviewed, b[6].risky), (1, 0, 0));
        assert_eq!((b[5].denied, b[5].reviewed, b[5].risky), (0, 1, 1));
        assert_eq!(
            b.iter().map(|x| x.denied).sum::<usize>(),
            1,
            "nine days back is outside"
        );
        let hours = trend(&facts, Some(1), now);
        assert_eq!(hours.len(), 24);
        assert_eq!(hours[23].denied, 1);
    }

    #[test]
    fn the_scale_steps_roundly() {
        assert_eq!(scale(0), (1, 1));
        assert_eq!(scale(5), (5, 1));
        assert_eq!(scale(10), (10, 1));
        assert_eq!(scale(11), (12, 2));
        assert_eq!(scale(47), (50, 5));
        assert_eq!(scale(129), (140, 20));
    }

    #[test]
    fn severity_and_kind_name_what_is_worth_a_look() {
        let mut f = fact("a", 0, Some("DENY"), None);
        assert_eq!(Severity::of(&f), Some(Severity::High));
        f.worst = Some("exfiltration".into());
        assert_eq!(Severity::of(&f), Some(Severity::Critical));
        let mut f = fact("a", 0, Some("REVIEW"), None);
        f.tool = Some("Bash".into());
        f.target = Some("rm -rf data/backups".into());
        assert_eq!(
            Severity::of(&f),
            Some(Severity::Critical),
            "3-1 by the draft rules"
        );
        let mut f = fact("a", 0, Some("ALLOW"), None);
        assert_eq!(Severity::of(&f), None);
        assert_eq!(detection_kind(&f), None);
        f.irreversible = true;
        assert_eq!(Severity::of(&f), Some(Severity::After));
        assert_eq!(detection_kind(&f).as_deref(), Some(ALLOWED_IRREVERSIBLE));
    }

    #[test]
    fn by_kind_stacks_each_day() {
        let now = Local
            .with_ymd_and_hms(2026, 11, 10, 12, 0, 0)
            .unwrap()
            .timestamp_millis();
        let mut a = fact("a", now, Some("DENY"), None);
        a.worst = Some("exfiltration".into());
        let mut b = fact("a", now - DAY, Some("ALLOW"), None);
        b.irreversible = true;
        let (labels, buckets, kinds) = by_kind(&[a.clone(), a, b], Some(7), now);
        assert_eq!(labels.len(), 7);
        assert_eq!(buckets[6].get("exfiltration"), Some(&2));
        assert_eq!(buckets[5].get(ALLOWED_IRREVERSIBLE), Some(&1));
        assert_eq!(kinds[0], ("exfiltration".to_owned(), 2));
    }

    #[test]
    fn prompts_are_read_on_one_line_and_counted_by_stops() {
        assert_eq!(
            prompt_line("고쳐줘\\n그리고 \\t돌려줘\\"),
            "고쳐줘 그리고 돌려줘"
        );
        let stop = |prompt: Option<&str>, at: i64, seq: i64| CallFact {
            prompt: prompt.map(str::to_owned),
            seq,
            ..fact("a", at, Some("DENY"), None)
        };
        let list = stopping_prompts(&[
            stop(Some("배포해줘"), 10, 1),
            stop(Some("배포해줘"), 30, 7),
            stop(Some("고쳐줘"), 20, 3),
            stop(None, 5, 9),
            fact("a", 40, Some("ALLOW"), None),
        ]);
        assert_eq!(
            list[0],
            ("배포해줘".to_owned(), 2, ("a-s".to_owned(), 7)),
            "the latest stop opens"
        );
        assert_eq!(list.len(), 3);
        assert!(list.iter().any(|p| p.0 == "(프롬프트를 찾지 못함)"));
    }

    #[test]
    fn approved_calls_put_the_hard_to_undo_first() {
        let asked = |at: i64, ran: bool, target: &str| CallFact {
            ran,
            tool: Some("Bash".into()),
            target: Some(target.into()),
            ..fact("a", at, Some("REVIEW"), None)
        };
        let list = approved_calls(&[
            asked(30, true, "ls"),
            asked(10, true, "git push --force origin main"),
            asked(40, false, "rm -rf data"),
            asked(20, true, "cat README.md"),
        ]);
        let targets: Vec<&str> = list.iter().filter_map(|f| f.target.as_deref()).collect();
        assert_eq!(
            targets,
            ["git push --force origin main", "ls", "cat README.md"],
            "not run is not approved"
        );
    }

    #[test]
    fn the_mode_says_whether_the_engine_blocks() {
        assert_eq!(mode_line(Some("enforce")).1, "차단 중");
        assert_eq!(mode_line(Some("shadow")).0, "warn");
        assert_eq!(mode_line(None).1, "판정 기록 없음");
    }
}
