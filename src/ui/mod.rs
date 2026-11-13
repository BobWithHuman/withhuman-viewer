//! The app's screens. One state, shared by context, drives them all: the sessions, the one
//! open, its graph, and what is selected in it.

mod compare;
mod flow;
pub(crate) mod home;
mod inspector;
mod metro;
mod settings;
mod setup;
// The session list, kept while sessions are picked in the route list's region.
#[allow(dead_code)]
mod sidebar;
mod trace;
mod update;

use std::sync::Arc;
use std::time::Duration;

use dioxus::prelude::*;

use crate::api::Backend;
use crate::layout::Model;
use crate::menu;
use crate::model::Session;
use crate::prefs::{Prefs, WindowPlace};
use crate::settings::Public;

pub const CSS: &str = include_str!("style.css");

/// The main page: the developer's sessions worth a look, or the security team's overview of
/// every machine.
#[cfg(feature = "admin")]
#[component]
fn MainPage() -> Element {
    rsx! { crate::admin::Main {} }
}
#[cfg(not(feature = "admin"))]
#[component]
fn MainPage() -> Element {
    rsx! { home::Home {} }
}

/// The screen the app opens on after the setup: the main page, or the one `VIEWER_SCREEN` names
/// (`work`, `a`, `b`) so a screen can be looked at without clicking to it.
fn first_screen() -> Screen {
    match std::env::var("VIEWER_SCREEN").as_deref() {
        Ok("work") => Screen::Work,
        Ok("a") => Screen::FeatureA,
        Ok("b") => Screen::FeatureB,
        #[cfg(feature = "admin")]
        Ok("mcp") => Screen::Mcp,
        #[cfg(feature = "admin")]
        Ok("secrets") => Screen::Secrets,
        #[cfg(feature = "admin")]
        Ok("inventory") => Screen::Inventory,
        _ => Screen::Home,
    }
}

/// The top bar's other two screens: the security team's 감사 and 정책, or two features to come.
#[cfg(feature = "admin")]
#[component]
fn FeaturePage(second: bool) -> Element {
    if second {
        rsx! { crate::admin::Policy {} }
    } else {
        rsx! { crate::admin::Audit {} }
    }
}
#[cfg(not(feature = "admin"))]
#[component]
fn FeaturePage(second: bool) -> Element {
    let (name, tone) = if second {
        ("기능 B", "b")
    } else {
        ("기능 A", "a")
    };
    rsx! {
        div { class: "soon {tone}",
            span { class: "soon-mark", "{name}" }
            h1 { "{name}" }
            p { "곧 추가될 기능입니다. 이 자리에 들어갈 내용이 정해지면 채웁니다." }
        }
    }
}

/// The security team's other products; nothing in the developer's app, which never opens them.
#[cfg(feature = "admin")]
#[component]
fn ProductPage(screen: Screen) -> Element {
    match screen {
        Screen::Secrets => rsx! { crate::admin::Secrets {} },
        Screen::Inventory => rsx! { crate::admin::InventoryPage {} },
        _ => rsx! { crate::admin::Mcp {} },
    }
}
#[cfg(not(feature = "admin"))]
#[component]
fn ProductPage(screen: Screen) -> Element {
    let _ = screen;
    rsx! {}
}

/// The top bar's names for the route map and the other two screens.
#[cfg(feature = "admin")]
const SCREEN_LABELS: (&str, &str, &str) = ("기록", "감사", "정책");
#[cfg(not(feature = "admin"))]
const SCREEN_LABELS: (&str, &str, &str) = ("노선도", "기능 A", "기능 B");

/// The developer's app and the security team's app share every screen but the main page.
#[cfg(feature = "admin")]
pub const APP_NAME: &str = "WithHuman Console";
#[cfg(not(feature = "admin"))]
pub const APP_NAME: &str = "WithHuman Viewer";
/// The main page's name in the top bar.
#[cfg(feature = "admin")]
const HOME_LABEL: &str = "메인";
#[cfg(not(feature = "admin"))]
const HOME_LABEL: &str = "홈";

