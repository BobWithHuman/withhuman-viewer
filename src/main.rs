//! WithHuman Viewer: the execution graph the WithHuman engine records, read from Neo4j (or a
//! bundled demo) in a desktop app built with Dioxus.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod api;
mod demo;
mod engine;
mod flow;
mod graph;
mod layout;
mod menu;
mod metro;
mod model;
mod prefs;
mod settings;
mod ui;
mod updater;

use std::sync::Arc;

use dioxus::desktop::{Config, LogicalSize, WindowBuilder};

fn main() {
    let backend = Arc::new(api::Backend::load());
    // The first launch opens small, on the setup; the app grows to its full size after it.
    let setup = backend.needs_setup();
    let (size, min) = if setup {
        (ui::SETUP_SIZE, ui::SETUP_SIZE)
    } else {
        (ui::WORK_SIZE, ui::WORK_MIN)
    };
    let window = WindowBuilder::new()
        .with_title("WithHuman Viewer")
        .with_inner_size(LogicalSize::new(size.0, size.1))
        .with_min_inner_size(LogicalSize::new(min.0, min.1))
        .with_resizable(!setup);
    // No title bar: the content runs to the top, the traffic lights over it as in a Mac app.
    #[cfg(target_os = "macos")]
    let window = {
        use dioxus::desktop::tao::platform::macos::WindowBuilderExtMacOS;
        window
            .with_titlebar_transparent(true)
            .with_title_hidden(true)
            .with_fullsize_content_view(true)
    };
    let head = format!("<meta charset=\"utf-8\"><style>{}</style>", ui::CSS);
    let config = Config::new()
        .with_window(window)
        .with_menu(menu::build())
        .with_custom_head(head);
    dioxus::LaunchBuilder::desktop()
        .with_cfg(config)
        .with_context(backend)
        .launch(ui::App);
}
