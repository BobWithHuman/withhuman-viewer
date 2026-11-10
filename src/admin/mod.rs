//! The security team's app: what only it shows. The route map (기록) is the developer's app's.
//!
//! - 메인: what to look at now, over every machine; its 탐지 tab lists the Calls worth a look,
//!   the worst first, each opened beside the list.
//! - 감사: what the engine stopped, held or let through though hard to undo, and how people
//!   answered when asked; exported for an audit.
//! - MCP 사용: the console's second product, from the same Calls: the MCP servers and web hosts
//!   the agents used.
//! - 민감 정보: the sensitive values found in the sessions, followed to the Calls that used them.
//! - 정책: the rules to give the engine, kept as a draft until the engine takes them, each
//!   previewed on the Calls already recorded.

mod audit;
mod detections;
mod main_page;
mod mcp;
mod policy;
mod posture;
mod secrets;

use std::sync::Arc;

use dioxus::prelude::*;

pub use audit::Audit;
pub use main_page::Main;
pub use mcp::Mcp;
pub use secrets::Secrets;
pub use policy::Policy;

use crate::api::Backend;
use crate::model::CallFact;
use crate::ui::AppState;

/// Every Call the security team's pages count, read once for all of them, and when they were
/// last read.
#[derive(Clone, Copy)]
pub struct Facts(pub Signal<Option<Result<Vec<CallFact>, String>>>, pub Signal<Option<i64>>);

/// The main page sending the security team to 감사's approvals: `Some(None)` opens the tab,
/// `Some(Some((session, seq)))` opens it with that Call's row marked. 감사 takes it and lets go.
#[derive(Clone, Copy)]
pub struct ToApprovals(pub Signal<Option<Option<(String, i64)>>>);

/// Reads the Calls on launch, on reload and after the connection changes.
pub fn use_facts_loader(backend: Arc<Backend>, state: AppState) {
    use_context_provider(|| ToApprovals(Signal::new(None)));
    let mut facts = use_context_provider(|| Facts(Signal::new(None), Signal::new(None)));
    use_effect(move || {
        let _ = (state.reload)();
        let _ = (state.public)();
        let b = backend.clone();
        spawn(async move {
            let found = b.call_facts().await;
            facts.0.set(Some(found));
            facts.1.set(Some(main_page::now_ms()));
        });
    });
}
