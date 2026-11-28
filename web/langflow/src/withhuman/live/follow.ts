// A session that is still running, on its canvas. Each `render` that brings it with more steps is
// set on the canvas by flow/sync.ts; here:
// - the nodes and step rows that were not there before come in (fade and slide), and nodes that
//   moved to make room glide (a transform transition while the session is live);
// - the view follows the newest step (centred at the current zoom) unless the person has moved
//   the canvas (a press on the pane, a wheel) since the session opened or since they asked to
//   follow again (the header's 실시간 chip, `follow()`).
// Imported after flow/sync.ts (main.tsx); nothing in Langflow's stores is written.

import { create } from "zustand";
import useFlowStore from "@/stores/flowStore";
import { current, subscribe } from "../bridge";
import { fieldKey } from "../flow/convert";
import { router } from "../router";

type Live = { live: boolean; following: boolean };
/** Whether the open session is live, and whether the canvas follows it. */
export const useLive = create<Live>(() => ({ live: false, following: true }));

const onCanvas = () => router()?.state.location.pathname.startsWith("/flow/") ?? false;
const isLive = () => {
  const rec = current().record;
  if (!rec?.session.id) return false;
  return !!rec.shell?.sessions.find((s) => s.id === rec.session.id)?.live;
};

// --- what was there before -----------------------------------------------------------------

let flowId: string | null = null;
let seen = new Map<string, Set<string>>();

function fieldsOf(n: { data?: any }) {
  return new Set(Object.keys(n.data?.node?.template ?? {}).filter((k) => k.startsWith("step_")));
}

const ENTER: Keyframe[] = [
  { opacity: 0, transform: "translateY(10px) scale(0.98)" },
  { opacity: 1, transform: "none" },
];
const ROW: Keyframe[] = [
  { opacity: 0, transform: "translateX(-8px)", background: "hsl(var(--accent-emerald) / 0.9)" },
  { opacity: 1, transform: "none", background: "hsl(var(--accent-emerald) / 0.9)", offset: 0.35 },
  { opacity: 1, transform: "none", background: "transparent" },
];

/** Plays the entrance once React Flow has drawn the new parts. */
function enter(nodes: string[], rows: string[]) {
  let tries = 0;
  const run = () => {
    const missing: string[] = [];
    for (const id of nodes) {
      const el = document.querySelector<HTMLElement>(`.react-flow__node[data-id="${CSS.escape(id)}"] > *`);
      if (el) el.animate(ENTER, { duration: 420, easing: "cubic-bezier(.2,.7,.2,1)" });
      else missing.push(id);
    }
    for (const k of rows) {
      const el = document.querySelector<HTMLElement>(`[data-withhuman-field="${CSS.escape(k)}"]`);
      if (el) el.animate(ROW, { duration: 1400, easing: "ease-out" });
    }
    if (missing.length && ++tries < 10) {
      nodes = missing;
      rows = [];
      requestAnimationFrame(run);
    }
  };
  requestAnimationFrame(() => requestAnimationFrame(run));
}

// --- following -----------------------------------------------------------------------------

/** Centres the newest step's row at the current zoom. */
function centreOnNewest(duration = 450) {
  const rec = current().record;
  const flow = useFlowStore.getState();
  const rf = flow.reactFlowInstance;
  if (!rec || !rf) return;
  const keys = rec.steps.map((s) => fieldKey(s.id));
  for (let i = keys.length - 1; i >= 0; i--) {
    const node = flow.nodes.find((n) => fieldsOf(n).has(keys[i]));
    if (!node) continue;
    const zoom = rf.getZoom();
    const w = node.measured?.width ?? 320;
    let y = node.position.y + (node.measured?.height ?? 200) / 2;
    // The row's own height on the canvas, when drawn.
    const nodeEl = document.querySelector<HTMLElement>(`.react-flow__node[data-id="${CSS.escape(node.id)}"]`);
    const rowEl = nodeEl?.querySelector<HTMLElement>(`[data-withhuman-field="${CSS.escape(keys[i])}"]`);
    if (nodeEl && rowEl) {
      const a = nodeEl.getBoundingClientRect();
      const b = rowEl.getBoundingClientRect();
      y = node.position.y + (b.top - a.top + b.height / 2) / zoom;
    }
    void rf.setCenter(node.position.x + w / 2, y, { zoom, duration });
    return;
  }
}

/** Follow the newest step again (the 실시간 chip). */
export function follow() {
  useLive.setState({ following: true });
  centreOnNewest();
}

// A drag or a wheel over the canvas: the person moved it; stop following. (A plain click, to pick
// a step, does not count.)
let press: { x: number; y: number } | null = null;
const onMain = (t: EventTarget | null) => {
  const el = t as HTMLElement | null;
  return !!el?.closest(".react-flow") && !el.closest("[data-compare-pane]") && !el.closest(".react-flow__controls, .react-flow__panel");
};
const moved = () => {
  if (useLive.getState().live && useLive.getState().following) useLive.setState({ following: false });
};
window.addEventListener("pointerdown", (e) => (press = onMain(e.target) ? { x: e.clientX, y: e.clientY } : null), true);
window.addEventListener("pointerup", () => (press = null), true);
window.addEventListener(
  "pointermove",
  (e) => {
    if (press && e.buttons && Math.hypot(e.clientX - press.x, e.clientY - press.y) > 4) {
      press = null;
      moved();
    }
  },
  true,
);
window.addEventListener("wheel", (e) => onMain(e.target) && moved(), { capture: true, passive: true });

// While live, nodes glide to their new places.
const glide = document.createElement("style");
glide.textContent = ".react-flow__node{transition:transform .38s cubic-bezier(.2,.7,.2,1)}";
const setGlide = (on: boolean) => {
  if (on && !glide.isConnected) document.head.appendChild(glide);
  if (!on && glide.isConnected) glide.remove();
};

subscribe(() => {
  const live = isLive();
  if (live !== useLive.getState().live) useLive.setState({ live });
  setGlide(live && onCanvas());
});

useFlowStore.subscribe((state, prev) => {
  if (state.nodes === prev.nodes) return;
  const id = state.currentFlow?.id ?? null;
  const now = new Map(state.nodes.map((n) => [n.id, fieldsOf(n)]));
  if (id !== flowId || seen.size === 0) {
    // Another session (or the first): what it has is not new; follow it afresh.
    flowId = id;
    seen = now;
    useLive.setState({ following: true, live: isLive() });
    setGlide(isLive() && onCanvas());
    return;
  }
  const newNodes: string[] = [];
  const newRows: string[] = [];
  for (const [n, fields] of now) {
    const before = seen.get(n);
    if (!before) {
      newNodes.push(n);
      continue;
    }
    for (const f of fields) if (!before.has(f)) newRows.push(f);
  }
  seen = now;
  if (!newNodes.length && !newRows.length) return;
  enter(newNodes, newRows);
  if (useLive.getState().following && onCanvas()) window.setTimeout(() => centreOnNewest(), 60);
});