/// Something loaded from the data source.
#[derive(Clone, Debug, PartialEq)]
pub enum Load<T> {
    Loading,
    Ready(T),
    Failed(String),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Selection {
    None,
    Node(i64),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Mode {
    Graph,
    Compare,
}

/// Which of the app's three faces is up: the first launch's setup in a small window, the main
/// page that gathers what the app does, or the work (the route map, the comparison).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Screen {
    Setup,
    Home,
    Work,
    /// Two features to come, each a screen of its own in the top bar for now.
    FeatureA,
    FeatureB,
    /// The security team's other products, opened from the rail: MCP 사용, 민감 정보, 인벤토리.
    #[cfg_attr(not(feature = "admin"), allow(dead_code))]
    Mcp,
    #[cfg_attr(not(feature = "admin"), allow(dead_code))]
    Secrets,
    #[cfg_attr(not(feature = "admin"), allow(dead_code))]
    Inventory,
}

impl Screen {
    /// One of the rail's products other than 이상 행위 탐지.
    pub fn is_product(self) -> bool {
        matches!(self, Screen::Mcp | Screen::Secrets | Screen::Inventory)
    }
}

/// The window's size and smallest size for the work and the main page.
pub const WORK_SIZE: (f64, f64) = (1440.0, 900.0);
pub const WORK_MIN: (f64, f64) = (1000.0, 620.0);
/// The setup's small window.
pub const SETUP_SIZE: (f64, f64) = (560.0, 760.0);

#[derive(Clone, Copy)]
pub struct AppState {
    pub public: Signal<Public>,
    pub prefs: Signal<Prefs>,
    pub sessions: Signal<Load<Vec<Session>>>,
    pub current: Signal<Option<String>>,
    pub graph: Signal<Load<Arc<Model>>>,
    /// The session `graph` was read for: it stays on screen while the next one loads.
    pub graph_of: Signal<Option<String>>,
    /// The setup, the main page, or the work.
    pub screen: Signal<Screen>,
    /// The sidebar is open (it folds away from its own button).
    pub sidebar: Signal<bool>,
    /// The graph or the comparison.
    pub mode: Signal<Mode>,
    pub selected: Signal<Selection>,
    /// The inspector beside the map: closed until the route's detail asks for it.
    pub inspect: Signal<bool>,
    pub call_details_open: Signal<bool>,
    /// A node for the canvas to bring into view.
    pub focus: Signal<Option<i64>>,
    pub settings_open: Signal<bool>,
    pub live: Signal<bool>,
    pub inspector_width: Signal<f64>,
    /// Bumped by the menu to look for an update.
    pub check_updates: Signal<u32>,
    /// Bumped to read the sessions, the graph, or fit the canvas again.
    pub reload: Signal<u32>,
    pub refresh: Signal<u32>,
    pub fit: Signal<u32>,
}

impl AppState {
    /// Opens a node of the session on screen: selects it and brings it into view.
    pub fn go_to(mut self, seq: i64) {
        self.selected.set(Selection::Node(seq));
        self.focus.set(Some(seq));
    }

    pub fn show_call(mut self, seq: i64) {
        self.selected.set(Selection::Node(seq));
        self.call_details_open.set(true);
    }

    /// Opens a Call of any session in the graph, from the comparison.
    pub fn open_call(mut self, session: String, seq: i64) {
        self.mode.set(Mode::Graph);
        self.open_session(session);
        self.go_to(seq);
    }

    pub fn open_session(mut self, id: String) {
        if self.current.peek().as_deref() == Some(id.as_str()) {
            return;
        }
        self.call_details_open.set(false);
        self.selected.set(Selection::None);
        self.prefs.write().last_session = Some(id.clone());
        self.current.set(Some(id));
    }

