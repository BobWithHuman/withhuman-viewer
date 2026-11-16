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

/// When each machine's latest Call was seen, by client ID: a machine that called is heard from,
/// whatever its sessions say.
pub(super) fn last_calls(facts: &[CallFact]) -> HashMap<String, i64> {
    let mut latest: HashMap<String, i64> = HashMap::new();
    for f in facts {
        if let (Some(client), Some(at)) = (&f.client, f.at) {
            let t = latest.entry(client.clone()).or_insert(at);
            *t = (*t).max(at);
        }
    }
    latest
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
    called: &HashMap<String, i64>,
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
    // Shadow AI is known from a check of the projects, or from the EDR's process scans.
    page["shadow_known"] = json!(inv.shadow.is_some() || inv.edr.is_some());
    page["devices"] = devices(inv, modes, called);
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
    let mut found: Vec<Value> = shadow
        .iter()
        .map(|a| {
            json!({
                "device": a.client_id,
                "host": a.client_id.as_deref().and_then(|c| host.get(c)),
                "agent": a.agent, "project": a.project,
                "last_seen": a.last_seen, "why": "project",
            })
        })
        .collect();
    found.extend(edr_shadow(inv, &host));
    found.sort_by_key(|a| std::cmp::Reverse(a["last_seen"].as_i64()));
    page["shadow"] = json!(found);
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

/// Each session's Calls: how many, how many held for the user and how many blocked, and the
/// engine's mode (`enforce`, `shadow` or `record`) at its latest Call that had one, by session.
pub(super) fn session_calls(facts: &[CallFact]) -> Value {
    #[derive(Default)]
    struct Of<'a> {
        n: [u32; 3],
        mode: Option<(i64, &'a str)>,
    }
    let mut by: BTreeMap<&str, Of> = BTreeMap::new();
    for f in facts {
        let of = by.entry(f.session.as_str()).or_default();
        of.n[0] += 1;
        match f.decision.as_deref() {
            Some("REVIEW") => of.n[1] += 1,
            Some("DENY") => of.n[2] += 1,
            _ => {}
        }
        if let Some(mode) = f.mode.as_deref() {
            let at = f.at.unwrap_or(i64::MIN);
            if of.mode.is_none_or(|(latest, _)| at >= latest) {
                of.mode = Some((at, mode));
            }
        }
    }
    by.into_iter()
        .map(|(session, of)| {
            let [n, review, deny] = of.n;
            let mode = of.mode.map(|(_, m)| m);
            (
                session.to_owned(),
                json!({ "n": n, "review": review, "deny": deny, "mode": mode }),
            )
        })
        .collect::<serde_json::Map<_, _>>()
        .into()
}

/// How far apart a scan and a session's start or end may be and still meet: the clocks of the
/// EDR and the Hooks differ a little.
const SLACK_MS: i64 = 2 * 60_000;
/// A session that never said it ended is taken as open this long after its last signal.
const OPEN_FOR_MS: i64 = 24 * 60 * 60_000;

/// Two sightings further apart than this are two runs, not one: a scan or two can miss a run
/// that goes on (the EDR scans hourly by default).
const RUN_GAP_MS: i64 = 3 * 60 * 60_000;

/// A run of sightings: the first and last scan's time, how many scans, the agent, the machine's
/// name then, and the first command line seen in it.
type Run<'a> = (i64, i64, u32, &'a str, Option<&'a str>, Option<&'a str>);
/// A sighting: the scan's time, the agent, the machine's name then, and the command line.
type Seen<'a> = (i64, &'a str, Option<&'a str>, Option<&'a str>);

