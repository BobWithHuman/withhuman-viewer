// The flow page's right-hand panel for a recorded session, in Langflow's panel look:
// - nothing picked: the session's timeline, every step in order (each turn under its prompt),
//   with the filter chips 외부 지시 / 차단 / 승인 요청;
// - a node picked on the canvas: its steps, the flagged one opened in place;
// - a step picked (in the timeline, a field row or a way on the canvas): its detail (상세) and the
//   way back to the request it came from (경로), which also lights the canvas.
// Resizable at its left edge, closable; the canvas toolbar (RecordToolbar) opens it again.

import { type PointerEvent as ReactPointerEvent, useEffect, useMemo, useRef } from "react";
import ForwardedIconComponent from "@/components/common/genericIconComponent";
import ShadTooltip from "@/components/common/shadTooltipComponent";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Separator } from "@/components/ui/separator";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import useFlowsManagerStore from "@/stores/flowsManagerStore";
import { cn } from "@/utils/utils";
import { useWithHuman } from "../bridge";
import type { RecordData, Step } from "../types";
import { CanvasLight, type Converted, frame, useCanvasPicking, useConverted } from "./canvas";
import { DetailBody, KindTile, VerdictBadge } from "./DetailBody";
import { type Hop, ONLY, type Only, TIE, TONE, type Way, clock, flagged, judgeLabel, opening, pct, toneOf, wayBack } from "./model";
import { DEFAULT_W, useRecordPanel } from "./store";
import { Output } from "./text";

/** The open session's record, while its canvas is the one shown. */
export function useOpenRecord() {
  const { record } = useWithHuman();
  const flowId = useFlowsManagerStore((s) => s.currentFlowId);
  return record && record.session.id === flowId ? record : null;
}

const nameOf = (s: Step) => (s.quote ? `"${s.name}"` : s.name);
/** A text's first lines as one reading line: markdown's marks off, spaces folded. */
const plain = (text: string) =>
  text
    .replace(/```[a-z]*|`/g, " ")
    .replace(/^\s*#{1,6}\s*/gm, "")
    .replace(/\*\*|__|~~/g, "")
    .replace(/^\s*[-*>]\s+/gm, "")
    .replace(/\s+/g, " ")
    .trim();

// --- frame -------------------------------------------------------------------

function Header({ children, onBack }: { children: React.ReactNode; onBack?: () => void }) {
  const setOpen = useRecordPanel((s) => s.setOpen);
  return (
    <div className="flex h-12 shrink-0 items-center gap-2 px-3">
      {onBack && (
        <ShadTooltip content="타임라인">
          <Button variant="ghost" size="iconSm" className="h-7 w-7 shrink-0" onClick={onBack} aria-label="타임라인" data-testid="record-panel-back">
            <ForwardedIconComponent name="ArrowLeft" className="h-4 w-4" />
          </Button>
        </ShadTooltip>
      )}
      <div className="flex min-w-0 flex-1 items-center gap-2">{children}</div>
      <ShadTooltip content="닫기">
        <Button variant="ghost" size="iconSm" className="h-7 w-7 shrink-0" onClick={() => setOpen(false)} aria-label="닫기" data-testid="record-panel-close">
          <ForwardedIconComponent name="PanelRightClose" className="h-4 w-4" />
        </Button>
      </ShadTooltip>
    </div>
  );
}

function ResizeHandle() {
  const width = useRecordPanel((s) => s.width);
  const setWidth = useRecordPanel((s) => s.setWidth);
  const down = (e: ReactPointerEvent) => {
    e.preventDefault();
    const x0 = e.clientX, w0 = width;
    let w = w0;
    const move = (m: PointerEvent) => { w = w0 + x0 - m.clientX; setWidth(w); };
    const up = () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
      document.body.style.cursor = "";
      setWidth(w, true);
    };
    document.body.style.cursor = "col-resize";
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
  };
  return (
    <div
      role="separator"
      aria-orientation="vertical"
      onPointerDown={down}
      onDoubleClick={() => setWidth(DEFAULT_W, true)}
      className="group absolute inset-y-0 -left-[3px] z-20 w-[6px] cursor-col-resize"
    >
      <i className="mx-auto block h-full w-px transition-colors group-hover:bg-border" />
    </div>
  );
}

// --- rows ----------------------------------------------------------------------

