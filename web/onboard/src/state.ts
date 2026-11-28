// The onboarding's data and messages, exactly as the app has them (src/ui/onboard.rs, `OnboardPage`;
// the same as web/console's pages/onboard.tsx and web/langflow's withhuman/onboard):
//   in:  window.WHOnboard.render(data), called on load and whenever the state changes;
//   out: send(m) = parent.__whOnboardSend(m), m one of
//        { do: "drag" } { do: "zoom" }
//        { do: "check_jev", key } { do: "model", model } { do: "pick", agent } { do: "python" }
//        { do: "start", jev, key, model, agents } { do: "open", try }
//        { do: "server_test", url, user, database, password, engine, key } { do: "server_save", ...same }
// Opened on its own (no app), what is sent is logged and answered here as the app would, from demo
// data; the query or hash sets a state to look at (see `fromQuery`).

import { q } from "./lib";

export type State = "idle" | "busy" | "ok" | "err";
export type Status = { state: State; text: string | null };
export type Agent = {
  kind: string; name: string; found: boolean; per_folder: boolean; per_profile: boolean;
  /** The folder picked for an agent set up per project folder. */
  folder: string | null;
} & Status;
export type Data = {
  /** `developer`: a developer's machine, set up on the company's engine first. */
  role?: "personal" | "developer";
  /** Python 3.8 or later, which the hooks run with; none when there is none. */
  python: string | null;
  /** A Jev key is already kept in the keychain. */
  kept_key: boolean;
  jev: Status & { models: string[] };
  model: string;
  parts: { name: string; up: boolean }[];
  agents: Agent[];
  run: Status;
  server: Status & { sessions: number | null };
  /** Everything is up: the app can open. */
  done: boolean;
};

/** Where the app's own engine listens. */
export const ENGINE = "127.0.0.1:47380";
/** The store form of the company's engine, as the old page had it. */
export const STORE = { url: "postgres://127.0.0.1:15432/withhuman", user: "withhuman_viewer", database: "withhuman", password: "", engine: "", key: "" };
/** Where an agent's hook goes: every folder (Claude Code), one folder, or a profile. */
export const scope = (a: Agent) => (a.kind === "claude" ? "모든 폴더" : a.per_folder ? "폴더별" : "프로필");

/** Before the app's first data: nothing found yet. */
export const EMPTY: Data = {
  python: null, kept_key: false, jev: { state: "idle", text: null, models: [] }, model: "jev-1.14.0",
  parts: [], agents: [], run: { state: "idle", text: null }, server: { state: "idle", text: null, sessions: null }, done: false,
};

type Host = { __whOnboardSend?: (m: unknown) => void };
const host = (): Host | null => { try { const p = parent as unknown as Host; return p !== (window as unknown) && p.__whOnboardSend ? p : null; } catch { return null; } };
/** Opened on its own: no app to answer. */
export const standalone = !host();

let data: Data = EMPTY, fresh = false;
const subs: ((d: Data, first: boolean) => void)[] = [];
export const onData = (f: (d: Data, first: boolean) => void) => { subs.push(f); f(data, false); };
const put = (d: Data) => { const first = !fresh; fresh = true; data = d; subs.forEach((f) => f(d, first)); };
export const current = () => data;

/** Asks the app. */
export function send(m: Record<string, unknown>) {
  const h = host();
  if (h) { h.__whOnboardSend!(m); return; }
  if (m.do !== "drag") console.log("[onboard] send", JSON.stringify(m));
  void answer(m);
}

(window as unknown as { WHOnboard: { render: (d: Data) => void } }).WHOnboard = { render: (d) => put(d ?? EMPTY) };

// ---- Standalone: demo data, and the app's answers played out ----
const PARTS = ["PostgreSQL", "Decision", "Generator", "Collector"];
const MODELS = ["jev-1.14.0", "jev-1.13.2", "jev-latest"];
const FOLDER = "~/work/shop";
const AGENTS = [
  { kind: "claude", name: "Claude Code", found: true, per_folder: false, per_profile: false },
  { kind: "codex", name: "Codex", found: true, per_folder: true, per_profile: false },
  { kind: "cursor", name: "Cursor", found: true, per_folder: true, per_profile: false },
  { kind: "copilot", name: "Copilot", found: false, per_folder: true, per_profile: false },
  { kind: "antigravity", name: "Antigravity", found: false, per_folder: true, per_profile: false },
  { kind: "hermes", name: "Hermes", found: false, per_folder: false, per_profile: true },
  { kind: "openclaw", name: "OpenClaw", found: false, per_folder: false, per_profile: true },
].map((a): Agent => ({ ...a, folder: null, state: "idle", text: null }));
export const DEMO: Data = {
  role: "personal", python: "/opt/homebrew/bin/python3", kept_key: false,
  jev: { state: "idle", text: null, models: [] }, model: "jev-1.14.0",
  parts: PARTS.map((name) => ({ name, up: false })), agents: AGENTS,
  run: { state: "idle", text: null }, server: { state: "idle", text: null, sessions: null }, done: false,
};

