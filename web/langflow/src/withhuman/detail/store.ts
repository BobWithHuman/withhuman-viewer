// The record panel's state, shared by the panel, the canvas toolbar and the canvas listeners:
// what is picked (a step, or a node of the canvas), which tab of a step is open (상세 or 경로), the
// filter chip, and whether the panel is shown and how wide. Picking from the panel asks the canvas
// to bring the steps into view (`want`); picking on the canvas does not move it.

import { create } from "zustand";
import type { RecordData } from "../types";
import { type Only, opening } from "./model";

export type View = "timeline" | "node" | "step";
export type Tab = "detail" | "path";

const WIDTH_KEY = "withhuman.recordPanel.width";
const OPEN_KEY = "withhuman.recordPanel.open";
export const MIN_W = 300;
export const MAX_W = 640;
export const DEFAULT_W = 380;

const read = (k: string) => { try { return localStorage.getItem(k); } catch { return null; } };
const write = (k: string, v: string) => { try { localStorage.setItem(k, v); } catch { /* this window only */ } };

type State = {
  session: string | null;
  view: View;
  /** The step shown (상세) or whose way is drawn (경로). */
  picked: string | null;
  /** The canvas node shown (node view). */
  node: string | null;
  /** The step opened in place: in the node's list, or on the way. */
  inner: string | null;
  tab: Tab;
  only: Only | null;
  open: boolean;
  width: number;
  /** Steps to bring into view, numbered so the same steps can be asked for again. */
  want: { steps: string[]; seq: number } | null;

  /** A new session (or the first): opened on its focus or latest refused Call with its way drawn
   * (not framed). */
  start: (data: RecordData) => void;
  pickStep: (id: string, from?: "panel" | "canvas") => void;
  pickNode: (node: string, open: string | null) => void;
  openInner: (id: string | null, from?: "panel" | "canvas") => void;
  setTab: (tab: Tab) => void;
  back: () => void;
  setOnly: (only: Only | null) => void;
  setOpen: (open: boolean) => void;
  setWidth: (w: number, save?: boolean) => void;
  bring: (steps: string[]) => void;
};

let seq = 0;

export const useRecordPanel = create<State>((set, get) => ({
  session: null,
  view: "timeline",
  picked: null,
  node: null,
  inner: null,
  tab: "path",
  only: null,
  open: read(OPEN_KEY) !== "0",
  width: Math.min(MAX_W, Math.max(MIN_W, Number(read(WIDTH_KEY)) || DEFAULT_W)),
  want: null,

  start: (data) => {
    const first = opening(data);
    set({
      session: data.session.id,
      view: first ? "step" : "timeline",
      picked: first,
      node: null,
      inner: null,
      tab: "path",
      only: null,
      // Framed by the caller once Langflow has fitted the canvas (flow/sync.ts).
      want: null,
    });
  },
  pickStep: (id, from = "panel") =>
    set({
      view: "step",
      picked: id,
      node: null,
      inner: null,
      open: true,
      want: from === "panel" ? { steps: [id], seq: ++seq } : get().want,
    }),
  pickNode: (node, open) => set({ view: "node", node, inner: open, picked: null, open: true }),
  openInner: (id, from = "panel") =>
    set({ inner: id, want: from === "panel" && id ? { steps: [id], seq: ++seq } : get().want }),
  setTab: (tab) => {
    const { picked } = get();
    set({ tab, inner: null, want: picked ? { steps: [picked], seq: ++seq } : null });
  },
  back: () => set({ view: "timeline", picked: null, node: null, inner: null }),
  setOnly: (only) => set({ only }),
  setOpen: (open) => {
    write(OPEN_KEY, open ? "1" : "0");
    set({ open });
  },
  setWidth: (w, save) => {
    const width = Math.min(MAX_W, Math.max(MIN_W, Math.round(w)));
    if (save) write(WIDTH_KEY, String(width));
    set({ width });
  },
  bring: (steps) => set({ want: { steps, seq: ++seq } }),
}));