/** A step as a list row: its kind, its name, what the engine said, when. */
function StepRow({ s, on, onClick, indent }: { s: Step; on?: boolean; onClick: () => void; indent?: boolean }) {
  const loud = flagged(s);
  return (
    <button
      type="button"
      onClick={onClick}
      data-testid={`record-step-${s.id}`}
      className={cn(
        "flex w-full min-w-0 items-center gap-2.5 rounded-md px-2 py-1.5 text-left text-sm transition-colors",
        on ? "bg-muted" : "hover:bg-muted/60",
        indent && "pl-4",
      )}
    >
      <KindTile s={s} />
      <span className={cn("min-w-0 flex-1 truncate", loud || s.kind === "prompt" ? "font-medium text-foreground" : "text-secondary-foreground")}>{nameOf(s)}</span>
      <VerdictBadge s={s} quiet />
      <span className="shrink-0 text-xs tabular-nums text-muted-foreground">{clock(s.at)}</span>
    </button>
  );
}

function Chip({ label, n, on, tone, onClick }: { label: string; n: number; on: boolean; tone: string; onClick: () => void }) {
  return (
    <button
      type="button"
      onClick={onClick}
      disabled={!n && !on}
      aria-pressed={on}
      data-testid={`record-chip-${label}`}
      className={cn(
        "inline-flex h-7 items-center gap-1.5 rounded-full border px-2.5 text-xs transition-colors disabled:opacity-50",
        on ? "border-foreground/40 bg-muted text-foreground" : "border-border text-muted-foreground hover:bg-muted hover:text-foreground",
      )}
    >
      <i className="h-1.5 w-1.5 rounded-full" style={{ background: `hsl(${tone})` }} />
      {label}
      <span className="font-semibold tabular-nums text-foreground">{n}</span>
    </button>
  );
}

// --- the timeline ---------------------------------------------------------------

const CHIP_TONE: Record<Only, string> = { outside: TONE.catch.hsl, deny: TONE.deny.hsl, ask: TONE.hold.hsl };

function Timeline({ data }: { data: RecordData }) {
  const only = useRecordPanel((s) => s.only);
  const setOnly = useRecordPanel((s) => s.setOnly);
  const pickStep = useRecordPanel((s) => s.pickStep);
  const shown = only ? data.steps.filter(ONLY[only].is) : data.steps;
  return (
    <>
      <Header>
        <ForwardedIconComponent name="ListTree" className="h-4 w-4 shrink-0 text-muted-foreground" />
        <span className="truncate text-sm font-semibold">{only ? ONLY[only].label : "타임라인"}</span>
        <Badge variant="secondaryStatic" size="tag" className="tabular-nums">{shown.length}</Badge>
      </Header>
      <div className="flex shrink-0 flex-wrap gap-1.5 px-3 pb-3">
        {(Object.keys(ONLY) as Only[]).map((k) => (
          <Chip key={k} label={ONLY[k].label} n={data.steps.filter(ONLY[k].is).length} on={only === k} tone={CHIP_TONE[k]} onClick={() => setOnly(only === k ? null : k)} />
        ))}
      </div>
      <Separator />
      <div className="custom-scroll min-h-0 flex-1 overflow-y-auto px-2 py-2" data-testid="record-timeline">
        {shown.map((s, i) => (
          <div key={s.id}>
            {!only && s.kind === "prompt" && i > 0 && <Separator className="my-2" />}
            <StepRow s={s} indent={!only && s.kind !== "prompt"} onClick={() => pickStep(s.id)} />
          </div>
        ))}
        {!shown.length && <div className="py-10 text-center text-sm text-muted-foreground">없음</div>}
      </div>
    </>
  );
}

// --- a node's steps ---------------------------------------------------------------

function StepActions({ s }: { s: Step }) {
  const pickStep = useRecordPanel((x) => x.pickStep);
  const setTab = useRecordPanel((x) => x.setTab);
  const go = (tab: "detail" | "path") => { pickStep(s.id); setTab(tab); };
  return (
    <div className="flex gap-1.5">
      <Button variant="primary" size="xs" className="h-7 gap-1.5 text-xs" onClick={() => go("path")}>
        <ForwardedIconComponent name="Route" className="h-3.5 w-3.5" />경로
      </Button>
      <Button variant="ghost" size="xs" className="h-7 gap-1.5 text-xs" onClick={() => go("detail")}>
        <ForwardedIconComponent name="Maximize2" className="h-3.5 w-3.5" />상세
      </Button>
    </div>
  );
}

