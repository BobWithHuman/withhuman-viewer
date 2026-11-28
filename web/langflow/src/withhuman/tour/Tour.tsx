// The first session's tour: five coach marks in Langflow's popover (its PopoverContent, with
// Radix's arrow) anchored on the real page, each lighting its target: a tool call node, the red
// line from a tool result to the Call it told (or the 경로 button), the record panel's verdict and
// gauge, the session list, the engine menu. Each mark: an icon, a short label, chips, the step
// count, back / next arrows, 다음 and 건너뛰기 (Escape skips too). It waits for a session's canvas
// with its nodes (the try session may still be coming) and for the onboarding to be gone.

import * as PopoverPrimitive from "@radix-ui/react-popover";
import { type ReactNode, useEffect, useState } from "react";
import ForwardedIconComponent from "@/components/common/genericIconComponent";
import { Button } from "@/components/ui/button";
import { Popover, PopoverAnchor, PopoverContent } from "@/components/ui/popover";
import useFlowStore from "@/stores/flowStore";
import { cn } from "@/utils/utils";
import { current } from "../bridge";
import { useRecordPanel } from "../detail/store";
import { fieldKey } from "../flow/convert";
import { router } from "../router";
import { useTour } from "./store";

type Side = "top" | "right" | "bottom" | "left";
type Mark = {
  id: string;
  icon: string;
  label: string;
  chips: ReactNode;
  side: Side;
  align?: "start" | "center" | "end";
  /** Makes the target show (a button pressed, the canvas framed) before it is looked for. */
  prepare?: () => void;
  /** Undoes what `prepare` put on the page. */
  leave?: () => void;
  find: () => Element | null;
};

const $ = (sel: string) => document.querySelector(sel);
const visible = (el: Element | null) => {
  if (!el) return null;
  const r = el.getBoundingClientRect();
  return r.width > 2 && r.height > 2 && r.right > 0 && r.left < window.innerWidth ? el : null;
};

/** Frames these nodes on the canvas (Langflow's React Flow instance), not closer than 1:1. */
const frame = (ids: string[]) =>
  useFlowStore.getState().reactFlowInstance?.fitView({ nodes: ids.map((id) => ({ id })), duration: 350, maxZoom: 1, padding: 0.5 });

/** The node holding the last refused Call, else the first tool call node. */
function refusedNodeEl() {
  const steps = current().record?.steps ?? [];
  const refused = [...steps].reverse().find((s) => s.verdict === "deny" || s.verdict === "rejected");
  const row = refused ? $(`.react-flow__node [data-withhuman-field="${fieldKey(refused.id)}"]`) : null;
  return (row?.closest(".react-flow__node") as HTMLElement | null) ?? (document.querySelector(".react-flow__node-genericNode") as HTMLElement | null);
}

/** The edge into a box's "외부 지시" row: from the tool result's text to the Calls it told. */
function causeEdge() {
  for (const row of document.querySelectorAll<HTMLElement>(".react-flow__node [data-withhuman-field]")) {
    const field = row.dataset.withhumanField ?? "";
    if (field.startsWith("step_") || !row.textContent?.includes("외부 지시")) continue;
    const node = (row.closest(".react-flow__node") as HTMLElement | null)?.dataset.id;
    if (!node) continue;
    for (const e of document.querySelectorAll<SVGGElement>(".react-flow__edge")) {
      const id = e.getAttribute("data-id") ?? "";
      if (id.includes(`-${node}`) && id.includes(field)) return { el: e, id, target: node };
    }
  }
  return null;
}

/** While the cause mark is up: its edge red, dashed and moving, its two nodes not dimmed. */
const CAUSE_STYLE = "withhuman-tour-cause";
function markCause(on: boolean) {
  document.getElementById(CAUSE_STYLE)?.remove();
  const c = on ? causeEdge() : null;
  if (!c) return;
  const source = useFlowStore.getState().edges.find((e) => e.id === c.id)?.source;
  const q = (v: string) => JSON.stringify(v);
  const style = document.createElement("style");
  style.id = CAUSE_STYLE;
  style.textContent = [
    "@keyframes withhuman-tour-flow { to { stroke-dashoffset: -20; } }",
    `.react-flow__edge[data-id=${q(c.id)}] { opacity: 1 !important; }`,
    `.react-flow__edge[data-id=${q(c.id)}] .react-flow__edge-path { stroke: hsl(var(--destructive)) !important; stroke-width: 3px !important; stroke-dasharray: 8 6; animation: withhuman-tour-flow .8s linear infinite; }`,
    ...[c.target, source].filter(Boolean).map((n) => `.react-flow__node[data-id=${q(n!)}] { opacity: 1 !important; }`),
  ].join("\n");
  document.head.appendChild(style);
  frame([c.target, source].filter(Boolean) as string[]);
}

