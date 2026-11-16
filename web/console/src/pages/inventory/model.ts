// The inventory as the app hands it over (`src/admin/inventory_web.rs`), and what the page works
// out from it: which sessions are still going within the window picked and how each machine
// stands.

import { Eye, FileText, ShieldCheck, type LucideIcon } from "lucide-react";
import { isOn, type Mcp } from "./mcp/types";

export type Device = {
  id: string; host: string | null; os: string | null;
  agent: string | null; hook_version: string | null;
  registered_at: number | null; first_seen: number | null; last: number | null;
  sessions: number; permission_mode: string | null; engine_mode: string | null;
  /** The hardware serial number. */
  serial?: string | null;
  /** The EDR's agent on the machine, when the engine matched one. */
  edr?: { id: string; status: string | null } | null;
  /** The security team stopped the machine: every tool call from it is refused. */
  stopped?: { at: number; by: string | null; reason: string | null; denied: number | null } | null;
};
/** A machine the EDR lists that has no hooks: maybe with an agent program the EDR saw on it. */
export type EdrOnly = { id: string; name: string; os: string | null; status: string | null; serial: string | null; last: number | null; agent: string | null };
export const EDR_STATUS: Record<string, string> = { active: "연결됨", disconnected: "끊김", never_connected: "연결된 적 없음", pending: "대기" };
export type Project = { device: string; project: string; agent: string | null; hook_version: string | null; installed_at: number | null };
/** Shadow AI: a run of a coding agent the EDR's scans saw from `first` to `last_seen` (`scans`
 * of them) on a machine with no Hooks session open then (`no_session`), or on one that never
 * registered (`unregistered`); or a project a check found with no hooks (`project`). */
export type Shadow = {
  device: string | null; host: string | null; agent: string; project: string | null; last_seen: number | null;
  first?: number | null; scans?: number;
  why?: "project" | "no_session" | "unregistered"; edr?: string | null;
};
export const WHY: Record<string, string> = { project: "훅 없는 프로젝트", no_session: "훅 세션 없음", unregistered: "미등록 기기" };
export type Run = {
  id: string; device: string | null; host: string | null;
  title: string | null; folder: string | null; opens: boolean; source: string | null;
  permission_mode: string | null; model: string | null;
  created_at: number | null; last_seen_at: number | null; ended_at: number | null; end_reason: string | null;
};
/** The app's answer to the request with the same `req`. */
export type Reply = {
  req: number; ok: boolean; error?: string; done?: "admin" | "device_stopped" | "device_released";
};
export type Tab = "sessions" | "shadow" | "mcp";
export type SessionCalls = { n: number; review: number; deny: number; mode: string | null };
export type Data = {
  status: "ready" | "loading" | "failed" | "none"; error: string | null; now: number;
  source: { label: string; demo: boolean }; admin: { api: string; has_token: boolean };
  actor: string; reply: Reply | null; shadow_known?: boolean;
  devices?: Device[]; projects?: Project[]; shadow?: Shadow[]; sessions?: Run[];
  /** Each session's calls (how many, held for the user, blocked) and the engine's mode at its latest. */
  session_calls?: Record<string, SessionCalls>;
  /** Whether the engine reads an EDR's machines, and those of them with no hooks. */
  edr_known?: boolean; edr_only?: EdrOnly[];
  /** The tab the lists open on. */
  tab?: Tab;
  /** The MCP servers on the machines; null while the engine has not collected them. */
  mcp?: Mcp | null;
};

// ---------------------------------------------------------------- words and colours
export const ENGINE: Record<string, { label: string; color: string; icon: LucideIcon }> = {
  enforce: { label: "차단", color: "var(--allow)", icon: ShieldCheck },
  shadow: { label: "시범 운영", color: "var(--review)", icon: Eye },
  record: { label: "기록만", color: "var(--info)", icon: FileText },
};
export const ENGINE_KEYS = ["enforce", "shadow", "record"];
export type State = "running" | "idle" | "installed" | "unregistered";
export const STATE: Record<State, { label: string; cls: string }> = {
  running: { label: "활동 중", cls: "bg-allow/15 text-allow" },
  idle: { label: "쉬는 중", cls: "bg-white/8 text-foreground/80" },
  installed: { label: "설치만 됨", cls: "bg-catch/15 text-catch" },
  unregistered: { label: "등록 안 됨", cls: "bg-white/8 text-muted-foreground" },
};
export const AGENT: Record<string, { label: string; color: string }> = {
  "claude-code": { label: "Claude Code", color: "var(--send)" },
  codex: { label: "Codex", color: "var(--money)" },
};
export const agentLabel = (a: string | null) => (a ? AGENT[a]?.label ?? a : "–");
export const SOURCE: Record<string, string> = { startup: "새로 시작", resume: "이어 하기", clear: "지우고 시작", compact: "요약 뒤", fork: "갈라짐" };
export const END: Record<string, string> = { prompt_input_exit: "종료", logout: "로그아웃", clear: "지움", resume: "이어 하기로" };

// ---------------------------------------------------------------- what the page works out
export const DAY = 24 * 60 * 60e3;
export type Machine = Device & { name: string; state: State; running: boolean; shadow: number; projects: number };

export const hostOf = (d: { host: string | null; id: string }) => d.host ?? d.id;

export type View = ReturnType<typeof derive>;
export function derive(d: Data, windowMin: number) {
  const now = d.now, since = now - windowMin * 60e3;
  const going = (r: Run) => !r.ended_at && r.last_seen_at != null && r.last_seen_at >= since;
  const sessions = [...(d.sessions ?? [])].sort((a, b) => Number(going(b)) - Number(going(a)) || (b.last_seen_at ?? b.created_at ?? 0) - (a.last_seen_at ?? a.created_at ?? 0));
  const live = new Set(sessions.filter(going).map((r) => r.device));
  const shadow = d.shadow ?? [], projects = d.projects ?? [];
  const count = <T,>(list: T[], key: (t: T) => string | null) => {
    const m = new Map<string, number>();
    for (const t of list) { const k = key(t); if (k) m.set(k, (m.get(k) ?? 0) + 1); }
    return m;
  };
  const shadowOf = count(shadow, (a) => a.device), projectsOf = count(projects, (p) => p.device);
  const rank: Record<State, number> = { running: 0, idle: 1, installed: 2, unregistered: 3 };
  const machines: Machine[] = (d.devices ?? []).map((x) => {
    const running = live.has(x.id);
    const state: State = !x.registered_at ? "unregistered" : running ? "running" : x.last == null ? "installed" : "idle";
    return { ...x, name: hostOf(x), state, running, shadow: shadowOf.get(x.id) ?? 0, projects: projectsOf.get(x.id) ?? 0 };
  }).sort((a, b) => rank[a.state] - rank[b.state] || (b.last ?? 0) - (a.last ?? 0));
  const registered = machines.filter((m) => m.state !== "unregistered");
  const mcp = d.mcp ?? null;
  return {
    mcp,
    /** The MCP servers set up on a machine. */
    serversOn: (m: Machine) => (mcp?.servers ?? []).filter((s) => s.installs.some((i) => isOn(i.client, m))),
    now, going, sessions, machines, registered, shadow, projects,
    shadowKnown: d.shadow_known ?? false,
    edrKnown: d.edr_known ?? false, edrOnly: d.edr_only ?? [],
    sessionCalls: d.session_calls ?? {},
    unregistered: machines.length - registered.length,
    runningSessions: sessions.filter(going).length,
    noId: sessions.filter((r) => !r.device).length,
  };
}
