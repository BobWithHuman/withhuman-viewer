//! 인벤토리's data for the dashboards' page (`web/console/src/pages/inventory.tsx`), as JSON.
//!
//! The page decides what is running (a session not ended and heard from within the window the
//! security team picks there), so what goes over is what does not depend on that: each machine
//! joined with its sessions and its latest modes, the sessions with the titles the route map
//! knows, and `now`, the moment the inventory was read. People are not shown: a machine is known
//! by its own ID.

use std::collections::{BTreeMap, HashMap};

use serde_json::{Value, json};

use crate::model::{CallFact, Inventory, Session, SessionRun};

/// What the page shows around the inventory: where the data is from, the admin API's connection,
/// who acts by default, and the answer to the last thing the page asked for.
pub(super) struct Around {
    pub now: i64,
    pub source: String,
    pub demo: bool,
    pub engine_api: String,
    pub has_admin_token: bool,
    pub actor: String,
    pub reply: Option<Value>,
    /// The tab the page opens on.
    pub tab: &'static str,
}

/// What each machine's engine does with its decisions (`enforce`, `shadow` or `record`): the mode
/// of the machine's latest decided Call.
pub(super) fn engine_modes(facts: &[CallFact]) -> HashMap<String, String> {
    let mut latest: HashMap<&str, (i64, &str)> = HashMap::new();
    for f in facts {
        if let (Some(client), Some(mode), Some(at)) = (&f.client, &f.mode, f.at)
            && latest.get(client.as_str()).is_none_or(|(t, _)| at >= *t)
        {
            latest.insert(client.as_str(), (at, mode.as_str()));
        }
    }
    latest
        .into_iter()
        .map(|(client, (_, mode))| (client.to_owned(), mode.to_owned()))
        .collect()
}

/// The page's data when there is no inventory to show: still loading, failed to load, or an
/// engine that keeps none (`status` is `loading`, `failed` or `none`).
pub(super) fn without(status: &str, error: Option<&str>, around: &Around) -> Value {
    let mut page = frame(around);
    page["status"] = json!(status);
    page["error"] = json!(error);
    page
}

/// The page's data for an inventory.
pub(super) fn page(
    inv: &Inventory,
    titles: &HashMap<String, Session>,
    modes: &HashMap<String, String>,
    around: &Around,
) -> Value {
    let host: HashMap<&str, &str> = inv
        .clients
        .iter()
        .filter_map(|c| Some((c.client_id.as_str(), c.hostname.as_deref()?)))
        .collect();
    let mut shadow = inv.shadow.clone().unwrap_or_default();
    shadow.sort_by_key(|a| std::cmp::Reverse(a.last_seen));
    let mut page = frame(around);
    page["status"] = json!("ready");
    page["shadow_known"] = json!(inv.shadow.is_some());
    page["devices"] = devices(inv, modes);
    // The machines the EDR lists that no registered machine was matched to.
    page["edr_known"] = json!(inv.edr.is_some());
    page["edr_only"] = inv
        .edr
        .iter()
        .flatten()
        // Agent 000 is the EDR's own manager, not a machine of the company's.
        .filter(|e| e.agent_id != "000")
        .filter(|e| {
            !inv.clients
                .iter()
                .any(|d| d.edr_agent_id.as_deref() == Some(&e.agent_id))
        })
        .map(|e| {
            json!({
                "id": e.agent_id, "name": e.name, "os": e.os, "status": e.status,
                "serial": e.serial, "last": e.last_seen, "agent": e.agent,
            })
        })
        .collect();
    page["projects"] = inv
        .projects
        .iter()
        .map(|p| {
            json!({
                "device": p.client_id, "project": p.project, "agent": p.agent,
                "hook_version": p.hook_version, "installed_at": p.installed_at,
            })
        })
        .collect();
    page["shadow"] = shadow
        .iter()
        .map(|a| {
            json!({
                "device": a.client_id,
                "host": a.client_id.as_deref().and_then(|c| host.get(c)),
                "agent": a.agent, "project": a.project,
                "last_seen": a.last_seen,
            })
        })
        .collect();
    page["sessions"] = inv
        .sessions
        .iter()
        .map(|r| {
            let known = titles.get(&r.session_id);
            json!({
                "id": r.session_id, "device": r.client_id,
                "host": r.client_id.as_deref().and_then(|c| host.get(c)),
                "title": known.map(|s| s.title()).filter(|t| !t.is_empty()),
                "folder": known.map(Session::folder).filter(|f| !f.is_empty()),
                "opens": known.is_some(),
                "source": r.start_source, "permission_mode": r.permission_mode,
                "model": r.model, "created_at": r.created_at, "last_seen_at": r.last_seen_at,
                "ended_at": r.ended_at, "end_reason": r.end_reason,
            })
        })
        .collect();
    page
}