const Chip = ({ children, tone }: { children: ReactNode; tone?: "err" }) => (
  <span
    className={cn(
      "inline-flex h-[20px] items-center gap-1 rounded px-1.5 text-[11px] font-medium",
      tone === "err" ? "bg-error-background text-error-foreground" : "bg-muted text-muted-foreground",
    )}
  >
    {children}
  </span>
);
const Ico = ({ name }: { name: string }) => <ForwardedIconComponent name={name} className="h-3 w-3" />;

const MARKS: Mark[] = [
  {
    id: "node",
    icon: "Wrench",
    label: "도구 호출 노드",
    side: "right",
    chips: (
      <>
        <Chip>
          <Ico name="MousePointerClick" />
          상세
        </Chip>
        <Chip tone="err">차단</Chip>
      </>
    ),
    prepare: () => {
      const id = refusedNodeEl()?.dataset.id;
      if (id) frame([id]);
    },
    find: () => visible(refusedNodeEl()),
  },
  {
    id: "cause",
    icon: "Route",
    label: "외부 지시 선",
    side: "bottom",
    chips: (
      <>
        <Chip>
          <svg width="22" height="6" aria-hidden="true">
            <path d="M1 3 H21" stroke="hsl(var(--destructive))" strokeWidth="2" strokeDasharray="4 3" />
          </svg>
          결과 → 호출
        </Chip>
        <Chip>
          <Ico name="Route" />
          경로
        </Chip>
      </>
    ),
    prepare: () => markCause(true),
    leave: () => markCause(false),
    find: () => visible(causeEdge()?.el ?? null) ?? visible($('[data-testid="record-toolbar-path"]')),
  },
  {
    id: "panel",
    icon: "Gauge",
    label: "판정 · 점수",
    side: "left",
    chips: (
      <>
        <Chip tone="err">차단</Chip>
        <Chip>
          <Ico name="Gauge" />
          게이지
        </Chip>
        <Chip>
          <Ico name="Route" />
          경로
        </Chip>
      </>
    ),
    prepare: () => {
      if (!$('[data-testid="record-panel"]')) ($('[data-testid="record-toolbar-panel"]') as HTMLButtonElement | null)?.click();
      // 상세: the verdict, the deciding rule and the score on its gauge.
      const panel = useRecordPanel.getState();
      if (panel.view === "step") panel.setTab("detail");
    },
    find: () => visible($('[data-testid="record-panel"]')) ?? visible($('[data-testid="record-toolbar-panel"]')),
  },
  {
    id: "sessions",
    icon: "PanelLeft",
    label: "세션 목록",
    side: "right",
    chips: (
      <>
        <Chip>
          <kbd className="font-mono">/</kbd>
          검색
        </Chip>
        <Chip tone="err">차단 N</Chip>
      </>
    ),
    find: () => visible($('[data-sidebar="sidebar"]')) ?? visible($('[data-testid="shad-sidebar"]')),
  },
  {
    id: "engine",
    icon: "Server",
    label: "엔진 · 훅",
    side: "bottom",
    align: "end",
    chips: (
      <>
        <Chip>
          <Ico name="Webhook" />훅
        </Chip>
        <Chip>
          <Ico name="Compass" />
          둘러보기
        </Chip>
      </>
    ),
    find: () => visible($("#user_menu_button")),
  },
];

const PAD = 6;

/** A session's canvas with its nodes, and no onboarding over it. */
const ready = () =>
  (router()?.state.location.pathname ?? "").startsWith("/flow/") &&
  !!document.querySelector(".react-flow__node") &&
  !document.querySelector('[data-testid="onboard"]');