    pub fn toggle_demo(mut self, backend: &Backend) {
        let demo = !self.public.peek().demo;
        if let Ok(next) = backend.set_demo(demo) {
            self.public.set(next);
            self.current.set(None);
            self.selected.set(Selection::None);
            self.reload += 1;
        }
    }
}

#[component]
pub fn App() -> Element {
    let backend = use_context::<Arc<Backend>>();
    let prefs = use_hook(Prefs::load);
    let width = prefs.inspector_width.unwrap_or(INSPECTOR_WIDTH.1);
    let mut state = use_context_provider(|| AppState {
        public: Signal::new(backend.settings()),
        prefs: Signal::new(prefs),
        sessions: Signal::new(Load::Loading),
        current: Signal::new(None),
        graph: Signal::new(Load::Loading),
        graph_of: Signal::new(None),
        screen: Signal::new(if backend.needs_setup() {
            Screen::Setup
        } else {
            first_screen()
        }),
        sidebar: Signal::new(true),
        mode: Signal::new(Mode::Graph),
        selected: Signal::new(Selection::None),
        inspect: Signal::new(false),
        call_details_open: Signal::new(false),
        focus: Signal::new(None),
        settings_open: Signal::new(false),
        live: Signal::new(false),
        inspector_width: Signal::new(width),
        check_updates: Signal::new(0),
        reload: Signal::new(0),
        refresh: Signal::new(0),
        fit: Signal::new(0),
    });
    // The session list, opened from the session's name in the top bar.
    use_context_provider(|| flow::Picking(Signal::new(false)));
    use_loaders(backend.clone(), state);
    #[cfg(feature = "admin")]
    crate::admin::use_facts_loader(backend.clone(), state);
    use_menu(backend.clone(), state);
    use_window_place(state);

    // Dragging the line between the graph and the inspector sets the inspector's width.
    let mut dragging = use_signal(|| false);
    let drag = move |e: MouseEvent| {
        if !dragging() {
            return;
        }
        let window = dioxus::desktop::window();
        let scale = window.scale_factor();
        let right = window.inner_size().to_logical::<f64>(scale).width;
        let width = (right - e.client_coordinates().x).clamp(INSPECTOR_WIDTH.0, INSPECTOR_WIDTH.2);
        state.inspector_width.set(width);
    };
    let drop = move |_| {
        if dragging() {
            dragging.set(false);
            state.prefs.write().inspector_width = Some((state.inspector_width)());
        }
    };

    match (state.screen)() {
        Screen::Setup => return rsx! { setup::Setup {} },
        Screen::Home => {
            return rsx! {
                div { class: "app home-on",
                    TopBar {}
                    div { class: "app-body",
                        // The security team's main page is wide: a thin rail in the list's place.
                        if cfg!(feature = "admin") { Rail {} } else { Sidebar {} }
                        div { class: "app-main",
                            update::UpdateBanner {}
                            MainPage {}
                        }
                    }
                    if (state.settings_open)() {
                        settings::SettingsSheet {}
                    }
                }
            };
        }
        Screen::FeatureA | Screen::FeatureB | Screen::Mcp | Screen::Secrets | Screen::Inventory => {
            let screen = (state.screen)();
            return rsx! {
                div { class: "app",
                    TopBar {}
                    div { class: "app-body",
                        if cfg!(feature = "admin") { Rail {} }
                        div { class: "app-main",
                            if screen.is_product() {
                                ProductPage { key: "{screen:?}", screen }
                            } else {
                                FeaturePage { second: screen == Screen::FeatureB }
                            }
                        }
                    }
                    if (state.settings_open)() {
                        settings::SettingsSheet {}
                    }
                }
            };
        }
        Screen::Work => {}
    }

    rsx! {
        div {
            class: if dragging() { "app resizing" } else { "app" },
            onmousemove: drag,
            onmouseup: drop,
            onmouseleave: drop,
            TopBar {}
            div { class: "app-body",
            if (state.mode)() == Mode::Compare { Sidebar {} }
            div { class: "app-main",
            update::UpdateBanner {}
            if (state.mode)() == Mode::Compare {
                compare::Compare {}
            }
            div { class: "columns", hidden: (state.mode)() == Mode::Compare,
                main { class: "stage",
                    div { class: "canvas-wrap",
                        flow::FlowMap {}
                        StageNotice {}
                    }
                }
                if (state.inspect)() {
                div {
                    class: "splitter",
                    title: "끌어서 폭 조절 · 두 번 누르면 기본 폭",
                    onmousedown: move |e| {
                        e.prevent_default();
                        dragging.set(true);
                    },
                    ondoubleclick: move |_| {
                        state.inspector_width.set(INSPECTOR_WIDTH.1);
                        state.prefs.write().inspector_width = None;
                    },
                }
                inspector::Inspector {}
                }
            }
            }
            }
            if (state.settings_open)() {
                settings::SettingsSheet {}
            }
        }
    }
}

/// The strip where the title bar was: the traffic lights sit at its left, and it moves the
/// window (a double click zooms it, as a title bar does). Over the page when the page runs to
/// the top (the main page, the setup), else a band of its own above the work.
#[component]
pub fn TitleBar(overlay: bool) -> Element {
    rsx! {
        div {
            class: if overlay { "titlebar overlay" } else { "titlebar" },
            onmousedown: move |_| dioxus::desktop::window().drag(),
            ondoubleclick: move |_| {
                let window = dioxus::desktop::window();
                window.set_maximized(!window.is_maximized());
            },
        }
    }
}

/// The inspector's narrowest, default and widest width.
const INSPECTOR_WIDTH: (f64, f64, f64) = (300.0, 380.0, 760.0);

/// What the menu items and their shortcuts do.
fn use_menu(backend: Arc<Backend>, mut state: AppState) {
    dioxus::desktop::use_muda_event_handler(move |event| match event.id().as_ref() {
        menu::SETTINGS => state.settings_open.toggle(),
        menu::RELOAD => {
            state.reload += 1;
            state.refresh += 1;
        }
        menu::SEARCH => {
            document::eval(
                "const s = document.getElementById('session-search'); s?.focus(); s?.select();",
            );
        }
        menu::FIT => state.fit += 1,
        menu::DEMO => state.toggle_demo(&backend),
        menu::CHECK_UPDATES => state.check_updates += 1,
        _ => {}
    });
}

/// Puts the window where it was last time, if that place is still on a screen, and remembers
/// where it goes: once it has stayed put for half a second.
fn use_window_place(mut state: AppState) {
    use dioxus::desktop::tao::event::{Event, WindowEvent};
    use dioxus::desktop::{LogicalPosition, LogicalSize};

    use_hook(move || {
        if *state.screen.peek() == Screen::Setup {
            return;
        }
        let Some(place) = state.prefs.peek().window else {
            return;
        };
        let window = dioxus::desktop::window();
        let on_screen = window.available_monitors().any(|m| {
            let scale = m.scale_factor();
            let at = m.position().to_logical::<f64>(scale);
            let size = m.size().to_logical::<f64>(scale);
            place.x + 80.0 > at.x
                && place.x < at.x + size.width - 80.0
                && place.y >= at.y - 10.0
                && place.y < at.y + size.height - 80.0
        });
        window.set_inner_size(LogicalSize::new(place.width, place.height));
        if on_screen {
            window.set_outer_position(LogicalPosition::new(place.x, place.y));
        }
    });

    let mut moves = use_signal(|| 0u64);
    dioxus::desktop::use_wry_event_handler(move |event, _| {
        if !matches!(
            event,
            Event::WindowEvent {
                event: WindowEvent::Moved(_) | WindowEvent::Resized(_),
                ..
            }
        ) {
            return;
        }
        if *state.screen.peek() == Screen::Setup {
            return;
        }
        moves += 1;
        let seen = *moves.peek();
        spawn(async move {
            tokio::time::sleep(Duration::from_millis(500)).await;
            if *moves.peek() != seen {
                return;
            }
            let window = dioxus::desktop::window();
            if window.is_minimized() || window.fullscreen().is_some() || window.is_maximized() {
                return;
            }
            let scale = window.scale_factor();
            let (Ok(at), size) = (window.outer_position(), window.inner_size()) else {
                return;
            };
            let at = at.to_logical::<f64>(scale);
            let size = size.to_logical::<f64>(scale);
            let place = WindowPlace {
                x: at.x,
                y: at.y,
                width: size.width,
                height: size.height,
            };
            if state.prefs.peek().window != Some(place) {
                state.prefs.write().window = Some(place);
            }
        });
    });
}

/// Reads the sessions and the open session's graph whenever what they depend on changes.
fn use_loaders(backend: Arc<Backend>, mut state: AppState) {
    // Sessions: on launch, on reload, after the settings change.
    let b = backend.clone();
    use_effect(move || {
        let _ = (state.reload)();
        let _ = (state.public)();
        let b = b.clone();
        spawn(async move {
            let found = b.sessions().await;
            let mut current = state.current;
            match found {
                Ok(list) => {
                    // Keep the open session; else the last one opened, else the newest.
                    let open = current
                        .peek()
                        .clone()
                        .filter(|id| list.iter().any(|s| &s.id == id));
                    let last = state
                        .prefs
                        .peek()
                        .last_session
                        .clone()
                        .filter(|id| list.iter().any(|s| &s.id == id));
                    let pick = open.or(last).or_else(|| list.first().map(|s| s.id.clone()));
                    if *current.peek() != pick {
                        current.set(pick);
                    }
                    if *state.sessions.peek() != Load::Ready(list.clone()) {
                        state.sessions.set(Load::Ready(list));
                    }
                }
                Err(e) => {
                    current.set(None);
                    state.sessions.set(Load::Failed(e));
                }
            }
        });
    });

    // The graph: when another session opens, or on refresh.
    let b = backend.clone();
    use_effect(move || {
        let id = (state.current)();
        let _ = (state.refresh)();
        let b = b.clone();
        let Some(id) = id else {
            state.graph.set(Load::Loading);
            return;
        };
        if !matches!(*state.graph.peek(), Load::Ready(_)) {
            state.graph.set(Load::Loading);
        }
        spawn(async move {
            let found = b.graph(&id).await.map(|data| Arc::new(Model::build(&data)));
            if state.current.peek().as_deref() != Some(id.as_str()) {
                return;
            }
            let next = match found {
                Ok(model) => Load::Ready(model),
                Err(e) => Load::Failed(e),
            };
            if *state.graph.peek() != next {
                state.graph.set(next);
            }
            if state.graph_of.peek().as_deref() != Some(id.as_str()) {
                state.graph_of.set(Some(id));
            }
        });
    });

    // Live: reads again every 3 seconds while it is on.
    use_future(move || async move {
        loop {
            tokio::time::sleep(Duration::from_secs(3)).await;
            if *state.live.peek() {
                state.reload += 1;
                state.refresh += 1;
            }
        }
    });

    // What to remember between launches.
    use_effect(move || state.prefs.read().save());
}

fn logo_source() -> &'static str {
    use base64::Engine;
    static LOGO: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    LOGO.get_or_init(|| {
        format!(
            "data:image/png;base64,{}",
            base64::engine::general_purpose::STANDARD
                .encode(include_bytes!("../../icons/Vienna.png"))
        )
    })
}

