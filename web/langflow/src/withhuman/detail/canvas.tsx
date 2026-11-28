// The record panel's hold on Langflow's canvas, without touching its flow store (setting nodes
// there would count as an unsaved change):
// - `CanvasLight`: one <style> element. The node holding the picked step is outlined as Langflow
//   outlines a selected node, the step's field row is marked; on 경로 the nodes and edges on the way
//   are lit (edges in their tie's colour) and the rest dimmed, and in a lit node the rows off the
//   way are dimmed. Removing the element restores the canvas.
// - `useCanvasPicking`: a click on a step's field row (`data-withhuman-field`, set by Langflow's
//   NodeInputField) picks that step; a click elsewhere on a node shows that node's steps.
// - `frame`: brings nodes into view once React Flow has measured them.

import { useEffect, useMemo } from "react";
import useFlowStore from "@/stores/flowStore";
import useFlowsManagerStore from "@/stores/flowsManagerStore";
import { fieldKey, toFlow } from "../flow/convert";
import type { RecordData, Step } from "../types";
import { TIE, TONE, type Way, flagged, refused, toneOf } from "./model";
import { useRecordPanel } from "./store";

export type Converted = {
  /** Step id -> canvas node id. */
  nodeOf: Map<string, string>;
  /** Canvas node id -> its steps, in order. */
  stepsOf: Map<string, string[]>;
  /** Canvas node id -> its name and icon, as the node shows them. */
  nodes: Map<string, { name: string; icon: string }>;
  edges: { id: string; source: string; target: string }[];
  /** Field name -> step id. */
  byField: Map<string, string>;
};

/** The session as the canvas draws it (the same conversion the mock backend serves). */
export function useConverted(rec: RecordData | null): Converted | null {
  return useMemo(() => {
    if (!rec) return null;
    const { flow, nodeOf } = toFlow(rec, {});
    const stepsOf = new Map<string, string[]>();
    for (const s of rec.steps) {
      const n = nodeOf.get(s.id);
      if (n) stepsOf.set(n, [...(stepsOf.get(n) ?? []), s.id]);
    }
    const nodes = new Map<string, { name: string; icon: string }>(
      flow.data.nodes.map((n: any) => [n.id, { name: n.data.node.display_name, icon: n.data.node.icon }]),
    );
    const edges = flow.data.edges.map((e: any) => ({ id: e.id, source: e.source, target: e.target }));
    const byField = new Map(rec.steps.map((s) => [fieldKey(s.id), s.id]));
    return { nodeOf, stepsOf, nodes, edges, byField };
  }, [rec]);
}

/** The step a node's list opens on: its latest refused Call, else its first flagged step. */
export function nodeOpening(steps: Step[]) {
  return [...steps].reverse().find(refused)?.id ?? steps.find(flagged)?.id ?? null;
}

const P = "#react-flow-id";
const q = (v: string) => JSON.stringify(v);
const nodeSel = (id: string) => `${P} .react-flow__node[data-id=${q(id)}]`;
const rowSel = (step: string) => `[data-withhuman-field=${q(fieldKey(step))}]`;

/** The rules for what is picked and, on 경로, the way. */
function rules(conv: Converted, byId: Map<string, Step>, node: string | null, row: string | null, way: Way | null) {
  const out: string[] = [];
  if (node) {
    out.push(
      `${nodeSel(node)} .generic-node-div { border-color: hsl(var(--muted-foreground)) !important; box-shadow: 0 0 0 0.75px hsl(var(--muted-foreground)), 0 6px 16px -6px hsl(var(--foreground) / 0.25) !important; }`,
    );
  }
  const r = row ? byId.get(row) : undefined;
  if (r && conv.nodeOf.get(r.id)) {
    const tone = toneOf(r);
    const bar = tone === "quiet" ? "hsl(var(--foreground))" : `hsl(${TONE[tone].hsl})`;
    out.push(`${nodeSel(conv.nodeOf.get(r.id)!)} ${rowSel(r.id)} { background-color: hsl(var(--muted)); box-shadow: inset 3px 0 0 ${bar}; }`);
  }
  if (way && way.ids.length) {
    const lit = new Set(way.ids.map((id) => conv.nodeOf.get(id)).filter(Boolean) as string[]);
    const edgeTie = new Map<string, string>();
    for (const h of way.hops) {
      const a = conv.nodeOf.get(h.from), b = conv.nodeOf.get(h.to);
      if (!a || !b || a === b) continue;
      const e = conv.edges.find((x) => x.source === a && x.target === b);
      if (e) edgeTie.set(e.id, TIE[h.tie].hsl);
    }
    const not = (attr: string, ids: Iterable<string>) => [...ids].map((id) => `:not([${attr}=${q(id)}])`).join("");
    out.push(`${P} .react-flow__node${not("data-id", lit)} { opacity: 0.4; }`);
    out.push(`${P} .react-flow__edge${not("data-id", edgeTie.keys())} { opacity: 0.2; }`);
    for (const [id, hsl] of edgeTie)
      out.push(`${P} .react-flow__edge[data-id=${q(id)}] .react-flow__edge-path { stroke: hsl(${hsl}) !important; stroke-width: 2.5px !important; }`);
    const onWay = new Set(way.ids.map((id) => fieldKey(id)));
    for (const n of lit) out.push(`${nodeSel(n)} [data-withhuman-field^="step_"]${not("data-withhuman-field", onWay)} { opacity: 0.35; }`);
  }
  return out.join("\n");
}

