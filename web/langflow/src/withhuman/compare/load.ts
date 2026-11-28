// Two sessions held at once for the compare view. The app hands one open session at a time, so
// each is fetched through the mock's own way (`flowFor`: `send({do:"open", id})`, awaiting the
// `render` that brings it), one after the other, and kept here with its converted flow.
// Opened on its own, the demo session is the demo's and the others are made up (./demo.ts).

import { cloneDeep } from "lodash";
import { processFlows } from "@/utils/reactflowUtils";
import { current, standalone, type RecordData } from "../bridge";
import { toFlow } from "../flow/convert";
import { flowFor, sessions } from "../flow/sessions";
import { synthRecord } from "./demo";

type Json = any;
export type Loaded = { rec: RecordData; flow: Json };

const kept = new Map<string, Loaded>();

const prepare = (rec: RecordData, flow: Json): Loaded => {
  const f = cloneDeep(flow);
  processFlows([f]);
  return { rec, flow: f };
};

async function loadOne(id: string): Promise<Loaded | null> {
  const rec = current().record;
  // The open session is always the newest of it.
  if (rec?.session.id === id) {
    const got = prepare(rec, toFlow(rec, { id, name: rec.session.title }).flow);
    kept.set(id, got);
    return got;
  }
  const had = kept.get(id);
  if (had) return had;
  if (standalone) {
    const s = sessions().find((x) => x.id === id);
    if (!s) return null;
    const r = synthRecord(s);
    const got = prepare(r, toFlow(r, { id, name: s.title }).flow);
    kept.set(id, got);
    return got;
  }
  const flow = await flowFor(id).catch(() => null);
  const now = current().record;
  if (!flow || now?.session.id !== id) return null;
  const got = prepare(now, flow);
  kept.set(id, got);
  return got;
}

/** Both sessions, in turn (never two asks to the app at once). */
export async function loadPair(a: string, b: string): Promise<[Loaded | null, Loaded | null]> {
  // The one open now first: asking for the other replaces it in the app.
  if (current().record?.session.id === b) {
    const second = await loadOne(b);
    return [await loadOne(a), second];
  }
  const first = await loadOne(a);
  return [first, await loadOne(b)];
}
