// The last step's picture, on Langflow's canvas look: the engine as a node (its parts with their
// dots, how Calls are judged, Python) and each chosen agent as a node tied to it. A tie flows
// (dashed, moving) while that hook goes in, turns green when it is in, red when it failed.

import ForwardedIconComponent from "@/components/common/genericIconComponent";
import { cn } from "@/utils/utils";
import { AgentIcon } from "../chrome/agent-icon";
import { type OnboardAgent, type OnboardData, type State, ENGINE, scope } from "./data";
import { StateIcon } from "./parts";

const W = 660;
const EX = 16;
const EW = 252;
const AX = 404;
const AW = 240;
const HEAD = 46;
const ROW = 30;
const AH = 58;
const AGAP = 14;

const CSS = `
@keyframes wh-ob-flow { to { stroke-dashoffset: -24; } }
.wh-ob-busy { stroke-dasharray: 6 6; animation: wh-ob-flow .8s linear infinite; }
@media (prefers-reduced-motion: reduce) { .wh-ob-busy { animation: none; } }
`;

const stroke: Record<State, string> = {
  idle: "hsl(var(--border))",
  busy: "hsl(var(--accent-blue-foreground))",
  ok: "hsl(var(--accent-emerald-foreground))",
  err: "hsl(var(--destructive))",
};

