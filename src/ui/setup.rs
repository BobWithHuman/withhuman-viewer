//! The first launch, in a small window: the Jev key, checked with Jev before it is kept, and
//! where the engine runs (started on this Mac, or reached on a server). Done, the window grows
//! to the app's size and the main page opens.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use dioxus::desktop::{LogicalPosition, LogicalSize};
use dioxus::prelude::*;

use super::{AppState, SETUP_SIZE, Screen, Selection, WORK_MIN, WORK_SIZE};
use crate::api::Backend;
use crate::settings::{Change, Engine};

#[derive(Clone, PartialEq)]
enum Status {
    Idle,
    Busy,
    Ok(String),
    Err(String),
}

/// The window at the app's size: where it was last time, else in the middle of its screen.
pub fn grow(state: AppState) {
    let window = dioxus::desktop::window();
    window.set_resizable(true);
    window.set_min_inner_size(Some(LogicalSize::new(WORK_MIN.0, WORK_MIN.1)));
    let place = state.prefs.peek().window;
    let (w, h) = place.map_or(WORK_SIZE, |p| (p.width, p.height));
    window.set_inner_size(LogicalSize::new(w, h));
    match (place, window.current_monitor()) {
        (Some(p), _) => window.set_outer_position(LogicalPosition::new(p.x, p.y)),
        (None, Some(m)) => {
            let scale = m.scale_factor();
            let at = m.position().to_logical::<f64>(scale);
            let size = m.size().to_logical::<f64>(scale);
            window.set_outer_position(LogicalPosition::new(
                at.x + ((size.width - w) / 2.0).max(0.0),
                at.y + ((size.height - h) / 2.0).max(0.0),
            ));
        }
        (None, None) => {}
    }
}

/// Back to the setup, in its small window, centred where the window was.
pub fn shrink(mut state: AppState) {
    let window = dioxus::desktop::window();
    let scale = window.scale_factor();
    let (w, h) = SETUP_SIZE;
    if let (Ok(at), size) = (window.outer_position(), window.inner_size()) {
        let at = at.to_logical::<f64>(scale);
        let size = size.to_logical::<f64>(scale);
        window.set_min_inner_size(Some(LogicalSize::new(w, h)));
        window.set_inner_size(LogicalSize::new(w, h));
        window.set_outer_position(LogicalPosition::new(at.x + (size.width - w) / 2.0, at.y + (size.height - h) / 2.0));
    }
    window.set_resizable(false);
    state.screen.set(Screen::Setup);
}

