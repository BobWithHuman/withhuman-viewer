//! The demo's MCP allowlist, as the gateway would keep it: five approved servers and their tools,
//! one no one used for a month; the requests developers sent from their machines, four waiting,
//! one approved, one rejected; and the calls the gateway refused.

use crate::model::{
    McpCatalog, McpChange, McpOffered, McpRefusal, McpRequest, McpServer, McpTool, McpUse,
};

const MIN: i64 = 60_000;
const HOUR: i64 = 60 * MIN;
const DAY: i64 = 24 * HOUR;

/// The people and their machines, as the inventory's demo names them.
const KIM: (&str, &str) = ("dev.kim@example.com", "dev-mac-07");
const LEE: (&str, &str) = ("dev.lee@example.com", "dev-mac-02");
const PARK: (&str, &str) = ("dev.park@example.com", "dev-win-11");
const CI: (&str, &str) = ("ci-bot", "ci-runner-1");
const APPROVER: &str = "sec.choi@example.com";

impl super::Demo {
    /// The demo's allowlist, made when first asked for and changed by the page's actions until
    /// the app restarts.
    pub fn mcp(&mut self, now: i64) -> McpCatalog {
        self.mcp.get_or_insert_with(|| made(now)).clone()
    }

    pub fn mcp_change(&mut self, now: i64, change: &McpChange) {
        let catalog = self.mcp.get_or_insert_with(|| made(now));
        match change {
            McpChange::Level {
                server,
                tool,
                level,
            } => {
                if let Some(t) = find(catalog, server, tool) {
                    t.level = level.clone();
                }
            }
            McpChange::Decide {
                id,
                approve,
                actor,
                answer,
                levels,
                scope,
            } => decide(catalog, now, id, *approve, actor, answer, levels, scope),
        }
        catalog.updated_at = Some(now);
    }
}

fn find<'a>(catalog: &'a mut McpCatalog, server: &str, tool: &str) -> Option<&'a mut McpTool> {
    catalog
        .servers
        .iter_mut()
        .find(|s| s.name == server)?
        .tools
        .iter_mut()
        .find(|t| t.name == tool)
}

/// A request decided: approved, its server or tools join the allowlist at the levels chosen
/// (a tool offered and given none is denied), or the person joins the tool's scope.
#[allow(clippy::too_many_arguments)]
fn decide(
    c: &mut McpCatalog,
    now: i64,
    id: &str,
    approve: bool,
    actor: &str,
    answer: &str,
    levels: &[(String, String)],
    scope: &[String],
) {
    let Some(r) = c.requests.iter_mut().find(|r| r.id == id) else {
        return;
    };
    r.status = if approve { "approved" } else { "rejected" }.into();
    r.decided_by = Some(actor.to_owned());
    r.decided_at = Some(now);
    r.answer = Some(answer.to_owned()).filter(|a| !a.trim().is_empty());
    if !approve {
        return;
    }
    let r = r.clone();
    let level = |name: &str| {
        levels
            .iter()
            .find(|(t, _)| t == name)
            .map_or("deny".to_owned(), |(_, l)| l.clone())
    };
    let tool = |o: &McpOffered| McpTool {
        name: o.name.clone(),
        description: o.description.clone(),
        nature: o.nature.clone(),
        level: level(&o.name),
        scope: scope.to_vec(),
        approved_by: Some(actor.to_owned()),
        approved_at: Some(now),
        calls: Vec::new(),
    };
    match r.kind.as_str() {
        "server" => c.servers.push(McpServer {
            name: r.server.clone(),
            transport: r.transport.clone().unwrap_or_else(|| "remote".into()),
            endpoint: r.endpoint.clone().unwrap_or_default(),
            approved_by: Some(actor.to_owned()),
            approved_at: Some(now),
            request: Some(r.id.clone()),
            tools: r.offered.iter().map(tool).collect(),
        }),
        "tool" => {
            if let Some(s) = c.servers.iter_mut().find(|s| s.name == r.server) {
                s.tools.extend(r.offered.iter().map(tool));
            }
        }
        _ => {
            let who = r.user.split('@').next().unwrap_or(&r.user).to_owned();
            for name in &r.wanted {
                if let Some(t) = find(c, &r.server, name)
                    && !t.scope.is_empty()
                    && !t.scope.contains(&who)
                {
                    t.scope.push(who.clone());
                }
            }
        }
    }
}

