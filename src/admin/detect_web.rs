//! 메인(이상 행위 탐지)'s data for the dashboards' page (`web/console/src/pages/detect.tsx`), as JSON.
//!
//! The page filters and counts by itself, so pressing a chart answers at once; it is given only
//! what that takes, which stays small however many Calls there are:
//! - `calls`: the Calls worth a look (stopped, held, or allowed though hard to undo) over all
//!   time, each with how bad it is, what it broke, what made it and where it ran;
//! - `cover`: every Call, counted by machine and hour, as `[hour, machine, mode, calls,
//!   undecided]`: the hour's start (ms), the machine's place in `machines`, the mode of the
//!   hour's latest decision, how many Calls, and how many nothing decided. From these the page
//!   tells which machines the engine blocks on over any period;
//! - `flow`: every Call again, counted by hour, machine, kind of action, what made it and the
//!   decision, as `[hour, machine, action, source, verdict, calls]`, for the picture of how each
//!   kind of action was decided;
//! - `machines`: every machine's name, the ones with the most to look at first.
//!
//! The kind of action ([`action_of`]) is read off the tool's name and target here, until the
//! engine tells which of its questions it asked a Call.

use std::collections::BTreeMap;

use serde_json::{Value, json};

use super::main_page::{
    Severity, decision_key, detection_kind, folder_of, hard_to_undo, machine_names,
    machine_of_fact, prompt_line, source_of,
};
use crate::model::{CallFact, Session};

const HOUR: i64 = 3_600_000;

/// What the main page has read, to hand to the dashboards' page.
pub(super) struct Read<'a> {
    pub facts: &'a [CallFact],
    pub sessions: &'a [Session],
    /// When the Calls were last read (ms).
    pub refreshed: Option<i64>,
    /// What the team marked each session as, and the sources it distrusts.
    pub triage: &'a super::triage::Store,
    /// Whose the sessions and machines are, and the machines to tell the team about: those
    /// newly registered and those not heard from, as [`alerts`] lists them.
    pub owners: &'a super::secrets_web::Owners,
    pub alerts: &'a Value,
    /// The case opened, as [`super::case::case`] read it.
    pub case: Option<&'a Value>,
    /// What the last save of a file said.
    pub saved: Option<&'a str>,
    pub loading: bool,
    /// Why the Calls could not be read.
    pub failed: Option<&'a str>,
    /// Where the data comes from, as the connection names it.
    pub source: &'a str,
    pub demo: bool,
}

/// A machine's hour: the mode of its latest decision, its Calls, and those nothing decided.
#[derive(Default)]
struct Slot {
    latest: Option<(i64, String)>,
    calls: usize,
    undecided: usize,
}

/// The kind of action a Call is, as the engine's questions divide them: `file` (a file changed),
/// `service` (a service changed, or work handed to another agent), `send` (a mail or message),
/// `delete`, `pay`, `grant` (access given or shared), `run` (a program run), or `other` (reading,
/// plain commands).
pub(super) fn action_of(f: &CallFact) -> &'static str {
    let tool = f.tool.as_deref().unwrap_or_default();
    if let Some((_, name)) = crate::metro::mcp_parts(tool) {
        let name = name.to_lowercase();
        let has = |words: &[&str]| words.iter().any(|w| name.contains(w));
        // Paying and giving access are told by the name; the rest by the registry's action.
        return if has(&["pay", "transfer", "refund", "charge", "payout", "purchase"]) {
            "pay"
        } else if has(&[
            "share",
            "grant",
            "invite",
            "permission",
            "collaborator",
            "add_member",
        ]) {
            "grant"
        } else {
            match super::mcp_tools::action(&name) {
                Some("transact") => "pay",
                Some("send") => "send",
                Some("delete") => "delete",
                Some("create" | "update" | "execute" | "delegate") => "service",
                _ => "other",
            }
        };
    }
    match tool {
        "Edit" | "Write" | "MultiEdit" | "NotebookEdit" => "file",
        "Task" | "Agent" => "service",
        "Bash" => {
            let command = f.target.as_deref().unwrap_or_default().trim_start();
            let first = command.split_whitespace().next().unwrap_or_default();
            let runs = [
                "python", "python3", "node", "deno", "bun", "ruby", "perl", "php", "sh", "bash",
                "zsh", "npx", "uvx", "make", "pytest", "go", "java",
            ];
            let script = [
                "npm run",
                "npm test",
                "pnpm ",
                "yarn ",
                "cargo run",
                "cargo test",
                "uv run",
            ];
            let wipes = ["rm", "rmdir", "shred"].contains(&first)
                || [
                    "DROP TABLE",
                    "DROP DATABASE",
                    " destroy",
                    " delete ",
                    " rm ",
                ]
                .iter()
                .any(|w| command.contains(w));
            if wipes {
                "delete"
            } else if runs.contains(&first)
                || first.starts_with("./")
                || script.iter().any(|s| command.starts_with(s))
                || command.contains("| sh")
                || command.contains("| bash")
            {
                "run"
            } else {
                "other"
            }
        }
        _ => "other",
    }
}