function NodeView({ data, conv, find, go }: { data: RecordData; conv: Converted; find: (id: string) => Step | undefined; go: (id: string) => void }) {
  const node = useRecordPanel((s) => s.node)!;
  const inner = useRecordPanel((s) => s.inner);
  const openInner = useRecordPanel((s) => s.openInner);
  const back = useRecordPanel((s) => s.back);
  const meta = conv.nodes.get(node);
  const steps = (conv.stepsOf.get(node) ?? []).map((id) => find(id)!).filter(Boolean);
  return (
    <>
      <Header onBack={back}>
        <span className="grid h-6 w-6 shrink-0 place-items-center rounded-md bg-muted">
          <ForwardedIconComponent name={meta?.icon ?? "Box"} className="h-3.5 w-3.5" />
        </span>
        <span className="truncate text-sm font-semibold">{meta?.name ?? "노드"}</span>
        <Badge variant="secondaryStatic" size="tag" className="tabular-nums">{steps.length}</Badge>
      </Header>
      <Separator />
      <div className="custom-scroll min-h-0 flex-1 space-y-1 overflow-y-auto px-2 py-2" data-testid="record-node">
        {steps.map((s) => (
          <div key={s.id} className={cn("rounded-lg", inner === s.id && "border border-border")}>
            <StepRow s={s} on={inner === s.id} onClick={() => openInner(inner === s.id ? null : s.id)} />
            {inner === s.id && (
              <div className="space-y-3 px-3 pb-3 pt-2">
                <DetailBody s={s} find={find} go={go} />
                <StepActions s={s} />
              </div>
            )}
          </div>
        ))}
      </div>
    </>
  );
}

// --- a step: 상세 and 경로 -----------------------------------------------------------------

function HopLine({ h }: { h: Hop }) {
  const color = `hsl(${TIE[h.tie].hsl})`;
  return (
    <div className="relative flex h-7 items-center pl-8" data-testid={`record-hop-${h.tie}`}>
      <i className="absolute bottom-0 left-[15px] top-0 w-[2px] rounded-full" style={{ background: color, opacity: 0.55 }} />
      <span className="inline-flex items-center gap-1.5 text-xs font-semibold" style={{ color }}>
        <ForwardedIconComponent name="ArrowDown" className="h-3 w-3" />
        {h.label}
        {h.p != null && <span className="font-medium tabular-nums">{pct(h.p)}</span>}
      </span>
    </div>
  );
}

/** A step of the way: what it is, its own words where it is a prompt or an answer, the flagged
 * part where it is a tool result; opened in place, all of it. */
function WayItem({ s, end, open, onClick, find, go }: { s: Step; end: boolean; open: boolean; onClick: () => void; find: (id: string) => Step | undefined; go: (id: string) => void }) {
  const said = s.kind === "prompt" || s.kind === "answer" ? plain(s.text?.body ?? "") : null;
  const mark = s.text?.mark;
  return (
    <div
      className={cn(
        "rounded-lg border bg-background transition-colors",
        open ? "border-muted-foreground" : end ? "border-foreground/30" : "border-border",
      )}
      data-testid={`record-way-${s.id}`}
    >
      <button type="button" onClick={onClick} className="flex w-full min-w-0 items-start gap-2.5 px-2.5 py-2 text-left">
        <KindTile s={s} size="md" />
        <span className="min-w-0 flex-1">
          <span className="flex min-w-0 items-center gap-2">
            <span className="min-w-0 flex-1 truncate text-sm font-medium">{nameOf(s)}</span>
            <VerdictBadge s={s} quiet />
          </span>
          <span className="flex items-center gap-1.5 text-xs tabular-nums text-muted-foreground">
            {s.type}<span className="text-border">|</span>{clock(s.at)}
            {s.judge && judgeLabel(s.judge) && <><span className="text-border">|</span><span className={TONE[toneOf(s)].text}>{judgeLabel(s.judge)}</span></>}
          </span>
          {said && <span className="mt-1 line-clamp-2 text-xs leading-relaxed text-secondary-foreground">"{said}"</span>}
          {s.catch && (
            <span className="mt-1 flex flex-wrap items-center gap-1.5">
              <Badge variant="secondaryStatic" size="tag" className={TONE.catch.chip}>{s.catch.name}</Badge>
              <span className="font-mono text-[11px] text-muted-foreground">{s.catch.sub}</span>
            </span>
          )}
        </span>
        <ForwardedIconComponent name="ChevronDown" className={cn("mt-1 h-3.5 w-3.5 shrink-0 text-muted-foreground transition-transform", open && "rotate-180")} />
      </button>
      {!open && mark && (
        <div className="px-2.5 pb-2.5">
          <Output text={mark} shell={false} mark={mark} lines={3} />
        </div>
      )}
      {open && (
        <div className="border-t px-3 pb-3 pt-3">
          <DetailBody s={s} find={find} go={go} />
        </div>
      )}
    </div>
  );
}

