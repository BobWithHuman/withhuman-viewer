//! The demo's MCP servers, as the engine would tell them once it collects what each machine has
//! set up: eight servers and their tools, the machines each is installed on, and the tools' calls
//! with what the engine decided for each. Two servers no one used for a month, one never.

use crate::model::{McpCatalog, McpInstall, McpServer, McpTool, McpUse};

const MIN: i64 = 60_000;
const HOUR: i64 = 60 * MIN;
const DAY: i64 = 24 * HOUR;

/// The people and their machines, as the inventory's demo names them.
const KIM: (&str, &str) = ("dev.kim@example.com", "dev-mac-07");
const LEE: (&str, &str) = ("dev.lee@example.com", "dev-mac-02");
const PARK: (&str, &str) = ("dev.park@example.com", "dev-win-11");
const CHOI: (&str, &str) = ("ops.choi@example.com", "ops-mac-21");
const JUNG: (&str, &str) = ("data.jung@example.com", "data-linux-04");

impl super::Demo {
    /// The demo's servers, made when first asked for.
    pub fn mcp(&mut self, now: i64) -> McpCatalog {
        self.mcp.get_or_insert_with(|| made(now)).clone()
    }
}

/// `n` calls of a tool over the last four weeks, the newest first: more of them lately than long
/// ago, none at the weekend, the first person named calling the most, the targets in turn. A
/// tool the engine holds is held on most calls and let run by the person mostly; one it blocks
/// is blocked on most and never runs then.
fn calls(
    now: i64,
    n: usize,
    offset: i64,
    users: &[(&str, &str)],
    targets: &[&str],
    judged: &str,
) -> Vec<McpUse> {
    // Whose turn it is: of three people the first takes three turns, the next two, the last one.
    let turns: Vec<usize> = (0..users.len())
        .flat_map(|j| std::iter::repeat_n(j, users.len() - j))
        .collect();
    let mut calls: Vec<McpUse> = (0..n)
        .map(|i| {
            // A fixed scatter, so the demo looks lived in yet stays the same.
            let scatter = ((i as i64 * 7919 + offset / MIN * 104_729) % 97) as f64 / 97.0;
            let share = (i as f64 + scatter) / n as f64;
            let mut at = now - offset - (share.powf(1.5) * 27.0 * DAY as f64) as i64;
            // 1970-01-01 was a Thursday: a Saturday or Sunday falls back on the Friday before.
            match (at.div_euclid(DAY) + 4).rem_euclid(7) {
                0 => at -= 2 * DAY,
                6 => at -= DAY,
                _ => {}
            }
            let (user, client) = users[turns[i % turns.len()]];
            McpUse {
                at,
                user: user.into(),
                client: client.into(),
                target: targets[i % targets.len()].into(),
                decision: match judged {
                    "review" if i % 4 != 1 => "REVIEW",
                    "deny" if i % 3 != 2 => "DENY",
                    _ => "ALLOW",
                }
                .into(),
                ran: match judged {
                    "review" => i % 5 != 3,
                    "deny" => i % 3 == 2,
                    _ => true,
                },
            }
        })
        .collect();
    calls.sort_by_key(|u| std::cmp::Reverse(u.at));
    calls
}

struct Spec<'a> {
    name: &'a str,
    description: &'a str,
    nature: &'a str,
    /// How the engine judges its calls: `allow`, `review` or `deny`.
    judged: &'a str,
    n: usize,
    users: &'a [(&'a str, &'a str)],
    targets: &'a [&'a str],
}

fn tools(now: i64, specs: &[Spec]) -> Vec<McpTool> {
    specs
        .iter()
        .enumerate()
        .map(|(i, s)| McpTool {
            name: s.name.into(),
            description: s.description.into(),
            nature: s.nature.into(),
            calls: if s.n == 0 {
                Vec::new()
            } else {
                calls(
                    now,
                    s.n,
                    (i as i64 + 1) * 37 * MIN,
                    s.users,
                    s.targets,
                    s.judged,
                )
            },
        })
        .collect()
}

