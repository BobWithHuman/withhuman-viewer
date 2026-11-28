// Two sessions side by side (`/all?compare=<a>,<b>`): a strip of their counts as a butterfly chart
// (A to the left, B to the right, the difference after), and both sessions' canvases drawn with
// Langflow's own node and edge types in React Flow instances of their own, read-only. A count row
// pressed lights its steps on both canvases; 동기 moves both views together.

import {
  Background,
  BaseEdge,
  type EdgeProps,
  getBezierPath,
  type Edge,
  type Node,
  ReactFlow,
  ReactFlowProvider,
  type ReactFlowInstance,
  useReactFlow,
  useUpdateNodeInternals,
  type Viewport,
} from "@xyflow/react";
import { useEffect, useMemo, useRef, useState } from "react";
import ForwardedIconComponent from "@/components/common/genericIconComponent";
import ShadTooltip from "@/components/common/shadTooltipComponent";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { useCustomNavigate } from "@/customization/hooks/use-custom-navigate";
import { nodeTypes } from "@/pages/FlowPage/consts";
import { cn } from "@/utils/utils";
import { type RecordData, type ShellSession, type Step, useWithHuman } from "../bridge";
import { fieldKey } from "../flow/convert";
import { ago } from "../flow/sessions";
import { flowPath } from "../chrome/app";
import { C } from "../viz/colors";
import { type Loaded, loadPair } from "./load";

type Metric = "calls" | "denied" | "review" | "outside" | "steps" | "turns";
const METRICS: { id: Metric; label: string; tone: string; test?: (s: Step, rec: RecordData) => boolean }[] = [
  { id: "calls", label: "호출", tone: C.quiet, test: (s) => s.kind === "call" || s.kind === "group" },
  { id: "denied", label: "차단", tone: C.deny, test: (s) => s.verdict === "deny" || s.verdict === "rejected" },
  { id: "review", label: "확인", tone: C.review, test: (s) => s.verdict === "review" },
  { id: "outside", label: "외부 지시", tone: C.outside, test: (s, rec) => (!!s.catch && s.kind === "result") || rec.causes.some(([, c]) => c === s.id) },
  { id: "steps", label: "단계", tone: C.quiet },
  { id: "turns", label: "턴", tone: C.quiet, test: (s) => s.kind === "prompt" },
];

function count(m: Metric, s: ShellSession | undefined, rec: RecordData | undefined) {
  if (m === "steps") return rec?.steps.length ?? 0;
  if (m === "turns") return rec?.steps.filter((x) => x.kind === "prompt").length ?? 0;
  return s?.[m] ?? 0;
}

/** One row of the strip: A's bar grows left, B's right, from the middle. */
function Row({ m, a, b, on, pick }: { m: (typeof METRICS)[number]; a: number; b: number; on: boolean; pick?: () => void }) {
  const max = Math.max(a, b, 1);
  const d = b - a;
  const tip = (
    <span className="flex flex-col gap-0.5 tabular-nums">
      <span className="font-medium">{m.label}</span>
      <span>A {a} · B {b} · {d > 0 ? `+${d}` : d}</span>
    </span>
  );
  return (
    <ShadTooltip content={tip} side="top" styleClasses="z-50">
      <button
        type="button"
        onClick={pick}
        disabled={!pick}
        aria-pressed={on}
        data-testid={`compare-metric-${m.id}`}
        className={cn(
          "grid w-full grid-cols-[4.5rem_2.5rem_1fr_1fr_2.5rem_3rem] items-center gap-2 rounded-md px-2 py-1 text-xs tabular-nums",
          pick && "hover:bg-muted",
          on && "bg-muted ring-1 ring-border",
        )}
      >
        <span className="truncate text-left text-muted-foreground">{m.label}</span>
        <span className="text-right font-medium">{a}</span>
        <span className="flex h-3 justify-end">
          <i className="h-full rounded-l-[4px] transition-all" style={{ width: `${(a / max) * 100}%`, background: m.tone }} />
        </span>
        <span className="flex h-3 justify-start">
          <i className="h-full rounded-r-[4px] transition-all" style={{ width: `${(b / max) * 100}%`, background: m.tone }} />
        </span>
        <span className="text-left font-medium">{b}</span>
        <span className={cn("text-right", d > 0 ? "text-destructive" : d < 0 ? "text-accent-emerald-foreground" : "text-muted-foreground")}>
          {d === 0 ? "=" : d > 0 ? `+${d}` : d}
        </span>
      </button>
    </ShadTooltip>
  );
}