/// `n` calls of a tool over the last four weeks, the newest first: more of them lately than long
/// ago, none at the weekend, the first person named calling the most, the targets in turn; a
/// tool held for review let run by the person mostly.
fn calls(
    now: i64,
    n: usize,
    offset: i64,
    users: &[(&str, &str)],
    targets: &[&str],
    level: &str,
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
                decision: if level == "review" { "REVIEW" } else { "ALLOW" }.into(),
                ran: level != "review" || i % 5 != 3,
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
    level: &'a str,
    n: usize,
    users: &'a [(&'a str, &'a str)],
    targets: &'a [&'a str],
}

fn tools(now: i64, approved_at: i64, scope: &[&str], specs: &[Spec]) -> Vec<McpTool> {
    specs
        .iter()
        .enumerate()
        .map(|(i, s)| McpTool {
            name: s.name.into(),
            description: s.description.into(),
            nature: s.nature.into(),
            level: s.level.into(),
            scope: scope.iter().map(|x| (*x).to_owned()).collect(),
            approved_by: Some(APPROVER.into()),
            approved_at: Some(approved_at),
            calls: if s.n == 0 {
                Vec::new()
            } else {
                calls(
                    now,
                    s.n,
                    (i as i64 + 1) * 37 * MIN,
                    s.users,
                    s.targets,
                    s.level,
                )
            },
        })
        .collect()
}

fn server(
    name: &str,
    transport: &str,
    endpoint: &str,
    approved_at: i64,
    request: Option<&str>,
    tools: Vec<McpTool>,
) -> McpServer {
    McpServer {
        name: name.into(),
        transport: transport.into(),
        endpoint: endpoint.into(),
        approved_by: Some(APPROVER.into()),
        approved_at: Some(approved_at),
        request: request.map(str::to_owned),
        tools,
    }
}

fn offered(list: &[(&str, &str, &str)]) -> Vec<McpOffered> {
    list.iter()
        .map(|(name, nature, description)| McpOffered {
            name: (*name).into(),
            nature: (*nature).into(),
            description: (*description).into(),
        })
        .collect()
}