/**
 * A state to look at, from the query or hash (the same keys as web/langflow's demo):
 *   scene=intro|gate|where|judge|agents|ready|store   still   role=developer
 *   judge=jev  jev=busy|ok|err  kept=1        folder=1  python=0  agents=busy|done|err
 *   run=busy|ok|err  up=0..4  done=1          server=busy|ok|err
 */
function fromQuery(): Data {
  const d: Data = structuredClone(DEMO);
  if (q.get("role") === "developer") d.role = "developer";
  if (q.get("kept") === "1") d.kept_key = true;
  if (q.get("python") === "0") d.python = null;
  const jev = q.get("jev");
  if (jev === "busy") d.jev = { state: "busy", text: null, models: [] };
  if (jev === "ok") d.jev = { state: "ok", text: "모델 3", models: MODELS };
  if (jev === "err") d.jev = { state: "err", text: "401 Unauthorized", models: [] };
  if (q.get("folder") === "1") d.agents = d.agents.map((a) => (a.kind === "codex" ? { ...a, folder: FOLDER } : a));
  const up = Number(q.get("up") ?? (q.get("run") === "ok" ? PARTS.length : 0));
  d.parts = PARTS.map((name, i) => ({ name, up: i < up }));
  const run = q.get("run");
  if (run === "busy") d.run = { state: "busy", text: null };
  if (run === "ok") d.run = { state: "ok", text: "실행 중" };
  if (run === "err") d.run = { state: "err", text: "port 47380 in use" };
  const agents = q.get("agents");
  const states: Record<string, Record<string, State>> = {
    busy: { claude: "ok", codex: "busy", cursor: "idle" },
    done: { claude: "ok", codex: "ok", cursor: "ok" },
    err: { claude: "ok", codex: "ok", cursor: "err" },
  };
  if (agents && states[agents])
    d.agents = d.agents.map((a) => {
      const s = states[agents][a.kind]; if (!s) return a;
      return { ...a, folder: a.per_folder ? (a.kind === "codex" ? FOLDER : "~/work/web") : a.folder, state: s,
        text: s === "err" ? "hooks.json: permission denied" : s === "ok" ? "설치됨" : null };
    });
  const server = q.get("server");
  if (server === "busy") d.server = { state: "busy", text: null, sessions: null };
  if (server === "ok") d.server = { state: "ok", text: "세션 128", sessions: 128 };
  if (server === "err") d.server = { state: "err", text: "connection refused", sessions: null };
  if (q.get("done") === "1") d.done = true;
  return d;
}

const patch = (p: Partial<Data>) => put({ ...data, ...p });
const agent = (kind: string, p: Partial<Agent>) => patch({ agents: data.agents.map((a) => (a.kind === kind ? { ...a, ...p } : a)) });
const later = (ms: number) => new Promise((r) => setTimeout(r, ms));

/** What the app does with each message, played out in the page. */
async function answer(m: Record<string, unknown>) {
  switch (m.do) {
    case "check_jev": {
      patch({ jev: { state: "busy", text: null, models: [] } });
      await later(1200);
      if (String(m.key ?? "").startsWith("jev_")) patch({ jev: { state: "ok", text: `모델 ${MODELS.length}`, models: MODELS } });
      else patch({ jev: { state: "err", text: "401 Unauthorized", models: [] } });
      break;
    }
    case "model": patch({ model: String(m.model) }); break;
    case "pick": agent(String(m.agent), { folder: m.agent === "codex" ? FOLDER : "~/work/web" }); break;
    case "python": await later(1500); patch({ python: "/opt/homebrew/bin/python3" }); break;
    case "start": {
      const chosen = (m.agents as string[]) ?? [];
      patch({ run: { state: "busy", text: null }, done: false, agents: data.agents.map((a) => (chosen.includes(a.kind) ? { ...a, state: "idle", text: null } : a)) });
      for (let i = 0; i < data.parts.length; i++) { await later(650); patch({ parts: data.parts.map((p, j) => (j <= i ? { ...p, up: true } : p)) }); }
      patch({ run: { state: "ok", text: "실행 중" } });
      for (const kind of chosen) { agent(kind, { state: "busy", text: null }); await later(800); agent(kind, { state: "ok", text: "설치됨" }); }
      patch({ done: true });
      break;
    }
    case "server_test":
      patch({ server: { state: "busy", text: null, sessions: null } });
      await later(1000);
      patch({ server: { state: "ok", text: "세션 128", sessions: 128 } });
      break;
    case "server_save": patch({ server: { ...data.server, state: "busy" } }); break;
  }
}

/** Standalone: the demo's data, right away. */
export function startDemo() {
  if (!standalone) return;
  put(fromQuery());
  (window as unknown as { __whOnboardDemo: (p: Partial<Data>) => void }).__whOnboardDemo = patch;
}