/** The marks of a canvas: flagged rows barred; with a count picked, its steps lit, the rest dimmed. */
function marks(scope: string, rec: RecordData, nodes: Node[], focus: Metric | null) {
  const bar = (s: Step) =>
    s.verdict === "deny" || s.verdict === "rejected" ? C.deny : s.verdict === "review" ? C.review : s.catch ? C.outside : null;
  const rules: string[] = [];
  for (const s of rec.steps) {
    const c = bar(s);
    if (c) rules.push(`${scope} [data-withhuman-field="${fieldKey(s.id)}"]{box-shadow:inset 3px 0 0 ${c}}`);
  }
  const test = METRICS.find((m) => m.id === focus)?.test;
  if (focus && test) {
    const lit = new Set(rec.steps.filter((s) => test(s, rec)).map((s) => fieldKey(s.id)));
    for (const n of nodes) {
      const keys = Object.keys((n.data as any)?.node?.template ?? {});
      if (!keys.some((k) => lit.has(k))) rules.push(`${scope} .react-flow__node[data-id="${n.id}"]{opacity:.3}`);
    }
    rules.push(`${scope} [data-withhuman-field^="step_"]{opacity:.35}`);
    for (const k of lit) rules.push(`${scope} [data-withhuman-field="${k}"]{opacity:1;background:hsl(var(--muted))}`);
    rules.push(`${scope} .react-flow__edge{opacity:.25}`);
  }
  return rules.join("\n");
}

/** Langflow's edge reads its ends from the main canvas' store; here the ends React Flow gives. */
function CompareEdge({ sourceX, sourceY, targetX, targetY, sourcePosition, targetPosition, style, markerEnd }: EdgeProps) {
  const [path] = getBezierPath({ sourceX, sourceY, sourcePosition, targetX, targetY, targetPosition });
  return <BaseEdge path={path} style={style} markerEnd={markerEnd} />;
}
const edgeTypes = { default: CompareEdge };

/** Once Langflow's nodes have laid out their rows: their handles measured again (they were taken
 * before the rows had their height, so edges would start at the nodes' corners), then the whole
 * session fitted. */
function Settle({ ids }: { ids: string[] }) {
  const rf = useReactFlow();
  const update = useUpdateNodeInternals();
  const key = ids.join(",");
  useEffect(() => {
    const t1 = window.setTimeout(() => update(key.split(",")), 250);
    const t2 = window.setTimeout(() => void rf.fitView({ padding: 0.08, maxZoom: 0.9 }), 400);
    return () => {
      window.clearTimeout(t1);
      window.clearTimeout(t2);
    };
  }, [key, rf, update]);
  return null;
}

function Pane({
  side, s, got, focus, onInit, onMove,
}: {
  side: "A" | "B";
  s?: ShellSession;
  got: Loaded | null | undefined;
  focus: Metric | null;
  onInit: (i: ReactFlowInstance) => void;
  onMove: (v: Viewport) => void;
}) {
  const navigate = useCustomNavigate();
  const nodes: Node[] = useMemo(
    () => (got?.flow.data.nodes ?? []).map((n: Node) => ({ ...n, draggable: false, selectable: false, connectable: false })),
    [got],
  );
  const edges: Edge[] = useMemo(() => (got?.flow.data.edges ?? []).map((e: Edge) => ({ ...e, selectable: false })), [got]);
  const scope = `[data-compare-pane="${side}"]`;
  const css = useMemo(() => (got ? marks(scope, got.rec, nodes, focus) : ""), [got, nodes, focus, scope]);
  const folder = s?.folder?.split("/").filter(Boolean).pop();
  return (
    <section className="flex min-w-0 flex-1 flex-col overflow-hidden rounded-lg border" aria-label={side} data-compare-pane={side}>
      <header className="flex items-center gap-2 border-b bg-background px-3 py-2">
        <Badge variant="secondaryStatic" size="tag" className="shrink-0 font-semibold">
          {side}
        </Badge>
        <span className="min-w-0 truncate text-sm font-semibold">{s?.title ?? "—"}</span>
        {s?.live && <i aria-label="실행 중" className="h-1.5 w-1.5 shrink-0 animate-pulse rounded-full bg-accent-emerald-foreground" />}
        <span className="ml-auto flex shrink-0 items-center gap-1.5 text-xs text-muted-foreground">
          {folder && <span>{folder}</span>}
          {s && <span>{ago(s.at)}</span>}
          {s && (
            <ShadTooltip content="열기" side="bottom">
              <Button variant="ghost" size="iconSm" className="h-6 w-6" aria-label="열기" onClick={() => navigate(flowPath(s.id))}>
                <ForwardedIconComponent name="SquareArrowOutUpRight" className="h-3.5 w-3.5" />
              </Button>
            </ShadTooltip>
          )}
        </span>
      </header>
      <div className="relative min-h-0 flex-1 bg-canvas">
        <style>{css}</style>
        {got === undefined ? (
          <div className="flex h-full items-center justify-center">
            <ForwardedIconComponent name="Loader2" className="h-5 w-5 animate-spin text-muted-foreground" />
          </div>
        ) : got === null || nodes.length === 0 ? (
          <div className="flex h-full flex-col items-center justify-center gap-2 text-sm text-muted-foreground">
            <ForwardedIconComponent name="Workflow" className="h-6 w-6" />
            단계 없음
          </div>
        ) : (
          <ReactFlowProvider>
            <ReactFlow
              nodes={nodes}
              edges={edges}
              nodeTypes={nodeTypes}
              edgeTypes={edgeTypes}
              nodesDraggable={false}
              nodesConnectable={false}
              elementsSelectable={false}
              nodesFocusable={false}
              edgesFocusable={false}
              deleteKeyCode={[]}
              minZoom={0.1}
              maxZoom={2}
              onInit={onInit}
              onMove={(e, v) => {
                if (e) onMove(v);
              }}
              proOptions={{ hideAttribution: true }}
              className="theme-attribution"
            >
              <Settle ids={nodes.map((n) => n.id)} />
              <div aria-hidden="true" style={{ display: "contents" }}>
                <Background id={`compare-bg-${side}`} size={2} gap={20} />
              </div>
            </ReactFlow>
          </ReactFlowProvider>
        )}
      </div>
    </section>
  );
}

