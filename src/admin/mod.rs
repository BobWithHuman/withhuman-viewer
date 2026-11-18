//! The security team's app: what only it shows. The route map (기록) is the developer's app's.
//!
//! - 메인: what to look at now, over every machine; its 탐지 tab lists the Calls worth a look,
//!   the worst first, each opened beside the list.
//! - 인벤토리: the machines, people and sessions the engine protects, and the MCP servers the
//!   machines have set up; keys issued and revoked (a person's kill switch).
//! - 민감 정보: the sensitive values found in the sessions, followed to the Calls that used them.
//! - 정책: the rules to give the engine, kept as a draft until the engine takes them, each
//!   previewed on the Calls already recorded.

mod case;
mod charts;
mod inventory;
mod main_page;
mod policy;
mod posture;
mod secrets;
pub mod triage;
// The dashboards' page (web/console) in a frame, and each screen's data for it.
mod detect_web;
mod inventory_web;
mod mcp_tools;
mod mcp_web;
mod secrets_web;
pub mod web;

use std::sync::Arc;

use dioxus::prelude::*;

pub use inventory::InventoryPage;
pub use main_page::Main;
pub use policy::Policy;
pub use secrets::Secrets;

use crate::api::Backend;
use crate::model::CallFact;
use crate::ui::AppState;

/// Every Call the security team's pages count, read once for all of them, and when they were
/// last read.
#[derive(Clone, Copy)]
pub struct Facts(
    pub Signal<Option<Result<Vec<CallFact>, String>>>,
    pub Signal<Option<i64>>,
);

/// Reads the Calls on launch, on reload and after the connection changes.
pub fn use_facts_loader(backend: Arc<Backend>, state: AppState) {
    let mut facts = use_context_provider(|| Facts(Signal::new(None), Signal::new(None)));
    // The presses of 다시 읽기 already answered.
    let mut answered = use_signal(|| 0u32);
    use_effect(move || {
        // 다시 읽기 reads every Call again; a launch, live, or a new connection reads what is new.
        let _ = (state.reload)();
        let pressed = (state.full)();
        let full = pressed > *answered.peek();
        answered.set(pressed);
        let _ = (state.public)();
        let b = backend.clone();
        spawn(async move {
            let found = b.call_facts(full).await;
            facts.0.set(Some(found));
            facts.1.set(Some(main_page::now_ms()));
        });
    });
}
