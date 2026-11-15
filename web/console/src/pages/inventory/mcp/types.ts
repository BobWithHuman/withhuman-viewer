// The MCP part of the inventory, as the app hands it over (`src/admin/inventory_web.rs`): the
// servers set up on the machines, their tools, and how the engine judged each tool's calls over
// the last 30 days.

import { VERDICT, type Nature, type Verdict } from "@/kit";

export type Call = { at: number; user: string; client: string; target: string; decision: "ALLOW" | "REVIEW" | "DENY"; ran: boolean };
export type Tool = {
  name: string; description: string; nature: Nature;
  /** The engine's verdicts on its calls over the last 30 days; `calls` is their sum. */
  calls: number; allow: number; review: number; deny: number;
  users: { user: string; client: string; n: number }[];
  /** Its last call ever. */
  last: number | null;
  /** Calls day by day over the last 30 days, the oldest first. */
  days: number[];
  recent: Call[];
};
/** A machine a server is set up on; `client` is the machine's host name, or its ID. */
export type Install = { user: string; client: string; at: number };
export type Server = { name: string; transport: "remote" | "local"; endpoint: string; installs: Install[]; tools: Tool[] };
export type Mcp = {
  /** Every MCP call of the last 30 days by verdict, day by day, the oldest first. */
  days?: Record<Verdict, number>[];
  servers: Server[];
};

export const DAYS = 30;
export const VERDICTS: Verdict[] = ["allow", "review", "deny"];
export const NATURES: Nature[] = ["read", "write", "delete", "send"];
/** Where a tool stands on the board: the strongest verdict its calls got, or never called. */
type Column = Verdict | "none";
export const COLUMNS: Column[] = ["allow", "review", "deny", "none"];
export const COLUMN: Record<Column, { label: string; color: string }> = {
  allow: { label: "허용", color: VERDICT.allow.color },
  review: { label: "확인 요청", color: VERDICT.review.color },
  deny: { label: "차단", color: VERDICT.deny.color },
  none: { label: "호출 없음", color: "var(--info)" },
};
export const column = (t: Tool): Column => (t.deny > 0 ? "deny" : t.review > 0 ? "review" : t.calls > 0 ? "allow" : "none");
export const VERDICT_OF: Record<Call["decision"], Verdict> = { ALLOW: "allow", REVIEW: "review", DENY: "deny" };

export const toolKey = (server: string, tool: string) => `${server}\u0000${tool}`;
/** A tool's or a server's calls as the parts of one bar. */
export const split = (of: Pick<Tool, Verdict>[]) =>
  VERDICTS.map((v) => ({ key: v, label: COLUMN[v].label, color: COLUMN[v].color, value: of.reduce((a, t) => a + t[v], 0) }));
/** The machines a server is set up on, each once. */
export const machinesOf = (s: Server) => new Set(s.installs.map((i) => i.client)).size;
/** Whether an install is on this machine. */
export const isOn = (client: string, m: { host: string | null; id: string }) => client === m.host || client === m.id;
