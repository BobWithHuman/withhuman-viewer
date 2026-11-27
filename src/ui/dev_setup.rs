//! The personal app's 훅 page: the agents' hook setups found on this machine, each to take out,
//! and new ones, every found agent at once or one at a time. The first launch is `onboard`.

use std::path::PathBuf;
use std::sync::Arc;

use dioxus::prelude::*;

use super::AppState;
use crate::api::Backend;
use crate::hooks::{self, Scope};
use serde_json::{Value, json};

/// Whether hooks for `server` report to the app's own engine on this machine.
pub fn own_engine(server: &str) -> bool {
    server.trim().trim_end_matches('/') == crate::own::collector_url()
}

/// The hooks' own page, drawn by the console page (`web/console`, `pages/hooks.tsx`) in its frame: the
/// setups found on this machine (the device's, the user's, the folders the app installed in),
/// each with the engine it reports to and a button to take it out, and a new setup. What is
/// pressed there comes back here to be done.
#[component]
pub fn HookPage() -> Element {
    let backend = use_context::<Arc<Backend>>();
    let mut state = use_context::<AppState>();
    let mut looked = use_signal(|| 0u32);
    let mut folder = use_signal(|| None::<PathBuf>);
    let mut busy = use_signal(|| None::<String>);
    let mut note = use_signal(|| None::<(bool, String)>);
    // Which panel the note is of: `agents`, or the new setup's.
    let mut note_at = use_signal(|| "setup");
    let found = use_memo(move || {
        let _ = looked();
        hooks::installed(&state.prefs.read().hook_folders)
    });
    let machine = use_hook(hooks::this_machine);
    let agents = use_hook(|| hooks::Agent::ALL.to_vec());
    // The engine checked: the one the hooks use, or one typed to try (`check`).
    let mut try_server = use_signal(|| None::<String>);
    let health = use_resource(move || {
        let _ = looked();
        let server = try_server().unwrap_or_else(|| state.public.read().engine_api.clone());
        async move {
            let result = hooks::check_engine(&server).await;
            json!({
                "server": server,
                "ok": result.is_ok(),
                "ms": result.as_ref().ok().map(|ms| *ms as u64),
                "error": result.err(),
            })
        }
    });
    // What the engine keeps of this machine: when it registered and was last heard from, its
    // serial, its EDR agent, and whether the security team blocked it.
    let record = use_resource({
        let backend = backend.clone();
        move || {
            let b = backend.clone();
            let _ = looked();
            async move {
                let id = hooks::device_id()?;
                let inventory = b.inventory().await.ok().flatten()?;
                let d = inventory.clients.into_iter().find(|c| c.client_id == id)?;
                Some(json!({
                    "registered_at": d.registered_at,
                    "last_seen": d.last_seen,
                    "serial": d.serial.is_some(),
                    "edr": d.edr_status,
                    "stopped": d.stopped_at.map(|at| json!({"at": at, "by": d.stopped_by, "reason": d.stop_reason})),
                }))
            }
        }
    });
    let data = use_memo(move || {
        let _ = looked();
        let installs: Vec<Value> = found()
            .iter()
            .map(|i| {
                let (at, place) = match &i.at {
                    hooks::Where::Device => ("device", i.file.display().to_string()),
                    hooks::Where::Everywhere => ("all", i.file.display().to_string()),
                    hooks::Where::Folder(f) => ("folder", f.display().to_string()),
                    hooks::Where::Profile => ("profile", i.file.display().to_string()),
                };
                json!({"agent": i.agent.kind(), "agent_name": i.agent.name(), "at": at, "place": place, "file": i.file.display().to_string(), "engine": i.engine, "installed_at": i.at_ms, "removable": true})
            })
            .collect();
        json!({
            "machine": {"name": machine.0, "os": machine.1},
            "health": health.read().clone(),
            "record": record.read().clone().flatten(),
            "device": hooks::device_id(),
            "engine": state.public.read().engine_api,
            // The app's own engine: no company EDR or blocking, but how Calls are judged.
            "own": own_engine(&state.public.read().engine_api),
            "jev": crate::own::jev_in_use().is_some(),
            "installs": installs,
            "agents": agents.iter().map(|a| json!({
                "kind": a.kind(), "name": a.name(), "found": a.found(),
                "per_folder": a.per_folder(), "per_profile": a.per_profile(),
            })).collect::<Vec<_>>(),
            "folder": folder().map(|f| f.display().to_string()),
            "busy": busy(),
            "note": note().map(|(ok, text)| json!({"ok": ok, "text": text, "at": note_at()})),
        })
        .to_string()
    });
    use_effect(move || {
        let d = data();
        document::eval(&format!(
            "requestAnimationFrame(() => window.__whConsoleSet && window.__whConsoleSet('hooks', {d}))"
        ));
    });
    use_future(move || {
        let backend = backend.clone();
        async move {
            let mut eval = document::eval(
                "window.__whConsoleOn['hooks'] = (m) => dioxus.send(m); await new Promise(() => {});",
            );
            while let Ok(m) = eval.recv::<Value>().await {
                match m["do"].as_str() {
                    Some("check") => {
                        try_server.set(m["server"].as_str().map(str::to_owned));
                    }
                    Some("pick") => {
                        spawn(async move {
                            if let Some(f) = rfd::AsyncFileDialog::new().pick_folder().await {
                                folder.set(Some(f.path().to_path_buf()));
                            }
                        });
                    }
                    Some("install") => {
                        note_at.set("setup");
                        let scope = match m["scope"].as_str() {
                            Some("folder") => Scope::Folder,
                            Some("all" | "profile") => Scope::Everywhere,
                            _ => Scope::Device,
                        };
                        let server = m["server"].as_str().unwrap_or_default().to_owned();
                        let key = m["key"].as_str().unwrap_or_default().to_owned();
                        let agent = m["agent"]
                            .as_str()
                            .and_then(hooks::Agent::from_kind)
                            .unwrap_or(hooks::Agent::Claude);
                        let own = own_engine(&server);
                        let at = folder();
                        if let Ok(next) = backend.save_admin(&server, None) {
                            state.public.set(next);
                        }
                        busy.set(Some("install".into()));
                        note.set(None);
                        spawn(async move {
                            let into = at.clone();
                            let done = tokio::task::spawn_blocking(move || {
                                if agent == hooks::Agent::Claude {
                                    hooks::run(
                                        scope,
                                        into.as_deref(),
                                        &server,
                                        Some(&key),
                                        false,
                                        own,
                                    )
                                } else {
                                    hooks::run_agent(
                                        agent,
                                        into.as_deref(),
                                        &server,
                                        Some(&key),
                                        false,
                                    )
                                }
                            })
                            .await
                            .map_err(|e| e.to_string())
                            .and_then(|r| r);
                            let added = at
                                .filter(|_| {
                                    done.is_ok() && (scope == Scope::Folder || agent.per_folder())
                                })
                                .map(|f| f.display().to_string());
                            if let Some(at) = added {
                                let mut prefs = state.prefs.write();
                                if !prefs.hook_folders.contains(&at) {
                                    prefs.hook_folders.push(at);
                                }
                            }
                            note.set(Some(match done {
                                // The machine's ID shows in its card above.
                                Ok(_) => (true, "설치됨".into()),
                                Err(e) => (false, e),
                            }));
                            busy.set(None);
                            looked += 1;
                        });
                    }
                    Some("remove") => {
                        note_at.set("setup");
                        let file = m["file"].as_str().unwrap_or_default().to_owned();
                        let Some(install) = found
                            .peek()
                            .iter()
                            .find(|i| i.file.display().to_string() == file)
                            .cloned()
                        else {
                            continue;
                        };
                        let agent = install.agent;
                        let (scope, at) = match &install.at {
                            hooks::Where::Device => (Scope::Device, None),
                            hooks::Where::Everywhere | hooks::Where::Profile => {
                                (Scope::Everywhere, None)
                            }
                            hooks::Where::Folder(f) => (Scope::Folder, Some(f.clone())),
                        };
                        busy.set(Some(file));
                        note.set(None);
                        spawn(async move {
                            let from = at.clone();
                            let done = tokio::task::spawn_blocking(move || {
                                if agent == hooks::Agent::Claude {
                                    hooks::run(scope, from.as_deref(), "", None, true, false)
                                } else {
                                    hooks::run_agent(agent, from.as_deref(), "", None, true)
                                }
                            })
                            .await
                            .map_err(|e| e.to_string())
                            .and_then(|r| r);
                            match done {
                                Ok(_) => {
                                    // The folder stays listed while another agent's setup is in it.
                                    if let Some(at) = at.map(|f| f.display().to_string()) {
                                        let left = hooks::installed(std::slice::from_ref(&at)).iter().any(|i| {
                                            matches!(&i.at, hooks::Where::Folder(f) if f.display().to_string() == at)
                                        });
                                        if !left {
                                            state.prefs.write().hook_folders.retain(|f| *f != at);
                                        }
                                    }
                                    note.set(Some((true, "제거됨".into())));
                                }
                                Err(e) => note.set(Some((false, e))),
                            }
                            busy.set(None);
                            looked += 1;
                        });
                    }
                    // One agent's hooks, or every agent's found here: Claude Code for every
                    // folder, Hermes and OpenClaw in their profile, the others in the folder
                    // picked (left out without one).
                    Some(what @ ("install_agent" | "install_all")) => {
                        note_at.set("agents");
                        let server = state.public.peek().engine_api.clone();
                        let own = own_engine(&server);
                        let at = folder();
                        let agents: Vec<hooks::Agent> = if what == "install_all" {
                            hooks::Agent::ALL
                                .into_iter()
                                .filter(|a| {
                                    (*a == hooks::Agent::Claude || a.found())
                                        && (!a.per_folder() || at.is_some())
                                })
                                .collect()
                        } else {
                            m["agent"]
                                .as_str()
                                .and_then(hooks::Agent::from_kind)
                                .into_iter()
                                .collect()
                        };
                        if agents.iter().any(|a| a.per_folder()) && at.is_none() {
                            note.set(Some((false, "폴더를 골라 주세요.".into())));
                            continue;
                        }
                        busy.set(Some(if what == "install_all" {
                            "all".into()
                        } else {
                            format!("agent:{}", m["agent"].as_str().unwrap_or_default())
                        }));
                        note.set(None);
                        spawn(async move {
                            let mut failed = Vec::new();
                            for agent in agents {
                                let (server, into) = (server.clone(), at.clone());
                                let done = tokio::task::spawn_blocking(move || {
                                    if agent == hooks::Agent::Claude {
                                        hooks::run(
                                            Scope::Everywhere,
                                            None,
                                            &server,
                                            None,
                                            false,
                                            own,
                                        )
                                    } else {
                                        hooks::run_agent(
                                            agent,
                                            into.as_deref(),
                                            &server,
                                            None,
                                            false,
                                        )
                                    }
                                })
                                .await
                                .map_err(|e| e.to_string())
                                .and_then(|r| r);
                                match done {
                                    Ok(_) => {
                                        if let Some(f) = at.as_ref().filter(|_| agent.per_folder())
                                        {
                                            let f = f.display().to_string();
                                            let mut prefs = state.prefs.write();
                                            if !prefs.hook_folders.contains(&f) {
                                                prefs.hook_folders.push(f);
                                            }
                                        }
                                    }
                                    Err(e) => failed.push(format!("{} · {e}", agent.name())),
                                }
                                looked += 1;
                            }
                            note.set(Some(if failed.is_empty() {
                                (true, "설치됨".into())
                            } else {
                                (false, failed.join(" / "))
                            }));
                            busy.set(None);
                        });
                    }
                    Some("remove_agent") => {
                        note_at.set("agents");
                        let Some(agent) = m["agent"].as_str().and_then(hooks::Agent::from_kind)
                        else {
                            continue;
                        };
                        let targets: Vec<hooks::Install> = found
                            .peek()
                            .iter()
                            .filter(|i| i.agent == agent)
                            .cloned()
                            .collect();
                        busy.set(Some(format!("agent:{}", agent.kind())));
                        note.set(None);
                        spawn(async move {
                            let mut failed = Vec::new();
                            for install in targets {
                                let (scope, from) = match &install.at {
                                    hooks::Where::Device => (Scope::Device, None),
                                    hooks::Where::Everywhere | hooks::Where::Profile => {
                                        (Scope::Everywhere, None)
                                    }
                                    hooks::Where::Folder(f) => (Scope::Folder, Some(f.clone())),
                                };
                                let done = tokio::task::spawn_blocking(move || {
                                    if agent == hooks::Agent::Claude {
                                        hooks::run(scope, from.as_deref(), "", None, true, false)
                                    } else {
                                        hooks::run_agent(agent, from.as_deref(), "", None, true)
                                    }
                                })
                                .await
                                .map_err(|e| e.to_string())
                                .and_then(|r| r);
                                if let Err(e) = done {
                                    failed.push(e);
                                }
                            }
                            note.set(Some(if failed.is_empty() {
                                (true, "제거됨".into())
                            } else {
                                (false, failed.join(" / "))
                            }));
                            busy.set(None);
                            looked += 1;
                        });
                    }
                    Some("reset") => {
                        note_at.set("setup");
                        let folders = state.prefs.peek().hook_folders.clone();
                        busy.set(Some("reset".into()));
                        note.set(None);
                        spawn(async move {
                            let done = tokio::task::spawn_blocking(move || hooks::reset(&folders))
                                .await
                                .map_err(|e| e.to_string())
                                .and_then(|r| r);
                            match done {
                                Ok(()) => {
                                    state.prefs.write().hook_folders.clear();
                                    note.set(Some((true, "모두 지움".into())));
                                }
                                Err(e) => note.set(Some((false, e))),
                            }
                            busy.set(None);
                            looked += 1;
                        });
                    }
                    _ => {}
                }
            }
        }
    });
    rsx! {
        div { id: "wh-console-hooks", class: "wh-console" }
    }
}