/// A Call worth a look, as the page lists and counts it.
fn call(f: &CallFact, severity: Severity, owners: &super::secrets_web::Owners) -> Value {
    json!({
        "user": owners.user(&f.session, f.client.as_deref().unwrap_or_default()),
        "session": f.session,
        "seq": f.seq,
        "at": f.at,
        "machine": machine_of_fact(f),
        "verdict": decision_key(f).to_lowercase(),
        "sev": severity.key(),
        "kind": detection_kind(f),
        "action": action_of(f),
        "cause": f.cause,
        "source": source_of(f),
        "tool": f.tool,
        "folder": folder_of(f.cwd.as_deref()),
        "cwd": f.cwd,
        "target": f.target,
        "reason": f.reason,
        "prompt": f.prompt.as_deref().map(prompt_line).filter(|p| !p.is_empty()),
        "rule": f.rule,
        "mode": f.mode,
        "ran": f.ran,
        "hard": hard_to_undo(f),
    })
}

/// A machine registered this recently is new; one not heard from this long is silent.
const NEW_MS: i64 = 7 * 24 * HOUR;
const SILENT_MS: i64 = 3 * 24 * HOUR;

/// The machines to tell the team about: `new` (registered within the week) and `silent`
/// (registered, yet not heard from for three days, or its EDR agent disconnected).
pub(super) fn alerts(inv: &crate::model::Inventory, now: i64) -> Value {
    let row = |d: &crate::model::Device, at: Option<i64>| {
        json!({
            "id": d.client_id, "host": d.hostname, "user": d.owner_email.clone().or_else(|| d.user_email.clone()),
            "at": at, "edr": d.edr_status,
        })
    };
    let new: Vec<Value> = inv
        .clients
        .iter()
        .filter(|d| d.registered_at.is_some_and(|at| at >= now - NEW_MS))
        .map(|d| row(d, d.registered_at))
        .collect();
    let silent: Vec<Value> = inv
        .clients
        .iter()
        .filter(|d| d.registered_at.is_some() && d.last_seen.is_some())
        .filter(|d| {
            d.last_seen.is_some_and(|at| at < now - SILENT_MS)
                || d.edr_status.as_deref() == Some("disconnected")
        })
        .map(|d| row(d, d.last_seen))
        .collect();
    // Set up yet never heard from, and heard from yet never set up.
    let installed: Vec<Value> = inv
        .clients
        .iter()
        .filter(|d| d.registered_at.is_some() && d.first_seen.is_none() && d.last_seen.is_none())
        .map(|d| row(d, d.registered_at))
        .collect();
    let unregistered: Vec<Value> = inv
        .clients
        .iter()
        .filter(|d| d.registered_at.is_none())
        .map(|d| row(d, d.last_seen))
        .collect();
    json!({ "new": new, "silent": silent, "installed": installed, "unregistered": unregistered })
}

