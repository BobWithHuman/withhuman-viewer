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
use crate::model::{CallFact, McpCatalog, McpInstall, McpServer, McpTool, McpUse};

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

/// What a tool can do, as the page draws it, from the registry's action for its name: `money`
/// (it pays or moves an account), `send` (out of the company), `delete`, `write`, or `read`;
/// none when the registry cannot tell.
fn nature(tool: &str) -> Option<&'static str> {
    Some(match super::mcp_tools::action(tool)? {
        "transact" => "money",
        "send" | "delegate" => "send",
        "delete" => "delete",
        "create" | "update" | "execute" => "write",
        _ => "read",
    })
}

/// The MCP servers the machines called tools of, as the engine's calls show them: each server
/// with the machines that called it (since their first call) and the tools called, each with
/// what the registry says it does and its calls. A tool the registry cannot tell is `unknown`.
/// What a server offers but was never called, and where it runs, the calls do not show.
pub(super) fn called(facts: &[CallFact]) -> McpCatalog {
    let mut servers: BTreeMap<&str, BTreeMap<&str, Vec<McpUse>>> = BTreeMap::new();
    for f in facts {
        let (Some((server, op)), Some(at)) =
            (f.tool.as_deref().and_then(crate::metro::mcp_parts), f.at)
        else {
            continue;
        };
        servers
            .entry(server)
            .or_default()
            .entry(op)
            .or_default()
            .push(McpUse {
                at,
                user: String::new(),
                client: f.client.clone().unwrap_or_default(),
                target: f.target.clone().unwrap_or_default(),
                decision: f.decision.clone().unwrap_or_else(|| "ALLOW".into()),
                ran: f.ran,
            });
    }
    McpCatalog {
        servers: servers
            .into_iter()
            .map(|(name, tools)| {
                let mut first: BTreeMap<&str, i64> = BTreeMap::new();
                for u in tools.values().flatten().filter(|u| !u.client.is_empty()) {
                    let at = first.entry(u.client.as_str()).or_insert(u.at);
                    *at = (*at).min(u.at);
                }
                McpServer {
                    name: name.into(),
                    transport: String::new(),
                    endpoint: String::new(),
                    installs: first
                        .into_iter()
                        .map(|(client, at)| McpInstall {
                            user: String::new(),
                            client: client.into(),
                            at,
                        })
                        .collect(),
                    tools: tools
                        .into_iter()
                        .map(|(op, calls)| McpTool {
                            name: op.into(),
                            description: String::new(),
                            nature: nature(op).unwrap_or("unknown").into(),
                            calls,
                        })
                        .collect(),
                }
            })
            .collect(),
    }
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

    #[test]
    fn the_servers_called_are_told_from_the_calls() {
        let call = |tool: &str, client: &str, at: i64, decision: Option<&str>| CallFact {
            tool: Some(tool.into()),
            client: Some(client.into()),
            at: Some(at),
            decision: decision.map(str::to_owned),
            ..CallFact::default()
        };
        let facts = [
            call("mcp__slack__post_message", "a", NOW - 5, Some("DENY")),
            call("mcp__slack__post_message", "b", NOW - 9, None),
            call("mcp__slack__search_messages", "a", NOW - 3, Some("ALLOW")),
            call("mcp__bank__send_money", "a", NOW - 2, Some("REVIEW")),
            call("mcp__x__zzqq", "a", NOW - 1, None),
            call("Bash", "a", NOW - 1, None),
        ];
        let c = called(&facts);
        let names: Vec<&str> = c.servers.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, ["bank", "slack", "x"]);
        let slack = &c.servers[1];
        assert_eq!(
            slack
                .installs
                .iter()
                .map(|i| (i.client.as_str(), i.at))
                .collect::<Vec<_>>(),
            [("a", NOW - 5), ("b", NOW - 9)]
        );
        let nature =
            |s: &McpServer, t: &str| s.tools.iter().find(|x| x.name == t).unwrap().nature.clone();
        assert_eq!(nature(slack, "post_message"), "send");
        assert_eq!(nature(slack, "search_messages"), "read");
        assert_eq!(
            nature(&c.servers[0], "send_money"),
            "send",
            "the registry's action for it"
        );
        assert_eq!(nature(&c.servers[2], "zzqq"), "unknown");
        assert_eq!(
            slack
                .tools
                .iter()
                .find(|x| x.name == "post_message")
                .unwrap()
                .calls[1]
                .decision,
            "ALLOW",
            "none decided reads as allowed"
        );
    }
}
