//! 인벤토리: the machines, people and sessions the engine protects (withhuman-core's agent
//! inventory) and the MCP servers the machines have set up, on the rail beside 이상 행위 탐지.
//!
//! The screen is drawn by the dashboards' page (`web/console/src/pages/inventory.tsx`) in a
//! frame; this side reads the inventory, hands it over as JSON ([`super::inventory_web`]) and
//! does what the page asks for:
//! - `issue`: a key for a person, shown once in the page;
//! - `revoke_key`, `stop`: one key revoked, or a person stopped (every key), which refuses that
//!   person's Calls from then on. Those go to the engine's admin API, and its record is 감사's
//!   킬스위치 기록;
//! - `owner`: a machine's owner named, or the name taken off;
//! - `admin`: the admin API's address and token kept;
//! - `open_session`: a session opened on the route map; `copy`: a text put on the clipboard;
//! - `ack`: the last answer was seen, so a key just issued is not kept any longer.
//!
//! Each of the first four carries a `req` number, and the answer goes back in the data as
//! `reply` with the same number.

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
            let modes = match &*facts.0.read() {
                Some(Ok(list)) => inventory_web::engine_modes(list),
                _ => HashMap::new(),
            };
            let mut page = inventory_web::page(inv, &titles, &modes, &around(*at));
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
            "copy" => {
                let text = serde_json::to_string(&text("text")).unwrap_or_default();
                document::eval(&format!("navigator.clipboard.writeText({text})"));
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
            "owner" => {
                let (device, owner, actor) = (text("device"), text("email"), text("actor"));
                let (backend, req) = (backend.clone(), m["req"].clone());
                spawn(async move {
                    let owner = Some(owner.as_str()).filter(|o| !o.is_empty());
                    reply.set(Some(
                        match backend.assign_owner(&device, owner, &actor).await {
                            Ok(()) => {
                                bump += 1;
                                json!({ "req": req, "ok": true, "done": "owner" })
                            }
                            Err(e) => failed(e),
                        },
                    ));
                });
            }
            kind @ ("issue" | "revoke_key" | "stop") => {
                let (email, actor, key_id) = (text("email"), text("actor"), text("key_id"));
                let why = text(if kind == "issue" { "note" } else { "reason" });
                if email.is_empty() || actor.is_empty() || (kind != "issue" && why.is_empty()) {
                    reply.set(Some(failed("사용자, 실행자, 사유가 필요합니다.".into())));
                    return;
                }
                let (backend, kind, req) = (backend.clone(), kind.to_owned(), m["req"].clone());
                spawn(async move {
                    let done = match kind.as_str() {
                        "issue" => backend.issue_key(&email, &actor, &why).await.map(|(key_id, key)| {
                            json!({ "done": "issued", "email": email, "key_id": key_id, "key": key })
                        }),
                        "revoke_key" => backend.revoke(Some(&key_id), None, &actor, &why).await.map(|_| {
                            json!({ "done": "revoked", "email": email, "key_id": key_id })
                        }),
                        _ => backend.revoke(None, Some(&email), &actor, &why).await.map(|n| {
                            json!({ "done": "stopped", "email": email, "count": n })
                        }),
                    };
                    reply.set(Some(match done {
                        Ok(mut answer) => {
                            answer["req"] = req;
                            answer["ok"] = json!(true);
                            bump += 1;
                            answer
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
        Ok("people") => "people",
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