/// A server set up on each of `on` (a person's machine, and how many days ago).
fn server(
    now: i64,
    name: &str,
    transport: &str,
    endpoint: &str,
    on: &[((&str, &str), i64)],
    tools: Vec<McpTool>,
) -> McpServer {
    McpServer {
        name: name.into(),
        transport: transport.into(),
        endpoint: endpoint.into(),
        installs: on
            .iter()
            .map(|((user, client), days)| McpInstall {
                user: (*user).into(),
                client: (*client).into(),
                at: now - days * DAY,
            })
            .collect(),
        tools,
    }
}

fn made(now: i64) -> McpCatalog {
    let github = tools(
        now,
        &[
            Spec {
                name: "search_code",
                description: "Search for code across the repositories the token can read.",
                nature: "read",
                judged: "allow",
                n: 64,
                users: &[KIM, LEE, PARK],
                targets: &[
                    "repo:acme/payments refund_window",
                    "org:acme verify_signature",
                    "repo:acme/web useSession",
                ],
            },
            Spec {
                name: "get_issue",
                description: "Get the title, body and comments of an issue.",
                nature: "read",
                judged: "allow",
                n: 41,
                users: &[KIM, LEE],
                targets: &["acme/payments#142", "acme/payments#150", "acme/web#77"],
            },
            Spec {
                name: "list_pull_requests",
                description: "List the pull requests of a repository.",
                nature: "read",
                judged: "allow",
                n: 22,
                users: &[KIM, LEE, PARK],
                targets: &["acme/payments state:open", "acme/web state:open"],
            },
            Spec {
                name: "add_issue_comment",
                description: "Add a comment to an issue or a pull request.",
                nature: "write",
                judged: "allow",
                n: 9,
                users: &[KIM],
                targets: &["acme/payments#142", "acme/web#81"],
            },
            Spec {
                name: "create_pull_request",
                description: "Open a pull request from a branch.",
                nature: "write",
                judged: "review",
                n: 12,
                users: &[KIM, LEE],
                targets: &[
                    "acme/payments fix/refund-window → main",
                    "acme/web chore/deps → main",
                ],
            },
            Spec {
                name: "merge_pull_request",
                description: "Merge a pull request.",
                nature: "write",
                judged: "deny",
                n: 5,
                users: &[LEE, KIM],
                targets: &["acme/payments#151 → main", "acme/web#83 → main"],
            },
            Spec {
                name: "delete_repository",
                description: "Delete a repository and everything in it.",
                nature: "delete",
                judged: "deny",
                n: 2,
                users: &[LEE],
                targets: &["acme/sandbox-old"],
            },
        ],
    );

    let slack = tools(
        now,
        &[
            Spec {
                name: "search_messages",
                description: "Search the messages of the channels the bot has joined.",
                nature: "read",
                judged: "allow",
                n: 30,
                users: &[KIM, PARK],
                targets: &["#dev-help 배포 오류", "#payments 환불 지연"],
            },
            Spec {
                name: "get_thread",
                description: "Read the replies of a thread.",
                nature: "read",
                judged: "allow",
                n: 14,
                users: &[KIM, PARK],
                targets: &["#dev-help 1731.2201", "#payments 1731.0954"],
            },
            Spec {
                name: "post_message",
                description: "Post a message to a channel.",
                nature: "send",
                judged: "review",
                n: 8,
                users: &[KIM],
                targets: &["#payments 배포 알림", "#dev-help 답변"],
            },
            Spec {
                name: "upload_file",
                description: "Upload a file to a channel.",
                nature: "send",
                judged: "deny",
                n: 4,
                users: &[PARK],
                targets: &["#ext-partners build.log", "#general dump.sql"],
            },
        ],
    );

    let jira = tools(
        now,
        &[
            Spec {
                name: "get_issue",
                description: "Get a Jira issue with its fields and comments.",
                nature: "read",
                judged: "allow",
                n: 25,
                users: &[LEE, PARK],
                targets: &["PAY-381", "PAY-377", "PAY-390"],
            },
            Spec {
                name: "search_issues",
                description: "Search issues with JQL.",
                nature: "read",
                judged: "allow",
                n: 18,
                users: &[LEE, PARK],
                targets: &["project = PAY AND status = Open"],
            },
            Spec {
                name: "create_issue",
                description: "Create an issue in a project.",
                nature: "write",
                judged: "allow",
                n: 6,
                users: &[LEE],
                targets: &["PAY 환불 기간 오류"],
            },
            Spec {
                name: "transition_issue",
                description: "Move an issue to another status.",
                nature: "write",
                judged: "allow",
                n: 4,
                users: &[LEE],
                targets: &["PAY-381 → Done"],
            },
        ],
    );

    let pg = tools(
        now,
        &[
            Spec {
                name: "query",
                description: "Run a read-only SQL query.",
                nature: "read",
                judged: "review",
                n: 11,
                users: &[LEE, PARK],
                targets: &[
                    "SELECT count(*) FROM orders_tmp",
                    "SELECT * FROM refunds WHERE status = 'failed'",
                ],
            },
            Spec {
                name: "list_tables",
                description: "List the tables and their columns.",
                nature: "read",
                judged: "allow",
                n: 7,
                users: &[LEE],
                targets: &["schema public"],
            },
        ],
    );

    // Set up long ago, and not used for over a month.
    let mut sentry = tools(
        now,
        &[
            Spec {
                name: "list_issues",
                description: "List the recent issues of a project.",
                nature: "read",
                judged: "allow",
                n: 0,
                users: &[],
                targets: &[],
            },
            Spec {
                name: "get_issue_details",
                description: "Get an issue's stack trace and events.",
                nature: "read",
                judged: "allow",
                n: 0,
                users: &[],
                targets: &[],
            },
        ],
    );
    let old = |at: i64, target: &str| McpUse {
        at,
        user: KIM.0.into(),
        client: KIM.1.into(),
        target: target.into(),
        decision: "ALLOW".into(),
        ran: true,
    };
    sentry[0].calls = vec![
        old(now - 44 * DAY, "payments-api"),
        old(now - 50 * DAY, "web"),
    ];
    sentry[1].calls = vec![old(now - 61 * DAY, "PAYMENTS-API-3F")];

    let notion = tools(
        now,
        &[
            Spec {
                name: "search",
                description: "Search the workspace's pages and databases.",
                nature: "read",
                judged: "allow",
                n: 18,
                users: &[PARK, CHOI],
                targets: &["환불 정책", "온콜 런북", "2026 Q4 로드맵"],
            },
            Spec {
                name: "fetch",
                description: "Get a page's content as Markdown.",
                nature: "read",
                judged: "allow",
                n: 13,
                users: &[PARK],
                targets: &["온콜 런북", "결제 장애 회고"],
            },
            Spec {
                name: "update_page",
                description: "Change a page's content or properties.",
                nature: "write",
                judged: "review",
                n: 6,
                users: &[CHOI],
                targets: &["온콜 런북", "배포 체크리스트"],
            },
            Spec {
                name: "create_comment",
                description: "Add a comment to a page.",
                nature: "write",
                judged: "allow",
                n: 3,
                users: &[PARK],
                targets: &["결제 장애 회고"],
            },
        ],
    );

    let figma = tools(
        now,
        &[
            Spec {
                name: "get_design_context",
                description: "Get a frame's layout, styles and components as code.",
                nature: "read",
                judged: "allow",
                n: 9,
                users: &[PARK],
                targets: &["Checkout / Refund sheet", "Settings / Billing"],
            },
            Spec {
                name: "get_screenshot",
                description: "Render a frame as an image.",
                nature: "read",
                judged: "allow",
                n: 5,
                users: &[PARK],
                targets: &["Checkout / Refund sheet"],
            },
        ],
    );

    let drive = tools(
        now,
        &[
            Spec {
                name: "search_files",
                description: "Search the files the account can see.",
                nature: "read",
                judged: "allow",
                n: 8,
                users: &[JUNG],
                targets: &["매출 정산 2026-10", "name contains 'export'"],
            },
            Spec {
                name: "read_file_content",
                description: "Read a file's text.",
                nature: "read",
                judged: "allow",
                n: 6,
                users: &[JUNG],
                targets: &["매출 정산 2026-10.xlsx"],
            },
            Spec {
                name: "share_file",
                description: "Share a file with a person or anyone with the link.",
                nature: "send",
                judged: "deny",
                n: 3,
                users: &[JUNG],
                targets: &["customers_export.csv → anyone with link"],
            },
            Spec {
                name: "trash_file",
                description: "Move a file to the trash.",
                nature: "delete",
                judged: "review",
                n: 2,
                users: &[JUNG],
                targets: &["tmp_export_old.csv"],
            },
        ],
    );

    // Set up on one machine and never called.
    let filesystem = tools(
        now,
        &[
            Spec {
                name: "read_file",
                description: "Read a file under the allowed directories.",
                nature: "read",
                judged: "allow",
                n: 0,
                users: &[],
                targets: &[],
            },
            Spec {
                name: "write_file",
                description: "Create or overwrite a file under the allowed directories.",
                nature: "write",
                judged: "allow",
                n: 0,
                users: &[],
                targets: &[],
            },
            Spec {
                name: "move_file",
                description: "Move or rename a file.",
                nature: "write",
                judged: "allow",
                n: 0,
                users: &[],
                targets: &[],
            },
        ],
    );

    McpCatalog {
        servers: vec![
            server(
                now,
                "github",
                "remote",
                "https://api.githubcopilot.com/mcp/",
                &[(KIM, 71), (LEE, 64), (PARK, 40), (CHOI, 12)],
                github,
            ),
            server(
                now,
                "slack",
                "remote",
                "https://mcp.slack.com/mcp",
                &[(KIM, 58), (PARK, 33)],
                slack,
            ),
            server(
                now,
                "atlassian",
                "remote",
                "https://mcp.atlassian.com/v1/sse",
                &[(LEE, 42), (PARK, 42)],
                jira,
            ),
            server(
                now,
                "postgres-ro",
                "local",
                "npx -y @modelcontextprotocol/server-postgres postgres://readonly@db-stg:5432/app",
                &[(LEE, 23), (PARK, 20)],
                pg,
            ),
            server(
                now,
                "notion",
                "remote",
                "https://mcp.notion.com/mcp",
                &[(PARK, 16), (CHOI, 9)],
                notion,
            ),
            server(
                now,
                "figma",
                "remote",
                "https://mcp.figma.com/mcp",
                &[(PARK, 11)],
                figma,
            ),
            server(
                now,
                "google-drive",
                "remote",
                "https://drivemcp.googleapis.com/mcp/v1",
                &[(JUNG, 14)],
                drive,
            ),
            server(
                now,
                "sentry",
                "remote",
                "https://mcp.sentry.dev/mcp",
                &[(KIM, 96)],
                sentry,
            ),
            server(
                now,
                "filesystem",
                "local",
                "npx -y @modelcontextprotocol/server-filesystem ~/work",
                &[(JUNG, 5)],
                filesystem,
            ),
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_call_comes_from_a_machine_the_server_is_set_up_on() {
        let c = made(1_800_000_000_000);
        for s in &c.servers {
            for u in s.tools.iter().flat_map(|t| &t.calls) {
                assert!(
                    s.installs.iter().any(|i| i.client == u.client),
                    "{} called from {}",
                    s.name,
                    u.client
                );
            }
        }
    }

    #[test]
    fn a_blocked_call_never_ran() {
        let c = made(1_800_000_000_000);
        let calls = || {
            c.servers
                .iter()
                .flat_map(|s| &s.tools)
                .flat_map(|t| &t.calls)
        };
        assert!(calls().any(|u| u.decision == "DENY"));
        assert!(calls().any(|u| u.decision == "REVIEW"));
        assert!(calls().filter(|u| u.decision == "DENY").all(|u| !u.ran));
    }
}
