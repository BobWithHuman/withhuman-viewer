//! The developer's first launch, in a small window: the engine's store the record is read from
//! (its PostgreSQL, with the viewer's read-only login), tried before it is kept, and the hooks,
//! for one folder or every folder, installed with the engine's own setup. Done, the window grows
//! and the record opens.

use std::path::PathBuf;
use std::sync::Arc;

use dioxus::prelude::*;

use super::setup::grow;
use super::{AppState, Screen, Selection};
use crate::api::Backend;
use crate::hooks::{self, Scope};
use crate::settings::Change;

/// The engine's store as the engine's own Compose file serves it to the viewer.
const STORE: &str = "postgres://127.0.0.1:15432/withhuman";
const STORE_USER: &str = "withhuman_viewer";

#[derive(Clone, PartialEq)]
enum Status {
    Idle,
    Busy,
    Ok(String),
    Err(String),
}

#[component]
pub fn DevSetup() -> Element {
    let backend = use_context::<Arc<Backend>>();
    let mut state = use_context::<AppState>();
    let current = state.public.peek().clone();
    let postgres = current.neo4j_url.starts_with("postgres");
    let mut url = use_signal(|| {
        if postgres {
            current.neo4j_url.clone()
        } else {
            STORE.into()
        }
    });
    let mut user = use_signal(|| {
        if postgres {
            current.user.clone()
        } else {
            STORE_USER.into()
        }
    });
    let mut database = use_signal(|| {
        if postgres {
            current.database.clone()
        } else {
            "withhuman".into()
        }
    });
    let mut password = use_signal(String::new);
    let mut store = use_signal(|| Status::Idle);
    let change = move || Change {
        neo4j_url: url(),
        database: database(),
        user: user(),
        password: Some(password()).filter(|p| !p.is_empty()),
        demo: false,
    };
    let check = {
        let backend = backend.clone();
        move |_| {
            let backend = backend.clone();
            store.set(Status::Busy);
            spawn(async move {
                store.set(match backend.test(change()).await {
                    Ok(probe) => Status::Ok(format!("연결됨 · 세션 {}", probe.sessions)),
                    Err(e) => Status::Err(e),
                });
            });
        }
    };
    let start = {
        let backend = backend.clone();
        move |_| match backend.save(change()) {
            Ok(next) => {
                state.public.set(next);
                state.current.set(None);
                state.selected.set(Selection::None);
                state.reload += 1;
                grow(state);
                state.screen.set(Screen::Work);
            }
            Err(e) => store.set(Status::Err(e)),
        }
    };
    let connected = matches!(store(), Status::Ok(_));
    let busy = store() == Status::Busy;

    rsx! {
        div { class: "setup",
            super::TitleBar { overlay: true }
            div { class: "setup-glow" }
            header { class: "setup-top",
                span { class: "setup-mark", "W" }
                span { "WithHuman Viewer" }
                span { class: "setup-step", if connected { "2 / 2" } else { "1 / 2" } }
            }
            h1 { class: "setup-title", "엔진 연결" }

            section { class: "setup-block",
                div { class: "setup-k", span { class: "n", "01" } "기록 저장소" }
                fieldset { class: "setup-form", disabled: busy,
                    label { "주소"
                        input { r#type: "text", value: "{url}", placeholder: STORE, oninput: move |e| { url.set(e.value()); store.set(Status::Idle); } }
                    }
                    div { class: "row",
                        label { "사용자"
                            input { r#type: "text", value: "{user}", oninput: move |e| { user.set(e.value()); store.set(Status::Idle); } }
                        }
                        label { "데이터베이스"
                            input { r#type: "text", value: "{database}", oninput: move |e| { database.set(e.value()); store.set(Status::Idle); } }
                        }
                    }
                    div { class: "setup-field",
                        input { r#type: "password", value: "{password}", placeholder: "비밀번호",
                            oninput: move |e| { password.set(e.value()); store.set(Status::Idle); } }
                        button { class: "setup-pill", disabled: busy || url().trim().is_empty(), onclick: check,
                            if busy { "확인 중…" } else { "연결 확인" }
                        }
                    }
                }
                match store() {
                    Status::Ok(t) => rsx! { p { class: "setup-note ok", "✓ {t}" } },
                    Status::Err(t) => rsx! { p { class: "setup-note err", "{t}" } },
                    _ => rsx! {},
                }
            }

            section { class: "setup-block",
                div { class: "setup-k", span { class: "n", "02" } "훅" }
                HookSetup {}
            }

            footer { class: "setup-foot",
                button { class: "setup-go", disabled: !connected, onclick: start, "시작하기" }
            }
        }
    }
}

/// Where the hooks go and the engine they report to, and the button that installs them (or
/// takes them out). Shared by the first launch and the connection settings.
#[component]
pub fn HookSetup() -> Element {
    let backend = use_context::<Arc<Backend>>();
    let mut state = use_context::<AppState>();
    let mut server = use_signal(|| state.public.peek().engine_api.clone());
    let mut scope = use_signal(|| Scope::Everywhere);
    let mut folder = use_signal(|| None::<PathBuf>);
    let mut key = use_signal(String::new);
    let mut status = use_signal(|| match hooks::device_id() {
        Some(id) => Status::Ok(format!("기기 {id}")),
        None => Status::Idle,
    });
    let pick = move |_| {
        spawn(async move {
            if let Some(f) = rfd::AsyncFileDialog::new().pick_folder().await {
                folder.set(Some(f.path().to_path_buf()));
                scope.set(Scope::Folder);
            }
        });
    };
    let run = {
        let backend = backend.clone();
        move |uninstall: bool| {
            let backend = backend.clone();
            let (s, f, srv, k) = (scope(), folder(), server(), key());
            status.set(Status::Busy);
            spawn(async move {
                // The engine's address is kept for the next setup, as the security team's is.
                if let Some(Ok(next)) = (!uninstall).then(|| backend.save_admin(&srv, None)) {
                    state.public.set(next);
                }
                let done = tokio::task::spawn_blocking(move || {
                    hooks::run(s, f.as_deref(), &srv, Some(&k), uninstall)
                })
                .await
                .map_err(|e| e.to_string())
                .and_then(|r| r);
                status.set(match (done, uninstall) {
                    (Ok(_), true) => Status::Ok("훅 제거됨".into()),
                    (Ok(d), false) => Status::Ok(match d.client_id {
                        Some(id) => format!("설치됨 · 기기 {id}"),
                        None => "설치됨".into(),
                    }),
                    (Err(e), _) => Status::Err(e),
                });
            });
        }
    };
    let busy = status() == Status::Busy;
    let ready = !busy
        && !server().trim().is_empty()
        && (scope() == Scope::Everywhere || folder().is_some());
    let folder_name = folder()
        .map(|f| f.display().to_string())
        .unwrap_or_else(|| "폴더 고르기".into());

    rsx! {
        div { class: "setup-choice",
            button {
                class: if scope() == Scope::Everywhere { "on" } else { "" },
                disabled: busy,
                onclick: move |_| scope.set(Scope::Everywhere),
                strong { "모든 폴더" }
                span { "~/.claude/settings.json" }
            }
            button {
                class: if scope() == Scope::Folder { "on" } else { "" },
                disabled: busy,
                onclick: pick,
                strong { "이 폴더만" }
                span { title: "{folder_name}", "{folder_name}" }
            }
        }
        fieldset { class: "setup-form", disabled: busy,
            div { class: "row",
                label { "엔진 주소"
                    input { r#type: "text", value: "{server}", placeholder: "http://127.0.0.1:3000", oninput: move |e| server.set(e.value()) }
                }
                label { "키 (선택)"
                    input { r#type: "password", value: "{key}", placeholder: "whk_…", oninput: move |e| key.set(e.value()) }
                }
            }
        }
        div { class: "setup-actions",
            button { class: "setup-pill", disabled: !ready, onclick: { let mut run = run.clone(); move |_| run(false) },
                if busy { "설치 중…" } else { "훅 설치" }
            }
            button { class: "setup-link", disabled: busy || (scope() == Scope::Folder && folder().is_none()), onclick: { let mut run = run.clone(); move |_| run(true) }, "제거" }
        }
        match status() {
            Status::Ok(t) => rsx! { p { class: "setup-note ok", "✓ {t}" } },
            Status::Err(t) => rsx! { p { class: "setup-note err", "{t}" } },
            _ => rsx! {},
        }
    }
}
