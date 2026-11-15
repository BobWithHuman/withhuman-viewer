//! The MCP servers the machines have set up, for 인벤토리's MCP tab
//! (`web/console/src/pages/inventory/mcp`), as JSON.
//!
//! Each server comes with the machines it is set up on and its tools; each tool with its calls
//! over the last 30 days counted by what the engine decided, who called it, its use day by day
//! and its latest calls. The most called tool of a server comes first. With them, every call of
//! the 30 days counted day by day by what the engine decided.

use std::collections::BTreeMap;

use serde_json::{Value, json};

use super::main_page::DAY_MS;
use crate::model::{McpCatalog, McpTool, McpUse};

/// The days the tab counts and draws a tool's use over.
const SPAN: usize = 30;
/// The latest calls a tool's detail lists.
const RECENT: usize = 8;

/// Calls counted day by day over the last [`SPAN`] days, the oldest first. A day is 24 hours
/// back from now, so the last one is always whole.
fn daily(calls: &[&McpUse], now: i64) -> Vec<usize> {
    let mut days = vec![0; SPAN];
    for u in calls {
        let age = (now - u.at).div_euclid(DAY_MS);
        if (0..SPAN as i64).contains(&age) {
            days[SPAN - 1 - age as usize] += 1;
        }
    }
    days
}

fn tool(t: &McpTool, now: i64) -> Value {
    let since = now - SPAN as i64 * DAY_MS;
    let mut within: Vec<&McpUse> = t.calls.iter().filter(|u| u.at >= since).collect();
    within.sort_by_key(|u| std::cmp::Reverse(u.at));
    let judged = |d: &str| within.iter().filter(|u| u.decision == d).count();
    let (review, deny) = (judged("REVIEW"), judged("DENY"));
    let mut by: BTreeMap<(&str, &str), usize> = BTreeMap::new();
    for u in &within {
        *by.entry((u.user.as_str(), u.client.as_str())).or_default() += 1;
    }
    let mut users: Vec<_> = by.into_iter().collect();
    users.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    json!({
        "name": t.name,
        "description": t.description,
        "nature": t.nature,
        "calls": within.len(),
        "allow": within.len() - review - deny,
        "review": review,
        "deny": deny,
        "users": users.iter().map(|((user, client), n)| json!({ "user": user, "client": client, "n": n })).collect::<Vec<_>>(),
        "last": t.calls.iter().map(|u| u.at).max(),
        "days": daily(&within, now),
        "recent": within.iter().take(RECENT).map(|u| json!({
            "at": u.at, "user": u.user, "client": u.client, "target": u.target, "decision": u.decision, "ran": u.ran,
        })).collect::<Vec<_>>(),
    })
}

/// The tab's JSON as of `now`.
pub(super) fn mcp(c: &McpCatalog, now: i64) -> Value {
    let servers: Vec<Value> = c
        .servers
        .iter()
        .map(|s| {
            let mut tools: Vec<Value> = s.tools.iter().map(|t| tool(t, now)).collect();
            tools.sort_by_key(|t| std::cmp::Reverse(t["calls"].as_u64()));
            json!({
                "name": s.name,
                "transport": s.transport,
                "endpoint": s.endpoint,
                "installs": s.installs.iter().map(|i| json!({ "user": i.user, "client": i.client, "at": i.at })).collect::<Vec<_>>(),
                "tools": tools,
            })
        })
        .collect();
    // Every call of the 30 days, counted day by day by what the engine decided.
    let since = now - SPAN as i64 * DAY_MS;
    let mut days = vec![[0usize; 3]; SPAN];
    for u in c
        .servers
        .iter()
        .flat_map(|s| &s.tools)
        .flat_map(|t| &t.calls)
    {
        if u.at < since {
            continue;
        }
        let age = (now - u.at).div_euclid(DAY_MS);
        if (0..SPAN as i64).contains(&age) {
            let which = match u.decision.as_str() {
                "REVIEW" => 1,
                "DENY" => 2,
                _ => 0,
            };
            days[SPAN - 1 - age as usize][which] += 1;
        }
    }
    json!({
        "days": days.iter().map(|[allow, review, deny]| json!({ "allow": allow, "review": review, "deny": deny })).collect::<Vec<_>>(),
        "servers": servers,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{McpInstall, McpServer};

    const NOW: i64 = 1_800_000_000_000;

    fn a_tool(name: &str, calls: &[(i64, &str)]) -> McpTool {
        McpTool {
            name: name.into(),
            nature: "read".into(),
            calls: calls
                .iter()
                .map(|(age, decision)| McpUse {
                    at: NOW - age,
                    user: "a@x".into(),
                    client: "m".into(),
                    decision: (*decision).into(),
                    ran: *decision == "ALLOW",
                    ..McpUse::default()
                })
                .collect(),
            ..McpTool::default()
        }
    }

    #[test]
    fn a_tools_calls_are_counted_over_thirty_days_by_what_the_engine_decided() {
        let c = McpCatalog {
            servers: vec![McpServer {
                name: "s".into(),
                installs: vec![McpInstall {
                    user: "a@x".into(),
                    client: "m".into(),
                    at: NOW - 40 * DAY_MS,
                }],
                tools: vec![
                    a_tool("stale", &[(40 * DAY_MS, "ALLOW")]),
                    a_tool(
                        "used",
                        &[
                            (DAY_MS / 2, "ALLOW"),
                            (DAY_MS, "REVIEW"),
                            (2 * DAY_MS, "DENY"),
                            (35 * DAY_MS, "ALLOW"),
                        ],
                    ),
                ],
                ..McpServer::default()
            }],
        };
        let page = mcp(&c, NOW);
        let tools = page["servers"][0]["tools"].as_array().unwrap();
        assert_eq!(tools[0]["name"], "used", "the most called first");
        assert_eq!(
            (
                &tools[0]["calls"],
                &tools[0]["allow"],
                &tools[0]["review"],
                &tools[0]["deny"]
            ),
            (&json!(3), &json!(1), &json!(1), &json!(1))
        );
        assert_eq!(tools[0]["days"].as_array().unwrap().len(), SPAN);
        assert_eq!(tools[0]["days"][SPAN - 1], 1);
        assert_eq!(tools[0]["users"][0]["n"], 3);
        assert_eq!(
            tools[1]["calls"], 0,
            "a call before the 30 days is not counted"
        );
        assert_eq!(
            tools[1]["last"],
            NOW - 40 * DAY_MS,
            "yet its last call is told"
        );
        assert_eq!(page["servers"][0]["installs"][0]["client"], "m");
        assert_eq!(page["days"].as_array().unwrap().len(), SPAN);
        assert_eq!(
            page["days"][SPAN - 1],
            json!({ "allow": 1, "review": 0, "deny": 0 })
        );
        assert_eq!(page["days"][SPAN - 2]["review"], 1);
    }
}