/// The bar across the top, as a design app keeps one: the traffic lights at its left, the
/// sidebar's button, the mark, then the screens as one segmented control whose chosen screen
/// takes its own colour (the main page lime, the route map cyan, the comparison salmon); at its
/// right where the data comes from and the settings. It moves the window, and a double click
/// zooms it, as a title bar does.
#[component]
fn TopBar() -> Element {
    let logo = logo_source();
    let mut state = use_context::<AppState>();
    let p = (state.public)();
    let screen = (state.screen)();
    let mode = (state.mode)();
    let cur = match (screen, mode) {
        (Screen::Home, _) => "home",
        (Screen::FeatureA, _) => "a",
        (Screen::FeatureB, _) => "b",
        // The rail's other products sit under the main page in the top bar.
        (s, _) if s.is_product() => "home",
        _ => "graph",
    };
    let (dot, source) = match (p.demo, &*state.sessions.read()) {
        (true, _) => ("demo", "데모 데이터".to_owned()),
        (false, Load::Ready(_)) => (
            "ok",
            match p.engine {
                crate::settings::Engine::Local => "이 맥의 엔진".to_owned(),
                _ => p.host(),
            },
        ),
        (false, Load::Failed(_)) => ("down", "연결 안 됨".to_owned()),
        (false, Load::Loading) => ("wait", "연결하는 중".to_owned()),
    };
    let mut go = move |s: Screen, m: Mode| {
        state.mode.set(m);
        state.screen.set(s);
    };
    let seg = |key: &'static str| if cur == key { "on" } else { "" };
    rsx! {
        header {
            class: "topbar on-{cur}",
            onmousedown: move |_| dioxus::desktop::window().drag(),
            ondoubleclick: move |_| {
                let window = dioxus::desktop::window();
                window.set_maximized(!window.is_maximized());
            },
            div { class: "tb-lights" }
            button {
                class: "tb-icon",
                // Only the developer's main page has the sidebar to fold.
                hidden: screen != Screen::Home || cfg!(feature = "admin"),
                title: if (state.sidebar)() { "사이드바 접기" } else { "사이드바 보기" },
                onmousedown: move |e| e.stop_propagation(),
                ondoubleclick: move |e| e.stop_propagation(),
                onclick: move |_| state.sidebar.toggle(),
                {sidebar_icon()}
            }
            img { class: "tb-logo", src: "{logo}", alt: "Vienna", draggable: "false" }
            div { class: "tb-seg", onmousedown: move |e| e.stop_propagation(), ondoubleclick: move |e| e.stop_propagation(),
                button { class: "{seg(\"home\")}", onclick: move |_| go(Screen::Home, mode),
                    {line_icon("M4 11l8-6.5L20 11v8.5a1 1 0 0 1-1 1h-4.5V15h-5v5.5H5a1 1 0 0 1-1-1z")}
                    "{HOME_LABEL}"
                }
                button { class: "{seg(\"graph\")}", onclick: move |_| go(Screen::Work, Mode::Graph),
                    {line_icon("M6 20V11a3 3 0 0 1 3-3h9M14 4l4 4-4 4")}
                    "{SCREEN_LABELS.0}"
                }
                button { class: "{seg(\"a\")}", onclick: move |_| go(Screen::FeatureA, Mode::Graph),
                    {line_icon("M4 4.5h7v7H4zM13 4.5h7v7h-7zM4 13.5h7v7H4zM13 13.5h7v7h-7z")}
                    "{SCREEN_LABELS.1}"
                }
                button { class: "{seg(\"b\")}", onclick: move |_| go(Screen::FeatureB, Mode::Graph),
                    {line_icon("M12 3.5l2.4 5.3 5.6.6-4.2 3.8 1.2 5.6L12 16l-5 2.8 1.2-5.6L4 9.4l5.6-.6z")}
                    "{SCREEN_LABELS.2}"
                }
            }
            // On the route map, the session looked at, opening the list to switch.
            if screen == Screen::Work && mode == Mode::Graph {
                div { class: "tb-session", onmousedown: move |e| e.stop_propagation(), ondoubleclick: move |e| e.stop_propagation(),
                    flow::SessionPick { class: "tb-pick" }
                }
            }
            div { class: "tb-spacer" }
            span { class: "tb-status", i { class: "sb-dot {dot}" } "{source}" }
            button {
                class: "tb-btn",
                onmousedown: move |e| e.stop_propagation(),
                ondoubleclick: move |e| e.stop_propagation(),
                onclick: move |_| state.settings_open.set(true),
                "연결 설정…"
            }
        }
    }
}