fn made(now: i64) -> McpCatalog {
    let github_at = now - 71 * DAY;
    let github = tools(
        now,
        github_at,
        &[],
        &[
            Spec {
                name: "search_code",
                description: "Search for code across the repositories the token can read.",
                nature: "read",
                level: "allow",
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
                level: "allow",
                n: 41,
                users: &[KIM, LEE],
                targets: &["acme/payments#142", "acme/payments#150", "acme/web#77"],
            },
            Spec {
                name: "list_pull_requests",
                description: "List the pull requests of a repository.",
                nature: "read",
                level: "allow",
                n: 22,
                users: &[KIM, LEE, CI],
                targets: &["acme/payments state:open", "acme/web state:open"],
            },
            Spec {
                name: "add_issue_comment",
                description: "Add a comment to an issue or a pull request.",
                nature: "write",
                level: "allow",
                n: 9,
                users: &[KIM],
                targets: &["acme/payments#142", "acme/web#81"],
            },
            Spec {
                name: "create_pull_request",
                description: "Open a pull request from a branch.",
                nature: "write",
                level: "review",
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
                level: "deny",
                n: 0,
                users: &[],
                targets: &[],
            },
            Spec {
                name: "delete_repository",
                description: "Delete a repository and everything in it.",
                nature: "delete",
                level: "deny",
                n: 0,
                users: &[],
                targets: &[],
            },
        ],
    );

    let slack_at = now - 58 * DAY;
    let slack = tools(
        now,
        slack_at,
        &[],
        &[
            Spec {
                name: "search_messages",
                description: "Search the messages of the channels the bot has joined.",
                nature: "read",
                level: "allow",
                n: 30,
                users: &[KIM, PARK],
                targets: &["#dev-help 배포 오류", "#payments 환불 지연"],
            },
            Spec {
                name: "get_thread",
                description: "Read the replies of a thread.",
                nature: "read",
                level: "allow",
                n: 14,
                users: &[KIM, PARK],
                targets: &["#dev-help 1731.2201", "#payments 1731.0954"],
            },
            Spec {
                name: "post_message",
                description: "Post a message to a channel.",
                nature: "send",
                level: "review",
                n: 8,
                users: &[KIM],
                targets: &["#payments 배포 알림", "#dev-help 답변"],
            },
            Spec {
                name: "upload_file",
                description: "Upload a file to a channel.",
                nature: "send",
                level: "deny",
                n: 0,
                users: &[],
                targets: &[],
            },
        ],
    );

    let jira_at = now - 42 * DAY;
    let jira = tools(
        now,
        jira_at,
        &["결제팀"],
        &[
            Spec {
                name: "get_issue",
                description: "Get a Jira issue with its fields and comments.",
                nature: "read",
                level: "allow",
                n: 25,
                users: &[LEE, PARK],
                targets: &["PAY-381", "PAY-377", "PAY-390"],
            },
            Spec {
                name: "search_issues",
                description: "Search issues with JQL.",
                nature: "read",
                level: "allow",
                n: 18,
                users: &[LEE, PARK],
                targets: &["project = PAY AND status = Open"],
            },
            Spec {
                name: "create_issue",
                description: "Create an issue in a project.",
                nature: "write",
                level: "allow",
                n: 6,
                users: &[LEE],
                targets: &["PAY 환불 기간 오류"],
            },
            Spec {
                name: "transition_issue",
                description: "Move an issue to another status.",
                nature: "write",
                level: "allow",
                n: 4,
                users: &[LEE],
                targets: &["PAY-381 → Done"],
            },
        ],
    );

    let pg_at = now - 23 * DAY;
    let pg = tools(
        now,
        pg_at,
        &["결제팀"],
        &[
            Spec {
                name: "query",
                description: "Run a read-only SQL query.",
                nature: "read",
                level: "review",
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
                level: "allow",
                n: 7,
                users: &[LEE],
                targets: &["schema public"],
            },
        ],
    );

    // Approved long ago, and not used for over a month.
    let sentry_at = now - 96 * DAY;
    let mut sentry = tools(
        now,
        sentry_at,
        &[],
        &[
            Spec {
                name: "list_issues",
                description: "List the recent issues of a project.",
                nature: "read",
                level: "allow",
                n: 0,
                users: &[],
                targets: &[],
            },
            Spec {
                name: "get_issue_details",
                description: "Get an issue's stack trace and events.",
                nature: "read",
                level: "allow",
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

    let request = |id: &str,
                   at: i64,
                   (user, client): (&str, &str),
                   cwd: &str,
                   kind: &str,
                   server: &str,
                   wanted: &[&str],
                   message: &str,
                   attempts: i64| McpRequest {
        id: id.into(),
        at: now - at,
        user: user.into(),
        client: client.into(),
        cwd: Some(cwd.into()),
        kind: kind.into(),
        server: server.into(),
        wanted: wanted.iter().map(|w| (*w).to_owned()).collect(),
        message: message.into(),
        attempts,
        status: "pending".into(),
        ..McpRequest::default()
    };
    let mut notion = request(
        "r-1031",
        HOUR + 50 * MIN,
        PARK,
        "~/src/payments-retro",
        "server",
        "notion",
        &["search", "fetch"],
        "결제팀 장애 회고 문서가 Notion에 모여 있습니다. 회고를 정리할 때 에이전트가 관련 문서를 찾아 읽고 요약하게 하고 싶습니다. \
         읽기 도구(search, fetch)만 있으면 되고 쓰기는 필요 없습니다.",
        6,
    );
    notion.endpoint = Some("https://mcp.notion.com/mcp".into());
    notion.transport = Some("remote".into());
    notion.offered = offered(&[
        (
            "search",
            "read",
            "Search the pages and databases of the workspace.",
        ),
        ("fetch", "read", "Fetch a page's content."),
        ("create_pages", "write", "Create pages under a parent page."),
        (
            "update_page",
            "write",
            "Update a page's properties and content.",
        ),
    ]);
    let mut workflow = request(
        "r-1029",
        20 * HOUR,
        KIM,
        "~/src/payments",
        "tool",
        "github",
        &["run_workflow"],
        "PR을 만든 뒤 스테이징 배포 워크플로(deploy-staging.yml)를 바로 돌려서 확인하려고 합니다. 운영 배포 워크플로는 부르지 않겠습니다. \
         매번 확인 요청이 떠도 괜찮습니다.",
        3,
    );
    workflow.offered = offered(&[(
        "run_workflow",
        "write",
        "Trigger a GitHub Actions workflow run with the given inputs.",
    )]);
    let mut figma = request(
        "r-1027",
        DAY + HOUR,
        LEE,
        "~/src/web",
        "server",
        "figma",
        &["get_file", "get_comments"],
        "디자인 시안의 간격·색 값을 코드로 옮길 때 Figma 파일을 직접 읽게 하려고 합니다. 파일과 댓글 읽기만 씁니다.",
        4,
    );
    figma.endpoint = Some("https://mcp.figma.com/mcp".into());
    figma.transport = Some("remote".into());
    figma.offered = offered(&[
        ("get_file", "read", "Get a file's document tree and styles."),
        ("get_comments", "read", "List the comments of a file."),
        ("post_comment", "write", "Post a comment on a file."),
    ]);
    let jira_scope = request(
        "r-1022",
        5 * DAY,
        KIM,
        "~/src/payments",
        "scope",
        "atlassian",
        &["get_issue"],
        "결제팀과 같이 환불 버그(PAY-381)를 고치고 있어서 2주 동안 결제팀 Jira 티켓을 읽어야 합니다. get_issue만 열어 주세요.",
        2,
    );
    let mut pg_request = request(
        "r-0998",
        24 * DAY,
        LEE,
        "~/src/payments",
        "server",
        "postgres-ro",
        &["query", "list_tables"],
        "환불 데이터 정합성을 확인하려면 스테이징 DB를 조회해야 합니다. 읽기 전용 계정을 쓰겠습니다.",
        0,
    );
    pg_request.endpoint = Some(
        "npx -y @modelcontextprotocol/server-postgres postgres://readonly@db-stg:5432/app".into(),
    );
    pg_request.transport = Some("local".into());
    pg_request.offered = offered(&[
        ("query", "read", "Run a read-only SQL query."),
        ("list_tables", "read", "List the tables and their columns."),
    ]);
    pg_request.status = "approved".into();
    pg_request.decided_by = Some(APPROVER.into());
    pg_request.decided_at = Some(pg_at);
    pg_request.answer = Some(
        "스테이징 읽기 전용 계정으로만 엽니다. 결제팀 한정이고, query는 매번 확인 요청으로 둡니다."
            .into(),
    );
    let mut fs_request = request(
        "r-1018",
        5 * DAY - 2 * HOUR,
        PARK,
        "~/src/payments",
        "server",
        "filesystem",
        &["read_file"],
        "홈 폴더 밖의 설정 파일(~/.config 아래)을 같이 보려고 합니다.",
        2,
    );
    fs_request.endpoint = Some("npx -y @modelcontextprotocol/server-filesystem /".into());
    fs_request.transport = Some("local".into());
    fs_request.offered = offered(&[
        ("read_file", "read", "Read a file."),
        ("write_file", "write", "Write a file."),
        ("list_directory", "read", "List a directory."),
    ]);
    fs_request.status = "rejected".into();
    fs_request.decided_by = Some(APPROVER.into());
    fs_request.decided_at = Some(now - 4 * DAY);
    fs_request.answer = Some("작업 폴더 안의 파일은 이미 에이전트가 읽을 수 있습니다. 홈 폴더 밖까지 여는 서버는 허용하지 않습니다.".into());

    let refusal = |(user, client): (&str, &str),
                   server: &str,
                   tool: &str,
                   reason: &str,
                   first: i64,
                   last: i64,
                   count: i64,
                   request: Option<&str>| McpRefusal {
        first_at: now - first,
        last_at: now - last,
        count,
        user: user.into(),
        client: client.into(),
        server: server.into(),
        tool: Some(tool.into()),
        reason: reason.into(),
        request: request.map(str::to_owned),
    };
    let refused = vec![
        refusal(
            CI,
            "postgres-ro",
            "query",
            "scope",
            50 * MIN,
            41 * MIN,
            9,
            None,
        ),
        refusal(
            PARK,
            "notion",
            "search",
            "pending",
            40 * MIN,
            35 * MIN,
            2,
            Some("r-1031"),
        ),
        refusal(
            PARK,
            "notion",
            "search",
            "unapproved",
            3 * HOUR,
            2 * HOUR,
            6,
            Some("r-1031"),
        ),
        refusal(
            LEE,
            "figma",
            "get_file",
            "pending",
            22 * HOUR,
            22 * HOUR,
            1,
            Some("r-1027"),
        ),
        refusal(
            KIM,
            "github",
            "run_workflow",
            "pending",
            DAY,
            20 * HOUR,
            3,
            Some("r-1029"),
        ),
        refusal(
            LEE,
            "figma",
            "get_file",
            "unapproved",
            2 * DAY,
            DAY + 2 * HOUR,
            4,
            Some("r-1027"),
        ),
        refusal(
            LEE,
            "github",
            "merge_pull_request",
            "deny",
            4 * DAY,
            4 * DAY - 10 * MIN,
            2,
            None,
        ),
        refusal(
            PARK,
            "filesystem",
            "read_file",
            "unapproved",
            5 * DAY,
            5 * DAY - 3 * HOUR,
            2,
            Some("r-1018"),
        ),
        refusal(
            KIM,
            "atlassian",
            "get_issue",
            "scope",
            6 * DAY,
            5 * DAY + HOUR,
            2,
            Some("r-1022"),
        ),
        refusal(
            LEE,
            "github",
            "list_secrets",
            "unapproved",
            8 * DAY,
            8 * DAY,
            1,
            None,
        ),
        refusal(
            LEE,
            "github",
            "delete_repository",
            "deny",
            9 * DAY,
            9 * DAY,
            1,
            None,
        ),
    ];

    McpCatalog {
        version: "v12".into(),
        updated_at: Some(pg_at),
        servers: vec![
            server(
                "github",
                "remote",
                "https://mcp.example.com/github",
                github_at,
                None,
                github,
            ),
            server(
                "slack",
                "remote",
                "https://mcp.example.com/slack",
                slack_at,
                None,
                slack,
            ),
            server(
                "atlassian",
                "remote",
                "https://mcp.atlassian.example/v1",
                jira_at,
                None,
                jira,
            ),
            server(
                "postgres-ro",
                "local",
                "npx -y @modelcontextprotocol/server-postgres postgres://readonly@db-stg:5432/app",
                pg_at,
                Some("r-0998"),
                pg,
            ),
            server(
                "sentry",
                "remote",
                "https://mcp.sentry.example/sse",
                sentry_at,
                None,
                sentry,
            ),
        ],
        requests: vec![notion, workflow, figma, jira_scope, fs_request, pg_request],
        refused,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_approved_request_brings_its_tools_in_at_the_levels_chosen() {
        let now = 1_800_000_000_000;
        let mut c = made(now);
        let levels = vec![
            ("search".to_owned(), "allow".to_owned()),
            ("fetch".to_owned(), "review".to_owned()),
        ];
        decide(
            &mut c,
            now,
            "r-1031",
            true,
            "sec.choi",
            "읽기만 엽니다",
            &levels,
            &["dev.park".to_owned()],
        );
        let notion = c
            .servers
            .iter()
            .find(|s| s.name == "notion")
            .expect("added");
        let level = |t: &str| {
            notion
                .tools
                .iter()
                .find(|x| x.name == t)
                .unwrap()
                .level
                .clone()
        };
        assert_eq!(
            (level("search"), level("fetch"), level("create_pages")),
            ("allow".into(), "review".into(), "deny".into())
        );
        assert_eq!(notion.request.as_deref(), Some("r-1031"));
        let r = c.requests.iter().find(|r| r.id == "r-1031").unwrap();
        assert_eq!(
            (r.status.as_str(), r.answer.as_deref()),
            ("approved", Some("읽기만 엽니다"))
        );
    }

    #[test]
    fn a_scope_request_opens_the_tool_to_the_person() {
        let now = 1_800_000_000_000;
        let mut c = made(now);
        decide(&mut c, now, "r-1022", true, "sec.choi", "", &[], &[]);
        let t = find(&mut c, "atlassian", "get_issue").unwrap();
        assert_eq!(t.scope, ["결제팀", "dev.kim"]);
        decide(
            &mut c,
            now,
            "r-1027",
            false,
            "sec.choi",
            "디자인팀 계정으로 요청해 주세요",
            &[],
            &[],
        );
        assert!(
            c.servers.iter().all(|s| s.name != "figma"),
            "a rejected request adds nothing"
        );
    }
}
