//! The security team's main page (보안 현황): the Calls worth a look over every machine. It is
//! drawn by the dashboards' page (`web/console/src/pages/detect.tsx`) in a frame; this file hands
//! it the Calls ([`super::detect_web`]) and does what the page asks: open a Call in 기록, go to
//! 감사's approvals, open the connection's settings.
//!
//! What the engine's words mean to the security team is here too, shared with the other screens:
//! what a Call broke, what made the agent take it, how bad it is, the machine it came from.

use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};

use dioxus::prelude::*;
use serde_json::Value;

use super::policy::{Grade, grade_of};
use crate::model::{CallFact, Session};
use crate::ui::{AppState, Load, Screen};

pub(super) const NO_NAME: &str = "이름 없는 단말";

pub(super) const DAY_MS: i64 = 24 * 3_600_000;

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

/// The machines the sessions came from, as the hooks named them: the ones with the most to
/// look at first (stopped or hard to undo, then undecided), then the most recently active.
pub(super) fn machine_names(sessions: &[Session]) -> Vec<String> {
    let mut by: BTreeMap<String, (i64, i64, Option<i64>)> = BTreeMap::new();
    for s in sessions {
        let m = by.entry(machine_of(s)).or_default();
        m.0 += s.denied + s.risky;
        m.1 += s.undecided;
        m.2 = m.2.max(s.last_at.or(s.created_at));
    }
    let mut list: Vec<_> = by.into_iter().collect();
    list.sort_by_key(|(_, rank)| std::cmp::Reverse(*rank));
    list.into_iter().map(|(name, _)| name).collect()
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

    /// Its name as the page knows it.
    pub(super) fn key(self) -> &'static str {
        match self {
            Severity::Critical => "crit",
            Severity::High => "high",
            Severity::Medium => "med",
            Severity::After => "after",
        }
    }
}

#[component]
pub fn Main() -> Element {
    let mut state = use_context::<AppState>();
    let p = (state.public)();
    let super::Facts(found, refreshed) = use_context::<super::Facts>();
    let mut to_approvals = use_context::<super::ToApprovals>().0;

    let sessions: Vec<Session> = match &*state.sessions.read() {
        Load::Ready(list) => list.clone(),
        _ => Vec::new(),
    };
    let loading = matches!(*state.sessions.read(), Load::Loading) || found.read().is_none();
    let (facts, failed): (Vec<CallFact>, Option<String>) = match &*found.read() {
        Some(Ok(list)) => (list.clone(), None),
        Some(Err(e)) => (Vec::new(), Some(e.clone())),
        None => (Vec::new(), None),
    };
    let data = super::detect_web::data(&super::detect_web::Read {
        facts: &facts,
        sessions: &sessions,
        refreshed: refreshed(),
        loading,
        failed: failed.as_deref(),
        source: &p.store(),
        demo: p.demo,
    });

    rsx! {
        super::web::Frame {
            page: "detect",
            data,
            onmsg: move |m: Value| {
                // The Call a message is about, when it names one.
                let call = m
                    .get("session")
                    .and_then(Value::as_str)
                    .zip(m.get("seq").and_then(Value::as_i64))
                    .map(|(session, seq)| (session.to_owned(), seq));
                match (m.get("do").and_then(Value::as_str), call) {
                    (Some("open"), Some((session, seq))) => {
                        state.open_call(session, seq);
                        state.screen.set(Screen::Work);
                    }
                    (Some("approvals"), call) => {
                        to_approvals.set(Some(call));
                        state.screen.set(Screen::FeatureA);
                    }
                    (Some("settings"), _) => state.settings_open.set(true),
                    _ => {}
                }
            },
        }
    }
}

/// A prompt as a list shows it, on one line: the query cuts it at its first quote and leaves
/// JSON's escapes, so those are undone here.
pub(super) fn prompt_line(p: &str) -> String {
    let p = p
        .trim_end_matches('\\')
        .replace("\\n", " ")
        .replace("\\t", " ");
    p.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Hard to undo: the engine said so, or the draft grade rules put it at 2-1 or 3-1.
pub(super) fn hard_to_undo(f: &CallFact) -> bool {
    f.irreversible || matches!(grade_of(f), Grade::TwoOne | Grade::ThreeOne)
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
    fn machines_put_the_riskiest_first() {
        let names = machine_names(&[
            session(Some("a"), 0, 0, 1, 30),
            session(Some("b"), 1, 1, 0, 10),
            session(Some("b"), 0, 0, 0, 20),
            session(None, 0, 0, 0, 5),
            session(Some("c"), 0, 0, 0, 40),
        ]);
        assert_eq!(names, ["b", "a", "c", NO_NAME]);
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
    fn prompts_are_read_on_one_line() {
        assert_eq!(
            prompt_line("고쳐줘\\n그리고 \\t돌려줘\\"),
            "고쳐줘 그리고 돌려줘"
        );
    }
}
