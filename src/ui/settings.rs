//! The connection sheet: where the engine's data is (its PostgreSQL, or Neo4j before
//! withhuman-core #118), tried before it is saved, or the demo instead; and in the security
//! team's app, the engine's admin API (its address and token), which stopping and deleting a
//! machine call.

use std::sync::Arc;

use dioxus::prelude::*;

use super::{AppState, Selection};
use crate::api::{Backend, Probe};
use crate::settings::Change;

#[derive(Clone, PartialEq)]
enum Status {
    Idle,
    Busy,
    Ok(String),
    Err(String),
}

#[component]
pub fn SettingsSheet() -> Element {
    let state = use_context::<AppState>();
    #[cfg(not(feature = "admin"))]
    if state.public.peek().engine == crate::settings::Engine::Own {
        return rsx! { OwnSheet {} };
    }
    let _ = state;
    rsx! { StoreSheet {} }
}

/// The app's own engine, drawn by the console page (`web/console`, `pages/engine.tsx`) in the
/// sheet: its parts; how Calls are judged (the rules here, or Jev with its key and model),
/// started again to apply it; the try; and taking the app off this machine.
///
/// `inline`: drawn as the 설정 page of 기록's frame (`web/record`), handed its data there, rather
/// than in a sheet of its own.
#[cfg(not(feature = "admin"))]
#[component]
pub(super) fn OwnSheet(#[props(default)] inline: bool) -> Element {
    use crate::own::Part;
    use serde_json::{Value, json};
    let backend = use_context::<Arc<Backend>>();
    let mut state = use_context::<AppState>();
    let mut models = use_signal(|| vec![crate::own::DEFAULT_MODEL.to_owned()]);
    let mut status = use_signal(|| Status::Idle);
    let mut parts = use_signal(Vec::<(Part, bool)>::new);
    let mut looked = use_signal(|| 0u32);
    use_future(move || async move {
        loop {
            parts.set(crate::own::health().await);
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        }
    });
    // The models the kept key can ask.
    use_future({
        let backend = backend.clone();
        move || {
            let backend = backend.clone();
            async move {
                if let Some(k) = crate::settings::jev_key() {
                    let mut found = backend.jev_models(&k).await;
                    let model = crate::own::Config::load().model;
                    if !found.contains(&model) {
                        found.push(model);
                    }
                    models.set(found);
                }
            }
        }
    });
    let data = use_memo(move || {
        let _ = looked();
        let config = crate::own::Config::load();
        let has_key = crate::settings::jev_key().is_some();
        let (state_name, text) = match status() {
            Status::Idle => ("idle", String::new()),
            Status::Busy => ("busy", String::new()),
            Status::Ok(t) => ("ok", t),
            Status::Err(t) => ("err", t),
        };
        json!({
            "parts": parts().iter().map(|(p, up)| json!({"name": p.name(), "up": up})).collect::<Vec<_>>(),
            "url": crate::own::collector_url(),
            "has_key": has_key,
            "jev": config.jev && has_key,
            "models": models(),
            "model": config.model,
            "status": {"state": state_name, "text": text},
        })
        .to_string()
    });
    let onmsg = {
        let backend = backend.clone();
        move |m: Value| {
            let backend = backend.clone();
            match m["do"].as_str() {
                Some("close") => {
                    state.settings_open.set(false);
                    state.web.set("record");
                }
                Some("logs") => {
                    let _ =
                        std::process::Command::new(if cfg!(windows) { "explorer" } else { "open" })
                            .arg(crate::own::logs())
                            .spawn();
                }
                Some("apply") => {
                    let on = m["jev"].as_bool().unwrap_or(false);
                    let k = m["key"].as_str().unwrap_or_default().trim().to_owned();
                    let model = m["model"]
                        .as_str()
                        .map(str::to_owned)
                        .unwrap_or_else(|| crate::own::Config::load().model);
                    status.set(Status::Busy);
                    spawn(async move {
                        if on && k.is_empty() && crate::settings::jev_key().is_none() {
                            status.set(Status::Err("Jev API 키".into()));
                            return;
                        }
                        if on
                            && !k.is_empty()
                            && let Err(e) = backend.check_jev(&k).await
                        {
                            status.set(Status::Err(e));
                            return;
                        }
                        match backend.start_own(&k, &model, on).await {
                            Ok(change) => match backend.finish_own(change) {
                                Ok(next) => {
                                    state.public.set(next);
                                    state.reload += 1;
                                    status.set(Status::Ok("다시 시작됨".into()));
                                }
                                Err(e) => status.set(Status::Err(e)),
                            },
                            Err(e) => status.set(Status::Err(e)),
                        }
                        looked += 1;
                    });
                }
                Some("try") => {
                    status.set(Status::Busy);
                    spawn(async move {
                        match crate::trial::run(&crate::own::collector_url()).await {
                            Ok(_) => {
                                status.set(Status::Idle);
                                state.reload += 1;
                                state.settings_open.set(false);
                                state.web.set("record");
                            }
                            Err(e) => status.set(Status::Err(e)),
                        }
                    });
                }
                Some("remove") => {
                    let folders = state.prefs.peek().hook_folders.clone();
                    status.set(Status::Busy);
                    spawn(async move {
                        let done = tokio::task::spawn_blocking(move || {
                            backend.remove_everything(&folders)
                        })
                        .await
                        .map_err(|e| e.to_string())
                        .and_then(|r| r);
                        match done {
                            Ok(()) => std::process::exit(0),
                            Err(e) => status.set(Status::Err(e)),
                        }
                    });
                }
                _ => {}
            }
        }
    };
    // In 기록's frame: the data handed there, and what is pressed there heard here.
    use_effect(move || {
        if inline {
            let d = data();
            document::eval(&format!(
                "requestAnimationFrame(() => window.__whRecordView && window.__whRecordView('engine', {d}))"
            ));
        }
    });
    let heard = onmsg.clone();
    use_future(move || {
        let mut heard = heard.clone();
        async move {
            if !inline {
                return;
            }
            let mut eval = document::eval(
                "window.__whConsoleOn['engine'] = (m) => dioxus.send(m); await new Promise(() => {});",
            );
            while let Ok(m) = eval.recv::<Value>().await {
                heard(m);
            }
        }
    });
    if inline {
        return rsx! {};
    }

    rsx! {
        div {
            class: "sheet-backdrop",
            tabindex: "-1",
            onmounted: move |e| async move {
                let _ = e.set_focus(true).await;
            },
            onkeydown: move |e| {
                if e.key() == Key::Escape {
                    state.settings_open.set(false);
                }
            },
            onclick: move |_| state.settings_open.set(false),
            div { class: "sheet engine-sheet", onclick: move |e| e.stop_propagation(),
                super::console::Frame { page: "engine", data: data(), onmsg }
            }
        }
    }
}