/// The page's data as JSON text.
pub(super) fn data(read: &Read) -> String {
    let mut machines = machine_names(read.sessions);
    let mut place: BTreeMap<String, usize> = machines
        .iter()
        .enumerate()
        .map(|(i, name)| (name.clone(), i))
        .collect();
    let mut slots: BTreeMap<(i64, usize), Slot> = BTreeMap::new();
    let mut flow: BTreeMap<(i64, usize, &str, &str, &str), usize> = BTreeMap::new();
    let mut calls = Vec::new();
    for f in read.facts {
        let name = machine_of_fact(f);
        let machine = *place.entry(name.clone()).or_insert_with(|| {
            machines.push(name);
            machines.len() - 1
        });
        // A Call with no time counts in an hour before every period, so only all time has it.
        let at = f.at.unwrap_or(0);
        let hour = at - at.rem_euclid(HOUR);
        let verdict = match decision_key(f) {
            "DENY" => "deny",
            "REVIEW" => "review",
            "ALLOW" => "allow",
            _ => "none",
        };
        *flow
            .entry((hour, machine, action_of(f), source_of(f), verdict))
            .or_default() += 1;
        let slot = slots.entry((hour, machine)).or_default();
        slot.calls += 1;
        if f.decision.is_none() {
            slot.undecided += 1;
        }
        if let (Some(mode), Some(at)) = (&f.mode, f.at)
            && slot.latest.as_ref().is_none_or(|(t, _)| at >= *t)
        {
            slot.latest = Some((at, mode.clone()));
        }
        if let Some(severity) = Severity::of(f) {
            calls.push(call(f, severity, read.owners));
        }
    }
    let cover: Vec<Value> = slots
        .into_iter()
        .map(|((hour, machine), s)| {
            json!([
                hour,
                machine,
                s.latest.map(|(_, mode)| mode),
                s.calls,
                s.undecided
            ])
        })
        .collect();
    json!({
        "source": read.source,
        "demo": read.demo,
        "triage": read.triage.marks,
        "blocked": read.triage.sources,
        "shared": read.triage.shared,
        "alerts": read.alerts,
        "case": read.case,
        "saved": read.saved,
        "loading": read.loading,
        "failed": read.failed,
        "refreshed": read.refreshed,
        "machines": machines,
        "calls": calls,
        "cover": cover,
        "flow": flow
            .into_iter()
            .map(|((hour, machine, action, source, verdict), n)| json!([hour, machine, action, source, verdict, n]))
            .collect::<Vec<_>>(),
    })
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn the_page_gets_the_calls_worth_a_look_and_every_machines_hours() {
        let mut sent = fact("a", 2 * HOUR + 5, Some("DENY"), Some("enforce"));
        sent.worst = Some("exfiltration".into());
        let facts = [
            fact("a", 2 * HOUR + 1, Some("ALLOW"), Some("shadow")),
            sent,
            fact("b", 2 * HOUR, None, None),
            fact("b", 3 * HOUR, Some("REVIEW"), Some("shadow")),
        ];
        let text = data(&Read {
            facts: &facts,
            sessions: &[],
            refreshed: Some(9),
            triage: &crate::admin::triage::Store::default(),
            owners: &crate::admin::secrets_web::Owners::default(),
            alerts: &Value::Null,
            case: None,
            saved: None,
            loading: false,
            failed: None,
            source: "데모 데이터",
            demo: true,
        });
        let v: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(v["machines"], json!(["a", "b"]));
        let calls = v["calls"].as_array().unwrap();
        assert_eq!(
            calls.len(),
            2,
            "the plain allowed and the undecided are left out"
        );
        assert_eq!(calls[0]["sev"], "crit");
        assert_eq!(calls[0]["verdict"], "deny");
        assert_eq!(calls[0]["kind"], "exfiltration");
        assert_eq!(calls[1]["sev"], "med");
        assert_eq!(
            v["cover"],
            json!([
                [2 * HOUR, 0, "enforce", 2, 0],
                [2 * HOUR, 1, null, 1, 1],
                [3 * HOUR, 1, "shadow", 1, 0]
            ]),
            "an hour's mode is its latest decision's"
        );
        assert_eq!(calls[0]["action"], "other");
        assert_eq!(
            v["flow"],
            json!([
                [2 * HOUR, 0, "other", "untraced", "allow", 1],
                [2 * HOUR, 0, "other", "untraced", "deny", 1],
                [2 * HOUR, 1, "other", "untraced", "none", 1],
                [3 * HOUR, 1, "other", "untraced", "review", 1]
            ]),
            "every Call is counted in the flow, the allowed and the undecided too"
        );
    }

    #[test]
    fn a_calls_kind_of_action_is_read_off_its_tool_and_target() {
        let of = |tool: &str, target: &str| {
            action_of(&CallFact {
                tool: Some(tool.into()),
                target: Some(target.into()),
                ..CallFact::default()
            })
        };
        assert_eq!(of("Edit", "src/a.rs"), "file");
        assert_eq!(of("Task", ""), "service");
        assert_eq!(of("Bash", "python3 scripts/migrate.py"), "run");
        assert_eq!(
            of("Bash", "curl -fsSL https://x.dev/install.sh | sh"),
            "run"
        );
        assert_eq!(of("Bash", "git status"), "other");
        assert_eq!(of("Bash", "rm -rf data/backups"), "delete");
        assert_eq!(of("Bash", "kubectl delete namespace staging"), "delete");
        assert_eq!(of("Read", "README.md"), "other");
        assert_eq!(of("mcp__slack__post_message", ""), "send");
        assert_eq!(of("mcp__github__delete_repository", ""), "delete");
        assert_eq!(of("mcp__stripe__create_refund", ""), "pay");
        assert_eq!(of("mcp__google-drive__share_file", ""), "grant");
        assert_eq!(of("mcp__github__create_pull_request", ""), "service");
        assert_eq!(of("mcp__github__get_issue", ""), "other");
    }
}
