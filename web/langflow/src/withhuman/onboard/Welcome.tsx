// The first step's picture: a small session drawn as Langflow draws a flow (its dotted canvas,
// its node cards with an icon box, a title and field rows, its handles and bezier edges). The
// request comes in, the tool calls follow it, and a line from a tool result's text (red, dashed)
// reaches a Call the engine refused. It plays once, then the red line keeps flowing.

import ForwardedIconComponent from "@/components/common/genericIconComponent";
import { cn } from "@/utils/utils";

const W = 700;
const H = 270;
const HEAD = 46;
const ROW = 34;

type Row = { name: string; value: string; tone?: "deny" | "cause"; out?: boolean; in?: boolean };
type Box = { id: string; x: number; y: number; w: number; icon: string; title: string; rows: Row[]; delay: number; deny?: boolean };

const BOXES: Box[] = [
  { id: "p", x: 14, y: 92, w: 164, icon: "MessageSquare", title: "프롬프트", rows: [{ name: "요청", value: "회고 문서 요약", out: true }], delay: 0.1 },
  {
    id: "t1", x: 230, y: 34, w: 184, icon: "Wrench", title: "도구 호출 2",
    rows: [{ name: "요청에 따라", value: "", in: true }, { name: "Read", value: "retro.md" }, { name: "WebFetch", value: "notion.so/…", out: true, tone: "cause" }],
    delay: 0.7,
  },
  {
    id: "t2", x: 516, y: 132, w: 176, icon: "Wrench", title: "도구 호출 1", deny: true,
    rows: [{ name: "외부 지시 72%", value: "", in: true, tone: "cause" }, { name: "Bash · 차단", value: "curl -d @.env", tone: "deny" }],
    delay: 1.5,
  },
];

const rowY = (b: Box, i: number) => b.y + HEAD + i * ROW + ROW / 2;
const handle = (id: string, side: "out" | "in") => {
  const b = BOXES.find((x) => x.id === id)!;
  const i = b.rows.findIndex((r) => (side === "out" ? r.out : r.in));
  return { x: side === "out" ? b.x + b.w : b.x, y: rowY(b, i) };
};
const curve = (a: { x: number; y: number }, b: { x: number; y: number }) =>
  `M${a.x},${a.y} C${a.x + 48},${a.y} ${b.x - 48},${b.y} ${b.x},${b.y}`;

const CSS = `
@keyframes wh-ob-in { from { opacity: 0; transform: translateY(8px) scale(.97); } to { opacity: 1; transform: none; } }
@keyframes wh-ob-draw { from { stroke-dashoffset: 320; } to { stroke-dashoffset: 0; } }
@keyframes wh-ob-flow { to { stroke-dashoffset: -24; } }
@keyframes wh-ob-fade { from { opacity: 0; } to { opacity: 1; } }
@keyframes wh-ob-ring { 0%,100% { box-shadow: 0 0 0 0 hsl(var(--destructive) / .0); } 50% { box-shadow: 0 0 0 6px hsl(var(--destructive) / .16); } }
.wh-ob-node { opacity: 0; animation: wh-ob-in .5s cubic-bezier(.2,.7,.2,1) forwards; }
.wh-ob-deny { animation: wh-ob-in .5s cubic-bezier(.2,.7,.2,1) forwards, wh-ob-ring 2.4s ease-in-out 2.6s infinite; }
.wh-ob-edge { stroke-dasharray: 320; stroke-dashoffset: 320; animation: wh-ob-draw .7s ease-out forwards; }
.wh-ob-cause { opacity: 0; stroke-dasharray: 6 6; animation: wh-ob-fade .4s ease-out forwards, wh-ob-flow .9s linear infinite; }
.wh-ob-chip { opacity: 0; animation: wh-ob-fade .4s ease-out forwards; }
@media (prefers-reduced-motion: reduce) { .wh-ob-node, .wh-ob-deny, .wh-ob-edge, .wh-ob-cause, .wh-ob-chip { animation: none; opacity: 1; stroke-dashoffset: 0; } }
`;

function Node({ b }: { b: Box }) {
  return (
    <div
      className={cn(
        "generic-node-div absolute rounded-xl border shadow-sm",
        b.deny ? "wh-ob-deny border-destructive" : "wh-ob-node",
      )}
      style={{ left: b.x, top: b.y, width: b.w, animationDelay: `${b.delay}s` }}
    >
      <div className="flex h-[46px] items-center gap-2.5 border-b px-3">
        <span className="flex h-7 w-7 shrink-0 items-center justify-center rounded-md bg-muted">
          <ForwardedIconComponent name={b.icon} className="h-4 w-4 text-foreground" />
        </span>
        <span className="truncate text-sm font-medium">{b.title}</span>
      </div>
      <div className="pb-1">
        {b.rows.map((r) => (
          <div key={r.name} className="relative flex h-[34px] items-center gap-2 px-3 text-xs">
            <span
              className={cn(
                "shrink-0 font-medium",
                r.tone === "deny" ? "text-destructive" : r.tone === "cause" ? "text-destructive" : "text-foreground",
              )}
            >
              {r.name}
            </span>
            {r.value && <span className="min-w-0 truncate font-mono text-[11px] text-muted-foreground">{r.value}</span>}
            {(r.in || r.out) && (
              <i
                className={cn(
                  "absolute top-1/2 h-2.5 w-2.5 -translate-y-1/2 rounded-full border-2 bg-background",
                  r.in ? "-left-[6px]" : "-right-[6px]",
                  r.tone === "cause" ? "border-destructive" : "border-accent-blue-foreground",
                )}
              />
            )}
          </div>
        ))}
      </div>
    </div>
  );
}

export function WelcomeCanvas({ className }: { className?: string }) {
  const ask = curve(handle("p", "out"), handle("t1", "in"));
  const cause = curve(handle("t1", "out"), handle("t2", "in"));
  const mid = { x: (handle("t1", "out").x + handle("t2", "in").x) / 2, y: (handle("t1", "out").y + handle("t2", "in").y) / 2 };
  return (
    <div
      className={cn("relative overflow-hidden rounded-xl border bg-background", className)}
      style={{ width: W + 24, height: H + 24 }}
      aria-hidden="true"
      data-testid="onboard-welcome-canvas"
    >
      <style>{CSS}</style>
      <div className="absolute inset-0 [background-image:radial-gradient(hsl(var(--border))_1px,transparent_1px)] [background-size:16px_16px]" />
      <div className="absolute left-3 top-3" style={{ width: W, height: H }}>
        <svg className="absolute inset-0 overflow-visible" width={W} height={H}>
          <path d={ask} className="wh-ob-edge" fill="none" stroke="hsl(var(--accent-blue-foreground))" strokeWidth={2} style={{ animationDelay: "1.1s" }} />
          <path d={cause} className="wh-ob-cause" fill="none" stroke="hsl(var(--destructive))" strokeWidth={2.5} style={{ animationDelay: "2.1s, 2.1s" }} />
        </svg>
        {BOXES.map((b) => (
          <Node key={b.id} b={b} />
        ))}
        <span
          className="wh-ob-chip absolute -translate-x-1/2 -translate-y-1/2 rounded-full border border-destructive/40 bg-background px-2 py-0.5 text-[11px] font-medium tabular-nums text-destructive shadow-sm"
          style={{ left: mid.x, top: mid.y, animationDelay: "2.4s" }}
        >
          72%
        </span>
      </div>
    </div>
  );
}