/// How many days of each machine's Calls [`activity`] gathers, today the last.
pub(super) const DAYS: usize = 14;
/// How many of a machine's latest Calls each day go over as they are.
const RECENT: usize = 30;

/// What each machine did over the last [`DAYS`] days, from its Calls, by client ID. Its Calls are
/// counted by day, tool and session together (`cells`: day, tool, session as indexes into
/// `tools` and `sessions`, then the Calls allowed, held for the user and blocked), so the page
/// can narrow any of the three by the others; the latest [`RECENT`] of each day go over as they
/// are. Days
/// are this Mac's calendar days, day 0 the first and `DAYS - 1` today.
pub(super) fn activity(facts: &[CallFact], now: i64) -> Value {
    use chrono::TimeZone;
    let day_of = |ms: i64| {
        chrono::Local
            .timestamp_millis_opt(ms)
            .single()
            .map(|t| t.date_naive())
    };
    let Some(today) = day_of(now) else {
        return json!({});
    };
    #[derive(Default)]
    struct Of<'a> {
        tools: Vec<&'a str>,
        sessions: Vec<&'a str>,
        cells: BTreeMap<(usize, usize, usize), [u32; 3]>,
        calls: Vec<(usize, &'a CallFact)>,
    }
    let mut by: BTreeMap<&str, Of> = BTreeMap::new();
    for f in facts {
        let (Some(client), Some(at)) = (f.client.as_deref(), f.at) else {
            continue;
        };
        let Some(day) = day_of(at) else { continue };
        let ago = (today - day).num_days();
        if !(0..DAYS as i64).contains(&ago) {
            continue;
        }
        let of = by.entry(client).or_default();
        let tool = f.tool.as_deref().unwrap_or("?");
        let t = index(&mut of.tools, tool);
        let s = index(&mut of.sessions, f.session.as_str());
        let verdict = match f.decision.as_deref() {
            Some("DENY") => 2,
            Some("REVIEW") => 1,
            _ => 0,
        };
        let day = DAYS - 1 - ago as usize;
        of.cells.entry((day, t, s)).or_default()[verdict] += 1;
        of.calls.push((day, f));
    }
    by.into_iter()
        .map(|(client, mut of)| {
            of.calls.sort_by_key(|(_, f)| std::cmp::Reverse(f.at));
            let mut kept = [0usize; DAYS];
            let recent: Vec<Value> = of
                .calls
                .iter()
                .filter(|(day, _)| {
                    kept[*day] += 1;
                    kept[*day] <= RECENT
                })
                .map(|(_, f)| {
                    json!({
                        "session": f.session, "seq": f.seq, "at": f.at, "tool": f.tool,
                        "target": f.target.as_deref().map(|t| t.chars().take(200).collect::<String>()),
                        "decision": f.decision, "ran": f.ran,
                    })
                })
                .collect();
            let cells: Vec<Value> = of
                .cells
                .iter()
                .map(|(&(d, t, s), n)| json!([d, t, s, n[0], n[1], n[2]]))
                .collect();
            (
                client.to_owned(),
                json!({ "tools": of.tools, "sessions": of.sessions, "cells": cells, "recent": recent }),
            )
        })
        .collect::<serde_json::Map<_, _>>()
        .into()
}

/// Where `x` is in `list`, put at its end when it is not there yet.
fn index<'a>(list: &mut Vec<&'a str>, x: &'a str) -> usize {
    list.iter().position(|y| *y == x).unwrap_or_else(|| {
        list.push(x);
        list.len() - 1
    })
}

/// What every state of the page carries.
fn frame(around: &Around) -> Value {
    json!({
        "now": around.now,
        "source": { "label": around.source, "demo": around.demo },
        "admin": { "api": around.engine_api, "has_token": around.has_admin_token },
        "actor": around.actor,
        "reply": around.reply,
        "tab": around.tab,
        "mcp": null,
    })
}