function WayList({ way, find, go }: { way: Way; find: (id: string) => Step | undefined; go: (id: string) => void }) {
  const inner = useRecordPanel((s) => s.inner);
  const openInner = useRecordPanel((s) => s.openInner);
  const order = [...way.ids].reverse();
  const turns = order.filter((id) => find(id)?.kind === "prompt").length;
  const outside = way.hops.filter((h) => h.tie === "cause").length;
  return (
    <div className="space-y-3">
      <div className="flex flex-wrap gap-1.5">
        <Badge variant="secondaryStatic" size="tag" className="tabular-nums">{order.length}단계</Badge>
        <Badge variant="secondaryStatic" size="tag" className="tabular-nums">턴 {turns}</Badge>
        {outside > 0 && <Badge variant="secondaryStatic" size="tag" className={cn("tabular-nums", TONE.deny.chip)}>외부 지시 {outside}</Badge>}
      </div>
      <div>
        {order.map((id, i) => {
          const s = find(id);
          if (!s) return null;
          const hop = i > 0 ? way.hops.find((h) => h.to === id) : undefined;
          return (
            <div key={id}>
              {hop && <HopLine h={hop} />}
              <WayItem s={s} end={i === order.length - 1} open={inner === id} onClick={() => openInner(inner === id ? null : id)} find={find} go={go} />
            </div>
          );
        })}
      </div>
    </div>
  );
}

function StepView({ s, way, find, go }: { s: Step; way: Way; find: (id: string) => Step | undefined; go: (id: string) => void }) {
  const tab = useRecordPanel((x) => x.tab);
  const setTab = useRecordPanel((x) => x.setTab);
  const back = useRecordPanel((x) => x.back);
  const body = useRef<HTMLDivElement>(null);
  useEffect(() => { body.current?.scrollTo({ top: 0 }); }, [s.id, tab]);
  return (
    <>
      <Header onBack={back}>
        <KindTile s={s} size="md" />
        <span className="min-w-0 flex-1 truncate text-sm font-semibold">{nameOf(s)}</span>
      </Header>
      <div className="flex shrink-0 flex-wrap items-center gap-1.5 px-3 pb-2 text-xs tabular-nums text-muted-foreground">
        <span>{s.type}</span>
        <span className="text-border">|</span>
        <span>{clock(s.at)}</span>
        <VerdictBadge s={s} />
        {s.catch && s.verdict && s.verdict !== "allow" && (
          <Badge variant="secondaryStatic" size="tag" className={cn("tabular-nums", TONE.catch.chip)}>지시 {s.catch.score}%</Badge>
        )}
      </div>
      <Tabs value={tab} onValueChange={(v) => setTab(v as "detail" | "path")} className="shrink-0">
        <TabsList className="h-9 justify-start gap-1 border-b px-2">
          <TabsTrigger value="detail" className="h-9 px-2" data-testid="record-tab-detail">상세</TabsTrigger>
          <TabsTrigger value="path" className="h-9 gap-1.5 px-2" data-testid="record-tab-path">
            경로<span className="text-xs tabular-nums text-muted-foreground">{way.ids.length}</span>
          </TabsTrigger>
        </TabsList>
      </Tabs>
      <div ref={body} className="custom-scroll min-h-0 flex-1 overflow-y-auto px-3 py-3" data-testid={`record-${tab}`}>
        {tab === "detail" ? <DetailBody s={s} find={find} go={go} /> : <WayList way={way} find={find} go={go} />}
      </div>
    </>
  );
}

// --- the panel ---------------------------------------------------------------------