#[component]
pub fn Setup() -> Element {
    let backend = use_context::<Arc<Backend>>();
    let mut state = use_context::<AppState>();
    let found = use_hook(crate::engine::find_dir);
    // The key the engine's own .env holds, offered so it need not be typed again.
    let env_key = use_hook(|| {
        found
            .as_ref()
            .and_then(|d| crate::engine::read_env(d).get("JEV_API_KEY").cloned())
            .filter(|k| !k.is_empty() && !k.starts_with("replace-with"))
    });
    let mut key = use_signal(String::new);
    let mut key_status = use_signal(|| Status::Idle);
    let mut engine = use_signal(|| if found.is_some() { Engine::Local } else { Engine::Server });
    let mut dir = use_signal(|| found.as_ref().map(|d| d.display().to_string()).unwrap_or_default());
    let current = state.public.peek().clone();
    let mut url = use_signal(|| current.neo4j_url.clone());
    let mut database = use_signal(|| current.database.clone());
    let mut user = use_signal(|| current.user.clone());
    let mut password = use_signal(String::new);
    let mut run = use_signal(|| Status::Idle);
    // What Docker printed while the engine starts, read from the thread that runs it.
    let log = use_hook(|| Arc::new(Mutex::new(Vec::<String>::new())));
    let mut lines = use_signal(Vec::<String>::new);
    {
        let log = log.clone();
        use_future(move || {
            let log = log.clone();
            async move {
                loop {
                    tokio::time::sleep(Duration::from_millis(300)).await;
                    let now = log.lock().map(|l| l.clone()).unwrap_or_default();
                    if *lines.peek() != now {
                        lines.set(now);
                    }
                }
            }
        });
    }

    let key_ok = matches!(key_status(), Status::Ok(_));
    let mut check = {
        let backend = backend.clone();
        move |_| {
            let backend = backend.clone();
            let k = key();
            key_status.set(Status::Busy);
            spawn(async move {
                key_status.set(match backend.check_jev(&k).await {
                    Ok(()) => Status::Ok("Jev가 이 키를 받았습니다.".into()),
                    Err(e) => Status::Err(e),
                });
            });
        }
    };
    let finish = {
        let backend = backend.clone();
        move |change: Change, kind: Engine, folder: Option<String>| match backend.finish_setup(&key(), change, kind, folder) {
            Ok(next) => {
                state.public.set(next);
                state.current.set(None);
                state.selected.set(Selection::None);
                state.reload += 1;
                grow(state);
                state.screen.set(Screen::Home);
            }
            Err(e) => run.set(Status::Err(e)),
        }
    };
    let start = {
        let backend = backend.clone();
        let log = log.clone();
        move |_| {
            let backend = backend.clone();
            let log = log.clone();
            let mut finish = finish.clone();
            if let Ok(mut l) = log.lock() {
                l.clear();
            }
            run.set(Status::Busy);
            spawn(async move {
                match engine() {
                    Engine::Local => {
                        let folder = dir();
                        match backend.start_local(PathBuf::from(&folder), key(), log).await {
                            Ok(change) => finish(change, Engine::Local, Some(folder)),
                            Err(e) => run.set(Status::Err(e)),
                        }
                    }
                    _ => {
                        let change = || Change {
                            neo4j_url: url(),
                            database: database(),
                            user: user(),
                            password: Some(password()).filter(|p| !p.is_empty()),
                            demo: false,
                        };
                        match backend.test(change()).await {
                            Ok(_) => finish(change(), Engine::Server, None),
                            Err(e) => run.set(Status::Err(e)),
                        }
                    }
                }
            });
        }
    };
    let demo = {
        let backend = backend.clone();
        move |_| {
            if let Ok(next) = backend.set_demo(true) {
                state.public.set(next);
                state.current.set(None);
                state.reload += 1;
                grow(state);
                state.screen.set(Screen::Home);
            }
        }
    };
    let busy = run() == Status::Busy;
    let ready = key_ok
        && !busy
        && match engine() {
            Engine::Local => !dir().trim().is_empty(),
            _ => !url().trim().is_empty(),
        };

    rsx! {
        div { class: "setup",
            super::TitleBar { overlay: true }
            div { class: "setup-glow" }
            header { class: "setup-top",
                span { class: "setup-mark", "W" }
                span { "WithHuman Viewer" }
                span { class: "setup-step", if key_ok { "2 / 2" } else { "1 / 2" } }
            }
            h1 { class: "setup-title", "에이전트가 누가 시킨" br {} "일을 했는지 봅니다" }
            p { class: "setup-lede", "시작하려면 Jev API 키를 등록하고, 엔진을 어디서 돌릴지 골라 주세요." }

            section { class: "setup-block",
                div { class: "setup-k", span { class: "n", "01" } "Jev API 키" }
                div { class: "setup-field",
                    input {
                        r#type: "password",
                        placeholder: "jev_…",
                        value: "{key}",
                        disabled: busy,
                        oninput: move |e| {
                            key.set(e.value());
                            key_status.set(Status::Idle);
                        },
                        onkeydown: {
                            let mut check = check.clone();
                            move |e: KeyboardEvent| if e.key() == Key::Enter { check(()) }
                        },
                    }
                    button { class: "setup-pill", disabled: busy || key_status() == Status::Busy || key().trim().is_empty(), onclick: move |_| check(()),
                        if key_status() == Status::Busy { "확인 중…" } else { "확인" }
                    }
                }
                if let Some(k) = env_key.clone() {
                    if key().is_empty() {
                        button { class: "setup-link", onclick: move |_| key.set(k.clone()), "withhuman-core/.env에 있는 키 넣기" }
                    }
                }
                match key_status() {
                    Status::Ok(t) => rsx! { p { class: "setup-note ok", "✓ {t}" } },
                    Status::Err(t) => rsx! { p { class: "setup-note err", "{t}" } },
                    _ => rsx! { p { class: "setup-note", "키는 Jev(api.typesafe.ai)에 확인한 뒤 macOS 키체인에만 저장됩니다." } },
                }
            }

            section { class: if key_ok { "setup-block" } else { "setup-block off" },
                div { class: "setup-k", span { class: "n", "02" } "엔진" }
                div { class: "setup-choice",
                    button {
                        class: if engine() == Engine::Local { "on" } else { "" },
                        disabled: !key_ok || busy,
                        onclick: move |_| engine.set(Engine::Local),
                        strong { "이 맥에서 실행" }
                        span { "Docker로 엔진을 띄우고 이 키를 넘깁니다" }
                    }
                    button {
                        class: if engine() == Engine::Server { "on" } else { "" },
                        disabled: !key_ok || busy,
                        onclick: move |_| engine.set(Engine::Server),
                        strong { "서버에 접속" }
                        span { "이미 떠 있는 엔진의 Neo4j를 읽습니다" }
                    }
                }
                fieldset { class: "setup-form", disabled: !key_ok || busy,
                    if engine() == Engine::Local {
                        label { "엔진 폴더 (withhuman-core)"
                            input { r#type: "text", value: "{dir}", placeholder: "~/withhuman-core", oninput: move |e| dir.set(e.value()) }
                        }
                    } else {
                        label { "Neo4j 주소"
                            input { r#type: "text", value: "{url}", oninput: move |e| url.set(e.value()) }
                        }
                        div { class: "row",
                            label { "사용자"
                                input { r#type: "text", value: "{user}", oninput: move |e| user.set(e.value()) }
                            }
                            label { "데이터베이스"
                                input { r#type: "text", value: "{database}", oninput: move |e| database.set(e.value()) }
                            }
                        }
                        label { "비밀번호"
                            input { r#type: "password", value: "{password}", placeholder: "엔진 .env의 NEO4J_PASSWORD", oninput: move |e| password.set(e.value()) }
                        }
                    }
                }
            }

            if busy || !lines().is_empty() {
                pre { class: "setup-log",
                    for (i, l) in lines().iter().rev().take(6).rev().enumerate() {
                        div { key: "{i}", "{l}" }
                    }
                }
            }
            if let Status::Err(t) = run() {
                p { class: "setup-note err", "{t}" }
            }
            footer { class: "setup-foot",
                button { class: "setup-go", disabled: !ready, onclick: start,
                    if busy {
                        if engine() == Engine::Local { "엔진을 띄우는 중…" } else { "연결하는 중…" }
                    } else {
                        "시작하기"
                    }
                }
                button { class: "setup-link", disabled: busy, onclick: demo, "키 없이 데모로 둘러보기" }
            }
        }
    }
}
