//! The personal app's first launch, drawn by the console page (`web/console`,
//! `pages/onboard.tsx`, shadcn's parts) in its frame: the engine on this machine or the company's; how Calls are judged; which agents' hooks
//! go in; then the engine and the hooks coming up. This side does what is pressed there and
//! hands back how each step went.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use dioxus::prelude::*;
use serde_json::{Value, json};

use super::setup::grow;
use super::{AppState, Screen, Selection};
use crate::api::Backend;
use crate::hooks::{self, Agent, Scope};
use crate::own::Part;
use crate::settings::Change;

#[derive(Clone, PartialEq)]
enum Status {
    Idle,
    Busy,
    Ok(String),
    Err(String),
}

impl Status {
    fn json(&self) -> Value {
        match self {
            Status::Idle => json!({"state": "idle", "text": null}),
            Status::Busy => json!({"state": "busy", "text": null}),
            Status::Ok(t) => json!({"state": "ok", "text": t}),
            Status::Err(t) => json!({"state": "err", "text": t}),
        }
    }

    fn settled(&self) -> bool {
        matches!(self, Status::Ok(_) | Status::Err(_))
    }
}

/// The store a server's engine serves the viewer, as the setup form sends it.
fn store(m: &Value) -> Change {
    let text = |k: &str| m[k].as_str().unwrap_or_default().trim().to_owned();
    Change {
        neo4j_url: text("url"),
        database: text("database"),
        user: text("user"),
        password: Some(m["password"].as_str().unwrap_or_default().to_owned())
            .filter(|p| !p.is_empty()),
        demo: false,
    }
}

/// Opens the app's main window on 기록.
fn open(mut state: AppState) {
    state.current.set(None);
    state.selected.set(Selection::None);
    state.reload += 1;
    grow(state);
    state.screen.set(Screen::Work);
}

