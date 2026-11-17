// The riskiest first: the machines, the sessions or the tools (a shell command by its program)
// ranked by the weight of the Calls worth a look on each (a critical one weighs most), each row a
// bar by severity. Pressing a row narrows the page to it.

import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { Empty, ago } from "@/kit";
import { cn } from "@/lib/utils";
import { type Call, DIM, type Group, SEV, SEVS, groups, named, toolName } from "./data";

export type RankBy = "machine" | "session" | "tool";
const NO_PROMPT = "프롬프트 없음";

/** A session as a row of the ranking names it: its name over two lines at most, and under it
 * its machine and when it last had a Call worth a look. */
function SessionName({ r }: { r: Group }) {
  const last = r.calls.reduce((a, c) => Math.max(a, c.at ?? 0), 0);
  return (
    <span className="flex min-w-0 flex-1 flex-col gap-0.5">
      <span className="line-clamp-2 text-[13px] leading-snug break-words" title={r.calls[0].prompt ?? undefined}>{named(r.calls[0]) ?? NO_PROMPT}</span>
      <span className="flex min-w-0 items-center gap-1.5 text-[11px] text-muted-foreground">
        <span className="min-w-0 truncate font-mono">{r.calls[0].machine}</span>
        {last > 0 && <><span className="text-white/20">·</span><span className="tnum shrink-0">{ago(last)}</span></>}
      </span>
    </span>
  );
}

export function Ranking({ calls, by, picked, onPick }: {
  calls: Call[]; by: RankBy; picked?: string; onPick: (key: string) => void;
}) {
  const rows = groups(calls, DIM[by].of).slice(0, 50);
  const max = rows[0]?.risk ?? 1;
  if (!rows.length) return <Empty />;
  return (
    <div className="flex flex-col gap-0.5">
      {rows.map((r, i) => (
        <Tooltip key={r.key}>
          <TooltipTrigger asChild>
            <button
              onClick={() => onPick(r.key)} aria-pressed={picked === r.key}
              className={cn(
                "group grid grid-cols-[18px_minmax(0,1fr)] gap-x-2 gap-y-1.5 rounded-lg px-2 py-2 text-left transition-colors hover:bg-accent/70",
                picked === r.key && "bg-accent ring-1 ring-ring", picked !== undefined && picked !== r.key && "opacity-45 hover:opacity-100",
              )}
            >
              <span className={cn("tnum row-span-2 pt-px text-[12px]", i < 3 ? "font-semibold text-foreground" : "text-muted-foreground")}>{i + 1}</span>
              <span className={cn("flex min-w-0 gap-2", by === "session" ? "items-start" : "items-baseline")}>
                {by === "session"
                  ? <SessionName r={r} />
                  : <span className="min-w-0 flex-1 truncate font-mono text-[12px]">{by === "tool" ? toolName(r.key) || "도구 없음" : r.key}</span>}
                {r.crit > 0 && <span className="tnum shrink-0 text-[11.5px] font-medium text-deny">치명 {r.crit}</span>}
                <span className="tnum shrink-0 text-right text-[13px] font-semibold">{r.total.toLocaleString()}</span>
              </span>
              <span className="flex h-1.5 w-full overflow-hidden rounded-full bg-white/[0.05]">
                <span className="flex h-full gap-px" style={{ width: `${Math.max(3, (r.risk / max) * 100)}%` }}>
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
