// Where the agents read values from: one row for each document, command or tool whose result
// held them, those whose values went out first. A row's bar is its values by how they ended; a
// row pressed narrows the page to what came out of it.

import { FileText, SquareTerminal } from "lucide-react";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { Empty, ServiceLogo } from "@/kit";
import { cn } from "@/lib/utils";
import { STATUS, STATUSES, byStatus, categoryLabel, short, type Source } from "./data";

/** A source's name in two parts: what tells it apart, and where that is. An MCP tool is its
 * tool and its server; a document its file and its folder; a command is itself. */
function parts(name: string): { server?: string; main: string; rest?: string; command: boolean } {
  const mcp = /^mcp__(.+?)__(.+)$/.exec(name);
  if (mcp) return { server: mcp[1], main: mcp[2], rest: mcp[1], command: false };
  if (name.includes(" ")) return { main: name, command: true };
  const file = short(name);
  return { main: file, rest: file === name ? undefined : name.slice(0, name.length - file.length).replace(/[/\\]+$/, ""), command: false };
}

export function SourceBars({ sources, picked, onPick, height }: { sources: Source[]; picked?: string; onPick: (name: string) => void; height: number }) {
  const rows = sources
    .map((s) => ({ s, n: byStatus(s.trails) }))
    .sort((a, b) => b.n.out - a.n.out || b.n.blocked - a.n.blocked || b.s.trails.length - a.s.trails.length);
  const most = Math.max(...rows.map((r) => r.s.trails.length), 1);
  return (
    <div className="scroll -mx-2 overflow-auto" style={{ height }}>
      {!rows.length && <Empty />}
      {rows.map(({ s, n }) => {
        const p = parts(s.name), total = s.trails.length;
        return (
          <Tooltip key={s.name}>
            <TooltipTrigger asChild>
              <button onClick={() => onPick(s.name)} aria-pressed={picked === s.name}
                className={cn("grid h-9 w-full grid-cols-[minmax(0,260px)_minmax(0,1fr)_44px_92px_minmax(0,150px)] items-center gap-4 rounded-md px-2 text-left text-[12.5px] transition-colors hover:bg-accent",
                  picked === s.name && "bg-accent ring-1 ring-ring ring-inset", picked !== undefined && picked !== s.name && "opacity-45")}>
                <span className="flex min-w-0 items-center gap-2">
                  {p.server ? <ServiceLogo name={p.server} size={18} />
                    : p.command ? <SquareTerminal className="size-4 shrink-0 text-muted-foreground" strokeWidth={1.9} />
                      : <FileText className="size-4 shrink-0 text-muted-foreground" strokeWidth={1.9} />}
                  <span className="min-w-12 truncate font-mono text-[12px]">{p.main}</span>
                  {p.rest && <span className="min-w-0 shrink-[1000] truncate text-[11.5px] text-muted-foreground">{p.rest}</span>}
                </span>
                {/* The bar is as long as the source's share of the largest; its worst ends come first. */}
                <span className="flex h-2.5 min-w-0 items-center">
                  <span className="flex h-full gap-px overflow-hidden rounded-full" style={{ width: `${Math.max((total / most) * 100, 3)}%` }}>
                    {STATUSES.filter((k) => n[k] > 0).map((k) => (
                      <i key={k} className="h-full" style={{ flex: `${n[k]} 0 4px`, background: STATUS[k].color, opacity: k === "seen" ? 0.35 : 1 }} />
                    ))}
                  </span>
                </span>
                <b className="tnum text-right font-medium">{total.toLocaleString()}</b>
                <span className="tnum flex items-center gap-2.5 text-[12px]">
                  {(["out", "blocked"] as const).map((k) => (
                    <span key={k} className={cn("inline-flex w-10 items-center gap-1", !n[k] && "opacity-25")} style={{ color: n[k] ? STATUS[k].color : undefined }}>
                      <i className="size-1.5 rounded-full bg-current" />{n[k]}
                    </span>
                  ))}
                </span>
                <span className="flex min-w-0 gap-1 overflow-hidden">
                  {s.categories.map((c) => <span key={c} className="shrink-0 rounded bg-secondary px-1.5 text-[11px] leading-[18px] text-muted-foreground">{categoryLabel(c)}</span>)}
                </span>
              </button>
            </TooltipTrigger>
            <TooltipContent side="bottom" align="start" className="max-w-[520px]">
              <div className="font-mono text-[11.5px] break-all">{s.name}</div>
              <div className="tnum mt-1 flex flex-wrap gap-x-3 text-[11.5px]">
                {STATUSES.filter((k) => n[k] > 0).map((k) => <span key={k}>{STATUS[k].label} {n[k].toLocaleString()}</span>)}
                <span>세션 {s.sessions}</span>
              </div>
            </TooltipContent>
          </Tooltip>
        );
      })}
    </div>
  );
}
