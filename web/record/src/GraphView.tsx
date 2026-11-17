import {
  type Edge, type EdgeProps, type Node, type NodeProps, BaseEdge, EdgeLabelRenderer, Handle, MarkerType, Position, ReactFlow, useReactFlow,
} from "@xyflow/react";
import { ChevronRight, Eye, ListTree, MapPin, Plus, Scan, TriangleAlert, ZoomIn, ZoomOut } from "lucide-react";
import { createContext, useContext, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import type { RecordData, Step } from "./data";
import { DetailBody, KIND_ICON, LogoDisc, SvcLogo, Tip, VerdictChip, cx, flagged, tone } from "./parts";

// 기록's graph: what the agent asked for and what came back, left to right. A step that was
// caught or refused is drawn large with its verdict; a quiet one is a small dot. A Call a tool
// result made is joined to that result by an orange line. The timeline on the right holds the
// same steps, each opening to its detail.

type D = { step: Step; picked: boolean; dim: boolean };
const Pick = createContext<(id: string) => void>(() => {});

export const clock = (at: Step["at"]) => {
  if (at === undefined) return "";
  if (typeof at === "string") return at;
  const d = new Date(at), z = (n: number) => String(n).padStart(2, "0");
  return `${z(d.getHours())}:${z(d.getMinutes())}:${z(d.getSeconds())}`;
};
const big = (s: Step) => flagged(s) || s.kind === "prompt";

function StepNode({ data }: NodeProps<Node<D>>) {
  const { step: s, picked, dim } = data;
  const pick = useContext(Pick);
  const size = big(s) ? 64 : 22;
  return (
    <div className={cx("relative transition-opacity duration-300", dim && "opacity-25")} style={{ width: size, height: size }} onClick={() => pick(s.id)}>
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
        <div className={cx("truncate", big(s) ? "text-[14px] font-semibold text-fg" : "text-[12px] text-mut")}>{s.kind === "prompt" ? `"${s.name}"` : s.name}</div>
        {big(s) && <div className="mt-1 flex justify-center"><VerdictChip s={s} /></div>}
      </div>
    </div>
  );
}
const nodeTypes = { step: StepNode };

/** A cause: an arch from the result's Call over the line to the Call it made, higher the farther. */
function CauseEdge({ id, sourceX, sourceY, targetX, targetY, markerEnd, style }: EdgeProps) {
  const h = 46 + Math.abs(targetX - sourceX) * 0.22;
  const top = Math.min(sourceY, targetY) - h;
  const path = `M${sourceX},${sourceY} C${sourceX},${top} ${targetX},${top} ${targetX},${targetY}`;
  const lx = (sourceX + targetX) / 2, ly = Math.min(sourceY, targetY) - h * 0.75;
  return (
    <>
      <BaseEdge id={id} path={path} markerEnd={markerEnd} style={style} />
      <EdgeLabelRenderer>
        <div className="nodrag nopan pointer-events-none absolute rounded bg-bg px-1.5 py-px text-[11px] font-semibold text-hold" style={{ transform: `translate(-50%,-50%) translate(${lx}px,${ly}px)` }}>응답이 유도</div>
      </EdgeLabelRenderer>
    </>
  );
}
const edgeTypes = { cause: CauseEdge };

function ToolStrip({ focus, setFocus, timeline, setTimeline }: { focus: boolean; setFocus: (v: boolean) => void; timeline: boolean; setTimeline: (v: boolean) => void }) {
  const rf = useReactFlow();
  const btn = (label: string, icon: ReactNode, onClick?: () => void, on?: boolean) => (
    <Tip label={label} side="right" key={label}>
      <button onClick={onClick} className={cx("grid size-8 place-items-center rounded-md transition-colors [&_svg]:size-[17px]", on ? "bg-act/20 text-act" : "text-mut hover:bg-white/8 hover:text-fg")}>{icon}</button>
    </Tip>
  );
  return (
    <div className="absolute top-3 left-3 z-10 flex flex-col items-center gap-0.5 rounded-lg bg-panel p-1 shadow-[0_8px_24px_rgba(0,0,0,.35)]">
      {btn("원인 길만", <Eye />, () => setFocus(!focus), focus)}
      {btn("타임라인", <ListTree />, () => setTimeline(!timeline), timeline)}
      <i className="my-1 h-px w-5 bg-white/10" />
      {btn("확대", <ZoomIn />, () => rf.zoomIn({ duration: 250 }))}
      {btn("전체 보기", <Scan />, () => rf.fitView({ duration: 400, padding: 0.2 }))}
      {btn("축소", <ZoomOut />, () => rf.zoomOut({ duration: 250 }))}
    </div>
  );
}

function Card({ s, open, onToggle, onLocate, cardRef }: { s: Step; open: boolean; onToggle: () => void; onLocate: () => void; cardRef: (el: HTMLDivElement | null) => void }) {
  return (
    <div ref={cardRef} className="relative pl-9">
      <span className={cx("absolute top-3 left-0 grid size-6 place-items-center rounded-md [&_svg]:size-3.5 [&_svg]:stroke-[2.4]", tone(s))}>{KIND_ICON[s.kind]}</span>
      <div className={cx("rounded-lg border bg-bg transition-colors", open ? "border-pick shadow-[0_0_0_1px_#6488b4]" : "border-transparent hover:border-white/10")}>
        <div className="flex cursor-pointer items-start gap-2 px-3 py-2.5" onClick={onToggle}>
          <ChevronRight className={cx("mt-0.5 size-4 shrink-0 text-mut transition-transform", open && "rotate-90")} />
          <div className="min-w-0 flex-1">
            <div className="flex items-center gap-2"><span className="truncate text-[14px] font-semibold text-fg">{s.kind === "prompt" ? `"${s.name}"` : s.name}</span><VerdictChip s={s} /></div>
            <div className="mt-0.5 text-[12px] text-mut">{s.type} <span className="mx-1 text-dim">|</span> {clock(s.at)}</div>
          </div>
          <Tip label="그래프에서 찾기"><button onClick={(e) => { e.stopPropagation(); onLocate(); }} className="grid size-7 place-items-center rounded-md text-mut hover:bg-white/8 hover:text-fg"><MapPin className="size-4" /></button></Tip>
        </div>
        {open && <div className="px-4 pt-1 pb-4"><DetailBody s={s} /></div>}
      </div>
    </div>
  );
}

/** Columns in order of the steps; a step on another lane (a subagent, a fork) sits above or below. */
function layout(data: RecordData, picked: string | null, focus: boolean) {
  const onPath = new Set(data.causes.flat());
  const nodes: Node<D>[] = data.steps.map((s, i) => {
    const half = big(s) ? 32 : 11;
    return {
      id: s.id, type: "step", position: { x: i * 210 - half, y: (s.lane ?? 0) * 150 - half },
      data: { step: s, picked: picked === s.id, dim: focus && !onPath.has(s.id) }, draggable: false, selectable: false,
    };
  });
  const cause = new Set(data.causes.map(([a, b]) => `${a}>${b}`));
  const edges: Edge[] = data.links.map(([a, b]) => {
    const c = cause.has(`${a}>${b}`);
    return {
      id: `${a}>${b}`, source: a, target: b, type: "default",
      style: c ? { stroke: "#f2a93b", strokeWidth: 2.2 } : { stroke: "#8a8a8a", strokeWidth: 1.6, opacity: focus ? 0.25 : 1 },
      markerEnd: { type: MarkerType.ArrowClosed, color: c ? "#f2a93b" : "#8a8a8a", width: 15, height: 15 },
    };
  });
  // A cause that is not also the next step: its own orange line, over the others.
  for (const [a, b] of data.causes) {
    if (data.links.some(([x, y]) => x === a && y === b)) continue;
    edges.push({
      id: `cause:${a}>${b}`, source: a, target: b, sourceHandle: "t-src", targetHandle: "t-tgt", type: "cause", zIndex: 5,
      style: { stroke: "#f2a93b", strokeWidth: 2, opacity: 0.9 },
      markerEnd: { type: MarkerType.ArrowClosed, color: "#f2a93b", width: 14, height: 14 },
    });
  }
  return { nodes, edges };
}

export function GraphView({ data }: { data: RecordData }) {
  const first = data.steps.find(flagged)?.id ?? null;
  const [picked, setPicked] = useState<string | null>(first);
  const [focus, setFocus] = useState(false);
  const [timeline, setTimeline] = useState(true);
  const rf = useReactFlow();
  const refs = useRef<Record<string, HTMLDivElement | null>>({});
  useEffect(() => { setPicked(data.steps.find(flagged)?.id ?? null); }, [data]);
  useEffect(() => { if (picked) refs.current[picked]?.scrollIntoView({ behavior: "smooth", block: "nearest" }); }, [picked]);
  // Open on the first step worth a look, at a size its name can be read; the whole is a button away.
  useEffect(() => {
    const id = data.steps.find(flagged)?.id;
    const k = id ? data.steps.findIndex((s) => s.id === id) : -1;
    requestAnimationFrame(() => {
      if (k < 0) rf.fitView({ padding: 0.22, maxZoom: 1 });
      else rf.setCenter(k * 210 + 120, (data.steps[k].lane ?? 0) * 150 - 70, { zoom: 0.85 });
    });
  }, [data, rf]);
  const { nodes, edges } = useMemo(() => layout(data, picked, focus), [data, picked, focus]);
  const locate = (id: string) => {
    const n = nodes.find((x) => x.id === id);
    if (n) rf.setCenter(n.position.x + 32, n.position.y + 40, { zoom: Math.max(rf.getZoom(), 0.9), duration: 500 });
  };
  const sess = data.session;
  const meta: [string, ReactNode][] = [
    ["단말", sess.device], ["사람", sess.person], ["에이전트", sess.agent && <span className="inline-flex items-center gap-1.5"><span className="grid size-4 place-items-center rounded bg-white"><SvcLogo s="claude" size={11} /></span>{sess.agent}</span>],
    ["모델", sess.model], ["작업 폴더", sess.folder && <code className="font-mono text-[12px]">{sess.folder}</code>], ["세션 ID", <code className="font-mono text-[12px]">{sess.id}</code>], ["시작", sess.start], ["엔진 모드", sess.mode],
  ];
  return (
    <Pick.Provider value={(id) => setPicked(id)}>
      <div className="relative min-w-0 flex-1">
        <ReactFlow nodes={nodes} edges={edges} nodeTypes={nodeTypes} edgeTypes={edgeTypes} minZoom={0.2} maxZoom={1.8} proOptions={{ hideAttribution: true }} nodesConnectable={false} />
        <ToolStrip focus={focus} setFocus={setFocus} timeline={timeline} setTimeline={setTimeline} />
        <div className="pointer-events-none absolute bottom-3 left-1/2 -translate-x-1/2 rounded-md bg-panel/90 px-3 py-1 text-[12px] text-mut">마디 {data.steps.length} · 원인 {data.causes.length}</div>
      </div>
      {timeline && (
        <aside className="my-3 mr-3 flex min-h-0 w-[480px] shrink-0 flex-col rounded-xl bg-panel shadow-[0_12px_40px_rgba(0,0,0,.45)]">
          <div className="flex h-12 shrink-0 items-center gap-2 px-4"><ListTree className="size-4" /><h2 className="text-[15px] font-semibold">타임라인</h2><span className="rounded-full bg-chip px-2 text-[11.5px] text-mut">{data.steps.length}</span></div>
          <dl className="grid grid-cols-[84px_1fr] gap-x-4 gap-y-2 px-5 pb-4 text-[13px]">
            {meta.filter(([, v]) => v).map(([k, v]) => <div key={k} className="contents"><dt className="text-right font-semibold">{k}</dt><dd className="m-0 truncate text-mut">{v}</dd></div>)}
          </dl>
          <div className="scroll relative min-h-0 flex-1 space-y-2.5 overflow-auto border-t border-white/8 px-4 pt-3 pb-6">
            <i className="absolute top-3 bottom-2 left-[27px] w-px bg-white/12" />
            {data.steps.map((s) => (
              <Card key={s.id} s={s} open={picked === s.id} cardRef={(el) => (refs.current[s.id] = el)}
                onToggle={() => setPicked(picked === s.id ? null : s.id)} onLocate={() => { setPicked(s.id); locate(s.id); }} />
            ))}
          </div>
        </aside>
      )}
    </Pick.Provider>
  );
}