/// An outline icon in the current colour.
fn line_icon(d: &'static str) -> Element {
    rsx! {
        svg { width: "16", height: "16", view_box: "0 0 24 24",
            path { d, fill: "none", stroke: "currentColor", stroke_width: "1.8", stroke_linecap: "round", stroke_linejoin: "round" }
        }
    }
}

/// The sidebar, as a design app's panels: the latest sessions under a tab bar, then where the
/// data comes from and the settings. The row open is blue.
#[component]
fn Sidebar() -> Element {
    let backend = use_context::<Arc<Backend>>();
    let mut state = use_context::<AppState>();
    let p = (state.public)();
    let screen = (state.screen)();
    if !(state.sidebar)() {
        return rsx! {};
    }
    let recent: Vec<Session> = match &*state.sessions.read() {
        Load::Ready(list) => list.iter().take(14).cloned().collect(),
        _ => Vec::new(),
    };
    let current = (state.current)();
    rsx! {
        nav { class: "sidebar",
            div { class: "sb-tabs", i { class: "sb-grip" } span { class: "sb-tab on", "최근 세션" } }
            div { class: "sb-list sb-recent",
                if recent.is_empty() {
                    p { class: "sb-empty", "세션이 없습니다" }
                }
                for s in recent {
                    {
                        let id = s.id.clone();
                        let tone = if s.denied > 0 { "red" } else if s.reviewed > 0 { "amber" } else { "plain" };
                        let on = screen == Screen::Work && current.as_deref() == Some(id.as_str());
                        rsx! {
                            button {
                                key: "{id}",
                                class: if on { "sb-row on" } else { "sb-row" },
                                title: "{s.title()}",
                                onclick: move |_| {
                                    state.open_session(id.clone());
                                    state.mode.set(Mode::Graph);
                                    state.screen.set(Screen::Work);
                                },
                                span { class: "sb-doc {tone}" }
                                span { class: "sb-label", "{s.title()}" }
                            }
                        }
                    }
                }
            }
            div { class: "sb-tabs", i { class: "sb-grip" } span { class: "sb-tab on", "데이터" } }
            div { class: "sb-list",
                SbRow {
                    label: if p.demo { "데모 데이터 · 엔진 데이터로" } else { "엔진 데이터 · 데모로" },
                    icon: "M5 6c0-1.4 3.1-2.6 7-2.6s7 1.2 7 2.6-3.1 2.6-7 2.6S5 7.4 5 6zM5 6v12c0 1.4 3.1 2.6 7 2.6s7-1.2 7-2.6V6M5 12c0 1.4 3.1 2.6 7 2.6s7-1.2 7-2.6",
                    on: false,
                    onclick: move |_| state.toggle_demo(&backend),
                }
                SbRow { label: "연결 설정", icon: "M12 8.8a3.2 3.2 0 1 0 0 6.4 3.2 3.2 0 0 0 0-6.4zM12 2.8v2.6M12 18.6v2.6M2.8 12h2.6M18.6 12h2.6M5.5 5.5l1.8 1.8M16.7 16.7l1.8 1.8M5.5 18.5l1.8-1.8M16.7 7.3l1.8-1.8", on: false, onclick: move |_| state.settings_open.set(true) }
            }
        }
    }
}