/// Shadow AI the EDR saw, as runs: the scans that saw a coding agent running on a machine with no
/// Hooks session open then, one after another, are one run from the first to the last
/// (`why: "no_session"`: the Hooks are off, removed, or not in that folder; `"unregistered"` on a
/// machine that never registered with the engine). The scans are the engine's record of them
/// (`edr_sightings`), else each machine's latest (`edr_devices.running_at`).
fn edr_shadow(inv: &Inventory, host: &HashMap<&str, &str>) -> Vec<Value> {
    let named: HashMap<&str, &str> = inv
        .edr
        .iter()
        .flatten()
        .map(|e| (e.agent_id.as_str(), e.name.as_str()))
        .collect();
    let mut by: BTreeMap<&str, Vec<Seen>> = BTreeMap::new();
    match &inv.sightings {
        Some(list) => {
            for x in list {
                by.entry(&x.agent_id).or_default().push((
                    x.running_at,
                    &x.agent,
                    x.name.as_deref(),
                    x.command.as_deref().filter(|c| !c.trim().is_empty()),
                ));
            }
        }
        None => {
            for e in inv.edr.iter().flatten() {
                if let (Some(at), Some(agent)) = (e.running_at, e.agent.as_deref()) {
                    by.entry(&e.agent_id)
                        .or_default()
                        .push((at, agent, None, None));
                }
            }
        }
    }
    let mut found = Vec::new();
    for (edr, mut seen) in by {
        seen.sort_by_key(|x| x.0);
        let clients: Vec<&str> = inv
            .clients
            .iter()
            .filter(|c| c.edr_agent_id.as_deref() == Some(edr))
            .map(|c| c.client_id.as_str())
            .collect();
        let open = |at: i64| {
            inv.sessions.iter().any(|r| {
                r.client_id.as_deref().is_some_and(|c| clients.contains(&c))
                    && r.created_at.is_some_and(|start| start <= at + SLACK_MS)
                    && match r.ended_at {
                        Some(end) => end + SLACK_MS >= at,
                        None => r
                            .last_seen_at
                            .or(r.created_at)
                            .is_some_and(|last| last + OPEN_FOR_MS >= at),
                    }
            })
        };
        let device = clients.first().copied();
        let name = |fallback: Option<&str>| {
            device
                .and_then(|c| host.get(c).copied())
                .or_else(|| named.get(edr).copied())
                .or(fallback)
                .unwrap_or(edr)
                .to_owned()
        };
        let mut run: Option<Run> = None;
        let mut runs = Vec::new();
        for (at, agent, then, command) in seen {
            if open(at) {
                runs.extend(run.take());
                continue;
            }
            run = match run {
                Some((first, last, n, a, t, c)) if a == agent && at - last <= RUN_GAP_MS => {
                    Some((first, at, n + 1, a, t, c.or(command)))
                }
                other => {
                    runs.extend(other);
                    Some((at, at, 1, agent, then, command))
                }
            };
        }
        runs.extend(run);
        for (first, last, scans, agent, then, command) in runs {
            found.push(json!({
                "device": device, "host": name(then), "agent": agent, "project": null,
                "command": command,
                "first": first, "last_seen": last, "scans": scans,
                "why": if device.is_some() { "no_session" } else { "unregistered" },
                "edr": edr,
            }));
        }
    }
    found
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
fn devices(
    inv: &Inventory,
    modes: &HashMap<String, String>,
    called: &HashMap<String, i64>,
) -> Value {
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
                .max(runs.iter().filter_map(|r| r.last_seen_at).max())
                .max(called.get(&d.client_id).copied());
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
        let rows = devices(&inv, &engine_modes(&facts), &last_calls(&facts));
        assert_eq!(rows[0]["last"], 90, "heard later through a session");
        assert_eq!(rows[0]["sessions"], 2);
        assert_eq!(rows[0]["permission_mode"], "bypassPermissions");
        assert_eq!(rows[0]["engine_mode"], "enforce", "its latest decision's");
        assert_eq!(rows[1]["last"], Value::Null, "registered, never heard");
        assert_eq!(rows[1]["engine_mode"], Value::Null);
    }

    #[test]
    fn a_session_s_calls_are_counted_and_its_latest_engine_mode_kept() {
        let call = |session: &str, at: i64, decision: Option<&str>, mode: Option<&str>| CallFact {
            session: session.into(),
            at: Some(at),
            decision: decision.map(str::to_owned),
            mode: mode.map(str::to_owned),
            ..CallFact::default()
        };
        let facts = [
            call("s1", 1, Some("ALLOW"), Some("shadow")),
            call("s1", 3, Some("DENY"), Some("enforce")),
            call("s1", 2, Some("REVIEW"), Some("shadow")),
            call("s1", 4, None, None),
            call("s2", 1, Some("ALLOW"), None),
        ];
        let calls = session_calls(&facts);
        assert_eq!(
            calls["s1"],
            json!({"n": 4, "review": 1, "deny": 1, "mode": "enforce"})
        );
        assert_eq!(
            calls["s2"],
            json!({"n": 1, "review": 0, "deny": 0, "mode": null})
        );
    }

    #[test]
    fn an_agent_the_edr_saw_running_with_no_session_open_is_shadow_ai() {
        use crate::model::EdrDevice;
        let at = 1_000_000_000;
        let edr = |id: &str, name: &str, running_at: Option<i64>| EdrDevice {
            agent_id: id.into(),
            name: name.into(),
            agent: Some("claude-code".into()),
            running_at,
            ..EdrDevice::default()
        };
        let client = |id: &str, agent: &str| Device {
            client_id: id.into(),
            hostname: Some(format!("{id}-host")),
            edr_agent_id: Some(agent.into()),
            ..Device::default()
        };
        let session = |client: &str, start: i64, end: Option<i64>| SessionRun {
            session_id: format!("{client}-{start}"),
            client_id: Some(client.into()),
            created_at: Some(start),
            last_seen_at: Some(start),
            ended_at: end,
            ..SessionRun::default()
        };
        let inv = Inventory {
            clients: vec![client("a", "001"), client("b", "002"), client("c", "003")],
            sessions: vec![
                // a: a session open at the scan.
                session("a", at - 3_600_000, None),
                // b: its session ended an hour before the scan.
                session("b", at - 7_200_000, Some(at - 3_600_000)),
                // c: its session started after the scan.
                session("c", at + 3_600_000, None),
            ],
            edr: Some(vec![
                edr("001", "a-host", Some(at)),
                edr("002", "b-host", Some(at)),
                edr("003", "c-host", Some(at)),
                edr("004", "nobody", Some(at)),
                // Installed but not running at the scan: nothing to compare.
                edr("005", "idle", None),
            ]),
            ..Inventory::default()
        };
        let host: HashMap<&str, &str> = [("b", "b-host"), ("c", "c-host")].into();
        let found = edr_shadow(&inv, &host);
        let of = |edr: &str| found.iter().find(|f| f["edr"] == edr);
        assert!(of("001").is_none(), "a session was open");
        assert_eq!(of("002").unwrap()["why"], "no_session");
        assert_eq!(of("002").unwrap()["device"], "b");
        assert_eq!(of("003").unwrap()["why"], "no_session");
        assert_eq!(of("004").unwrap()["why"], "unregistered");
        assert_eq!(of("004").unwrap()["host"], "nobody");
        assert!(of("005").is_none());
        assert_eq!(found.len(), 3);
    }

    #[test]
    fn the_scans_one_after_another_with_no_session_are_one_run() {
        use crate::model::EdrSighting;
        const H: i64 = 60 * 60_000;
        let t = 100 * H;
        let seen = |at: i64| EdrSighting {
            agent_id: "001".into(),
            running_at: at,
            agent: "claude-code".into(),
            name: Some("old-name".into()),
            command: (at == t + H).then(|| "claude -p fix it".into()),
        };
        let inv = Inventory {
            clients: vec![Device {
                client_id: "a".into(),
                edr_agent_id: Some("001".into()),
                ..Device::default()
            }],
            // A session open from t+4h to t+5h covers the scan at t+4h30.
            sessions: vec![SessionRun {
                session_id: "s".into(),
                client_id: Some("a".into()),
                created_at: Some(t + 4 * H),
                last_seen_at: Some(t + 5 * H),
                ended_at: Some(t + 5 * H),
                ..SessionRun::default()
            }],
            sightings: Some(vec![
                seen(t),
                seen(t + H),
                seen(t + 2 * H),
                seen(t + 4 * H + H / 2),
                seen(t + 6 * H),
                // Ten hours on: another run.
                seen(t + 16 * H),
            ]),
            ..Inventory::default()
        };
        let host: HashMap<&str, &str> = [("a", "a-host")].into();
        let runs = edr_shadow(&inv, &host);
        let spans: Vec<(i64, i64, i64)> = runs
            .iter()
            .map(|r| {
                (
                    r["first"].as_i64().unwrap(),
                    r["last_seen"].as_i64().unwrap(),
                    r["scans"].as_i64().unwrap(),
                )
            })
            .collect();
        assert_eq!(
            spans,
            [
                (t, t + 2 * H, 3),
                (t + 6 * H, t + 6 * H, 1),
                (t + 16 * H, t + 16 * H, 1)
            ],
            "the open session splits the first run from the second"
        );
        assert!(
            runs.iter()
                .all(|r| r["host"] == "a-host" && r["why"] == "no_session")
        );
        assert_eq!(
            runs[0]["command"], "claude -p fix it",
            "the first command line in the run"
        );
        assert_eq!(runs[1]["command"], Value::Null);
    }
}