export function CompareView({ a, b }: { a: string; b: string }) {
  const shell = useWithHuman().record?.shell;
  const navigate = useCustomNavigate();
  const [got, setGot] = useState<[Loaded | null | undefined, Loaded | null | undefined]>([undefined, undefined]);
  const [focus, setFocus] = useState<Metric | null>(null);
  const [synced, setSynced] = useState(false);
  const inst = useRef<(ReactFlowInstance | null)[]>([null, null]);

  useEffect(() => {
    let gone = false;
    setGot([undefined, undefined]);
    void loadPair(a, b).then((pair) => {
      if (!gone) setGot(pair);
    });
    return () => {
      gone = true;
    };
  }, [a, b]);

  const sa = shell?.sessions.find((s) => s.id === a);
  const sb = shell?.sessions.find((s) => s.id === b);
  const ra = got[0]?.rec;
  const rb = got[1]?.rec;
  const move = (from: 0 | 1) => (v: Viewport) => {
    if (synced) inst.current[from === 0 ? 1 : 0]?.setViewport(v);
  };
  const toggleSync = () => {
    const next = !synced;
    setSynced(next);
    const v = inst.current[0]?.getViewport();
    if (next && v) inst.current[1]?.setViewport(v, { duration: 300 });
  };

  return (
    <div className="flex h-full w-full flex-col gap-3 p-4" data-testid="compare-view">
      <div className="flex items-center gap-2 text-sm font-medium">
        <Button variant="ghost" size="iconSm" aria-label="기록" onClick={() => navigate("/all")} data-testid="compare-back">
          <ForwardedIconComponent name="ArrowLeft" className="h-4 w-4" />
        </Button>
        기록
        <ForwardedIconComponent name="ChevronRight" className="h-3.5 w-3.5 text-muted-foreground" />
        비교
        <div className="ml-auto flex items-center gap-1.5">
          {focus && (
            <Badge variant="secondaryStatic" size="tag" className="cursor-pointer gap-1" onClick={() => setFocus(null)}>
              {METRICS.find((m) => m.id === focus)?.label}
              <ForwardedIconComponent name="X" className="h-3 w-3" />
            </Badge>
          )}
          <ShadTooltip content="동기" side="bottom">
            <Button
              variant={synced ? "default" : "outline"}
              size="sm"
              className="h-7 gap-1.5"
              aria-pressed={synced}
              onClick={toggleSync}
              data-testid="compare-sync"
            >
              <ForwardedIconComponent name={synced ? "Link" : "Unlink"} className="h-3.5 w-3.5" />
              동기
            </Button>
          </ShadTooltip>
        </div>
      </div>
      <div className="grid grid-cols-1 gap-x-6 rounded-lg border bg-background p-2 lg:grid-cols-2" data-testid="compare-strip">
        {[METRICS.slice(0, 3), METRICS.slice(3)].map((half, i) => (
          <div key={i} className="flex flex-col">
            <div className="grid grid-cols-[4.5rem_2.5rem_1fr_1fr_2.5rem_3rem] gap-2 px-2 pb-1 text-xxs font-semibold text-muted-foreground">
              <span />
              <span className="text-right">A</span>
              <span />
              <span />
              <span>B</span>
              <span className="text-right">Δ</span>
            </div>
            {half.map((m) => (
              <Row
                key={m.id}
                m={m}
                a={count(m.id, sa, ra)}
                b={count(m.id, sb, rb)}
                on={focus === m.id}
                pick={m.test ? () => setFocus(focus === m.id ? null : m.id) : undefined}
              />
            ))}
          </div>
        ))}
      </div>
      <div className="flex min-h-0 flex-1 gap-3">
        <Pane side="A" s={sa} got={got[0]} focus={focus} onInit={(i) => (inst.current[0] = i)} onMove={move(0)} />
        <Pane side="B" s={sb} got={got[1]} focus={focus} onInit={(i) => (inst.current[1] = i)} onMove={move(1)} />
      </div>
    </div>
  );
}
