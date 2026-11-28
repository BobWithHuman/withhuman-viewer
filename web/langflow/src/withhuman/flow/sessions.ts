// The app's sessions as Langflow flows: the list (`GET flows/`, the project's page) from
// `shell.sessions`, and one flow (`GET flows/<id>`) from the session the app last rendered,
// asking the app to open another one when the page asks for it.

import { current, send, subscribe, type RecordData, type ShellSession } from "../bridge";
import { toFlow } from "./convert";

type Json = any;

export const USER_ID = "7a1d9c2e-0b6f-4c8e-9d3a-5f2e1b4c6a01";
export const PROJECT_ID = "3c9e1f7a-2d4b-4e6a-8f1c-0a5b7d9e2c11";
export const PROJECT_NAME = "기록";
/** How long the app has to bring a session asked for. */
const OPEN_TIMEOUT = 15_000;

const inApp = () =>
  window.parent !== window && !!(window.parent as unknown as { __whRecordSend?: unknown }).__whRecordSend;

const iso = (ms: number) => new Date(ms).toISOString();

/** How long ago, in Korean, as the list says it. */
export function ago(ms: number, now = Date.now()) {
  const min = Math.max(0, Math.round((now - ms) / 60_000));
  if (min < 1) return "방금";
  if (min < 60) return `${min}분 전`;
  const h = Math.round(min / 60);
  if (h < 24) return `${h}시간 전`;
  const d = Math.round(h / 24);
  if (d < 30) return `${d}일 전`;
  return new Date(ms).toLocaleDateString("ko-KR");
}

export function sessions(): ShellSession[] {
  const rec = current().record;
  const list = [...(rec?.shell?.sessions ?? [])];
  // Opened on a session the frame does not list (the security team's app has no frame).
  // (Not the app's empty frame: id "" when there is no session at all.)
  if (rec && rec.session.id && !list.some((s) => s.id === rec.session.id))
    list.push({ id: rec.session.id, title: rec.session.title, folder: rec.session.folder, at: Date.now(), calls: 0, denied: 0, review: 0, outside: 0 });
  return list.sort((a, b) => b.at - a.at);
}

/** A session as the flows list carries it (no graph: Langflow keeps the open flow's own). */
function meta(s: ShellSession): Json {
  const folder = s.folder?.split("/").filter(Boolean).pop();
  return {
    id: s.id,
    name: s.title,
    description: [folder, ago(s.at)].filter(Boolean).join(" · "),
    data: null,
    is_component: false,
    updated_at: iso(s.at),
    date_created: iso(s.at),
    folder_id: PROJECT_ID,
    user_id: USER_ID,
    endpoint_name: null,
    icon: null,
    gradient: null,
    tags: [],
    locked: true,
    public: false,
    access_type: "PRIVATE",
    webhook: false,
    mcp_enabled: false,
    version_token: `wh-${s.at}`,
  };
}

export const sessionFlows = (): Json[] => sessions().map(meta);
export const isSession = (id: string) => sessions().some((s) => s.id === id);

function metaOf(rec: RecordData): Json {
  const s = sessions().find((x) => x.id === rec.session.id)!;
  return meta(s);
}

/** The open session as a flow, or null when none was rendered. */
export function openFlow() {
  const rec = current().record;
  return rec?.session.id ? toFlow(rec, metaOf(rec)) : null;
}

/** Ids the page asked the app for and has not seen arrive. */
const asked = new Set<string>();
/** Whether `id` was rendered because the page asked for it (else the app opened it). */
export const wasAsked = (id: string) => asked.has(id);
export const settle = (id: string) => asked.delete(id);

/** Sessions asked of the app and not yet arrived: one request each, however often Langflow asks. */
const waiting = new Map<string, Promise<Json | null>>();

/** The session `id` as a flow: the one rendered, or asked of the app and awaited. */
export function flowFor(id: string): Promise<Json | null> {
  const rec = current().record;
  if (rec?.session.id === id) return Promise.resolve(openFlow()!.flow);
  if (!inApp() || !isSession(id)) return Promise.resolve(null);
  const pending = waiting.get(id);
  if (pending) return pending;
  asked.add(id);
  send({ do: "open", id });
  const p = new Promise<Json | null>((resolve, reject) => {
    const timer = window.setTimeout(() => {
      stop();
      asked.delete(id);
      waiting.delete(id);
      reject(new Error(`session ${id} did not arrive`));
    }, OPEN_TIMEOUT);
    const stop = subscribe(() => {
      if (current().record?.session.id !== id) return;
      window.clearTimeout(timer);
      stop();
      waiting.delete(id);
      resolve(openFlow()!.flow);
    });
  });
  waiting.set(id, p);
  return p;
}
