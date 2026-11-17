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

/// What made the agent take a Call, as the filter names it.
pub(super) fn source_of(f: &CallFact) -> &'static str {
    match f.link.as_deref() {
        Some("request") => "request",
        Some("segment") => "segment",
        Some("none") => "none",
        _ => "untraced",
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
    let super::Facts(found, refreshed) = use_context::<super::Facts>();
    let backend = use_context::<std::sync::Arc<crate::api::Backend>>();
    // What the team marked each session as, the case opened, and what a save said.
    // Read again after each change; the engine's where it keeps them, else this Mac's file.
    let mut marked = use_signal(|| 0u32);
    let triage = use_resource({
        let backend = backend.clone();
        move || {
            let b = backend.clone();
            let _ = (state.reload)();
            let _ = (state.public)();
            let _ = marked();
            async move { b.triage().await }
        }
    });
    let mut marks = use_signal(super::triage::Store::default);
    // The Calls that sent sensitive values out, read after the Calls, for the queue.
    let leaks = use_resource({
        let backend = backend.clone();
        move || {
            let b = backend.clone();
            let _ = (state.reload)();
            let _ = (state.public)();
            async move {
                b.secrets()
                    .await
                    .map(|all| super::secrets::leaks(&all))
                    .unwrap_or_default()
            }
        }
    });
    use_effect(move || {
        if let Some(store) = &*triage.read() {
            marks.set(store.clone());
        }
    });
    // The inventory, for whose each session is and which machines to tell the team about.
    let inventory = use_resource({
        let backend = backend.clone();
        move || {
            let b = backend.clone();
            let _ = (state.reload)();
            let _ = (state.public)();
            async move { b.inventory().await.ok().flatten() }
        }
    });
    let mut case = use_signal(|| None::<Value>);
    let mut saved = use_signal(|| None::<String>);

    // The page's data is built again only when what it is built from changes.
    let data = use_memo(move || {
        let p = (state.public)();
        let sessions: Vec<Session> = match &*state.sessions.read() {
            Load::Ready(list) => list.clone(),
            _ => Vec::new(),
        };
        let loading = matches!(*state.sessions.read(), Load::Loading) || found.read().is_none();
        let found = found.read();
        let (facts, failed): (&[CallFact], Option<&str>) = match &*found {
            Some(Ok(list)) => (list, None),
            Some(Err(e)) => (&[], Some(e)),
            None => (&[], None),
        };
        super::detect_web::data(&super::detect_web::Read {
            facts,
            sessions: &sessions,
            refreshed: refreshed(),
            triage: &marks.read(),
            leaks: &leaks.read().clone().unwrap_or_default(),
            alerts: &match &*inventory.read() {
                Some(Some(inv)) => super::detect_web::alerts(inv, now_ms()),
                _ => Value::Null,
            },
            case: case.read().as_ref(),
            saved: saved.read().as_deref(),
            loading,
            failed,
            source: &p.store(),
            demo: p.demo,
        })
    });

    rsx! {
        super::web::Frame {
            page: "detect",
            data: data(),
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
                    // The case of a Call, read from its session's graph.
                    (Some("case"), Some((session, seq))) => {
                        let backend = backend.clone();
                        case.set(Some(serde_json::json!({ "session": session, "seq": seq, "loading": true })));
                        spawn(async move {
                            let facts = match &*found.peek() {
                                Some(Ok(list)) => list.clone(),
                                _ => Vec::new(),
                            };
                            let read = super::case::case(&backend, &session, seq, &facts).await;
                            // Only when that case is still the one open.
                            if case.peek().as_ref().is_some_and(|c| c["session"] == read["session"] && c["seq"] == read["seq"]) {
                                case.set(Some(read));
                            }
                        });
                    }
                    // One session marked, or every session of a case at once.
                    (Some("triage"), _) => {
                        let by = std::env::var("USER").unwrap_or_else(|_| "security-admin".into());
                        let state = m.get("state").and_then(Value::as_str).map(str::to_owned);
                        let sessions: Vec<String> = m
                            .get("sessions")
                            .and_then(Value::as_array)
                            .into_iter()
                            .flatten()
                            .filter_map(Value::as_str)
                            .chain(m.get("session").and_then(Value::as_str))
                            .map(str::to_owned)
                            .collect();
                        let (backend, shared) = (backend.clone(), marks.peek().shared);
                        spawn(async move {
                            if let Err(e) = backend.mark_sessions(shared, &sessions, state.as_deref(), &by).await {
                                saved.set(Some(format!("표시 실패 · {e}")));
                            }
                            marked += 1;
                        });
                    }
                    // A source distrusted, or trusted again (`on` false).
                    (Some("source"), call) => {
                        if let Some(source) = m.get("source").and_then(Value::as_str).map(str::to_owned) {
                            let by = std::env::var("USER").unwrap_or_else(|_| "security-admin".into());
                            let on = m.get("on").and_then(Value::as_bool) != Some(false);
                            let (backend, shared) = (backend.clone(), marks.peek().shared);
                            spawn(async move {
                                if let Err(e) = backend.distrust(shared, &source, on, &by, call).await {
                                    saved.set(Some(format!("출처 차단 실패 · {e}")));
                                }
                                marked += 1;
                            });
                        }
                    }
                    // A case's machine stopped: every tool call from it refused.
                    (Some("stop_device"), call) => {
                        if let Some(device) = m.get("device").and_then(Value::as_str).map(str::to_owned) {
                            let backend = backend.clone();
                            let by = std::env::var("USER").unwrap_or_else(|_| "security-admin".into());
                            let why = call.map_or_else(|| "보안 현황에서 멈춤".to_owned(), |(s, n)| format!("사건 {s} #{n}"));
                            spawn(async move {
                                saved.set(Some(match backend.stop_device(&device, true, &by, &why).await {
                                    Ok(()) => format!("단말 멈춤 · {device}"),
                                    Err(e) => format!("단말 멈춤 실패 · {e}"),
                                }));
                            });
                        }
                    }
                    (Some("inventory"), _) => state.screen.set(Screen::Inventory),
                    // A file the page made (a list, or one case's evidence), saved where asked.
                    (Some("export"), _) => {
                        let name = m.get("name").and_then(Value::as_str).unwrap_or("withhuman.json").to_owned();
                        let text = m.get("text").and_then(Value::as_str).unwrap_or_default().to_owned();
                        spawn(async move {
                            let kind = name.rsplit('.').next().unwrap_or("json").to_owned();
                            let Some(file) = rfd::AsyncFileDialog::new()
                                .add_filter(kind.to_uppercase(), &[kind.as_str()])
                                .set_file_name(&name)
                                .save_file()
                                .await
                            else {
                                return;
                            };
                            let path = file.path().to_path_buf();
                            saved.set(Some(match std::fs::write(&path, text) {
                                Ok(()) => format!("저장됨 · {}", path.display()),
                                Err(e) => format!("저장 실패 · {e}"),
                            }));
                        });
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