/// `inline`: drawn by 기록's frame (`web/langflow`, its `onboard` page), handed its data there.
#[component]
pub fn OnboardPage(#[props(default)] inline: bool) -> Element {
    let backend = use_context::<Arc<Backend>>();
    let mut state = use_context::<AppState>();
    let mut jev = use_signal(|| Status::Idle);
    let mut models = use_signal(Vec::<String>::new);
    let mut model = use_signal(|| crate::own::Config::load().model);
    let mut parts = use_signal(Vec::<(Part, bool)>::new);
    let mut python = use_signal(|| None::<PathBuf>);
    let mut hook = use_signal(HashMap::<Agent, Status>::new);
    let mut folders = use_signal(HashMap::<Agent, PathBuf>::new);
    let mut run = use_signal(|| Status::Idle);
    let mut server = use_signal(|| Status::Idle);
    let mut sessions = use_signal(|| None::<i64>);
    let mut done = use_signal(|| false);
    let kept_key = use_hook(|| crate::settings::jev_key().is_some());
    let found = use_hook(|| {
        Agent::ALL
            .into_iter()
            .map(|a| (a, a == Agent::Claude || a.found()))
            .collect::<Vec<_>>()
    });
    // The engine's parts every second, Python every few: it may be installed meanwhile.
    use_future(move || async move {
        let mut tick = 0u32;
        loop {
            parts.set(crate::own::health().await);
            if tick.is_multiple_of(3) && python.peek().is_none() {
                let at = tokio::task::spawn_blocking(hooks::python_found)
                    .await
                    .ok()
                    .flatten();
                python.set(at);
            }
            tick += 1;
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    });

    let data = use_memo(move || {
        let agents: Vec<Value> = found
            .iter()
            .map(|(a, here)| {
                let mut row = json!({
                    "kind": a.kind(), "name": a.name(), "found": here,
                    "per_folder": a.per_folder(), "per_profile": a.per_profile(),
                    "folder": folders().get(a).map(|f| f.display().to_string()),
                });
                let status = hook().get(a).cloned().unwrap_or(Status::Idle).json();
                if let (Some(row), Some(status)) = (row.as_object_mut(), status.as_object()) {
                    row.extend(status.clone());
                }
                row
            })
            .collect();
        let mut jev_json = jev().json();
        jev_json["models"] = json!(models());
        let mut server_json = server().json();
        server_json["sessions"] = json!(sessions());
        json!({
            // A developer's machine starts on the company's engine.
            "role": if crate::api::launch().developer { "developer" } else { "personal" },
            "python": python().map(|p| p.display().to_string()),
            "kept_key": kept_key,
            "jev": jev_json,
            "model": model(),
            "parts": parts().iter().map(|(p, up)| json!({"name": p.name(), "up": up})).collect::<Vec<_>>(),
            "agents": agents,
            "run": run().json(),
            "server": server_json,
            "done": done(),
        })
        .to_string()
    });
    use_effect(move || {
        let d = data();
        let set = if inline {
            "__whRecordView"
        } else {
            "__whConsoleSet"
        };
        document::eval(&format!(
            "requestAnimationFrame(() => window.{set} && window.{set}('onboard', {d}))"
        ));
    });

    use_future(move || {
        let backend = backend.clone();
        async move {
            let mut eval = document::eval(
                "window.__whConsoleOn['onboard'] = (m) => dioxus.send(m); await new Promise(() => {});",
            );
            while let Ok(m) = eval.recv::<Value>().await {
                match m["do"].as_str() {
                    Some("check_jev") => {
                        let key = m["key"].as_str().unwrap_or_default().to_owned();
                        let backend = backend.clone();
                        jev.set(Status::Busy);
                        spawn(async move {
                            match backend.check_jev(&key).await {
                                Ok(()) => {
                                    let found = backend.jev_models(&key).await;
                                    if !found.contains(&model.peek()) {
                                        model.set(crate::own::DEFAULT_MODEL.into());
                                    }
                                    jev.set(Status::Ok(format!("모델 {}", found.len())));
                                    models.set(found);
                                }
                                Err(e) => jev.set(Status::Err(e)),
                            }
                        });
                    }
                    Some("model") => {
                        if let Some(m) = m["model"].as_str() {
                            model.set(m.to_owned());
                        }
                    }
                    Some("pick") => {
                        let Some(agent) = m["agent"].as_str().and_then(Agent::from_kind) else {
                            continue;
                        };
                        spawn(async move {
                            if let Some(f) = rfd::AsyncFileDialog::new().pick_folder().await {
                                folders.write().insert(agent, f.path().to_path_buf());
                            }
                        });
                    }
                    Some("python") => hooks::get_python(),
                    Some("start") => {
                        let on = m["jev"].as_bool().unwrap_or(false);
                        let key = m["key"].as_str().unwrap_or_default().to_owned();
                        let chosen: Vec<Agent> = m["agents"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .filter_map(|k| k.as_str().and_then(Agent::from_kind))
                            .collect();
                        let backend = backend.clone();
                        run.set(Status::Busy);
                        hook.set(chosen.iter().map(|a| (*a, Status::Idle)).collect());
                        done.set(false);
                        spawn(async move {
                            // Rehearsed: the engine left as it is, its start a moment's wait.
                            let change = if hooks::rehearsal() {
                                tokio::time::sleep(Duration::from_millis(900)).await;
                                None
                            } else {
                                match backend.start_own(&key, &model.peek(), on).await {
                                    Ok(c) => Some(c),
                                    Err(e) => {
                                        run.set(Status::Err(e));
                                        return;
                                    }
                                }
                            };
                            run.set(Status::Ok("실행 중".into()));
                            let server = crate::own::collector_url();
                            let mut installed = Vec::new();
                            for agent in chosen {
                                hook.write().insert(agent, Status::Busy);
                                let at = server.clone();
                                let folder = folders.peek().get(&agent).cloned();
                                let into = folder.clone();
                                let result = tokio::task::spawn_blocking(move || {
                                    if agent == Agent::Claude {
                                        hooks::run(Scope::Everywhere, None, &at, None, false, true)
                                    } else {
                                        hooks::run_agent(agent, into.as_deref(), &at, None, false)
                                    }
                                })
                                .await
                                .map_err(|e| e.to_string())
                                .and_then(|r| r);
                                let status = match result {
                                    Ok(_) => {
                                        if let Some(f) = folder.filter(|_| agent.per_folder()) {
                                            installed.push(f.display().to_string());
                                        }
                                        Status::Ok("설치됨".into())
                                    }
                                    Err(e) => Status::Err(e),
                                };
                                hook.write().insert(agent, status);
                            }
                            {
                                let mut prefs = state.prefs.write();
                                for f in installed {
                                    if !prefs.hook_folders.contains(&f) {
                                        prefs.hook_folders.push(f);
                                    }
                                }
                            }
                            match change.map(|c| backend.finish_own(c)) {
                                None => done.set(hook.peek().values().all(Status::settled)),
                                Some(Ok(next)) => {
                                    state.public.set(next);
                                    done.set(hook.peek().values().all(Status::settled));
                                }
                                Some(Err(e)) => run.set(Status::Err(e)),
                            }
                        });
                    }
                    Some("open") => {
                        let try_it = m["try"].as_bool().unwrap_or(false);
                        open(state);
                        if try_it {
                            spawn(async move {
                                let _ = crate::trial::run(&crate::own::collector_url()).await;
                                state.reload += 1;
                            });
                        }
                    }
                    Some("server_test") => {
                        let change = store(&m);
                        let backend = backend.clone();
                        server.set(Status::Busy);
                        spawn(async move {
                            match backend.test(change).await {
                                Ok(probe) => {
                                    sessions.set(Some(probe.sessions));
                                    server.set(Status::Ok(format!("세션 {}", probe.sessions)));
                                }
                                Err(e) => server.set(Status::Err(e)),
                            }
                        });
                    }
                    Some("server_save") => {
                        let change = store(&m);
                        let engine = m["engine"].as_str().unwrap_or_default().trim().to_owned();
                        let key = m["key"].as_str().unwrap_or_default().trim().to_owned();
                        let backend = backend.clone();
                        server.set(Status::Busy);
                        spawn(async move {
                            if !engine.is_empty() {
                                if let Ok(next) = backend.save_admin(&engine, None) {
                                    state.public.set(next);
                                }
                                let at = engine.clone();
                                let installed = tokio::task::spawn_blocking(move || {
                                    hooks::run(
                                        Scope::Everywhere,
                                        None,
                                        &at,
                                        Some(&key),
                                        false,
                                        false,
                                    )
                                })
                                .await
                                .map_err(|e| e.to_string())
                                .and_then(|r| r);
                                if let Err(e) = installed {
                                    server.set(Status::Err(e));
                                    return;
                                }
                            }
                            match backend.save_server(change) {
                                Ok(next) => {
                                    state.public.set(next);
                                    open(state);
                                }
                                Err(e) => server.set(Status::Err(e)),
                            }
                        });
                    }
                    _ => {}
                }
            }
        }
    });

    if inline {
        return rsx! {};
    }
    rsx! {
        div { class: "onboard",
            super::TitleBar { overlay: true }
            div { id: "wh-console-onboard", class: "wh-console" }
        }
    }
}