/// A thin rail of icons, as a console's: its products first (이상 행위 탐지, MCP 사용, 민감 정보,
/// and the inventory to come), then read again, fill the screen, switch the data, the connection.
#[component]
fn Rail() -> Element {
    let backend = use_context::<Arc<Backend>>();
    let mut state = use_context::<AppState>();
    let demo = (state.public)().demo;
    let screen = (state.screen)();
    let on = move |s: Screen| if screen == s { "product on" } else { "product" };
    let on_detect = !screen.is_product();
    let icon = |d: &'static str| {
        rsx! {
            svg { width: "18", height: "18", view_box: "0 0 24 24",
                path { d, fill: "none", stroke: "currentColor", stroke_width: "1.7", stroke_linecap: "round", stroke_linejoin: "round" }
            }
        }
    };
    rsx! {
        nav { class: "rail",
            // As tall as the pages' header strip, so the icons start beside the tabs under it;
            // where the page has the strip, its line runs on across the rail.
            div { class: if matches!(screen, Screen::Home) || screen.is_product() { "rail-head line" } else { "rail-head" } }
            button {
                class: if on_detect { "product on" } else { "product" },
                title: "이상 행위 탐지 · 메인 · 기록 · 감사 · 정책",
                onclick: move |_| if !on_detect { state.screen.set(Screen::Home) },
                {icon("M12 3l7.5 3v5.5c0 4.6-3.2 8.3-7.5 9.5-4.3-1.2-7.5-4.9-7.5-9.5V6zM9 12l2.2 2.2L15.5 10")}
            }
            button {
                class: on(Screen::Mcp),
                title: "MCP 사용 현황",
                onclick: move |_| state.screen.set(Screen::Mcp),
                {icon("M9 3.5v4M15 3.5v4M6.5 7.5h11v3.5a5.5 5.5 0 0 1-11 0zM12 16.5v4")}
            }
            button {
                class: on(Screen::Secrets),
                title: "민감 정보: 비밀값·개인정보의 흐름",
                onclick: move |_| state.screen.set(Screen::Secrets),
                {icon("M7 10.5V8a5 5 0 0 1 10 0v2.5M5.5 10.5h13v9.5h-13zM12 14.5v2")}
            }
            button {
                class: on(Screen::Inventory),
                title: "인벤토리: 기기 · 사용자 · 세션, 키 발급과 회수",
                onclick: move |_| state.screen.set(Screen::Inventory),
                {icon("M4 6.5h16M4 12h16M4 17.5h16M7 4.5v4M7 10v4M7 15.5v4")}
            }
            i { class: "sep" }
            button { title: "다시 읽기", onclick: move |_| state.reload += 1,
                {icon("M19.5 12a7.5 7.5 0 1 1-2.2-5.3M19.5 4.5v4h-4")}
            }
            button {
                title: "화면 채우기",
                onclick: move |_| {
                    let window = dioxus::desktop::window();
                    window.set_maximized(!window.is_maximized());
                },
                {icon("M4.5 9V4.5H9M15 4.5h4.5V9M19.5 15v4.5H15M9 19.5H4.5V15")}
            }
            span { class: "fill" }
            button {
                class: if demo { "demo" } else { "" },
                title: if demo { "데모 데이터를 보는 중: 엔진 데이터로 바꾸기" } else { "엔진 데이터를 보는 중: 데모 데이터로 바꾸기" },
                onclick: move |_| state.toggle_demo(&backend),
                {icon("M5 6c0-1.4 3.1-2.6 7-2.6s7 1.2 7 2.6-3.1 2.6-7 2.6S5 7.4 5 6zM5 6v12c0 1.4 3.1 2.6 7 2.6s7-1.2 7-2.6V6M5 12c0 1.4 3.1 2.6 7 2.6s7-1.2 7-2.6")}
            }
            button { title: "연결 설정", onclick: move |_| state.settings_open.set(true),
                {icon("M12 8.8a3.2 3.2 0 1 0 0 6.4 3.2 3.2 0 0 0 0-6.4zM12 2.8v2.6M12 18.6v2.6M2.8 12h2.6M18.6 12h2.6M5.5 5.5l1.8 1.8M16.7 16.7l1.8 1.8M5.5 18.5l1.8-1.8M16.7 7.3l1.8-1.8")}
            }
        }
    }
}

