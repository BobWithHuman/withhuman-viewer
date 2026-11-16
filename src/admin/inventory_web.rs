//! 인벤토리's data for the dashboards' page (`web/console/src/pages/inventory.tsx`), as JSON.
//!
//! The page decides what is running (a session not ended and heard from within the window the
//! person picks there), so what goes over is what does not depend on that: each machine joined
//! with its sessions and its latest modes, each person with their keys counted, the sessions with
//! the titles the route map knows, and `now`, the moment the inventory was read.

use std::collections::{BTreeMap, HashMap, HashSet};

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
                "user": a.user_email, "agent": a.agent, "project": a.project,
                "last_seen": a.last_seen,
            })
        })
        .collect();
    page["people"] = people(inv);
    page["keys"] = inv
        .keys
        .iter()
        .map(|k| {
            json!({
                "id": k.key_id, "email": k.user_email, "created_at": k.created_at,
                "created_by": k.created_by, "note": k.note, "last_used_at": k.last_used_at,
                "revoked_at": k.revoked_at, "revoked_by": k.revoked_by,
                "revoke_reason": k.revoke_reason, "first_denied_at": k.first_denied_at,
                "denied_count": k.denied_count,
            })
        })
        .collect();
    page["events"] = inv
        .events
        .iter()
        .map(|e| {
            json!({
                "kind": e.kind, "actor": e.actor, "reason": e.reason, "email": e.user_email,
                "key_id": e.key_id, "at": e.at,
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
                "user": r.user_email,
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
/// its latest one was let run, and whose it is when it did not say at install.
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
                "id": d.client_id, "host": d.hostname, "team": d.team, "os": d.os,
                "os_user": d.os_user, "agent": d.agent, "hook_version": d.hook_version,
                "user": d.user_email.clone().or_else(|| latest.and_then(|r| r.user_email.clone())),
                "registered_at": d.registered_at, "first_seen": d.first_seen, "last": last,
                "sessions": runs.len(),
                "permission_mode": latest.and_then(|r| r.permission_mode.clone()),
                "engine_mode": modes.get(&d.client_id),
                "serial": d.serial, "owner": d.owner_email,
                "stopped": d.stopped_at.map(|at| json!({
                    "at": at, "by": d.stopped_by, "reason": d.stop_reason, "denied": d.stop_denied_count,
                })),
                "edr": d.edr_agent_id.as_ref().map(|id| json!({ "id": id, "status": d.edr_status })),
            })
        })
        .collect()
}

/// Everyone a key was issued to, with their machines, sessions and keys counted; the people
/// stopped (every key revoked) first, then those who used a key most recently.
fn people(inv: &Inventory) -> Value {
    let mut emails: Vec<&str> = inv.users.iter().map(|u| u.email.as_str()).collect();
    for k in &inv.keys {
        if !emails.contains(&k.user_email.as_str()) {
            emails.push(&k.user_email);
        }
    }
    let mut rows: Vec<(bool, Option<i64>, Value)> = emails
        .into_iter()
        .map(|email| {
            let keys: Vec<_> = inv.keys.iter().filter(|k| k.user_email == email).collect();
            let runs: Vec<_> = inv
                .sessions
                .iter()
                .filter(|r| r.user_email.as_deref() == Some(email))
                .collect();
            let mut machines: HashSet<&str> =
                runs.iter().filter_map(|r| r.client_id.as_deref()).collect();
            machines.extend(
                inv.clients
                    .iter()
                    .filter(|c| c.user_email.as_deref() == Some(email))
                    .map(|c| c.client_id.as_str()),
            );
            let mut machines: Vec<&str> = machines.into_iter().collect();
            machines.sort_unstable();
            let active = keys.iter().filter(|k| k.revoked_at.is_none()).count();
            let revoked = keys.len() - active;
            let stopped = active == 0 && revoked > 0;
            let last_used = keys.iter().filter_map(|k| k.last_used_at).max();
            let row = json!({
                "email": email,
                "kind": inv.users.iter().find(|u| u.email == email).and_then(|u| u.kind.clone()),
                "devices": machines, "sessions": runs.len(),
                "active_keys": active, "revoked_keys": revoked, "stopped": stopped,
                "last_used": last_used,
                "denied": keys.iter().filter_map(|k| k.denied_count).sum::<i64>(),
            });
            (stopped, last_used, row)
        })
        .collect();
    rows.sort_by_key(|(stopped, last_used, _)| (!stopped, std::cmp::Reverse(*last_used)));
    rows.into_iter().map(|(_, _, row)| row).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ApiKey, Device};

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
    fn a_machine_takes_its_person_and_modes_from_its_latest_session() {
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
        assert_eq!(rows[0]["user"], "kim");
        assert_eq!(rows[0]["last"], 90, "heard later through a session");
        assert_eq!(rows[0]["sessions"], 2);
        assert_eq!(rows[0]["permission_mode"], "bypassPermissions");
        assert_eq!(rows[0]["engine_mode"], "enforce", "its latest decision's");
        assert_eq!(rows[1]["last"], Value::Null, "registered, never heard");
        assert_eq!(rows[1]["engine_mode"], Value::Null);
    }

    #[test]
    fn a_person_with_every_key_revoked_is_stopped_and_listed_first() {
        let key = |id: &str, user: &str, revoked: bool| ApiKey {
            key_id: id.into(),
            user_email: user.into(),
            revoked_at: revoked.then_some(5),
            denied_count: revoked.then_some(2),
            ..ApiKey::default()
        };
        let inv = Inventory {
            keys: vec![
                key("k1", "kim", false),
                key("k2", "park", true),
                key("k3", "lee", true),
                key("k4", "lee", false),
            ],
            sessions: vec![
                run(Some("a"), Some("kim"), 10),
                run(Some("b"), Some("kim"), 20),
            ],
            ..Inventory::default()
        };
        let rows = people(&inv);
        let rows = rows.as_array().unwrap();
        assert_eq!(rows[0]["email"], "park");
        assert_eq!(rows[0]["stopped"], true);
        let of = |email: &str| rows.iter().find(|r| r["email"] == email).unwrap();
        assert_eq!(of("lee")["stopped"], false, "one key still active");
        assert_eq!(of("lee")["denied"], 2);
        assert_eq!(of("kim")["devices"], json!(["a", "b"]));
    }
}
