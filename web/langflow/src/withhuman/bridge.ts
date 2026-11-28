// What the personal app (Rust, src/ui/record.rs) hands the page and what the page asks back: the
// same contract as web/record's — `window.WHRecord.render(data)` with the open session and the
// app's frame (`shell`), `window.WHRecord.view(page, data)` with the hooks' and the engine's data,
// and `parent.__whRecordSend(message)` for what is asked (open a session, a hooks or engine action,
// move the window). Opened on its own (no app), it shows web/record's demo.

import { useSyncExternalStore } from "react";
import { DEMO_PERSONAL, DEMO_VIEWS, type RecordData } from "./types";
import type { OnboardData } from "./onboard/data";

export * from "./types";

export type Scope = "device" | "all" | "folder" | "profile";

export type Install = { agent: string; agent_name: string; at: Scope; place: string; file: string; engine: string | null; installed_at: number | null; removable: boolean };

export type HookAgent = { kind: string; name: string; found: boolean; per_folder: boolean; per_profile: boolean };

export type HooksData = {
  machine: { name: string; os: string };
  /** The engine checked: the hooks' own, or one typed to try. */
  health: { server: string; ok: boolean; ms: number | null; error: string | null } | null;
  record: { registered_at: number | null; last_seen: number | null; serial: boolean; edr: string | null; stopped: { at: number; by: string | null; reason: string | null } | null } | null;
  device: string | null;
  engine: string;
  own: boolean;
  jev: boolean;
  installs: Install[];
  agents: HookAgent[];
  folder: string | null;
  /** What is being done: `install`, `all`, `reset`, `agent:<kind>`, or the file being taken out. */
  busy: string | null;
  note: { ok: boolean; text: string; at: "agents" | "setup" } | null;
};


export type EngineData = {
  parts: { name: string; up: boolean }[];
  /** Where the hooks send their events. */
  url: string;
  has_key: boolean;
  /** Calls go to Jev now. */
  jev: boolean;
  models: string[];
  model: string;
  /** What the last thing pressed ended with. */
  status: { state: "idle" | "busy" | "ok" | "err"; text: string | null };
};


export type Views = { hooks?: HooksData | null; engine?: EngineData | null; onboard?: OnboardData | null };
type State = { record: RecordData | null; views: Views };

declare global {
  interface Window {
    WHRecord: { render: (d: RecordData) => void; view: (page: string, d: unknown) => void };
  }
}

const inApp = window.parent !== window && !!(window.parent as unknown as { __whRecordSend?: unknown }).__whRecordSend;
let state: State = inApp ? { record: null, views: {} } : { record: DEMO_PERSONAL, views: DEMO_VIEWS as Views };
/** Opened on its own, what is asked goes here (the onboarding's demo answers it). */
let demoHandler: ((m: Record<string, unknown>) => void) | null = null;
export const onDemoSend = (f: (m: Record<string, unknown>) => void) => { demoHandler = f; };
/** Opened on its own: the app's part of the demo, changed in place. */
export const setDemo = (f: (s: State) => State) => { if (!inApp) set(f(state)); };
export const standalone = !inApp;
const subs = new Set<() => void>();
const set = (next: State) => { state = next; subs.forEach((f) => f()); };

window.WHRecord = {
  render: (d) => set({ ...state, record: d }),
  view: (page, d) => set({ ...state, views: { ...state.views, [page]: d } }),
};

/** The latest of what the app handed, outside React (the mock backend reads it). */
export const current = () => state;
/** Calls `f` each time the app hands something; returns the unsubscribe. */
export const subscribe = (f: () => void) => { subs.add(f); return () => { subs.delete(f); }; };
/** What the app handed, in React. */
export const useWithHuman = () => useSyncExternalStore(subscribe, current);

/** Asks the app to act. Opened on its own, it only logs. */
export function send(m: Record<string, unknown>) {
  const up = (window.parent as unknown as { __whRecordSend?: (m: unknown) => void }).__whRecordSend;
  if (up && window.parent !== window) up(m);
  else {
    console.log("send", m);
    demoHandler?.(m);
  }
}
