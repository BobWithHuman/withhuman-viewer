//! 민감 정보's data for the dashboards' page (`web/console/src/pages/secrets.tsx`), as JSON.
//!
//! The page gets every value's trail and does its own counting, so a period or a band picked
//! there needs no round trip. A value is only ever sent masked, as the engine keeps it.

use std::collections::HashMap;

use serde_json::{Value, json};

use super::main_page::NO_NAME;
use super::secrets::{
    Trail, destination, held, in_call, kind_label, node_label, outward, source_of,
};
use crate::model::SecretMention;

/// What the page shows around the trails: when they were read, from where, and what went wrong.
pub(super) struct Head {
    pub now: i64,
    /// When the values were last read; none while the first read is still under way.
    pub loaded: Option<i64>,
    pub failed: Option<String>,
    pub store: String,
    pub demo: bool,
}

/// How a Call holding a value was decided, as the page names it.
fn verdict(m: &SecretMention) -> Option<&'static str> {
    match (m.decision.as_deref(), m.ran) {
        (Some("DENY"), _) => Some("deny"),
        (Some("REVIEW"), true) => Some("approved"),
        (Some("REVIEW"), false) => Some("asked"),
        (Some("ALLOW"), _) => Some("allow"),
        _ => None,
    }
}

/// One place a value showed in its session. Only a Call says what it acted on and where it sent.
fn mention(m: &SecretMention) -> Value {
    let call = in_call(m);
    let out = call && outward(m);
    json!({
        "seq": m.seq,
        "at": m.at,
        "where": node_label(m),
        "call": call,
        "out": out,
        "held": out && held(m),
        "what": call.then(|| m.target.clone().or_else(|| m.tool.clone())).flatten(),
        "place": out.then(|| destination(m)),
        "verdict": verdict(m),
    })
}

/// The page's data: the trails, the worst first, each with its session's title.
pub(super) fn page(trails: &[Trail], titles: &HashMap<String, String>, head: &Head) -> String {
    let trails: Vec<Value> = trails
        .iter()
        .map(|t| {
            let source = source_of(t);
            json!({
                "id": format!("{}|{}", t.session, t.key),
                "session": t.session,
                "title": titles.get(&t.session).filter(|s| !s.is_empty()),
                "kind": kind_label(&t.kind),
                "category": t.category,
                "text": t.text,
                "from": t.from,
                "source": source,
                "sourceTool": source.as_ref().and_then(|_| t.mentions.first()?.tool.clone()),
                "client": if t.client.is_empty() { NO_NAME } else { &t.client },
                "calls": t.calls,
                "outRan": t.out_ran,
                "outStopped": t.out_stopped,
                "status": t.status().1,
                "last": t.last,
                "mentions": t.mentions.iter().map(mention).collect::<Vec<_>>(),
            })
        })
        .collect();
    json!({
        "now": head.now,
        "loaded": head.loaded,
        "error": head.failed,
        "store": head.store,
        "demo": head.demo,
        "trails": trails,
    })
    .to_string()
}
