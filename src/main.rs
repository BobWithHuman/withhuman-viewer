//! WithHuman Viewer: the execution graph the WithHuman engine records, read from Neo4j (or a
//! bundled demo) in a desktop app built with Dioxus.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(all(feature = "personal", feature = "admin"))]
compile_error!(
    "build one app: the personal app (`personal`) or the security team's (`admin`), not both"
);
#[cfg(not(any(feature = "personal", feature = "admin")))]
compile_error!("build one app: `--features personal` or `--no-default-features --features admin`");

#[cfg(feature = "admin")]
mod admin;
mod api;
mod demo;
// Running the engine on this Mac, from the security team's first launch.
#[cfg_attr(not(feature = "admin"), allow(dead_code))]
mod engine;
mod flow;
mod graph;
// The developer's hooks, installed from the app with the engine's own setup.
#[cfg(not(feature = "admin"))]
mod hooks;
mod layout;
// Each client ID's machine, by serial, else host name, from the inventory.
#[cfg_attr(not(feature = "admin"), allow(dead_code))]
mod machines;
mod menu;
mod metro;
mod model;
// The personal app's notifications of refused and held Calls.
#[cfg(not(feature = "admin"))]
mod notify;
// The developer's own engine, run by the app's binary with `--engine`.
#[cfg(not(feature = "admin"))]
mod own;
mod pg;
mod prefs;
mod settings;
// The personal app's try: a made-up session sent through its own engine.
#[cfg(not(feature = "admin"))]
mod trial;
mod ui;
mod updater;

use std::sync::Arc;

use dioxus::desktop::{Config, LogicalSize, WindowBuilder};

fn main() {
    #[cfg(not(feature = "admin"))]
    if std::env::args().nth(1).as_deref() == Some("--engine") {
        std::process::exit(own::serve());
    }
    api::set_launch(std::env::args().skip(1));
    let backend = Arc::new(api::Backend::load());
    // The developer's own engine runs whenever the app is open, and after it on a Mac.
    #[cfg(not(feature = "admin"))]
    if backend.settings().engine == settings::Engine::Own {
        std::thread::spawn(|| {
            if let Err(e) = own::ensure_started() {
                eprintln!("withhuman engine: {e}");
            }
        });
    }
    // The first launch opens small, on the setup; the app grows to its full size after it.
    let setup = backend.needs_setup();
    let (size, min) = if setup {
        (ui::SETUP_SIZE, ui::SETUP_SIZE)
    } else {
        (ui::WORK_SIZE, ui::WORK_MIN)
    };
    let window = WindowBuilder::new()
        .with_title(ui::APP_NAME)
        .with_inner_size(LogicalSize::new(size.0, size.1))
        .with_min_inner_size(LogicalSize::new(min.0, min.1))
        // A Mac keeps the setup's small window as it is; elsewhere a window that cannot be
        // resized cannot be sized back from maximized.
        .with_resizable(!setup || !cfg!(target_os = "macos"));
    // No title bar: the content runs to the top, the traffic lights over it as in a Mac app.
    #[cfg(target_os = "macos")]
    let window = {
        use dioxus::desktop::tao::platform::macos::WindowBuilderExtMacOS;
        window
            .with_titlebar_transparent(true)
            .with_title_hidden(true)
            .with_fullsize_content_view(true)
    };
    // Elsewhere the window keeps its own title bar: no room is left for traffic lights.
    let native_bar = if cfg!(target_os = "macos") {
        ""
    } else {
        "<style>.titlebar, .tb-lights { display: none; }</style>"
    };
    let head = format!(
        "<meta charset=\"utf-8\"><style>{}</style>{native_bar}{}",
        ui::CSS,
        ui::head_extra()
    );
    let config = Config::new()
        .with_window(window)
        .with_menu(menu::build())
        .with_custom_head(head);
    // The personal app stays in the menu bar on a Mac: closing the window hides it, ⌘Q quits.
    // The menu bar icon opens its menu; the window comes back from the menu's 기록 열기.
    #[cfg(all(not(feature = "admin"), target_os = "macos"))]
    let config = config
        .with_close_behaviour(dioxus::desktop::WindowCloseBehaviour::WindowHides)
        .with_tray_icon_show_window_on_click(false);
    dioxus::LaunchBuilder::desktop()
        .with_cfg(config)
        .with_context(backend)
        .launch(ui::App);
}
