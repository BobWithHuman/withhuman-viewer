//! The security team's app: what only it shows. The route map (기록) is the developer's app's.
//!
//! - 메인: what to look at now, over every machine; its 탐지 tab lists the Calls worth a look,
//!   the worst first, each opened beside the list.
//! - 감사: what the engine stopped, held or let through though hard to undo, and how people
//!   answered when asked; exported for an audit.
//! - MCP 사용: the console's second product: the MCP tools the gateway's allowlist lets through,
//!   the requests developers send to have more let through, and the calls the gateway refused.
//! - 인벤토리: the machines, people and sessions the engine protects; keys issued and revoked
//!   (a person's kill switch).
//! - 민감 정보: the sensitive values found in the sessions, followed to the Calls that used them.
//! - 정책: the rules to give the engine, kept as a draft until the engine takes them, each
//!   previewed on the Calls already recorded.

mod audit;
mod charts;
mod detections;
mod hub;
mod inventory;
mod main_page;
mod mcp;
mod policy;
mod posture;
mod route;
mod secrets;

use std::sync::Arc;

use dioxus::prelude::*;

pub use audit::Audit;
pub use hub::Hub;
pub use inventory::InventoryPage;
pub use main_page::Main;
pub use mcp::Mcp;
pub use policy::Policy;
pub use route::RouteScene;
pub use secrets::Secrets;

/// The main page's 3D scene: three.js and `web/hub-scene/scene.js`, bundled (`npm run build`
/// there). The page's head loads it, so the scene is there when the main page mounts.
pub const HUB_SCENE_JS: &str = include_str!("hub_scene.js");

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
