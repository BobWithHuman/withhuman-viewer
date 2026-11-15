// The Calls worth a look split one way (by machine on the page), each row a bar stacked by
// severity; pressing a row narrows the page to it.

import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { Empty, pct } from "@/kit";
import { cn } from "@/lib/utils";
import { type Call, DIM, type Dim, SEV, SEVS, tally } from "./data";

export function Breakdown({ calls, dim, picked, onPick }: { calls: Call[]; dim: Dim; picked?: string; onPick: (key: string) => void }) {
  const rows = tally(calls, DIM[dim].of);
  const max = rows[0]?.total ?? 1;
  if (!rows.length) return <Empty />;
  return (
    <div className="flex flex-col gap-0.5">
      {rows.map((r) => (
        <Tooltip key={r.key}>
          <TooltipTrigger asChild>
            <button
              onClick={() => onPick(r.key)} aria-pressed={picked === r.key}
              className={cn(
                "group flex flex-col gap-1.5 rounded-lg px-2 py-2 text-left transition-colors hover:bg-accent/70",
                picked === r.key && "bg-accent ring-1 ring-ring", picked !== undefined && picked !== r.key && "opacity-45 hover:opacity-100",
              )}
            >
              <span className="flex w-full items-baseline gap-2">
                <span className={cn("min-w-0 flex-1 truncate text-[13px]", (dim === "machine" || dim === "tool" || dim === "folder") && "font-mono text-[12px]")}>{DIM[dim].name(r.key)}</span>
                {r.crit > 0 && <span className="tnum shrink-0 text-[11.5px] font-medium text-deny">치명 {r.crit}</span>}
                <span className="tnum w-8 shrink-0 text-right text-[13px] font-semibold">{r.total}</span>
                <span className="tnum w-8 shrink-0 text-right text-[11.5px] text-muted-foreground">{pct(r.total, calls.length)}%</span>
              </span>
              <span className="flex h-1.5 w-full overflow-hidden rounded-full bg-white/[0.05]">
                <span className="flex h-full gap-px" style={{ width: `${Math.max(2, (r.total / max) * 100)}%` }}>
                  {SEVS.filter((s) => r[s] > 0).map((s) => <i key={s} className="h-full first:rounded-l-full last:rounded-r-full" style={{ flex: r[s], background: SEV[s].color }} />)}
                </span>
              </span>
            </button>
          </TooltipTrigger>
          <TooltipContent side="left">
            <span className="tnum flex items-center gap-2.5">
              {SEVS.filter((s) => r[s] > 0).map((s) => <span key={s} className="inline-flex items-center gap-1"><i className="size-2 rounded-full" style={{ background: SEV[s].color }} />{SEV[s].label} {r[s]}</span>)}
            </span>
          </TooltipContent>
        </Tooltip>
      ))}
    </div>
  );
}
