// The picture over the board: every MCP call of the last 30 days by the engine's verdict, a bar
// a day, and the tools by what they can do. A part of the split lights the tools of that nature.

import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { FIGURE, Legend, NATURE, NatureIcon, num, type Nature } from "@/kit";
import { cn } from "@/lib/utils";
import { DaysChart } from "./parts";
import { COLUMN, DAYS, NATURES, VERDICTS, split, type Mcp } from "./types";

export function Band({ mcp, now, nature, onNature }: { mcp: Mcp; now: number; nature: Nature | null; onNature: (n: Nature) => void }) {
  const tools = mcp.servers.flatMap((s) => s.tools);
  const parts = split(tools);
  const calls = parts.reduce((a, p) => a + p.value, 0);
  // Without the verdicts by day, the days show the calls alone.
  const byDay = mcp.days ?? Array.from({ length: DAYS }, (_, i) => ({ calls: tools.reduce((a, t) => a + (t.days[i] ?? 0), 0) }));
  const series = mcp.days ? VERDICTS.map((v) => ({ key: v, label: COLUMN[v].label, color: COLUMN[v].color })) : [{ key: "calls", label: "호출", color: "var(--info)" }];
  const natures = NATURES.map((n) => ({ n, count: tools.filter((t) => t.nature === n).length })).filter((x) => x.count > 0);
  return (
    <div className="grid h-[112px] grid-cols-[304px_minmax(0,1fr)_328px] border-b">
      <div className="flex min-w-0 flex-col justify-between px-4 py-3">
        <span className="text-[12px] text-muted-foreground">MCP 호출 · 30일</span>
        <span className={FIGURE}>{num(calls)}</span>
        <Legend items={parts} />
      </div>
      <DaysChart className="h-full min-w-0 py-2" rows={byDay} series={series} now={now} />
      <div className="flex min-w-0 flex-col justify-between border-l px-4 py-3">
        <span className="text-[12px] text-muted-foreground">도구 성격 <b className="tnum ml-1 font-medium text-foreground">{tools.length}</b></span>
        <div className="flex gap-1">
          {natures.map(({ n, count }) => (
            <Tooltip key={n}>
              <TooltipTrigger asChild>
                <button onClick={() => onNature(n)} aria-pressed={nature === n} style={{ flexGrow: count }}
                  className={cn("group flex min-w-[58px] basis-0 flex-col gap-1.5 text-left transition-opacity outline-none", nature && nature !== n && "opacity-40 hover:opacity-70")}>
                  <span className={cn("flex h-7 w-full items-center gap-1.5 rounded-md px-2 ring-1 ring-transparent transition-[filter,box-shadow] group-hover:brightness-125 group-focus-visible:ring-ring", nature === n && "ring-foreground/60")}
                    style={{ background: `color-mix(in oklab, ${NATURE[n].color} 22%, transparent)` }}>
                    <NatureIcon n={n} className="size-3.5" />
                    <b className="tnum text-[12.5px] font-semibold">{count}</b>
                  </span>
                  <span className={cn("truncate text-[11.5px] text-muted-foreground", nature === n && "text-foreground")}>{NATURE[n].label}</span>
                </button>
              </TooltipTrigger>
              <TooltipContent>{NATURE[n].label} {count}</TooltipContent>
            </Tooltip>
          ))}
        </div>
      </div>
    </div>
  );
}