export function RecordPanel() {
  const data = useOpenRecord();
  const conv = useConverted(data);
  const st = useRecordPanel();
  const byId = useMemo(() => new Map((data?.steps ?? []).map((s) => [s.id, s])), [data]);
  // A step by its ID, or the fold holding that Call.
  const find = (id: string) => byId.get(id) ?? data?.steps.find((x) => x.inside?.includes(`#${id.slice(1)}`));

  // A session opened here (or one whose picked step went away): start it over.
  useEffect(() => {
    if (!data) return;
    if (st.session !== data.session.id) st.start(data);
    else if (st.picked && !byId.has(st.picked)) st.start(data);
    else if (st.node && !conv?.stepsOf.has(st.node)) st.back();
  }, [data, byId]); // eslint-disable-line react-hooks/exhaustive-deps

  const picked = data && st.view === "step" && st.picked ? byId.get(st.picked) : undefined;
  const way = useMemo(() => (data && picked ? wayBack(data, picked.id) : { ids: [], hops: [] }), [data, picked]);
  const onPath = !!picked && st.tab === "path" && way.ids.length > 0;

  // Asked to bring steps into view: on 경로 the whole way, else the steps' nodes.
  useEffect(() => {
    if (!st.want || !data || !conv) return;
    const steps = onPath && st.want.steps[0] === picked?.id ? way.ids : st.want.steps;
    const nodes = [...new Set(steps.map((id) => conv.nodeOf.get(id)).filter(Boolean) as string[])];
    frame(nodes, data.session.id);
  }, [st.want?.seq]); // eslint-disable-line react-hooks/exhaustive-deps

  useCanvasPicking(conv, data, !!data);
  // Escape goes back to the timeline.
  useEffect(() => {
    const key = (e: KeyboardEvent) => {
      const t = e.target as HTMLElement | null;
      if (e.key !== "Escape" || document.querySelector("[role=dialog]") || (t && (t.tagName === "INPUT" || t.tagName === "TEXTAREA"))) return;
      if (useRecordPanel.getState().view !== "timeline") useRecordPanel.getState().back();
    };
    window.addEventListener("keydown", key);
    return () => window.removeEventListener("keydown", key);
  }, []);

  if (!data || !conv || !st.open) return null;

  const go = (id: string) => { if (byId.has(id)) st.pickStep(id); };
  const node = st.view === "node" ? st.node : picked ? conv.nodeOf.get(picked.id) ?? null : null;
  const row = st.view === "node" ? st.inner : onPath ? st.inner ?? picked?.id ?? null : picked?.id ?? null;
  const innerNode = onPath && st.inner ? conv.nodeOf.get(st.inner) ?? node : node;

  return (
    <aside
      className="relative flex h-full shrink-0 flex-col border-l bg-background"
      style={{ width: st.width }}
      data-testid="record-panel"
    >
      <ResizeHandle />
      <CanvasLight conv={conv} data={data} node={innerNode} row={row} way={onPath ? way : null} />
      {st.view === "node" && st.node ? (
        <NodeView data={data} conv={conv} find={find} go={go} />
      ) : picked ? (
        <StepView s={picked} way={way} find={find} go={go} />
      ) : (
        <Timeline data={data} />
      )}
    </aside>
  );
}

/** The canvas toolbar (where Langflow keeps Playground and Share): 경로 on and off for the step
 * picked (or the session's opening one), and the panel shown or hidden. */
export function RecordToolbar() {
  const data = useOpenRecord();
  const st = useRecordPanel();
  if (!data) return null;
  const onPath = st.open && st.view === "step" && !!st.picked && st.tab === "path";
  const togglePath = () => {
    if (onPath) return st.setTab("detail");
    const id = st.view === "step" ? st.picked : st.view === "node" ? st.inner : opening(data);
    if (!id) return st.setOpen(true);
    st.pickStep(id);
    st.setTab("path");
  };
  return (
    <div className="flex items-center gap-1">
      <ShadTooltip content="경로">
        <Button variant={onPath ? "ghostActive" : "ghost"} size="md" className="!gap-1.5 !px-2 !font-normal" onClick={togglePath} aria-pressed={onPath} data-testid="record-toolbar-path">
          <ForwardedIconComponent name="Route" className="h-4 w-4" />
          <span className="text-mmd">경로</span>
        </Button>
      </ShadTooltip>
      <ShadTooltip content={st.open ? "패널 닫기" : "패널 열기"}>
        <Button variant={st.open ? "ghostActive" : "ghost"} size="md" className="!gap-1.5 !px-2 !font-normal" onClick={() => st.setOpen(!st.open)} aria-pressed={st.open} data-testid="record-toolbar-panel">
          <ForwardedIconComponent name={st.open ? "PanelRightClose" : "PanelRightOpen"} className="h-4 w-4" />
          <span className="text-mmd">상세</span>
        </Button>
      </ShadTooltip>
    </div>
  );
}
