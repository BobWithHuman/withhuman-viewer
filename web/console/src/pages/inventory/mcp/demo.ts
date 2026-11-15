// The MCP servers the demo inventory shows: eight servers set up on the demo's own machines,
// their tools with a month of calls and the engine's verdicts on them, made the same way every
// time from a fixed seed.

import type { Nature } from "@/kit";
import { DAYS, VERDICT_OF, type Call, type Install, type Mcp, type Server, type Tool } from "./types";

const MIN = 60e3, DAY = 24 * 60 * MIN;

type Spec = {
  name: string; nature: Nature; description: string;
  /** Calls in the 30 days, how many of the server's machines made them, and how many were held and blocked. */
  n: number; by: number; review: number; deny: number;
  /** Days since its last call, for a tool not called in the 30 days. */
  old: number | null; targets: string[];
};
const tool = (name: string, nature: Nature, description: string, n = 0, by = 0, targets: string[] = [], review = 0, deny = 0, old: number | null = null): Spec =>
  ({ name, nature, description, n, by, review, deny, old, targets });

const SERVERS: { name: string; transport: Server["transport"]; endpoint: string; on: number[]; tools: Spec[] }[] = [
  { name: "github", transport: "remote", endpoint: "https://api.githubcopilot.com/mcp/", on: [0, 1, 2, 4, 6, 8, 9, 11, 14, 17, 20], tools: [
    tool("search_code", "read", "Search for code across the repositories the token can read.", 412, 8, ["repo:acme/payments refund_window", "org:acme verify_signature", "repo:acme/web useSession", "repo:acme/infra aws_iam_role path:modules"]),
    tool("get_file_contents", "read", "Get the contents of a file or a directory at a ref.", 388, 9, ["acme/web src/session/useSession.ts@main", "acme/payments internal/refund/window.go@main", "acme/infra modules/iam/main.tf@main", "acme/collector cmd/collector/main.go@release-0.9"]),
    tool("get_issue", "read", "Get the title, body and comments of an issue.", 141, 5, ["acme/payments#142", "acme/payments#150", "acme/web#77", "acme/gateway#311"]),
    tool("list_pull_requests", "read", "List the pull requests of a repository.", 96, 4, ["acme/payments state:open", "acme/web state:open", "acme/gateway state:open author:@me"]),
    tool("add_issue_comment", "write", "Add a comment to an issue or a pull request.", 44, 3, ["acme/payments#142", "acme/web#81", "acme/gateway#311"], 3),
    tool("create_pull_request", "write", "Open a pull request from a branch.", 61, 4, ["acme/payments fix/refund-window → main", "acme/web chore/deps → main", "acme/gateway feat/tls-rotate → main"], 9),
    tool("merge_pull_request", "write", "Merge a pull request.", 12, 2, ["acme/web#84 squash", "acme/payments#151 merge", "acme/infra#39 squash"], 4, 5),
    tool("delete_repository", "delete", "Delete a repository and everything in it."),
  ] },
  { name: "slack", transport: "remote", endpoint: "https://mcp.slack.com/mcp", on: [1, 3, 5, 9, 12, 15, 18], tools: [
    tool("search_messages", "read", "Search the messages of the channels the bot has joined.", 132, 5, ["in:#payments-incident refund", "in:#web-release rollback", "from:@oncall 502", "in:#data-platform backfill"]),
    tool("get_channel_history", "read", "Read the latest messages of a channel.", 77, 3, ["#payments-incident", "#web-release", "#oncall"]),
    tool("list_channels", "read", "List the channels of the workspace.", 41, 3, ["types:public_channel", "types:public_channel,private_channel"]),
    tool("post_message", "send", "Post a message to a channel or a person.", 58, 4, ["#web-release 배포 완료 v2.14.1", "#payments-incident 환불 창 수정 PR 올림", "#general 장애 회고 초안", "@ops.choi 스테이징 DB 접속 정보"], 7, 2),
    tool("upload_file", "send", "Upload a file to a channel.", 9, 2, ["#web-release build.log", "#data-platform customers_2026q3.csv", "@partner-acme .env.production"], 2, 3),
  ] },
  { name: "atlassian", transport: "remote", endpoint: "https://mcp.atlassian.com/v1/sse", on: [0, 4, 7, 10, 13, 16], tools: [
    tool("search_issues", "read", "Search Jira issues with JQL.", 118, 5, ["project = PAY AND status = \"In Progress\"", "assignee = currentUser() AND sprint in openSprints()", "project = WEB AND labels = regression"]),
    tool("get_issue", "read", "Get a Jira issue with its fields and comments.", 93, 4, ["PAY-1182", "WEB-640", "PLAT-207", "DATA-88"]),
    tool("get_page", "read", "Get a Confluence page as Markdown.", 52, 3, ["ENG/Runbook: 결제 장애 대응", "ENG/ADR-031 게이트웨이 TLS", "DATA/Feature store 스키마"]),
    tool("create_issue", "write", "Create a Jira issue.", 27, 3, ["PAY Bug: 환불 창이 UTC 기준으로 닫힘", "WEB Task: 세션 갱신 재시도", "PLAT Task: collector 메모리 프로파일"]),
    tool("update_issue", "write", "Change the fields or the status of a Jira issue.", 33, 2, ["PAY-1182 status → Done", "WEB-640 assignee → front.shin", "PLAT-207 priority → High"], 2),
    tool("delete_issue", "delete", "Delete a Jira issue.", 3, 1, ["PAY-1175", "PAY-1176", "WEB-611"], 0, 3),
  ] },
  { name: "postgres-ro", transport: "local", endpoint: "npx -y @modelcontextprotocol/server-postgres postgresql://readonly@db.internal.example.com:5432/app", on: [4, 5, 6, 19, 21], tools: [
    tool("query", "read", "Run a read-only SQL query.", 264, 4, ["SELECT count(*) FROM orders WHERE created_at > now() - interval '1 day'", "SELECT id, status FROM refunds WHERE order_id = 88213", "EXPLAIN ANALYZE SELECT * FROM ledger_entries WHERE account_id = 42", "SELECT email, phone FROM customers LIMIT 500", "SELECT card_last4, billing_address FROM payment_methods"], 11, 6),
    tool("list_tables", "read", "List the tables of a schema.", 71, 3, ["public", "billing", "analytics"]),
    tool("describe_table", "read", "Show the columns and indexes of a table.", 88, 4, ["public.orders", "billing.refunds", "public.customers", "billing.ledger_entries"]),
  ] },
  { name: "sentry", transport: "remote", endpoint: "https://mcp.sentry.dev/mcp", on: [2, 8, 9, 12], tools: [
    tool("search_issues", "read", "Search the issues of a project.", 64, 3, ["project:web is:unresolved TypeError", "project:payments level:error refund", "project:ios-app release:5.2.0"]),
    tool("get_issue_details", "read", "Get an issue with its latest event and stack trace.", 82, 3, ["WEB-3F2", "PAYMENTS-91A", "IOS-APP-17C", "WEB-41B"]),
    tool("get_trace", "read", "Get the spans of a trace.", 21, 2, ["a1f4c09e77b24d3e", "0c7be2d15a9f4e61"]),
    tool("update_issue", "write", "Resolve, ignore or assign an issue.", 6, 1, ["WEB-3F2 status → resolved", "PAYMENTS-91A assignedTo → pay.hong"], 1),
  ] },
  { name: "notion", transport: "remote", endpoint: "https://mcp.notion.com/mcp", on: [10, 11, 13], tools: [
    tool("search", "read", "Search the pages and databases of the workspace.", 47, 3, ["디자인 토큰", "온콜 로테이션", "2026 Q4 로드맵"]),
    tool("fetch", "read", "Get a page or a database by its URL or ID.", 59, 3, ["Design system / Tokens", "Eng wiki / 배포 체크리스트", "Product / Q4 roadmap"]),
    tool("create_pages", "write", "Create pages under a page or in a database.", 11, 2, ["Eng wiki / 회고 2026-11-12", "Design system / Tailwind 4 이전 메모"], 2),
    tool("update_page", "write", "Change a page's properties or content.", 0, 0, ["Eng wiki / 배포 체크리스트"], 0, 0, 44),
  ] },
  { name: "figma", transport: "local", endpoint: "http://127.0.0.1:3845/mcp", on: [10, 12], tools: [
    tool("get_design_context", "read", "Get the layout, styles and code hints of the selected node.", 36, 2, ["Console / Inventory 1:204", "Design system / Button 12:88", "Landing / Hero 4:19"]),
    tool("get_screenshot", "read", "Render the selected node as an image.", 28, 2, ["Console / Inventory 1:204", "Landing / Hero 4:19"]),
    tool("get_variable_defs", "read", "List the variables and styles a node uses.", 9, 1, ["Design system / Button 12:88"]),
  ] },
  { name: "filesystem", transport: "local", endpoint: "npx -y @modelcontextprotocol/server-filesystem /Users/song/work /Users/song/Downloads", on: [22], tools: [
    tool("read_text_file", "read", "Read a file under the allowed directories as text."),
    tool("write_file", "write", "Create a file or overwrite one under the allowed directories."),
  ] },
];