export function Tour() {
  const { waiting, active, step, begin, go, end } = useTour();
  const [rect, setRect] = useState<DOMRect | null>(null);

  // Wanted: wait for the canvas (and a moment for Langflow's fit and the panel's framing).
  useEffect(() => {
    if (!waiting || active) return;
    let seen = 0;
    const t = window.setInterval(() => {
      seen = ready() ? seen + 1 : 0;
      if (seen >= 3) begin();
    }, 400);
    // Asked from the menu while not on a canvas: the open session's.
    const r = router();
    const id = current().record?.shell?.current || current().record?.session.id;
    if (r && id && !r.state.location.pathname.startsWith("/flow/")) void r.navigate(`/flow/${id}`);
    return () => window.clearInterval(t);
  }, [waiting, active, begin]);

  // Follow the mark's target (it moves with pans, the panel's width, the window).
  const mark = MARKS[step];
  useEffect(() => {
    if (!active) return;
    mark.prepare?.();
    let raf = 0;
    let last = "";
    let missing = 0;
    const tick = () => {
      const el = mark.find();
      const r = el?.getBoundingClientRect() ?? null;
      const sig = r ? `${r.x | 0},${r.y | 0},${r.width | 0},${r.height | 0}` : "";
      if (sig !== last) {
        last = sig;
        setRect(r);
      }
      // A target that never shows (no sidebar, no panel): its mark is passed over.
      if (!r && ++missing > 90) {
        if (step < MARKS.length - 1) go(step + 1);
        else end();
        return;
      }
      raf = requestAnimationFrame(tick);
    };
    // After the prepare's click has rendered.
    const t = window.setTimeout(() => (raf = requestAnimationFrame(tick)), 160);
    return () => {
      window.clearTimeout(t);
      cancelAnimationFrame(raf);
      mark.leave?.();
    };
  }, [active, step]); // eslint-disable-line react-hooks/exhaustive-deps

  useEffect(() => {
    if (!active) return;
    const key = (e: KeyboardEvent) => {
      if (e.key === "Escape") end();
      else if (e.key === "ArrowRight") step < MARKS.length - 1 ? go(step + 1) : end();
      else if (e.key === "ArrowLeft" && step > 0) go(step - 1);
    };
    window.addEventListener("keydown", key, true);
    return () => window.removeEventListener("keydown", key, true);
  }, [active, step, go, end]);

  if (!active || !rect) return null;
  const box = { left: rect.left - PAD, top: rect.top - PAD, width: rect.width + PAD * 2, height: rect.height + PAD * 2 };
  const anchor = { current: { getBoundingClientRect: () => new DOMRect(box.left, box.top, box.width, box.height) } };
  const last = step === MARKS.length - 1;

  return (
    <>
      <div
        className="pointer-events-none fixed z-[58] rounded-lg transition-all duration-300 ease-out"
        style={{ ...box, boxShadow: "0 0 0 2px hsl(var(--ring)), 0 0 0 9999px rgb(0 0 0 / 0.38)" }}
        data-testid="tour-spot"
      />
      <Popover open modal={false}>
        <PopoverAnchor virtualRef={anchor as never} />
        <PopoverContent
          side={mark.side}
          align={mark.align ?? "center"}
          sideOffset={10}
          collisionPadding={12}
          className="z-[59] w-[264px] p-0"
          onOpenAutoFocus={(e) => e.preventDefault()}
          onInteractOutside={(e) => e.preventDefault()}
          onEscapeKeyDown={() => end()}
          data-testid={`tour-${mark.id}`}
        >
          <div className="flex items-center gap-2.5 px-3 pb-2 pt-3">
            <span className="flex h-7 w-7 shrink-0 items-center justify-center rounded-md bg-muted">
              <ForwardedIconComponent name={mark.icon} className="h-4 w-4" />
            </span>
            <span className="min-w-0 flex-1 truncate text-sm font-semibold">{mark.label}</span>
            <span className="text-xs tabular-nums text-muted-foreground">
              {step + 1}/{MARKS.length}
            </span>
          </div>
          <div className="flex flex-wrap items-center gap-1.5 px-3 pb-3 pl-[50px]">{mark.chips}</div>
          <div className="flex items-center gap-1 border-t px-2 py-2">
            <Button variant="ghost" size="xs" className="h-7 text-muted-foreground" onClick={end} data-testid="tour-skip">
              건너뛰기
            </Button>
            <div className="ml-auto flex items-center gap-1">
              <Button variant="ghost" size="iconMd" className="h-7 w-7" disabled={step === 0} onClick={() => go(step - 1)} aria-label="이전" data-testid="tour-prev">
                <ForwardedIconComponent name="ChevronLeft" className="h-4 w-4" />
              </Button>
              <Button size="xs" className="h-7" onClick={() => (last ? end() : go(step + 1))} data-testid="tour-next">
                {last ? "완료" : "다음"}
                {!last && <ForwardedIconComponent name="ChevronRight" className="h-3.5 w-3.5" />}
              </Button>
            </div>
          </div>
          <PopoverPrimitive.Arrow width={14} height={7} className="fill-popover [filter:drop-shadow(0_1px_0_hsl(var(--border)))]" />
        </PopoverContent>
      </Popover>
    </>
  );
}