#[component]
fn StoreSheet() -> Element {
    let backend = use_context::<Arc<Backend>>();
    let mut state = use_context::<AppState>();
    let current = state.public.peek().clone();
    let mut url = use_signal(|| current.neo4j_url.clone());
    let mut database = use_signal(|| current.database.clone());
    let mut user = use_signal(|| current.user.clone());
    // Empty keeps the stored password.
    let mut password = use_signal(String::new);
    let mut demo = use_signal(|| current.demo);
    let mut status = use_signal(|| Status::Idle);
    let mut api = use_signal(|| current.engine_api.clone());
    // Empty keeps the stored token.
    let mut token = use_signal(String::new);
    let admin = cfg!(feature = "admin");

    let change = move || Change {
        neo4j_url: url(),
        database: database(),
        user: user(),
        password: Some(password()).filter(|p| !p.is_empty()),
        demo: demo(),
    };
    let test = {
        let backend = backend.clone();
        move |_| {
            let backend = backend.clone();
            let change = change();
            status.set(Status::Busy);
            spawn(async move {
                status.set(match backend.test(change).await {
                    Ok(Probe { sessions, ms }) => {
                        Status::Ok(format!("연결됨 · 세션 {sessions}개 · {ms}ms"))
                    }
                    Err(e) => Status::Err(e),
                });
            });
        }
    };
    let save = move |_| match backend.save(change()).and_then(|next| {
        if !admin {
            return Ok(next);
        }
        let token = token();
        backend.save_admin(&api(), Some(token.trim()).filter(|t| !t.is_empty()))
    }) {
        Ok(next) => {
            state.public.set(next);
            state.current.set(None);
            state.selected.set(Selection::None);
            state.settings_open.set(false);
        }
        Err(e) => status.set(Status::Err(e)),
    };
    let busy = status() == Status::Busy;
    let postgres = url().trim().starts_with("postgres");
    let hint = match (current.has_password, postgres) {
        (true, _) => "비워 두면 저장된 비밀번호 유지",
        (false, true) => "엔진의 뷰어 계정 비밀번호",
        (false, false) => "엔진 .env의 NEO4J_PASSWORD",
    };

    rsx! {
        div {
            class: "sheet-backdrop",
            tabindex: "-1",
            onmounted: move |e| async move {
                let _ = e.set_focus(true).await;
            },
            onkeydown: move |e| {
                if e.key() == Key::Escape {
                    state.settings_open.set(false);
                }
            },
            onclick: move |_| state.settings_open.set(false),
            div { class: "sheet", onclick: move |e| e.stop_propagation(),
                h2 { "데이터 연결" }
                // The developer's app has no demo to choose.
                div { class: "choice", hidden: !cfg!(feature = "admin"),
                    button { class: if !demo() { "on" } else { "" }, onclick: move |_| demo.set(false),
                        strong { "엔진 데이터" }
                        span { "WithHuman 엔진이 기록한 실제 세션" }
                    }
                    button { class: if demo() { "on" } else { "" }, onclick: move |_| demo.set(true),
                        strong { "데모로 둘러보기" }
                        span { "엔진 없이 예시 세션으로" }
                    }
                }
                fieldset { disabled: demo(),
                    label { "주소"
                        input { r#type: "text", value: "{url}", placeholder: "postgres://127.0.0.1:5432/withhuman", oninput: move |e| url.set(e.value()) }
                    }
                    p { class: "muted", "지금 엔진은 PostgreSQL입니다: postgres://호스트:포트/DB. 예전 엔진(Neo4j)은 http://호스트:7474로 적습니다." }
                    p { class: "muted", "다른 기기의 서버는 인증서까지 확인해 암호화합니다. 클라우드가 준 CA 파일은 주소 끝에 ?sslrootcert=파일경로 로 붙입니다." }
                    div { class: "row",
                        label { "데이터베이스"
                            input { r#type: "text", value: "{database}", oninput: move |e| database.set(e.value()) }
                        }
                        label { "사용자"
                            input { r#type: "text", value: "{user}", oninput: move |e| user.set(e.value()) }
                        }
                    }
                    label { "비밀번호"
                        input { r#type: "password", value: "{password}", placeholder: hint, oninput: move |e| password.set(e.value()) }
                    }
                }
                if admin {
                    h2 { class: "sheet-sub", "엔진 관리 API" }
                    div { class: "row",
                        label { "주소"
                            input { r#type: "text", value: "{api}", placeholder: "https://엔진 주소", oninput: move |e| api.set(e.value()) }
                        }
                        label { "관리 토큰"
                            input { r#type: "password", value: "{token}",
                                placeholder: if current.has_admin_token { "저장된 토큰 유지" } else { "WITHHUMAN_ADMIN_TOKEN" },
                                oninput: move |e| token.set(e.value()) }
                        }
                    }
                }
                match status() {
                    Status::Idle => rsx! {},
                    Status::Busy => rsx! { p { class: "status", "연결하는 중…" } },
                    Status::Ok(text) => rsx! { p { class: "status ok", "{text}" } },
                    Status::Err(text) => rsx! { p { class: "status error", "{text}" } },
                }
                div { class: "actions",
                    span { class: "muted path", "{current.path}" }
                    button { onclick: move |_| state.settings_open.set(false), "취소" }
                    button { disabled: busy || demo(), onclick: test, "연결 테스트" }
                    button { class: "primary", disabled: busy, onclick: save, "저장" }
                }
            }
        }
    }
}
