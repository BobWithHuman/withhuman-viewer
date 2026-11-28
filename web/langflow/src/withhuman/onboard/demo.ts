// The first launch opened on its own (no app): `#/onboard` shows the onboarding over the demo
// session, and what it asks is answered here as the app would (a key checked, the engine's parts
// coming up one by one, each hook installed in turn, the app opened). Query parameters set a state
// to look at directly, for screenshots:
//   #/onboard?step=judge&judge=jev&jev=busy|ok|err&kept=1
//   #/onboard?step=agents&hooks=pick&folder=1&python=0&agents=busy|done|err
//   #/onboard?step=ready&run=busy|ok|err&up=2&agents=busy&done=1
//   #/onboard?role=developer&step=store&server=busy|ok|err
// `window.__whOnboardDemo(patch)` changes the demo's data in place.

import { onDemoSend, setDemo, standalone } from "../bridge";
import type { OnboardAgent, OnboardData, State } from "./data";

const AGENTS = [
  { kind: "claude", name: "Claude Code", found: true, per_folder: false, per_profile: false },
  { kind: "codex", name: "Codex", found: true, per_folder: true, per_profile: false },
  { kind: "cursor", name: "Cursor", found: true, per_folder: true, per_profile: false },
  { kind: "copilot", name: "Copilot", found: false, per_folder: true, per_profile: false },
  { kind: "antigravity", name: "Antigravity", found: false, per_folder: true, per_profile: false },
  { kind: "hermes", name: "Hermes", found: false, per_folder: false, per_profile: true },
  { kind: "openclaw", name: "OpenClaw", found: false, per_folder: false, per_profile: true },
].map((a): OnboardAgent => ({ ...a, folder: null, state: "idle", text: null }));

const PARTS = ["PostgreSQL", "Decision", "Generator", "Collector"];

export const DEMO_ONBOARD: OnboardData = {
  role: "personal",
  python: "/opt/homebrew/bin/python3",
  kept_key: false,
  jev: { state: "idle", text: null, models: [] },
  model: "jev-1.14.0",
  parts: PARTS.map((name) => ({ name, up: false })),
  agents: AGENTS,
  run: { state: "idle", text: null },
  server: { state: "idle", text: null, sessions: null },
  done: false,
};

const MODELS = ["jev-1.14.0", "jev-1.13.2", "jev-latest"];
const FOLDER = "~/work/shop";

/** The step the demo opens on, and the choices made before it (from the hash). */
export const demoStart: { step?: string; judge?: string; hooks?: string } = {};

let data: OnboardData = DEMO_ONBOARD;
const put = (next: OnboardData) => {
  data = next;
  setDemo((s) => ({ ...s, views: { ...s.views, onboard: data } }));
};
const patch = (p: Partial<OnboardData>) => put({ ...data, ...p });
const agent = (kind: string, p: Partial<OnboardAgent>) =>
  patch({ agents: data.agents.map((a) => (a.kind === kind ? { ...a, ...p } : a)) });
const later = (ms: number) => new Promise((r) => window.setTimeout(r, ms));

function fromHash(q: URLSearchParams): OnboardData {
  const d: OnboardData = structuredClone(DEMO_ONBOARD);
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
  const chosen = ["claude", "codex", "cursor"];
  const states: Record<string, Record<string, State>> = {
    busy: { claude: "ok", codex: "busy", cursor: "idle" },
    done: { claude: "ok", codex: "ok", cursor: "ok" },
    err: { claude: "ok", codex: "ok", cursor: "err" },
  };
  if (agents && states[agents])
    d.agents = d.agents.map((a) =>
      chosen.includes(a.kind)
        ? {
            ...a,
            folder: a.per_folder ? (a.kind === "codex" ? FOLDER : "~/work/web") : a.folder,
            state: states[agents][a.kind],
            text: states[agents][a.kind] === "err" ? "hooks.json: permission denied" : states[agents][a.kind] === "ok" ? "설치됨" : null,
          }
        : a,
    );
  const server = q.get("server");
  if (server === "busy") d.server = { state: "busy", text: null, sessions: null };
  if (server === "ok") d.server = { state: "ok", text: "세션 128", sessions: 128 };
  if (server === "err") d.server = { state: "err", text: "connection refused", sessions: null };
  if (q.get("done") === "1") d.done = true;
  return d;
}