/** The picked node and row and, on 경로, the way, drawn on the canvas while mounted. */
export function CanvasLight({ conv, data, node, row, way }: { conv: Converted; data: RecordData; node: string | null; row: string | null; way: Way | null }) {
  const byId = useMemo(() => new Map(data.steps.map((s) => [s.id, s])), [data]);
  const css = useMemo(() => rules(conv, byId, node, row, way), [conv, byId, node, row, way]);
  useEffect(() => {
    const el = document.createElement("style");
    el.dataset.withhuman = "record-panel";
    document.head.appendChild(el);
    return () => el.remove();
  }, []);
  useEffect(() => {
    const el = document.head.querySelector<HTMLStyleElement>('style[data-withhuman="record-panel"]');
    if (el) el.textContent = css;
  }, [css]);
  return null;
}

/** What a session's canvas always has, panel open or not: React Flow gives the nodes of a locked
 * flow no pointer events (an inline style: nothing selectable or draggable), so they are given
 * them back; rows
 * and nodes show they can be pressed. */
const BASE = [
  `${P} .react-flow__node-genericNode { pointer-events: all !important; cursor: pointer; }`,
  `${P} .react-flow__node, ${P} .react-flow__edge { transition: opacity 180ms ease; }`,
  `${P} [data-withhuman-field^="step_"] { cursor: pointer; transition: background-color 120ms ease, opacity 180ms ease; }`,
  `${P} [data-withhuman-field^="step_"]:hover { background-color: hsl(var(--muted) / 0.6); }`,
].join("\n");

/** Clicks on the canvas pick: a step's row picks the step, the rest of a node shows its steps. A
 * press that moved (a pan) picks nothing. */
export function useCanvasPicking(conv: Converted | null, data: RecordData | null, on: boolean) {
  useEffect(() => {
    if (!on) return;
    const el = document.createElement("style");
    el.dataset.withhuman = "record-canvas";
    el.textContent = BASE;
    document.head.appendChild(el);
    return () => el.remove();
  }, [on]);
  useEffect(() => {
    if (!on || !conv || !data) return;
    const byId = new Map(data.steps.map((s) => [s.id, s]));
    let down: { x: number; y: number } | null = null;
    const press = (e: PointerEvent) => { down = { x: e.clientX, y: e.clientY }; };
    const click = (e: MouseEvent) => {
      const t = e.target as HTMLElement | null;
      if (!t?.closest(P)) return;
      if (down && Math.hypot(e.clientX - down.x, e.clientY - down.y) > 4) return;
      const nodeEl = t.closest<HTMLElement>(".react-flow__node");
      const node = nodeEl?.dataset.id;
      if (!node || !conv.stepsOf.has(node)) return;
      const field = t.closest<HTMLElement>("[data-withhuman-field]")?.dataset.withhumanField;
      const step = field ? conv.byField.get(field) : undefined;
      const panel = useRecordPanel.getState();
      if (step) {
        if (panel.view === "node" && panel.node === node) panel.openInner(step, "canvas");
        else panel.pickStep(step, "canvas");
        return;
      }
      const steps = (conv.stepsOf.get(node) ?? []).map((id) => byId.get(id)!).filter(Boolean);
      panel.pickNode(node, nodeOpening(steps));
    };
    document.addEventListener("pointerdown", press, true);
    document.addEventListener("click", click);
    return () => {
      document.removeEventListener("pointerdown", press, true);
      document.removeEventListener("click", click);
    };
  }, [conv, data, on]);
}

/** Brings nodes into view, once React Flow has handed Langflow its instance and measured them. */
export function frame(nodes: string[], flowId: string, tries = 40) {
  if (!nodes.length || useFlowsManagerStore.getState().currentFlowId !== flowId) return;
  const rf = useFlowStore.getState().reactFlowInstance;
  if (rf && nodes.every((n) => rf.getNode(n)?.measured?.width)) {
    void rf.fitView({
      nodes: nodes.map((id) => ({ id })),
      maxZoom: 1,
      minZoom: nodes.length > 1 ? 0.55 : 0.6,
      padding: nodes.length > 1 ? 0.15 : 0.35,
      duration: 400,
    });
    return;
  }
  if (tries > 0) window.setTimeout(() => frame(nodes, flowId, tries - 1), 50);
}
