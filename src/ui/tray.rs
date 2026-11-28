//! The personal app in the menu bar. Its icon is the app's mark, whose right-hand node is filled
//! while the engine answers and hollow while it does not; its menu has today's counts, the last
//! sessions (each opens), 기록 열기, 설정 and 종료. Closing the window leaves the app here, the
//! engine running behind it.

use std::cell::RefCell;
use std::rc::Rc;

use dioxus::desktop::muda::{Menu, MenuItem, PredefinedMenuItem};
use dioxus::desktop::trayicon::{Icon, TrayIcon, TrayIconBuilder};
use dioxus::prelude::*;

use super::live::{DENIED, REVIEWED, bring_up, clip};
use super::{AppState, Load, Mode, Screen};
use crate::model::Session;

/// The menu's own items, by id: the app's menu bar handles them with its own.
pub const SHOW: &str = "tray-show";
pub const SETTINGS: &str = "tray-settings";
const SESSION: &str = "tray-session:";

/// Sessions listed in the menu.
const RECENT: usize = 5;

/// What the menu and the icon show.
#[derive(Clone, Debug, PartialEq)]
pub struct Summary {
    pub up: bool,
    pub calls: i64,
    pub denied: i64,
    pub reviewed: i64,
    /// The sessions last active, the latest first: each one's ID and name.
    pub recent: Vec<(String, String)>,
}

/// The counts of the sessions active since `since` (today's start), and the last sessions.
pub fn summary(list: &[Session], up: bool, since: i64) -> Summary {
    let at = |s: &Session| s.last_at.or(s.created_at).unwrap_or(0);
    let today: Vec<&Session> = list.iter().filter(|s| at(s) >= since).collect();
    let mut recent: Vec<&Session> = list.iter().collect();
    recent.sort_by_key(|s| std::cmp::Reverse(at(s)));
    Summary {
        up,
        calls: today.iter().map(|s| s.calls).sum(),
        denied: today.iter().map(|s| s.denied).sum(),
        reviewed: today.iter().map(|s| s.reviewed).sum(),
        recent: recent
            .into_iter()
            .take(RECENT)
            .map(|s| {
                let title = Some(s.title())
                    .filter(|t| !t.trim().is_empty())
                    .unwrap_or("세션");
                (s.id.clone(), clip(title, 40))
            })
            .collect(),
    }
}

/// The start of today, here, in milliseconds.
fn today_start() -> i64 {
    use chrono::{Local, TimeZone};
    let midnight = Local::now().date_naive().and_hms_opt(0, 0, 0);
    midnight
        .and_then(|m| Local.from_local_datetime(&m).earliest())
        .map_or(0, |t| t.timestamp_millis())
}

fn menu(s: &Summary) -> Menu {
    let menu = Menu::new();
    let line = |text: String| MenuItem::new(text, false, None);
    let _ = menu.append_items(&[
        &line(
            if s.up {
                "엔진 켜짐"
            } else {
                "엔진 꺼짐"
            }
            .into(),
        ),
        &line(format!(
            "오늘 호출 {} · {DENIED} {} · {REVIEWED} {}",
            s.calls, s.denied, s.reviewed
        )),
        &PredefinedMenuItem::separator(),
    ]);
    for (id, title) in &s.recent {
        let _ = menu.append(&MenuItem::with_id(
            format!("{SESSION}{id}"),
            title,
            true,
            None,
        ));
    }
    if !s.recent.is_empty() {
        let _ = menu.append(&PredefinedMenuItem::separator());
    }
    let _ = menu.append_items(&[
        &MenuItem::with_id(SHOW, "기록 열기", true, None),
        &MenuItem::with_id(SETTINGS, "설정…", true, None),
        &PredefinedMenuItem::separator(),
        &PredefinedMenuItem::quit(Some("종료")),
    ]);
    menu
}

/// The icon's side, in pixels: the menu bar's 18 points on a Retina display.
const SIDE: usize = 36;

/// The app's mark (`icons/source.svg`) as a template image: three nodes joined to a middle one,
/// the right-hand node filled while the engine answers, a ring while it does not. Drawn with 4×4
/// samples a pixel, in black with its coverage as alpha, as macOS tints a template.
pub fn mark(up: bool) -> Vec<u8> {
    // The mark's centre and scale in the source's 1024-unit square.
    let (cx, cy, scale) = (515.0, 515.0, 31.0 / 494.0);
    let at = |x: f64, y: f64| {
        (
            (x - cx) * scale + SIDE as f64 / 2.0,
            (y - cy) * scale + SIDE as f64 / 2.0,
        )
    };
    let (top, middle, bottom, right) = (
        at(330.0, 340.0),
        at(512.0, 512.0),
        at(330.0, 690.0),
        at(700.0, 512.0),
    );
    let (node, hub, line, ring) = (4.6, 5.4, 1.5, 1.6);
    // The line to the right-hand node stops at its edge, so a ring stays hollow.
    let right_edge = (right.0 - node, right.1);
    let inside = |x: f64, y: f64| {
        let disk = |c: (f64, f64), r: f64| (x - c.0).hypot(y - c.1) <= r;
        let segment = |a: (f64, f64), b: (f64, f64)| {
            let (dx, dy) = (b.0 - a.0, b.1 - a.1);
            let t = (((x - a.0) * dx + (y - a.1) * dy) / (dx * dx + dy * dy)).clamp(0.0, 1.0);
            (x - a.0 - t * dx).hypot(y - a.1 - t * dy) <= line
        };
        let d = (x - right.0).hypot(y - right.1);
        disk(top, node)
            || disk(bottom, node)
            || disk(middle, hub)
            || segment(top, middle)
            || segment(middle, bottom)
            || segment(middle, right_edge)
            || if up {
                d <= node
            } else {
                d <= node && d >= node - ring
            }
    };
    let mut out = vec![0u8; SIDE * SIDE * 4];
    for py in 0..SIDE {
        for px in 0..SIDE {
            let mut hits = 0;
            for sy in 0..4 {
                for sx in 0..4 {
                    let x = px as f64 + (sx as f64 + 0.5) / 4.0;
                    let y = py as f64 + (sy as f64 + 0.5) / 4.0;
                    hits += u32::from(inside(x, y));
                }
            }
            out[(py * SIDE + px) * 4 + 3] = (hits * 255 / 16) as u8;
        }
    }
    out
}