/// Each machine with what its sessions add: how many, when it was last heard through them, how
/// its latest one was let run.
fn devices(inv: &Inventory, modes: &HashMap<String, String>) -> Value {
    let mut by: BTreeMap<&str, Vec<&SessionRun>> = BTreeMap::new();
    for r in &inv.sessions {
        if let Some(c) = r.client_id.as_deref() {
            by.entry(c).or_default().push(r);
        }
    }
    inv.clients
        .iter()
        .map(|d| {
            let runs = by.get(d.client_id.as_str()).cloned().unwrap_or_default();
            let latest = runs.iter().max_by_key(|r| r.last_seen_at.or(r.created_at));
            let last = d
                .last_seen
                .max(runs.iter().filter_map(|r| r.last_seen_at).max());
            json!({
                "id": d.client_id, "host": d.hostname, "os": d.os,
                "agent": d.agent, "hook_version": d.hook_version,
                "registered_at": d.registered_at, "first_seen": d.first_seen, "last": last,
                "sessions": runs.len(),
                "permission_mode": latest.and_then(|r| r.permission_mode.clone()),
                "engine_mode": modes.get(&d.client_id),
                "serial": d.serial,
                "stopped": d.stopped_at.map(|at| json!({
                    "at": at, "by": d.stopped_by, "reason": d.stop_reason, "denied": d.stop_denied_count,
                })),
                "edr": d.edr_agent_id.as_ref().map(|id| json!({ "id": id, "status": d.edr_status })),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Device;

    fn run(client: Option<&str>, user: Option<&str>, last: i64) -> SessionRun {
        SessionRun {
            session_id: format!("{client:?}-{last}"),
            client_id: client.map(str::to_owned),
            user_email: user.map(str::to_owned),
            created_at: Some(last - 10),
            last_seen_at: Some(last),
            ..SessionRun::default()
        }
    }

    #[test]
    fn a_machine_takes_its_modes_from_its_latest_session() {
        let mut bypass = run(Some("a"), Some("kim"), 90);
        bypass.permission_mode = Some("bypassPermissions".into());
        let inv = Inventory {
            clients: vec![
                Device {
                    client_id: "a".into(),
                    last_seen: Some(50),
                    ..Device::default()
                },
                Device {
                    client_id: "b".into(),
                    ..Device::default()
                },
            ],
            sessions: vec![run(Some("a"), Some("lee"), 40), bypass, run(None, None, 95)],
            ..Inventory::default()
        };
        let facts = [
            CallFact {
                client: Some("a".into()),
                at: Some(10),
                mode: Some("shadow".into()),
                ..CallFact::default()
            },
            CallFact {
                client: Some("a".into()),
                at: Some(20),
                mode: Some("enforce".into()),
                ..CallFact::default()
            },
        ];
        let rows = devices(&inv, &engine_modes(&facts));
        assert_eq!(rows[0]["last"], 90, "heard later through a session");
        assert_eq!(rows[0]["sessions"], 2);
        assert_eq!(rows[0]["permission_mode"], "bypassPermissions");
        assert_eq!(rows[0]["engine_mode"], "enforce", "its latest decision's");
        assert_eq!(rows[1]["last"], Value::Null, "registered, never heard");
        assert_eq!(rows[1]["engine_mode"], Value::Null);
    }

    #[test]
    fn a_machine_s_calls_are_counted_by_day_tool_and_session() {
        let now = 1_763_280_000_000; // 2025-11-16, midday or so wherever the test runs
        let call = |session: &str, tool: &str, at: i64, decision: Option<&str>| CallFact {
            session: session.into(),
            client: Some("a".into()),
            tool: Some(tool.into()),
            at: Some(at),
            decision: decision.map(str::to_owned),
            ..CallFact::default()
        };
        let facts = vec![
            call("s1", "Bash", now, Some("ALLOW")),
            call("s1", "Bash", now, Some("DENY")),
            call("s1", "Read", now, Some("REVIEW")),
            call("s2", "Bash", now - 3 * 86_400_000, None),
            call("s3", "Bash", now - 30 * 86_400_000, Some("ALLOW")),
            CallFact {
                client: None,
                ..call("s4", "Bash", now, None)
            },
        ];
        let all = activity(&facts, now);
        let a = &all["a"];
        assert_eq!(a["tools"], json!(["Bash", "Read"]));
        assert_eq!(
            a["sessions"],
            json!(["s1", "s2"]),
            "a month ago is past the window"
        );
        let today = (DAYS - 1) as u64;
        let cells = a["cells"].as_array().unwrap();
        assert!(cells.contains(&json!([today, 0, 0, 1, 0, 1])), "{cells:?}");
        assert!(cells.contains(&json!([today, 1, 0, 0, 1, 0])));
        assert!(
            cells.contains(&json!([today - 3, 0, 1, 1, 0, 0])),
            "no decision counts as allowed"
        );
        assert_eq!(a["recent"].as_array().unwrap().len(), 4);
        assert_eq!(
            all.as_object().unwrap().len(),
            1,
            "a Call with no machine is no machine's"
        );
    }
}