export function ReadyCanvas({
  d,
  agents,
  engine,
  judge,
  started,
}: {
  d: OnboardData;
  agents: OnboardAgent[];
  engine: State;
  judge: "rules" | "jev";
  started: boolean;
}) {
  const rows = d.parts.length + 2;
  const EH = HEAD + rows * ROW + 8;
  const colH = agents.length ? agents.length * AH + (agents.length - 1) * AGAP : AH;
  const H = Math.max(EH, colH) + 56;
  const ey = (H - EH) / 2 - 8;
  const ay0 = (H - colH) / 2;
  const out = { x: EX + EW, y: ey + HEAD / 2 };
  const up = d.parts.filter((p) => p.up).length;
  // An agent waiting its turn after the engine is up is about to go in.
  const stateOf = (a: OnboardAgent): State => (a.state === "idle" && started && engine === "ok" ? "busy" : a.state);

  return (
    <div className="relative w-full overflow-hidden rounded-xl border bg-background" style={{ height: H + 16 }} data-testid="onboard-ready-canvas">
      <style>{CSS}</style>
      <div className="absolute inset-0 [background-image:radial-gradient(hsl(var(--border))_1px,transparent_1px)] [background-size:16px_16px]" />
      <div className="absolute left-1/2 top-2 -translate-x-1/2" style={{ width: W, height: H }}>
        <svg className="absolute inset-0 overflow-visible" width={W} height={H}>
          {(agents.length ? agents : [null]).map((a, i) => {
            const y = ay0 + i * (AH + AGAP) + AH / 2;
            const s = a ? stateOf(a) : "idle";
            return (
              <path
                key={a?.kind ?? "none"}
                d={`M${out.x},${out.y} C${out.x + 70},${out.y} ${AX - 70},${y} ${AX},${y}`}
                fill="none"
                stroke={stroke[s]}
                strokeWidth={s === "idle" ? 1.5 : 2}
                strokeDasharray={a ? undefined : "4 5"}
                className={s === "busy" ? "wh-ob-busy" : undefined}
              />
            );
          })}
        </svg>

        <div
          className={cn("generic-node-div absolute rounded-xl border shadow-sm", engine === "err" && "border-destructive", engine === "ok" && "border-accent-emerald-foreground")}
          style={{ left: EX, top: ey, width: EW, height: EH }}
          data-testid="onboard-engine-node"
        >
          <div className="flex h-[46px] items-center gap-2.5 border-b px-3">
            <span className="flex h-7 w-7 shrink-0 items-center justify-center rounded-md bg-muted">
              <ForwardedIconComponent name="Server" className="h-4 w-4" />
            </span>
            <span className="text-sm font-medium">엔진</span>
            <span className="ml-auto font-mono text-[11px] tabular-nums text-muted-foreground">
              {up}/{d.parts.length}
            </span>
            <StateIcon state={engine} text={d.run.text} />
            <i className={cn("absolute -right-[6px] top-[18px] h-2.5 w-2.5 rounded-full border-2 bg-background", engine === "ok" ? "border-accent-emerald-foreground" : "border-muted-foreground/50")} />
          </div>
          <div className="py-1">
            {d.parts.map((p) => (
              <div key={p.name} className="flex h-[30px] items-center gap-2 px-3 text-xs">
                <i className={cn("h-2 w-2 shrink-0 rounded-full transition-colors duration-300", p.up ? "bg-accent-emerald-foreground" : started ? "animate-pulse bg-muted-foreground/40" : "bg-muted-foreground/30")} />
                <span className="font-medium">{p.name}</span>
              </div>
            ))}
            <div className="flex h-[30px] items-center gap-2 px-3 text-xs">
              <ForwardedIconComponent name={judge === "jev" ? "Cpu" : "ListChecks"} className="h-3.5 w-3.5 text-muted-foreground" />
              <span className="font-medium">판정</span>
              <span className="ml-auto text-muted-foreground">{judge === "jev" ? `Jev · ${d.model}` : "로컬 규칙"}</span>
            </div>
            <div className="flex h-[30px] items-center gap-2 px-3 text-xs">
              <ForwardedIconComponent name="Terminal" className="h-3.5 w-3.5 text-muted-foreground" />
              <span className="font-medium">Python</span>
              <span className={cn("ml-auto min-w-0 truncate font-mono text-[11px]", d.python ? "text-muted-foreground" : "text-destructive")}>
                {d.python ?? "없음"}
              </span>
            </div>
          </div>
          <span className="absolute -bottom-6 left-3 font-mono text-[11px] text-muted-foreground">{ENGINE}</span>
        </div>

        {agents.length === 0 && (
          <div
            className="absolute flex items-center gap-2.5 rounded-xl border border-dashed px-3 text-sm text-muted-foreground"
            style={{ left: AX, top: ay0, width: AW, height: AH }}
          >
            <ForwardedIconComponent name="Webhook" className="h-4 w-4" />
            훅 · 나중에
          </div>
        )}
        {agents.map((a, i) => {
          const s = stateOf(a);
          return (
            <div
              key={a.kind}
              className={cn(
                "generic-node-div absolute flex-row items-center gap-2.5 rounded-xl border px-3 shadow-sm transition-colors",
                s === "ok" && "border-accent-emerald-foreground",
                s === "err" && "border-destructive",
              )}
              style={{ left: AX, top: ay0 + i * (AH + AGAP), width: AW, height: AH, display: "flex" }}
              data-testid={`onboard-ready-${a.kind}`}
            >
              <i className={cn("absolute -left-[6px] top-1/2 h-2.5 w-2.5 -translate-y-1/2 rounded-full border-2 bg-background", s === "ok" ? "border-accent-emerald-foreground" : s === "err" ? "border-destructive" : "border-muted-foreground/50")} />
              <span className="flex h-8 w-8 shrink-0 items-center justify-center rounded-md bg-muted">
                <AgentIcon kind={a.kind} className="h-[18px] w-[18px]" />
              </span>
              <div className="flex min-w-0 flex-1 flex-col">
                <span className="truncate text-sm font-medium">{a.name}</span>
                <span className={cn("truncate text-[11px]", s === "err" ? "text-destructive" : "text-muted-foreground", (a.folder || s === "err") && "font-mono")}>
                  {s === "err" ? a.text : a.folder ?? scope(a)}
                </span>
              </div>
              <StateIcon state={s} text={a.text} />
            </div>
          );
        })}
      </div>
    </div>
  );
}
