//! 인벤토리: the machines and sessions the engine protects (withhuman-core's agent inventory),
//! what each machine did, and the MCP servers the machines have set up, by machine alone.
//!
//! The screen is drawn by the dashboards' page (`web/console/src/pages/inventory.tsx`) in a
//! frame; this side reads the inventory, hands it over as JSON ([`super::inventory_web`]) and
//! does what the page asks for:
//! - `stop_device`: a machine stopped, or let go again, through the engine's admin API;
//! - `admin`: the admin API's address and token kept;
//! - `open_session`: a session opened on the route map;
//! - `ack`: the last answer was seen.
//!
//! The first two carry a `req` number, and the answer goes back in the data as `reply` with the
//! same number.

use std::collections::HashMap;
use std::sync::Arc;

use dioxus::prelude::*;
use serde_json::{Value, json};

use super::inventory_web::{self, Around};
use super::main_page::now_ms;
use crate::api::Backend;
use crate::model::Session;
use crate::ui::{AppState, Load, Mode, Screen};

#[component]
pub fn InventoryPage() -> Element {
    let mut state = use_context::<AppState>();
    let backend = use_context::<Arc<Backend>>();
    let facts = use_context::<super::Facts>();
    let mut bump = use_signal(|| 0u32);
    let found = use_resource({
        let backend = backend.clone();
        move || {
            let b = backend.clone();
            let _ = (state.reload)();
            let _ = (state.public)();
            let _ = bump();
            async move { (b.inventory().await, b.mcp_catalog().await, now_ms()) }
        }
    });
    let mut reply = use_signal(|| None::<Value>);
    let tab = use_hook(first_tab);
    let p = (state.public)();
    let around = |now: i64| Around {
        now,
        source: p.store(),
        demo: p.demo,
        engine_api: p.engine_api.clone(),
        has_admin_token: p.has_admin_token,
        actor: me(),
        reply: reply(),
        tab,
    };
    let data = match &*found.read() {
        None => inventory_web::without("loading", None, &around(now_ms())),
        Some((Err(e), _, at)) => inventory_web::without("failed", Some(e), &around(*at)),
        Some((Ok(None), _, at)) => inventory_web::without("none", None, &around(*at)),
        Some((Ok(Some(inv)), mcp, at)) => {
            let titles: HashMap<String, Session> = match &*state.sessions.read() {
                Load::Ready(list) => list.iter().map(|s| (s.id.clone(), s.clone())).collect(),
                _ => HashMap::new(),
            };
            let (modes, activity) = match &*facts.0.read() {
                Some(Ok(list)) => (
                    inventory_web::engine_modes(list),
                    inventory_web::activity(list, *at),
                ),
                _ => (HashMap::new(), json!({})),
            };
            let mut page = inventory_web::page(inv, &titles, &modes, &around(*at));
            page["activity"] = activity;
            page["days"] = json!(inventory_web::DAYS);
            // The servers the machines have set up; none while the engine does not collect them.
            if let Ok(Some(c)) = mcp {
                page["mcp"] = super::mcp_web::mcp(c, *at);
            }
            page
        }
    };

    let onmsg = move |m: Value| {
        let text = |k: &str| m[k].as_str().unwrap_or_default().trim().to_owned();
        let req = m["req"].clone();
        let failed = move |e: String| json!({ "req": req, "ok": false, "error": e });
        match m["type"].as_str().unwrap_or_default() {
            "open_session" => {
                state.open_session(text("id"));
                state.mode.set(Mode::Graph);
                state.screen.set(Screen::Work);
            }
            "ack" => reply.set(None),
            "admin" => {
                let token = text("token");
                let saved = backend
                    .save_admin(&text("api"), Some(token.as_str()).filter(|t| !t.is_empty()));
                reply.set(Some(match saved {
                    Ok(next) => {
                        state.public.set(next);
                        json!({ "req": m["req"], "ok": true, "done": "admin" })
                    }
                    Err(e) => failed(e),
                }));
            }
            "stop_device" => {
                let (device, actor, reason) = (text("device"), text("actor"), text("reason"));
                let stopped = m["stopped"].as_bool().unwrap_or(true);
                let (backend, req) = (backend.clone(), m["req"].clone());
                spawn(async move {
                    reply.set(Some(match backend.stop_device(&device, stopped, &actor, &reason).await {
                        Ok(()) => {
                            bump += 1;
                            json!({ "req": req, "ok": true, "done": if stopped { "device_stopped" } else { "device_released" } })
                        }
                        Err(e) => failed(e),
                    }));
                });
            }
            _ => {}
        }
    };

    rsx! {
        super::web::Frame { page: "inventory", data: data.to_string(), onmsg }
    }
}

/// The first tab, as VIEWER_TAB names it.
fn first_tab() -> &'static str {
    match std::env::var("VIEWER_TAB").as_deref() {
        Ok("sessions") => "sessions",
        Ok("shadow") => "shadow",
        Ok("mcp") => "mcp",
        _ => "devices",
    }
}

/// Who is acting, for the record: the person signed in to this Mac, until the engine knows the
/// security team's own accounts.
fn me() -> String {
    std::env::var("USER").unwrap_or_else(|_| "security-admin".into())
}