fn icon(up: bool) -> Option<Icon> {
    Icon::from_rgba(mark(up), SIDE as u32, SIDE as u32).ok()
}

/// Puts the app in the menu bar, and keeps its icon and menu up to date with the sessions read.
pub fn use_tray(state: AppState) {
    let tray = use_hook(|| Rc::new(RefCell::new(None::<TrayIcon>)));
    let shown = use_hook(|| Rc::new(RefCell::new(None::<Summary>)));
    use_effect(move || {
        let sessions = state.sessions.read();
        let (list, up) = match (&*sessions, (state.engine_up)()) {
            (Load::Ready(list), up) => (list.as_slice(), up.unwrap_or(true)),
            _ => (&[][..], false),
        };
        let now = summary(list, up, today_start());
        if shown.borrow().as_ref() == Some(&now) {
            return;
        }
        let mut tray = tray.borrow_mut();
        let tooltip = if now.up {
            "WithHuman · 엔진 켜짐"
        } else {
            "WithHuman · 엔진 꺼짐"
        };
        match tray.as_ref() {
            Some(t) => {
                t.set_menu(Some(Box::new(menu(&now))));
                if shown.borrow().as_ref().is_none_or(|s| s.up != now.up) {
                    let _ = t.set_icon(icon(now.up));
                    t.set_icon_as_template(true);
                }
                let _ = t.set_tooltip(Some(tooltip));
            }
            None => {
                let mut built = TrayIconBuilder::new()
                    .with_menu(Box::new(menu(&now)))
                    .with_icon_as_template(true)
                    .with_tooltip(tooltip);
                if let Some(icon) = icon(now.up) {
                    built = built.with_icon(icon);
                }
                match built.build() {
                    Ok(t) => *tray = Some(t),
                    Err(e) => eprintln!("menu bar: {e}"),
                }
            }
        }
        *shown.borrow_mut() = Some(now);
    });
}

/// What the menu's items do; false for an item not its own.
pub fn on_menu(id: &str, mut state: AppState) -> bool {
    if let Some(session) = id.strip_prefix(SESSION) {
        bring_up();
        state.web.set("record");
        state.open_session(session.to_owned());
        state.mode.set(Mode::Graph);
        state.screen.set(Screen::Work);
        return true;
    }
    match id {
        SHOW => {
            bring_up();
            if *state.screen.peek() != Screen::Setup {
                state.web.set("record");
                state.screen.set(Screen::Work);
            }
        }
        SETTINGS => {
            bring_up();
            if *state.screen.peek() == Screen::Setup {
                return true;
            }
            state.screen.set(Screen::Work);
            state.mode.set(Mode::Graph);
            if state.public.peek().engine == crate::settings::Engine::Own {
                state.web.set("settings");
            } else {
                state.settings_open.set(true);
            }
        }
        _ => return false,
    }
    true
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn the_menu_counts_todays_sessions_and_lists_the_last_active() {
        let s = |id: &str, last: i64, calls: i64, denied: i64, reviewed: i64| -> Session {
            serde_json::from_value(json!({"id": id, "last_at": last, "calls": calls,
                "denied": denied, "reviewed": reviewed, "prompt": format!("일 {id}")}))
            .unwrap()
        };
        let list = [
            s("a", 100, 4, 1, 0),
            s("b", 900, 10, 2, 1),
            s("c", 50, 3, 0, 0),
            s("d", 700, 1, 0, 1),
            s("e", 10, 1, 0, 0),
            s("f", 20, 1, 0, 0),
        ];
        let got = summary(&list, true, 100);
        assert_eq!(
            (got.calls, got.denied, got.reviewed),
            (15, 3, 2),
            "a, b and d"
        );
        let ids: Vec<&str> = got.recent.iter().map(|(id, _)| id.as_str()).collect();
        assert_eq!(ids, ["b", "d", "a", "c", "f"]);
        assert_eq!(got.recent[0].1, "일 b");
    }

    #[test]
    fn the_right_hand_node_is_filled_while_the_engine_answers() {
        let alpha = |img: &[u8], x: usize, y: usize| img[(y * SIDE + x) * 4 + 3];
        let (up, down) = (mark(true), mark(false));
        assert_eq!(up.len(), SIDE * SIDE * 4);
        // The right-hand node's centre: (700, 512) in the source.
        let (x, y) = (
            ((700.0 - 515.0) * 31.0 / 494.0 + 18.0) as usize,
            ((512.0 - 515.0) * 31.0 / 494.0 + 18.0) as usize,
        );
        assert_eq!(alpha(&up, x, y), 255);
        assert_eq!(alpha(&down, x, y), 0);
        // The middle node is there either way; the corners are empty.
        assert_eq!(alpha(&up, 18, 18), 255);
        assert_eq!(alpha(&down, 18, 18), 255);
        assert_eq!(alpha(&up, 0, 0), 0);
    }
}
