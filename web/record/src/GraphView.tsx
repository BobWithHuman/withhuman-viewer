import {
  type Edge, type EdgeProps, type Node, type NodeProps, BaseEdge, EdgeLabelRenderer, Handle, MarkerType, PanOnScrollMode, Position, ReactFlow, getBezierPath, useReactFlow,
} from "@xyflow/react";
import { ChevronRight, ListTree, Plus, Route, Rows3, Scan, TriangleAlert, ZoomIn, ZoomOut } from "lucide-react";
import { createContext, useContext, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import type { RecordData, Step } from "./data";
import { isLong } from "./longtext";
import { type Hop, PARENT_LABEL, TIE, wayBack } from "./path";
import { Story, StoryHead } from "./story";
import { DetailBody, KIND_ICON, LogoDisc, Nav, ONLY, SvcLogo, Tip, VerdictChip, clock, cx, flagged, tone, type Only } from "./parts";

// 기록's graph: what the agent asked for and what came back, left to right. A step that was
// caught or refused is drawn large with its verdict; a quiet one is a small dot. Seen two ways:
// 경로, a picked step's way back to the request it came from lit (a Call to its turn's prompt or
// the tool result that told it, a prompt to the step of the turn before it built on), and the
// timeline holding that way alone; 순서, every step in order with its causes and each prompt's
// parent drawn over the line.

type D = { step: Step; picked: boolean; dim: boolean; large?: boolean };
type Mode = "path" | "order";
const Pick = createContext<(id: string) => void>(() => {});
/** Opens or closes a box's folded steps on 경로. */
const Unfold = createContext<(n: number) => void>(() => {});

const bigStep = (s: Step) => flagged(s) || s.kind === "prompt";

function StepNode({ data }: NodeProps<Node<D>>) {
  const { step: s, picked, dim, large } = data;
  const big = (x: Step) => large || bigStep(x);
  const size = big(s) ? 64 : 22;
  return (
    <div className={cx("relative transition-opacity duration-300", dim && "opacity-25")} style={{ width: size, height: size }}>
      <Handle type="target" position={Position.Left} />
      <Handle type="source" position={Position.Right} />
      <Handle id="t-src" type="source" position={Position.Top} />
      <Handle id="t-tgt" type="target" position={Position.Top} />
      <div className={cx(
        "grid size-full cursor-pointer place-items-center transition-[box-shadow,transform] duration-200 hover:scale-[1.06]",
        big(s) ? "[&_svg]:size-7 [&_svg]:stroke-[2.2]" : "[&_svg]:hidden",
        s.catch && !s.verdict ? "rounded-[20px]" : "rounded-full",
        big(s) ? tone(s) : "bg-[#6d7f79]",
        picked ? "shadow-[0_0_0_3px_#282828,0_0_0_5px_#6488b4]" : "shadow-[0_0_0_3px_#282828]",
      )}>
        {big(s) && s.kind !== "group" && KIND_ICON[s.kind]}
        {s.kind === "group" && <span className={cx("font-bold text-[#1d1d1d]", big(s) ? "text-[16px]" : "text-[11px]")}>{s.inside?.length}</span>}
      </div>
      {s.svc && big(s) && <span className="absolute -top-1 -left-1 rounded-full shadow-[0_0_0_2px_#282828]"><LogoDisc s={s.svc} size={24} /></span>}
      {s.catch && <span className="absolute -top-1.5 -right-2 grid size-6 place-items-center rounded-full bg-catch text-[#1d1d1d] shadow-[0_0_0_2px_#282828]"><TriangleAlert className="size-3.5" strokeWidth={2.6} /></span>}
      {big(s) && <span className="absolute -right-1.5 -bottom-1 grid size-[18px] place-items-center rounded-full border border-act bg-bg text-act"><Plus className="size-3" strokeWidth={2.4} /></span>}
      <div className="pointer-events-none absolute left-1/2 w-[190px] -translate-x-1/2 text-center" style={{ top: size + 10 }}>
        <div className={cx("truncate", big(s) ? "text-[14px] font-semibold text-fg" : "text-[12px] text-mut")}>{s.quote ? `"${s.name}"` : s.name}</div>
        {big(s) && <div className="mt-1 flex justify-center"><VerdictChip s={s} /></div>}
      </div>
    </div>
  );
}
/** A SubGraph on 순서: a box of its steps, each a row to pick. */
type Match = ((s: Step) => boolean) | null;
/** A box's rows on 경로: a step of the way, or a run of steps off it, folded where they were. */
type Row = { s: Step } | { fold: Step[] };
type G = { n: number; ns: number[]; steps: Step[]; rows?: Row[]; open?: boolean; picked: string | null; lit: Match };
const ROW = 30, HEAD = 26, BOX = 236, FOLD = 24;
const KIND_NAME: Partial<Record<Step["kind"], string>> = { prompt: "프롬프트", call: "도구", result: "결과", answer: "답변", group: "묶음" };
/** How tall a row is, a folded run as its line or, opened, its steps under it. */
const rowHeight = (r: Row, open?: boolean) => ("fold" in r ? FOLD + (open ? r.fold.length * ROW : 0) : ROW);

/** A run of steps off the way: what kinds and how many, opening to them. */
function FoldRow({ steps, open, picked, onToggle }: { steps: Step[]; open?: boolean; picked: string | null; onToggle: () => void }) {
  const pick = useContext(Pick);
  const kinds = [...new Set(steps.map((s) => s.kind))];
  return (
    <>
      <button onClick={(e) => { e.stopPropagation(); onToggle(); }}
        className="flex w-full items-center gap-1.5 rounded-md px-1.5 text-left text-[11px] text-dim hover:bg-white/6 hover:text-mut" style={{ height: FOLD }}>
        <span className="flex w-[18px] shrink-0 justify-center"><i className="h-full w-px border-l border-dashed border-white/20" /></span>
        <span className="flex gap-0.5">{steps.slice(0, 6).map((s) => <i key={s.id} className={cx("size-2 rounded-[2px] opacity-70", tone(s))} />)}</span>
        <span className="tabular-nums">{kinds.map((k) => `${KIND_NAME[k] ?? k} ${steps.filter((s) => s.kind === k).length}`).join(" · ")}</span>
        <ChevronRight className={cx("ml-auto size-3 transition-transform", open && "rotate-90")} />
      </button>
      {open && steps.map((s) => (
        <button key={s.id} onClick={(e) => { e.stopPropagation(); pick(s.id); }}
          className={cx("flex w-full items-center gap-1.5 rounded-md px-1.5 text-left hover:bg-white/6", picked === s.id ? "bg-pick/25 opacity-100 ring-1 ring-pick" : "opacity-55 hover:opacity-90")} style={{ height: ROW }} title={s.name}>
          <span className={cx("grid size-[18px] shrink-0 place-items-center rounded [&_svg]:size-[11px] [&_svg]:stroke-[2.4]", tone(s))}>{KIND_ICON[s.kind]}</span>
          <span className="min-w-0 flex-1 truncate text-[12px] text-fg">{s.quote ? `"${s.name}"` : s.name}</span>
          {s.verdict && s.verdict !== "allow" && <VerdictChip s={s} />}
        </button>
      ))}
    </>
  );
}
function SgNode({ data }: NodeProps<Node<G>>) {
  const pick = useContext(Pick);
  const unfold = useContext(Unfold);
  const rows: Row[] = data.rows ?? data.steps.map((s) => ({ s }));
  const first = data.steps[0];
  const tag = first?.kind === "prompt" ? "프롬프트" : first?.kind === "answer" ? "답변" : "도구";
  const ns = data.ns.length > 1 ? `S${data.ns[0]}–${data.ns[data.ns.length - 1]}` : `S${data.n}`;
  let top = HEAD;
  const at = rows.map((r) => { const y = top; top += rowHeight(r, data.open); return y; });
  return (
    <div className="rounded-lg bg-panel shadow-[0_6px_18px_rgba(0,0,0,.35)] ring-1 ring-white/10" style={{ width: BOX }}>
      {rows.map((r, i) => "s" in r && (
        <span key={r.s.id}>
          <Handle id={`i${r.s.id}`} type="target" position={Position.Left} style={{ top: at[i] + ROW / 2 }} />
          <Handle id={`o${r.s.id}`} type="source" position={Position.Right} style={{ top: at[i] + ROW / 2 }} />
          <Handle id={`l${r.s.id}`} type="source" position={Position.Left} style={{ top: at[i] + ROW / 2 }} />
        </span>
      ))}
      <div className="flex items-center gap-1.5 px-2.5 text-[10.5px] text-mut" style={{ height: HEAD }}>
        <span className="rounded bg-white/8 px-1 font-mono tabular-nums">{ns}</span>{tag}
      </div>
      <div className="px-1 pb-1">
        {rows.map((r) => "fold" in r ? <FoldRow key={`f${r.fold[0].id}`} steps={r.fold} open={data.open} picked={data.picked} onToggle={() => unfold(data.n)} /> : (
          <button key={r.s.id} onClick={(e) => { e.stopPropagation(); pick(r.s.id); }}
            className={cx("flex w-full items-center gap-1.5 rounded-md px-1.5 text-left transition-opacity", data.lit && !data.lit(r.s) && "opacity-30", data.picked === r.s.id ? "bg-pick/25 ring-1 ring-pick" : "hover:bg-white/6")}
            style={{ height: ROW }}>
            <span className={cx("grid size-[18px] shrink-0 place-items-center rounded [&_svg]:size-[11px] [&_svg]:stroke-[2.4]", tone(r.s))}>{KIND_ICON[r.s.kind]}</span>
            <span className="min-w-0 flex-1 truncate text-[12px] text-fg">{r.s.quote ? `"${r.s.name}"` : r.s.name}</span>
            {r.s.verdict && r.s.verdict !== "allow" && <VerdictChip s={r.s} />}
            {r.s.undecided && <span className="shrink-0 rounded-md bg-white/6 px-1.5 py-0.5 text-[11px] text-mut">판정 없음</span>}
            {r.s.fresh && <span className="shrink-0 rounded-md bg-act/15 px-1.5 py-0.5 text-[11px] font-semibold text-act">새 요청</span>}
            {r.s.catch && !r.s.verdict && <TriangleAlert className="size-3.5 shrink-0 fill-catch text-panel" />}
          </button>
        ))}
      </div>
    </div>
  );
}
const nodeTypes = { step: StepNode, sg: SgNode };

/** The tie a SubGraph hangs from, named as its link is. */
const SG_TIE: Record<string, { color: string; label?: string; dashed?: boolean }> = {
  ask: { color: TIE.ask.color },
  cause: { color: TIE.cause.color, label: TIE.cause.label },
  message: { color: TIE.parent.color, label: PARENT_LABEL.message },
  result: { color: TIE.parent.color, label: PARENT_LABEL.result },
  request: { color: TIE.parent.color, label: PARENT_LABEL.request },
  answer: { color: "#6f6f6f" },
  untraced: { color: "#7a7a7a", label: "추적 없음", dashed: true },
  loose: { color: "#ef6b5e", label: "출처 못 찾음", dashed: true },
};

/** What a line's colour says, for the legend under the graph. */
type Line = "ask" | "cause" | "parent" | "answer" | "next" | "untraced" | "loose";
const LINE: Record<Line, { color: string; label: string; dashed?: boolean }> = {
  next: { color: "#8a8a8a", label: "다음 단계" },
  ask: { color: TIE.ask.color, label: "요청에 따라" },
  cause: { color: TIE.cause.color, label: "외부 지시" },
  parent: { color: TIE.parent.color, label: "이어 물음" },
  answer: { color: "#6f6f6f", label: "답변" },
  untraced: { color: "#7a7a7a", label: "추적 없음", dashed: true },
  loose: { color: "#ef6b5e", label: "출처 못 찾음", dashed: true },
};
const LINES = Object.keys(LINE) as Line[];
const lineOf = (via: string): Line => (via === "message" || via === "result" || via === "request" ? "parent" : (LINES as string[]).includes(via) ? (via as Line) : "ask");

function Legend({ lines }: { lines: Set<Line> }) {
  const shown = LINES.filter((l) => lines.has(l));
  if (!shown.length) return null;
  return (
    <span className="flex items-center gap-2.5 border-l border-white/10 pl-2.5 text-[11px]">
      {shown.map((l) => (
        <span key={l} className="inline-flex items-center gap-1">
          <svg width="14" height="4" aria-hidden><line x1="0" y1="2" x2="14" y2="2" stroke={LINE[l].color} strokeWidth="2" strokeDasharray={LINE[l].dashed ? "3 2" : undefined} /></svg>
          {LINE[l].label}
        </span>
      ))}
    </span>
  );
}

/** The SubGraphs as boxes: the agent's steps that hang the same way from one SubGraph (the
 * tools a prompt asked for, one by one, and the turn's answer) share a box; what a tool result
 * told and a prompt written from the turn before keep their own, where the line branches. */
function boxes(data: RecordData) {
  const gs = [...(data.subgraphs ?? [])].sort((a, b) => a.n - b.n);
  const first = new Map(data.steps.map((s) => [s.id, s.kind]));
  const joins = (g: (typeof gs)[number]) =>
    g.parent != null && (g.via === "ask" || g.via === "answer") && first.get(g.steps[0]) !== "prompt";
  // Each SubGraph's box, by the first SubGraph in it: one joins the box of its siblings only
  // when that box is the last made, so the boxes keep the order the steps were made in.
  const boxOf = new Map<number, number>();
  const shared = new Map<number, number>();
  let last: number | undefined;
  for (const g of gs) {
    const into = joins(g) && shared.get(g.parent!) === last ? last : undefined;
    boxOf.set(g.n, into ?? g.n);
    if (joins(g) && into === undefined) shared.set(g.parent!, g.n);
    last = boxOf.get(g.n);
  }
  const out = new Map<number, { n: number; ns: number[]; parent: number | null; via: string; p?: number | null; steps: string[] }>();
  for (const g of gs) {
    const b = boxOf.get(g.n)!;
    const box = out.get(b) ?? { n: b, ns: [], parent: null, via: "none", p: null, steps: [] };
    box.ns.push(g.n);
    box.steps.push(...g.steps);
    if (b === g.n && g.parent != null) {
      box.parent = boxOf.get(g.parent) ?? g.parent;
      box.via = g.via;
      box.p = g.p;
    }
    out.set(b, box);
  }
  // A box's steps in the order they were made.
  const order = new Map(data.steps.map((s, i) => [s.id, i]));
  for (const box of out.values()) box.steps.sort((a, b) => (order.get(a) ?? 0) - (order.get(b) ?? 0));
  return [...out.values()];
}

/** 순서 by SubGraph: the boxes top to bottom in the order their first step was made, each in
 * the column of how far it hangs from one with no parent (the first column holds those), and
 * joined from the step it hangs from (the prompt that asked, the Call whose result told it, the
 * answer or result a prompt was written from) to its first. Where each step sits, to bring it
 * into view. */
function layoutTree(data: RecordData, picked: string | null, match: Match, way?: { ids: string[]; hops: Hop[] }, unfolded: Set<number> = new Set()) {
  const order = new Map(data.steps.map((s, i) => [s.id, i]));
  const all = boxes(data).sort((a, b) => (order.get(a.steps[0]) ?? 0) - (order.get(b.steps[0]) ?? 0));
  // On 경로: only the boxes the way passes through, each with only its steps on the way.
  const kept = way && new Set(way.ids);
  // Each box's steps off the way stay where they were, folded, so what came between shows.
  const gs = kept
    ? all.filter((g) => g.steps.some((id) => kept.has(id))).map((g) => ({ ...g, all: g.steps, steps: g.steps.filter((id) => kept.has(id)) }))
    : all.map((g) => ({ ...g, all: undefined as string[] | undefined }));
  const byN = new Map(gs.map((g) => [g.n, g]));
  const step = new Map(data.steps.map((s) => [s.id, s]));
  const hangs = (g: { parent: number | null; n: number }) => g.parent != null && byN.has(g.parent) && g.parent !== g.n;
  const depth = new Map<number, number>();
  // On 경로 the way runs one box after another, each one column further in.
  if (kept) gs.forEach((g, i) => depth.set(g.n, i));
  const depthOf = (n: number, seen: Set<number> = new Set()): number => {
    if (depth.has(n)) return depth.get(n)!;
    const g = byN.get(n)!;
    seen.add(n);
    const d = hangs(g) && !seen.has(g.parent!) ? depthOf(g.parent!, seen) + 1 : 0;
    depth.set(n, d);
    return d;
  };
  // On 경로 each box has its own row, so a small step in is enough to show the way going on.
  const GAP = kept ? 22 : 12, COL = kept ? 56 : BOX + 64;
  const spots = new Map<string, { x: number; y: number }>();
  let y = 0;
  const nodes: Node<G>[] = gs.map((g) => {
    const steps = g.steps.map((id) => step.get(id)).filter((s): s is Step => !!s);
    let rows: Row[] | undefined;
    if (g.all) {
      rows = [];
      for (const id of g.all) {
        const s = step.get(id);
        if (!s) continue;
        const last = rows[rows.length - 1];
        if (kept!.has(id)) rows.push({ s });
        else if (last && "fold" in last) last.fold.push(s);
        else rows.push({ fold: [s] });
      }
    }
    const open = unfolded.has(g.n);
    const x = depthOf(g.n) * COL;
    const at = y;
    let top = at + HEAD;
    for (const r of rows ?? steps.map((s) => ({ s }))) {
      if ("s" in r) spots.set(r.s.id, { x: x + BOX / 2, y: top + ROW / 2 });
      top += rowHeight(r, open);
    }
    y = top + 8 + GAP;
    return { id: `g${g.n}`, type: "sg", position: { x, y: at }, data: { n: g.n, ns: g.ns, steps, rows, open, picked, lit: match }, draggable: false, selectable: false };
  });
  if (way && kept) {
    // The way's links between boxes, from the step that led to the step it led to.
    const boxOfStep = new Map(gs.flatMap((g) => g.steps.map((id) => [id, g.n] as const)));
    const edges: Edge[] = way.hops
      .filter((h) => boxOfStep.get(h.from) !== boxOfStep.get(h.to))
      .map((h) => {
        const color = TIE[h.tie].color;
        return {
          id: `w${h.from}>${h.to}`, source: `g${boxOfStep.get(h.from)}`, target: `g${boxOfStep.get(h.to)}`,
          // Left to left: the way runs down the boxes' left side, clear of them.
          sourceHandle: `l${h.from}`, targetHandle: `i${h.to}`, type: "tie",
          style: { stroke: color, strokeWidth: 2.4 }, markerEnd: { type: MarkerType.ArrowClosed, color, width: 14, height: 14 },
        };
      });
    const lines = new Set<Line>(way.hops.map((h) => (h.tie === "parent" ? "parent" : h.tie === "cause" ? "cause" : h.tie === "result" ? "next" : "ask")));
    return { nodes: nodes as Node[], edges, spots, lines };
  }
  /** The step of the parent box a box hangs from. */
  const from = (g: (typeof gs)[number]): string | undefined => {
    const parent = byN.get(g.parent!)!;
    const inParent = (id: string) => parent.steps.includes(id);
    if (g.via === "cause") return data.causes.find(([a, b]) => g.steps.includes(b) && inParent(a))?.[0];
    if (g.via === "message" || g.via === "result" || g.via === "request")
      return data.parents?.find((p) => g.steps.includes(p.prompt) && inParent(p.from))?.from;
    return parent.steps.find((id) => step.get(id)?.kind === "prompt") ?? parent.steps[0];
  };
  const edges: Edge[] = gs.filter(hangs).map((g) => {
    const tie = SG_TIE[g.via] ?? SG_TIE.ask;
    const src = from(g) ?? byN.get(g.parent!)!.steps[0];
    const on = picked != null && (g.steps.includes(picked) || src === picked);
    return {
      id: `g${g.parent}>g${g.n}`, source: `g${g.parent}`, target: `g${g.n}`, sourceHandle: `o${src}`, targetHandle: `i${g.steps[0]}`,
      type: "default",
      style: { stroke: tie.color, strokeWidth: on ? 2.6 : 1.5, opacity: on ? 1 : match ? 0.3 : 0.8, strokeDasharray: tie.dashed ? "5 4" : undefined },
      markerEnd: { type: MarkerType.ArrowClosed, color: tie.color, width: 14, height: 14 }, zIndex: on ? 4 : 0,
    };
  });
  const lines = new Set<Line>(gs.filter(hangs).map((g) => lineOf(g.via)));
  return { nodes: nodes as Node[], edges, spots, lines };
}

/** A link that is not the next step: an arch over the line, higher the farther, named. */
function ArcEdge({ id, sourceX, sourceY, targetX, targetY, markerEnd, style, data }: EdgeProps<Edge<{ label: string; color: string; faint?: boolean }>>) {
  const h = 46 + Math.abs(targetX - sourceX) * 0.22;
  const top = Math.min(sourceY, targetY) - h;
  const path = `M${sourceX},${sourceY} C${sourceX},${top} ${targetX},${top} ${targetX},${targetY}`;
  const lx = (sourceX + targetX) / 2, ly = Math.min(sourceY, targetY) - h * 0.75;
  return (
    <>
      <BaseEdge id={id} path={path} markerEnd={markerEnd} style={style} />
      {data && !data.faint && (
        <EdgeLabelRenderer>
          <div className="nodrag nopan pointer-events-none absolute rounded bg-bg px-1.5 py-px text-[11px] font-semibold" style={{ color: data.color, transform: `translate(-50%,-50%) translate(${lx}px,${ly}px)` }}>{data.label}</div>
        </EdgeLabelRenderer>
      )}
    </>
  );
}
/** A link on the way: from one step of it to the next, named, with how many steps it passes. */
function HopEdge({ id, sourceX, sourceY, targetX, targetY, sourcePosition, targetPosition, markerEnd, style, data }: EdgeProps<Edge<{ label: string; color: string; p?: number; skipped: number }>>) {
  const [path, lx, ly] = getBezierPath({ sourceX, sourceY, targetX, targetY, sourcePosition, targetPosition });
  return (
    <>
      <BaseEdge id={id} path={path} markerEnd={markerEnd} style={style} />
      {data && (
        <EdgeLabelRenderer>
          <div className="nodrag nopan pointer-events-none absolute flex flex-col items-center gap-0.5" style={{ transform: `translate(-50%,-50%) translate(${lx}px,${ly - 22}px)` }}>
            <span className="rounded bg-bg px-1.5 py-px text-[12px] font-semibold whitespace-nowrap" style={{ color: data.color }}>
              {data.label}{data.p != null && <span className="ml-1 font-normal text-mut tabular-nums">{Math.round(data.p * 100)}%</span>}
            </span>
          </div>
          {data.skipped > 0 && (
            <div className="nodrag nopan pointer-events-none absolute rounded-full bg-chip px-1.5 text-[10.5px] text-mut tabular-nums" style={{ transform: `translate(-50%,-50%) translate(${lx}px,${ly + 20}px)` }}>+{data.skipped}</div>
          )}
        </EdgeLabelRenderer>
      )}
    </>
  );
}

/** A link of the way between two boxes on 경로: down their left side, its colour named in the
 * legend under the graph. */
function TieEdge({ id, sourceX, sourceY, targetX, targetY, markerEnd, style }: EdgeProps) {
  const bend = 64;
  return <BaseEdge id={id} path={`M${sourceX},${sourceY} C${sourceX - bend},${sourceY} ${targetX - bend},${targetY} ${targetX},${targetY}`} markerEnd={markerEnd} style={style} />;
}

const edgeTypes = { arc: ArcEdge, hop: HopEdge, tie: TieEdge };

function ToolStrip({ timeline, setTimeline, zoom }: { timeline: boolean; setTimeline: (v: boolean) => void; zoom: boolean }) {
  const rf = useReactFlow();
  const btn = (label: string, icon: ReactNode, onClick?: () => void, on?: boolean) => (
    <Tip label={label} side="right" key={label}>
      <button onClick={onClick} className={cx("grid size-8 place-items-center rounded-md transition-colors [&_svg]:size-[17px]", on ? "bg-act/20 text-act" : "text-mut hover:bg-white/8 hover:text-fg")}>{icon}</button>
    </Tip>
  );
  return (
    <div className="absolute top-3 left-3 z-10 flex flex-col items-center gap-0.5 rounded-lg bg-panel p-1 shadow-[0_8px_24px_rgba(0,0,0,.35)]">
      {btn("타임라인", <ListTree />, () => setTimeline(!timeline), timeline)}
      {zoom && <>
        <i className="my-1 h-px w-5 bg-white/10" />
        {btn("확대", <ZoomIn />, () => rf.zoomIn({ duration: 250 }))}
        {btn("전체 보기", <Scan />, () => rf.fitView({ duration: 400, padding: 0.2 }))}
        {btn("축소", <ZoomOut />, () => rf.zoomOut({ duration: 250 }))}
      </>}
    </div>
  );
}

function Card({ s, open, onToggle, onWay, cardRef }: { s: Step; open: boolean; onToggle: () => void; onWay?: () => void; cardRef: (el: HTMLDivElement | null) => void }) {
  return (
    <div ref={cardRef} className="relative pl-7">
      <span className={cx("absolute top-2 left-0 grid size-5 place-items-center rounded-md [&_svg]:size-3 [&_svg]:stroke-[2.4]", tone(s))}>{KIND_ICON[s.kind]}</span>
      <div className={cx("rounded-lg border bg-bg transition-colors", open ? "border-pick shadow-[0_0_0_1px_#6488b4]" : "border-transparent hover:border-white/10")}>
        <div className="flex cursor-pointer items-start gap-1.5 px-2 py-1.5" onClick={onToggle}>
          <ChevronRight className={cx("mt-0.5 size-3.5 shrink-0 text-mut transition-transform", open && "rotate-90")} />
          <div className="min-w-0 flex-1">
            <div className="flex items-center gap-2"><span className="truncate text-[12.5px] font-semibold text-fg">{s.quote ? `"${s.name}"` : s.name}</span><VerdictChip s={s} /></div>
            <div className="text-[11px] text-mut tabular-nums">{s.type} <span className="mx-1 text-dim">|</span> {clock(s.at)}{s.text && isLong(s.text.body) && <><span className="mx-1 text-dim">|</span>{s.text.body.length.toLocaleString("ko-KR")}자</>}</div>
          </div>
          {onWay && <Tip label="경로"><button onClick={(e) => { e.stopPropagation(); onWay(); }} className="grid size-6 place-items-center rounded-md text-mut hover:bg-white/8 hover:text-fg"><Route className="size-3.5" /></button></Tip>}
        </div>
        {open && <div className="px-3 pt-0.5 pb-3"><DetailBody s={s} /></div>}
      </div>
    </div>
  );
}

/** Columns in order of the steps; a step on another lane (a subagent, a fork) sits above or below.
 * On 경로, the steps off the way are dimmed and only the way's links are drawn strong. */
function layout(data: RecordData, picked: string | null, mode: Mode, way: { ids: string[]; hops: Hop[] }, match: Match) {
  const path = mode === "path" && way.ids.length > 0;
  const on = new Set(way.ids);
  const nodes: Node<D>[] = data.steps.map((s, i) => {
    const half = bigStep(s) ? 32 : 11;
    return {
      id: s.id, type: "step", position: { x: i * 210 - half, y: (s.lane ?? 0) * 150 - half },
      data: { step: s, picked: picked === s.id, dim: (path && !on.has(s.id)) || (!!match && !match(s)) }, draggable: false, selectable: false,
    };
  });
  const arrow = (color: string, w = 15) => ({ type: MarkerType.ArrowClosed, color, width: w, height: w });
  const linked = new Set(data.links.map(([a, b]) => `${a}>${b}`));
  const edges: Edge[] = data.links.map(([a, b]) => {
    const k = `${a}>${b}`;
    const h = way.hops.find((x) => `${x.from}>${x.to}` === k);
    const color = h ? TIE[h.tie].color : data.causes.some(([x, y]) => x === a && y === b) && !path ? TIE.cause.color : "#8a8a8a";
    return {
      id: k, source: a, target: b, type: "default",
      style: { stroke: color, strokeWidth: h ? 2.6 : 1.6, opacity: path && !h ? 0.18 : match ? 0.3 : 1 },
      markerEnd: arrow(color), zIndex: h ? 4 : 0,
    };
  });
  // What is not the next step: an arch over the line.
  const arcs: { from: string; to: string; color: string; label: string; strong: boolean }[] = path
    ? way.hops.filter((h) => !linked.has(`${h.from}>${h.to}`)).map((h) => ({ from: h.from, to: h.to, color: TIE[h.tie].color, label: h.label, strong: true }))
    : [
        ...data.causes.filter(([a, b]) => !linked.has(`${a}>${b}`)).map(([a, b]) => ({ from: a, to: b, color: TIE.cause.color, label: TIE.cause.label, strong: true })),
        ...(data.parents ?? []).filter((p) => !linked.has(`${p.from}>${p.prompt}`)).map((p) => ({ from: p.from, to: p.prompt, color: TIE.parent.color, label: `${PARENT_LABEL[p.via] ?? TIE.parent.label} ${Math.round(p.p * 100)}%`, strong: true })),
      ];
  for (const a of arcs) {
    edges.push({
      id: `arc:${a.from}>${a.to}`, source: a.from, target: a.to, sourceHandle: "t-src", targetHandle: "t-tgt", type: "arc", zIndex: 5,
      data: { label: a.label, color: a.color }, style: { stroke: a.color, strokeWidth: 2.2, opacity: match ? 0.3 : 0.95 }, markerEnd: arrow(a.color, 14),
    });
  }
  const lines = new Set<Line>(path ? way.hops.map((h) => (h.tie === "result" ? "next" : h.tie)) : [
    ...(data.links.length ? ["next" as const] : []), ...(data.causes.length ? ["cause" as const] : []), ...(data.parents?.length ? ["parent" as const] : []),
  ]);
  return { nodes, edges, lines };
}

/** 경로 or 순서, over the graph. */
function ModeSwitch({ mode, setMode }: { mode: Mode; setMode: (m: Mode) => void }) {
  const item = (m: Mode, label: string, icon: ReactNode) => (
    <button key={m} onClick={() => setMode(m)}
      className={cx("flex h-7 items-center gap-1.5 rounded-md px-3 text-[12.5px] font-semibold transition-colors [&_svg]:size-3.5", mode === m ? "bg-white/12 text-fg" : "text-mut hover:text-fg")}>{icon}{label}</button>
  );
  return (
    <div className="absolute top-3 left-1/2 z-10 flex -translate-x-1/2 gap-0.5 rounded-lg bg-panel p-1 shadow-[0_8px_24px_rgba(0,0,0,.35)]">
      {item("path", "경로", <Route />)}
      {item("order", "순서", <Rows3 />)}
    </div>
  );
}

/** Between two steps of the way on the timeline: how the one led to the next. */
function HopMark({ h }: { h: Hop }) {
  return (
    <div className="relative flex items-center gap-1.5 py-0.5 pl-7 text-[11px] font-semibold" style={{ color: TIE[h.tie].color }}>
      <i className="absolute left-[18px] size-[8px] rounded-full ring-2 ring-panel" style={{ background: TIE[h.tie].color }} />
      {h.label}{h.p != null && <span className="font-normal text-mut tabular-nums">{Math.round(h.p * 100)}%</span>}
    </div>
  );
}

/** The step to open on: the Call opened from another page, else the latest Call caught or
 * refused, else the first step worth a look. */
const opening = (data: RecordData) =>
  (data.focus && data.steps.some((s) => s.id === data.focus) ? data.focus : null)
  ?? [...data.steps].reverse().find((s) => s.kind === "call" && flagged(s))?.id ?? data.steps.find(flagged)?.id ?? null;

export function GraphView({ data, only, setOnly }: { data: RecordData; only: Only | null; setOnly: (o: Only | null) => void }) {
  const first = opening(data);
  const [picked, setPicked] = useState<string | null>(first);
  const [mode, setMode] = useState<Mode>("order");
  const [open, setOpen] = useState<string | null>(null);
  /** The boxes whose steps off the way are opened, on 경로. */
  const [unfolded, setUnfolded] = useState<Set<number>>(new Set());
  const [timeline, setTimeline] = useState(true);
  const rf = useReactFlow();
  const refs = useRef<Record<string, HTMLDivElement | null>>({});
  /** A step to bring into view once it is drawn, instead of the whole. */
  const goto = useRef<string | null>(null);
  useEffect(() => { setPicked(opening(data)); setOpen(null); setUnfolded(new Set()); }, [data]);
  // The card picked is brought to the upper part of the timeline, so what follows it shows below
  // and the eye is not pulled down; one already there stays.
  const bringUp = (id: string | null) => requestAnimationFrame(() => {
    const el = id ? refs.current[id] : null, box = el?.closest(".scroll") as HTMLElement | null;
    if (!el || !box) return;
    const top = el.getBoundingClientRect().top - box.getBoundingClientRect().top, h = box.clientHeight;
    if (top >= h * 0.08 && top <= h * 0.4 && top + el.offsetHeight <= h) return;
    box.scrollTo({ top: box.scrollTop + top - h * 0.22, behavior: "smooth" });
  });
  useEffect(() => { bringUp(picked); }, [picked]); // eslint-disable-line react-hooks/exhaustive-deps
  useEffect(() => { bringUp(open); }, [open]); // eslint-disable-line react-hooks/exhaustive-deps
  // On 경로, the way in view, as near as it fits; on 순서, the whole. The rest is a button away.
  // What was picked in the session shown before is not here: the new one opens on its own
  // until the effect catches up.
  const known = useMemo(() => new Set(data.steps.map((s) => s.id)), [data]);
  const sel = picked && !known.has(picked) ? opening(data) : picked;
  const wayKey = mode === "path" ? (sel ?? "") : "";
  useEffect(() => {
    if (goto.current) return;
    // 순서 of many steps opens on the step picked at a size its name reads; of few, on the whole.
    const near = mode === "order" && data.steps.length > 12 && sel ? spotOf(sel) : undefined;
    requestAnimationFrame(() => near
      ? rf.setCenter(near.x, near.y, { zoom: 0.85, duration: 300 })
      : rf.fitView({ padding: mode === "path" ? 0.3 : 0.18, maxZoom: 1, duration: 300 }));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [data, rf, mode, wayKey]);
  // A header chip picked: 순서 with its steps alone lit and listed, the first in view.
  useEffect(() => {
    if (!only) return;
    const first = data.steps.find(ONLY[only].is);
    setMode("order");
    setOpen(null);
    if (first) { setPicked(first.id); goto.current = first.id; }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [only]);
  const match: Match = mode === "order" && only ? ONLY[only].is : null;
  const go = (m: Mode) => { setMode(m); if (m === "path") setOnly(null); };
  const way = useMemo(() => wayBack(data, sel), [data, sel]);
  const boxed = (data.subgraphs?.length ?? 0) > 0;
  // 경로 is 순서 with the way alone: its boxes, and in them its steps.
  const onWay = mode === "path" && way.ids.length > 0;
  const { nodes, edges, spots, lines } = useMemo(
    (): { nodes: Node[]; edges: Edge[]; spots?: Map<string, { x: number; y: number }>; lines?: Set<Line> } =>
      boxed && (mode === "order" || onWay)
        ? layoutTree(data, onWay ? open || sel : sel, match, onWay ? way : undefined, unfolded)
        : layout(data, sel, mode, way, match),
    [data, sel, open, mode, way, boxed, onWay, match, unfolded],
  );
  /** Where a step is drawn, to bring it into view. */
  const spotOf = (id: string) => {
    const at = spots?.get(id);
    if (at) return at;
    const n = nodes.find((x) => x.id === id);
    return n && { x: n.position.x + 32, y: n.position.y + 40 };
  };
  // On 경로 the timeline holds the way alone, the earliest first, each link named between.
  const shown = mode === "path" && way.ids.length ? [...way.ids].reverse() : data.steps.filter((s) => !match || match(s)).map((s) => s.id);
  const byId = new Map(data.steps.map((s) => [s.id, s]));
  const locate = (id: string) => {
    const at = spotOf(id);
    if (at) rf.setCenter(at.x, at.y, { zoom: Math.max(rf.getZoom(), 0.9), duration: 500 });
  };
  useEffect(() => {
    const id = goto.current;
    if (!id) return;
    goto.current = null;
    requestAnimationFrame(() => locate(id));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [nodes]);
  // The arrow keys step: on 순서 through every step (← →) and from turn to turn (↑ ↓); on 경로
  // along the way, opening each step. Not while a text is being typed or a sheet is open.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (!["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown"].includes(e.key) || e.metaKey || e.ctrlKey || e.altKey) return;
      const t = e.target as HTMLElement | null;
      if (t && (t.tagName === "INPUT" || t.tagName === "TEXTAREA" || t.isContentEditable)) return;
      if (document.querySelector("[role=dialog]")) return;
      const by = e.key === "ArrowLeft" || e.key === "ArrowUp" ? -1 : 1;
      const center = (id: string) => {
        const at = spotOf(id);
        if (at) rf.setCenter(at.x, at.y, { zoom: Math.max(rf.getZoom(), 0.85), duration: 250 });
      };
      if (mode === "path" && way.ids.length) {
        if (e.key === "ArrowUp" || e.key === "ArrowDown") return;
        const order = [...way.ids].reverse();
        const at = order.indexOf(open || sel || "");
        const next = order[Math.min(order.length - 1, Math.max(0, (at < 0 ? order.length - 1 : at) + by))];
        e.preventDefault();
        setOpen(next);
        locate(next);
        return;
      }
      const steps = match ? data.steps.filter(match) : data.steps;
      const at = steps.findIndex((x) => x.id === sel);
      let i = at < 0 ? (by > 0 ? 0 : steps.length - 1) : at + by;
      if (!match && (e.key === "ArrowUp" || e.key === "ArrowDown")) {
        while (i >= 0 && i < steps.length && steps[i].kind !== "prompt") i += by;
      }
      if (i < 0 || i >= steps.length) return;
      e.preventDefault();
      setPicked(steps[i].id);
      setOpen(null);
      center(steps[i].id);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [mode, way, sel, open, data, nodes, spots, rf]);
  // A step clicked on the graph is picked: on 경로 it opens on the timeline, on 순서 its way is the one drawn.
  const pick = (id: string) => { if (mode === "path" && way.ids.length) setOpen(id); else { setPicked(id); setOpen(null); } };
  const sess = data.session;
  const meta: [string, ReactNode][] = [
    ["단말", sess.device], ["사람", sess.person], ["에이전트", sess.agent && <span className="inline-flex items-center gap-1.5"><span className="grid size-4 place-items-center rounded bg-white"><SvcLogo s="claude" size={11} /></span>{sess.agent}</span>],
    ["모델", sess.model], ["작업 폴더", sess.folder && <code className="font-mono text-[11px]">{sess.folder}</code>], ["세션 ID", <code className="font-mono text-[11px]">{sess.id}</code>], ["시작", sess.start], ["엔진 모드", sess.mode],
  ];
  return (
    <Pick.Provider value={pick}><Unfold.Provider value={(n) => setUnfolded((u) => { const v = new Set(u); if (v.has(n)) v.delete(n); else v.add(n); return v; })}><Nav.Provider value={{
      // The cards reach the rest of the record: a step by its ID (or the folded step holding that
      // Call), opened as a card is opened and brought into view.
      step: (id) => byId.get(id) ?? data.steps.find((x) => x.inside?.includes(`#${id.slice(1)}`)),
      go: (id) => {
        const to = byId.get(id) ? id : data.steps.find((x) => x.inside?.includes(`#${id.slice(1)}`))?.id;
        if (!to) return;
        if (mode === "path" && way.ids.length > 0) setOpen(to); else setPicked(to);
        locate(to);
      },
    }}>
      <div className="relative min-w-0 flex-1">
        <ReactFlow nodes={nodes} edges={edges} nodeTypes={nodeTypes} edgeTypes={edgeTypes} onNodeClick={(_, n) => n.type === "step" && pick(n.id)}
          panOnScroll panOnScrollMode={PanOnScrollMode.Free} zoomOnScroll={false} zoomOnPinch minZoom={0.2} maxZoom={1.8} proOptions={{ hideAttribution: true }} nodesConnectable={false} />
        <ToolStrip timeline={timeline} setTimeline={setTimeline} zoom />
        <ModeSwitch mode={mode} setMode={go} />
        <div className="pointer-events-none absolute bottom-3 left-1/2 flex -translate-x-1/2 items-center gap-2.5 rounded-md bg-panel/90 px-3 py-1 text-[12px] whitespace-nowrap text-mut tabular-nums">
          {mode === "path" ? (way.ids.length ? `경로 ${way.ids.length}단계 · 턴 ${way.ids.filter((id) => byId.get(id)?.kind === "prompt").length}` : "도구를 누르면 경로")
            : only && match ? `${ONLY[only].label} ${Math.max(0, shown.indexOf(sel ?? "")) + 1}/${shown.length}`
            : `노드 ${data.steps.length} · 외부 지시 ${data.steps.filter((s) => s.catch).length} · 이어 물음 ${data.parents?.length ?? 0}`}
          {lines && <Legend lines={lines} />}
          <span className="inline-flex gap-0.5 align-middle">
            {(mode === "path" && way.ids.length || match ? ["←", "→"] : ["←", "→", "↑", "↓"]).map((k) => (
              <kbd key={k} className="grid size-[17px] place-items-center rounded border border-white/15 bg-white/5 font-sans text-[10.5px] text-fg/70">{k}</kbd>
            ))}
          </span>
        </div>
      </div>
      {timeline && (
        <aside className="my-3 mr-3 flex min-h-0 w-[340px] shrink-0 min-[1500px]:w-[380px] flex-col rounded-xl bg-panel shadow-[0_12px_40px_rgba(0,0,0,.45)]">
          {onWay ? (
            // 경로: what is being explained at the top, then how it came to run.
            <>
              <div className="flex h-10 shrink-0 items-center gap-2 px-3.5"><Route className="size-3.5" /><h2 className="text-[13px] font-semibold">경위</h2></div>
              {byId.get(way.ids[0]) && <StoryHead s={byId.get(way.ids[0])!} steps={way.ids.length} turns={way.ids.filter((id) => byId.get(id)?.kind === "prompt").length} onClick={() => { setOpen(way.ids[0]); locate(way.ids[0]); }} />}
            </>
          ) : (
            <>
              <div className="flex h-10 shrink-0 items-center gap-2 px-3.5"><ListTree className="size-3.5" /><h2 className="text-[13px] font-semibold">{only && match ? ONLY[only].label : "타임라인"}</h2><span className="rounded-full bg-chip px-2 text-[11.5px] text-mut">{shown.length}</span></div>
              <dl className="grid grid-cols-[60px_minmax(0,1fr)] gap-x-3 gap-y-1 px-3.5 pb-3 text-[11.5px]">
                {meta.filter(([, v]) => v).map(([k, v]) => <div key={k} className="contents"><dt className="text-mut">{k}</dt><dd className="m-0 truncate text-fg/85">{v}</dd></div>)}
              </dl>
            </>
          )}
          <div className={cx("scroll relative min-h-0 flex-1 overflow-auto border-t border-white/8 px-3 pt-2.5 pb-5", !onWay && "space-y-1")}>
            {!onWay && <i className="absolute top-3 bottom-2 left-[21.5px] w-px bg-white/12" />}
            {onWay ? (
              <Story data={data} way={way} refs={refs} picked={open || sel}
                onPick={(id) => { setOpen(id); locate(id); }} />
            ) : shown.map((id, i) => {
              const s = byId.get(id)!;
              const onPath = mode === "path" && way.ids.length > 0;
              const h = onPath && i > 0 ? way.hops.find((x) => x.to === id) : undefined;
              return (
                <div key={id} className="space-y-1">
                  {h && <HopMark h={h} />}
                  <Card s={s} open={onPath ? open === id || (open === null && sel === id) : sel === id} cardRef={(el) => (refs.current[id] = el)}
                    onToggle={() => {
                      // Opening a card brings its step into view on the graph.
                      const opening = onPath ? open !== id : sel !== id;
                      if (onPath) setOpen(opening ? id : "");
                      else setPicked(opening ? id : null);
                      if (opening) locate(id);
                    }}
                    onWay={onPath || s.kind === "prompt" && !data.parents?.some((p) => p.prompt === id) ? undefined : () => { setPicked(id); setOpen(null); go("path"); }} />
                </div>
              );
            })}
          </div>
        </aside>
      )}
    </Nav.Provider></Unfold.Provider></Pick.Provider>
  );
}
