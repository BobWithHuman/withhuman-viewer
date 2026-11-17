//! 민감 정보's data for the dashboards' page (`web/console/src/pages/secrets.tsx`), as JSON.
//!
//! The page gets every value's trail and does its own counting, so a period or a band picked
//! there needs no round trip. A value is only ever sent masked, as the engine keeps it.

use std::collections::HashMap;

use serde_json::{Value, json};

use super::main_page::NO_NAME;
use super::secrets::{
    Trail, destination, held, in_call, kind_label, node_label, outward, source_of, source_tool,
    why_out,
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
        "why": (out && !held(m)).then(|| why_out(m)),
        "mode": m.mode,
        "tool": m.tool,
        "said": m.said,
        "what": call.then(|| m.target.clone().or_else(|| m.tool.clone())).flatten(),
        "place": out.then(|| destination(m)),
        "verdict": verdict(m),
    })
}

/// The machines' names (host names) by client ID, as the inventory tells.
#[derive(Default)]
pub(super) struct Hosts(HashMap<String, String>);

impl Hosts {
    pub(super) fn of(inv: &crate::model::Inventory) -> Self {
        Self(
            inv.clients
                .iter()
                .filter_map(|d| Some((d.client_id.clone(), d.hostname.clone()?)))
                .collect(),
        )
    }
}

/// Where a value the agent wrote first in its session came in before, in another session: the
/// latest session where a person typed it or a tool returned it before this one first showed it.
/// None for a value that came in in its own session, or that no session shows coming in.
pub(super) fn came_before<'a>(
    t: &Trail,
    all: impl IntoIterator<Item = &'a Trail>,
) -> Option<&'a Trail> {
    if t.from == "user" || t.from == "external" {
        return None;
    }
    let first = t.mentions.first()?.at?;
    let at = |x: &Trail| x.mentions.first().and_then(|m| m.at);
    all.into_iter()
        .filter(|x| x.key == t.key && x.session != t.session)
        .filter(|x| x.from == "user" || x.from == "external")
        .filter(|x| at(x).is_some_and(|a| a < first))
        .max_by_key(|x| at(x))
}

/// The page's data: the trails, the worst first, each with its session's title and its
/// machine's name. A value the agent wrote first comes in from an earlier session (`earlier`)
/// when one shows it coming in, with that session's source.
pub(super) fn page(
    trails: &[Trail],
    titles: &HashMap<String, String>,
    hosts: &Hosts,
    head: &Head,
) -> String {
    // The trails of each value, to look for where it came in before.
    let mut by_key: HashMap<&str, Vec<&Trail>> = HashMap::new();
    for t in trails {
        by_key.entry(t.key.as_str()).or_default().push(t);
    }
    let trails: Vec<Value> = trails
        .iter()
        .map(|t| {
            let before = came_before(t, by_key.get(t.key.as_str()).into_iter().flatten().copied());
            let source = source_of(before.unwrap_or(t));
            let tool = source
                .as_ref()
                .and_then(|_| source_tool(before.unwrap_or(t)));
            json!({
                "id": format!("{}|{}", t.session, t.key),
                "session": t.session,
                "title": titles.get(&t.session).filter(|s| !s.is_empty()),
                "kind": kind_label(&t.kind),
                "category": t.category,
                "text": t.text,
                "from": if before.is_some() { "earlier" } else { t.from.as_str() },
                "source": source,
                "sourceTool": tool,
                "before": before.map(|b| json!({
                    "session": b.session,
                    "title": titles.get(&b.session).filter(|s| !s.is_empty()),
                    "from": b.from,
                    "at": b.mentions.first().and_then(|m| m.at),
                })),
                "client": if t.client.is_empty() { NO_NAME } else { &t.client },
                "host": hosts.0.get(&t.client),
                "calls": t.calls,
                "outRan": t.out_ran,
                "outStopped": t.out_stopped,
                "normal": t.normal,
                "why": t.why,
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
