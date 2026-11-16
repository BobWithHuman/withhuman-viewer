// The board: a server is a row, and the columns are what the engine did with its tools' calls.
// A tool is a chip in the column of the strongest verdict it got; one never called sits dashed
// in the last. A chip opens the tool, a server's cell the server.

import { Badge } from "@/components/ui/badge";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { Empty, NATURE, NatureIcon, ServiceLogo, ago, num } from "@/kit";
import { cn } from "@/lib/utils";
import { Held, Reach, Spark } from "./parts";
import { COLUMN, COLUMNS, DAYS, VERDICTS, column, machinesOf, toolKey, type Server, type Tool } from "./types";

export function Board({ servers, lit, filtered, now, picked, pickedServer, onTool, onServer }: {
  servers: Server[];
  /** Whether a tool stays lit under the filters. */
  lit: (server: Server, t: Tool) => boolean;
  filtered: boolean;
  now: number;
  picked: string | null;
  pickedServer: string | null;
  onTool: (server: string, tool: string) => void;
  onServer: (server: string) => void;
}) {
  const all = servers.flatMap((s) => s.tools.map((t) => ({ s, t })));
  const most = Math.max(1, ...all.map((x) => x.t.calls));
  // Under a filter, a server with nothing lit leaves the board.
  const rows = servers.filter((s) => !filtered || s.tools.some((t) => lit(s, t)));
  return (
    <div className="grid grid-cols-[216px_minmax(0,1.9fr)_minmax(0,1fr)_minmax(0,1fr)_minmax(0,0.72fr)] text-[13px]">
      <div className="sticky top-12 z-10 flex h-10 items-center gap-1.5 border-b bg-card px-4 text-[12px] text-muted-foreground">
        서버 <b className="tnum font-medium text-foreground">{filtered ? `${rows.length} / ${servers.length}` : servers.length}</b>
        <span className="mx-0.5 opacity-40">·</span>도구 <b className="tnum font-medium text-foreground">{all.length}</b>
      </div>
      {COLUMNS.map((c) => {
        const here = all.filter((x) => column(x.t) === c);
        return (
          <div key={c} className="sticky top-12 z-10 flex h-10 min-w-0 items-center gap-2 border-b border-l bg-card px-3">
            <i className="size-2 shrink-0 rounded-full" style={{ background: COLUMN[c].color }} />
            <span className="truncate font-semibold">{COLUMN[c].label}</span>
            <span className="tnum shrink-0 text-[12px] text-muted-foreground">
              {filtered ? <><b className="font-medium text-foreground">{here.filter((x) => lit(x.s, x.t)).length}</b> / {here.length}</> : here.length}
            </span>
          </div>
        );
      })}
      {!rows.length && <div className="col-span-5"><Empty /></div>}
      {rows.map((s) => {
        const calls = s.tools.reduce((a, t) => a + t.calls, 0);
        const byDay = Array.from({ length: DAYS }, (_, i) => s.tools.reduce((a, t) => a + (t.days[i] ?? 0), 0));
        return (
          <div key={s.name} className="col-span-5 grid grid-cols-subgrid border-b last:border-b-0">
            <button onClick={() => onServer(s.name)} aria-pressed={pickedServer === s.name}
              className={cn("flex min-w-0 flex-col gap-2 px-4 py-3 text-left transition-colors outline-none hover:bg-white/4 focus-visible:bg-white/6", pickedServer === s.name && "bg-primary/10 hover:bg-primary/10")}>
              <span className="flex w-full min-w-0 items-center gap-2">
                <ServiceLogo name={s.name} size={24} />
                <span className="min-w-0 truncate font-mono text-[12.5px] font-semibold">{s.name}</span>
                <Badge variant="secondary" className="ml-auto h-[18px] shrink-0 border-0 px-1.5 text-[10.5px] font-medium text-muted-foreground">{s.transport === "local" ? "로컬" : "원격"}</Badge>
              </span>
              <Spark days={byDay} className="w-full" />
              <span className="tnum truncate text-[12px] text-muted-foreground">
                설치 단말 <b className="font-medium text-foreground">{machinesOf(s)}</b><span className="mx-1 opacity-40">·</span>호출 <b className={cn("font-medium", calls > 0 && "text-foreground")}>{num(calls)}</b>
              </span>
            </button>
            {COLUMNS.map((c) => (
              <div key={c} className={cn("grid content-start gap-1.5 border-l p-2.5", c === "allow" ? "grid-cols-[repeat(auto-fill,minmax(188px,1fr))]" : "grid-cols-1")}>
                {s.tools.filter((t) => column(t) === c).sort((a, b) => b.calls - a.calls || a.name.localeCompare(b.name)).map((t) => {
                  const key = toolKey(s.name, t.name);
                  const none = c === "none";
                  return (
                    <Tooltip key={t.name}>
                      <TooltipTrigger asChild>
                        <button onClick={() => onTool(s.name, t.name)} aria-pressed={picked === key}
                          className={cn(
                            "@container relative flex h-8 min-w-0 items-center gap-1.5 rounded-lg border px-2 text-left transition-[background-color,border-color,opacity] outline-none focus-visible:border-ring",
                            none ? "border-dashed border-white/15 bg-transparent hover:bg-white/4" : "border-transparent bg-secondary hover:border-white/20 hover:bg-[#3a3a40]",
                            picked === key && "border-primary bg-primary/12 hover:border-primary",
                            filtered && !lit(s, t) && "opacity-30",
                          )}>
                          {t.calls > 0 && <i className="absolute inset-y-0 left-0 rounded-l-[7px] bg-foreground/[0.055]" style={{ width: `${Math.max(4, (t.calls / most) * 100)}%` }} />}
                          <NatureIcon n={t.nature} className="relative size-3.5" />
                          <span className={cn("relative min-w-0 flex-1 truncate font-mono text-[12px]", none && "text-muted-foreground")}>{t.name}</span>
                          <Held t={t} className="relative" />
                          {/* A narrow chip drops the machines so the tool's name keeps its room. */}
                          <Reach clients={t.users.map((u) => u.client)} className="relative hidden @[176px]:inline-flex" />
                          {!none && <span className="tnum relative min-w-5 shrink-0 text-right text-[12px] font-medium">{num(t.calls)}</span>}
                        </button>
                      </TooltipTrigger>
                      <TooltipContent side="top" className="max-w-[320px]">
                        <div className="tnum font-medium">
                          {[NATURE[t.nature].label, ...VERDICTS.filter((v) => t[v] > 0).map((v) => `${COLUMN[v].label} ${num(t[v])}`), none && (t.last ? `마지막 ${ago(t.last, now)}` : "호출 없음")].filter(Boolean).join(" · ")}
                        </div>
                        <div className="opacity-70">{t.description}</div>
                      </TooltipContent>
                    </Tooltip>
                  );
                })}
              </div>
            ))}
          </div>
        );
      })}
    </div>
  );
}