/// The sidebar's own icon: a window with its left pane.
fn sidebar_icon() -> Element {
    rsx! {
        svg { width: "18", height: "18", view_box: "0 0 24 24",
            rect { x: "3", y: "5", width: "18", height: "14", rx: "3", fill: "none", stroke: "currentColor", stroke_width: "1.7" }
            path { d: "M9 5v14M5.5 8.5h1.5M5.5 11h1.5", fill: "none", stroke: "currentColor", stroke_width: "1.7", stroke_linecap: "round" }
        }
    }
}

/// One row of the sidebar: an outline icon and its name.
#[component]
fn SbRow(
    label: String,
    icon: &'static str,
    on: bool,
    onclick: EventHandler<MouseEvent>,
) -> Element {
    rsx! {
        button { class: if on { "sb-row on" } else { "sb-row" }, onclick: move |e| onclick.call(e),
            svg { width: "17", height: "17", view_box: "0 0 24 24",
                path { d: icon, fill: "none", stroke: "currentColor", stroke_width: "1.7", stroke_linecap: "round", stroke_linejoin: "round" }
            }
            span { class: "sb-label", "{label}" }
        }
    }
}

/// A card over the canvas when there is nothing to draw: the source failed, it has no
/// sessions, or the graph is on its way.
#[component]
fn StageNotice() -> Element {
    let backend = use_context::<Arc<Backend>>();
    let mut state = use_context::<AppState>();
    let demo = (state.public)().demo;
    let first_run = (state.public)().first_run;
    let sessions = (state.sessions)();
    let graph = (state.graph)();
    let retry = move |_| state.reload += 1;
    let b = backend.clone();
    let demo_button = rsx! {
        if !demo {
            button { onclick: move |_| state.toggle_demo(&b), "데모로 둘러보기" }
        }
    };
    match (sessions, graph) {
        // Nothing was set up yet: a greeting, not a failed connection.
        (Load::Failed(_), _) if first_run && !demo => rsx! {
            Notice {
                title: "WithHuman Viewer에 오신 걸 환영합니다",
                message: "WithHuman 엔진이 기록한 에이전트 세션을 그래프로 봅니다. 엔진의 데이터베이스(PostgreSQL)에 연결하거나, 엔진 없이 데모 데이터로 먼저 둘러보세요.",
                button { class: "primary", onclick: move |_| state.settings_open.set(true), "엔진에 연결" }
                {demo_button}
            }
        },
        (Load::Failed(message), _) => rsx! {
            Notice { title: if demo { "데모 데이터를 읽지 못했습니다" } else { "엔진 데이터베이스에 연결할 수 없습니다" }, message,
                button { class: "primary", onclick: move |_| state.settings_open.set(true), "연결 설정" }
                {demo_button}
                button { onclick: retry, "다시 시도" }
            }
        },
        (Load::Loading, _) => rsx! { div { class: "loading", "불러오는 중…" } },
        (Load::Ready(list), _) if list.is_empty() => rsx! {
            Notice {
                title: "아직 세션이 없습니다",
                message: "에이전트를 WithHuman에 연결해 실행하면 세션이 여기에 나타납니다.",
                {demo_button}
                button { onclick: retry, "새로고침" }
            }
        },
        (_, Load::Failed(message)) => rsx! {
            Notice { title: "그래프를 불러오지 못했습니다", message,
                button { onclick: move |_| state.refresh += 1, "다시 시도" }
            }
        },
        (_, Load::Loading) => rsx! { div { class: "loading", "그래프를 불러오는 중…" } },
        _ => rsx! {},
    }
}

#[component]
fn Notice(title: String, message: String, children: Element) -> Element {
    rsx! {
        div { class: "notice-wrap",
            div { class: "notice",
                h2 { "{title}" }
                p { "{message}" }
                div { class: "actions", {children} }
            }
        }
    }
}