/** The demo's servers, set up on the machines given (a person and the machine's host name each). */
export function demoMcp(machines: Omit<Install, "at">[], now: number): Mcp {
  let seed = 20261115;
  const rnd = () => ((seed = (seed * 1664525 + 1013904223) >>> 0) / 2 ** 32);
  const days = Array.from({ length: DAYS }, () => ({ allow: 0, review: 0, deny: 0 }));
  const dayOf = (at: number) => DAYS - 1 - Math.floor((now - at) / DAY);

  const servers = SERVERS.map((s): Server => {
    const installs = s.on.map((i) => ({ ...machines[i % machines.length], at: now - Math.floor((4 + rnd() * 80) * DAY) }));
    const tools = s.tools.map((spec): Tool => {
      // More calls lately and few at the weekend, the first machines calling the most.
      const all: Call[] = [];
      while (all.length < spec.n) {
        const at = now - Math.floor(rnd() ** 1.5 * (DAYS * DAY - 1));
        const weekday = new Date(at).getDay() % 6 !== 0;
        if (!weekday && rnd() < 0.85) continue;
        const i = all.length;
        const decision = i < spec.deny ? "DENY" : i < spec.deny + spec.review ? "REVIEW" : "ALLOW";
        const { user, client } = installs[Math.floor(rnd() ** 1.8 * spec.by)];
        // What is held or blocked draws on the riskier targets, listed last.
        const target = decision === "ALLOW" ? spec.targets[i % Math.max(1, spec.targets.length - 1)] : spec.targets[spec.targets.length - 1 - (i % 2)] ?? spec.targets[0];
        all.push({ at, user, client, target, decision, ran: decision === "ALLOW" || (decision === "REVIEW" && i % 3 !== 0) });
      }
      all.sort((a, b) => b.at - a.at);
      const byDay = Array.from({ length: DAYS }, () => 0);
      const users = new Map<string, Tool["users"][number]>();
      const count = { allow: 0, review: 0, deny: 0 };
      for (const c of all) {
        const v = VERDICT_OF[c.decision];
        byDay[dayOf(c.at)]++;
        days[dayOf(c.at)][v]++;
        count[v]++;
        const key = `${c.user} ${c.client}`;
        users.set(key, { user: c.user, client: c.client, n: (users.get(key)?.n ?? 0) + 1 });
      }
      const before: Call[] = spec.old === null ? [] : [{ user: installs[0].user, client: installs[0].client, at: now - spec.old * DAY, target: spec.targets[0], decision: "ALLOW", ran: true }];
      return {
        name: spec.name, description: spec.description, nature: spec.nature, calls: all.length, ...count,
        users: [...users.values()].sort((a, b) => b.n - a.n), last: all[0]?.at ?? before[0]?.at ?? null,
        days: byDay, recent: [...all, ...before].slice(0, 12),
      };
    });
    return { name: s.name, transport: s.transport, endpoint: s.endpoint, installs, tools };
  });
  return { days, servers };
}