/** What the app does with each message, played out in the page. */
async function answer(m: Record<string, unknown>) {
  if (m.page !== "onboard") {
    // The tour's 체험 / the empty list's try: nothing to run without the app.
    return;
  }
  switch (m.do) {
    case "check_jev": {
      patch({ jev: { state: "busy", text: null, models: [] } });
      await later(1200);
      const key = String(m.key ?? "");
      if (key.startsWith("jev_")) patch({ jev: { state: "ok", text: `모델 ${MODELS.length}`, models: MODELS } });
      else patch({ jev: { state: "err", text: "401 Unauthorized", models: [] } });
      break;
    }
    case "model":
      patch({ model: String(m.model) });
      break;
    case "pick":
      agent(String(m.agent), { folder: m.agent === "codex" ? FOLDER : "~/work/web" });
      break;
    case "python":
      await later(1500);
      patch({ python: "/opt/homebrew/bin/python3" });
      break;
    case "start": {
      const chosen = (m.agents as string[]) ?? [];
      patch({
        run: { state: "busy", text: null },
        done: false,
        agents: data.agents.map((a) => (chosen.includes(a.kind) ? { ...a, state: "idle", text: null } : a)),
      });
      for (let i = 0; i < data.parts.length; i++) {
        await later(650);
        patch({ parts: data.parts.map((p, j) => (j <= i ? { ...p, up: true } : p)) });
      }
      patch({ run: { state: "ok", text: "실행 중" } });
      for (const kind of chosen) {
        agent(kind, { state: "busy", text: null });
        await later(800);
        agent(kind, { state: "ok", text: "설치됨" });
      }
      patch({ done: true });
      break;
    }
    case "open":
      setDemo((s) => (s.record?.shell ? { ...s, record: { ...s.record, shell: { ...s.record.shell, page: "record" } } } : s));
      break;
    case "server_test":
      patch({ server: { state: "busy", text: null, sessions: null } });
      await later(1000);
      patch({ server: { state: "ok", text: "세션 128", sessions: 128 } });
      break;
    case "server_save":
      patch({ server: { ...data.server, state: "busy" } });
      await later(800);
      setDemo((s) => (s.record?.shell ? { ...s, record: { ...s.record, shell: { ...s.record.shell, page: "record" } } } : s));
      break;
  }
}

const hash = window.location.hash.replace(/^#/, "");
if (standalone && hash.startsWith("/onboard")) {
  const q = new URLSearchParams(hash.split("?")[1] ?? "");
  demoStart.step = q.get("step") ?? undefined;
  demoStart.judge = q.get("judge") ?? undefined;
  demoStart.hooks = q.get("hooks") ?? undefined;
  data = fromHash(q);
  setDemo((s) => ({
    record: s.record?.shell ? { ...s.record, shell: { ...s.record.shell, page: "onboard" } } : s.record,
    views: { ...s.views, onboard: data },
  }));
  onDemoSend((m) => void answer(m));
  (window as unknown as { __whOnboardDemo: (p: Partial<OnboardData>) => void }).__whOnboardDemo = patch;
}

// `?nokey` / `?nohook` (opened on its own): Jev picked with no key kept / no hook set up, for the
// engine menu's dots.
if (standalone && /[?&]nokey\b/.test(hash))
  setDemo((s) => ({
    ...s,
    views: { ...s.views, engine: s.views.engine ? { ...s.views.engine, has_key: false } : s.views.engine },
    record: s.record?.shell ? { ...s.record, shell: { ...s.record.shell, engine: { ...s.record.shell.engine, jev: false, jev_on: true, has_key: false } } } : s.record,
  }));
if (standalone && /[?&]nohook\b/.test(hash))
  setDemo((s) => ({
    ...s,
    views: { ...s.views, hooks: s.views.hooks ? { ...s.views.hooks, installs: [] } : s.views.hooks },
    record: s.record?.shell ? { ...s.record, shell: { ...s.record.shell, agents: [] } } : s.record,
  }));
