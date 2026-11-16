// The MCP tools that are a threat. A row is a server, the most dangerous first; the columns are
// what its tools can do, from moving money or accounts to deleting, writing, and what the registry
// cannot tell, with reading only counted. A tool is a chip edged in the strongest verdict its calls
// got. Hovering tells its calls and machines; pressing opens it, and a server's name opens the
// server. A column's head narrows the rows to the servers with such tools, and the machine picked
// beside narrows them to the tools it called.

import { Laptop } from "lucide-react";
import { useState, type ReactNode } from "react";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { Empty, NATURE, NatureIcon, ServiceLogo, num, type Nature } from "@/kit";
import { cn } from "@/lib/utils";
import { isOn, type Server, type Tool } from "./mcp/types";
import type { Machine } from "./model";

/** The columns, the most dangerous first; reading is counted at the end. */
const RISK: Nature[] = ["money", "send", "delete", "write", "unknown"];
const rank = (n: Nature) => (RISK.includes(n) ? RISK.indexOf(n) : RISK.length);
const verdictOf = (t: Tool) => (t.deny > 0 ? "deny" : t.review > 0 ? "review" : t.calls > 0 ? "allow" : "none");
const EDGE: Record<string, string> = { deny: "var(--deny)", review: "var(--review)", allow: "color-mix(in oklab, var(--foreground) 30%, transparent)", none: "transparent" };
const callsBy = (t: Tool, m: Machine) => t.users.reduce((a, u) => a + (isOn(u.client, m) ? u.n : 0), 0);

/** How a server ranks: by its most dangerous tool that was called, then its blocked and held calls. */
function score(s: Server) {
  const called = s.tools.filter((t) => t.calls > 0);
  const worst = Math.min(...called.map((t) => rank(t.nature)), RISK.length);
  const deny = s.tools.reduce((a, t) => a + t.deny, 0), review = s.tools.reduce((a, t) => a + t.review, 0);
  return { worst, deny, review, risky: called.filter((t) => rank(t.nature) < 3).reduce((a, t) => a + t.calls, 0) };
}

export function ThreatsLegend() {
  const item = (mark: ReactNode, label: string) => <span className="inline-flex items-center gap-1.5">{mark}{label}</span>;
  return (
    <div className="flex items-center gap-3.5 text-[12px] text-muted-foreground">
      {item(<i className="h-3 w-1 rounded-full bg-deny" />, "차단")}
      {item(<i className="h-3 w-1 rounded-full bg-review" />, "사용자 확인")}
      {item(<i className="h-3 w-1 rounded-full bg-foreground/30" />, "허용")}
      {item(<i className="size-2.5 rounded-sm border border-dashed border-foreground/40" />, "호출 없음")}
    </div>
  );
}

export function Threats({ servers, focus, onServer, onTool }: {
  servers: Server[]; focus: Machine | null;
  onServer: (name: string) => void; onTool: (server: string, tool: string) => void;
}) {
  const [only, setOnly] = useState<Nature | null>(null);
  const [mine, setMine] = useState(false);
  if (!servers.length) return <Empty>MCP 호출 없음</Empty>;

  // Only what some tool can do has a column.
  const columns = RISK.filter((n) => servers.some((s) => s.tools.some((t) => t.nature === n)));
  const shown = (t: Tool) => !mine || !focus || callsBy(t, focus) > 0;
  const rows = servers
    .map((s) => ({ s, sc: score(s), tools: s.tools.filter(shown) }))
    .filter((r) => r.tools.length && (!only || r.tools.some((t) => t.nature === only)))
    .sort((a, b) => a.sc.worst - b.sc.worst || b.sc.deny - a.sc.deny || b.sc.review - a.sc.review || b.sc.risky - a.sc.risky || a.s.name.localeCompare(b.s.name));
  const count = (n: Nature) => servers.reduce((a, s) => a + s.tools.filter((t) => t.nature === n && shown(t)).length, 0);
  const grid = { gridTemplateColumns: `minmax(140px,170px) ${columns.map(() => "minmax(0,1fr)").join(" ")} 52px` };

  return (
    <div className="flex h-full min-h-0 flex-col">
      <div className="scroll min-h-0 flex-1 overflow-y-auto">
        <div className="grid text-[12.5px]" style={grid}>
          {/* What the tools can do, each head narrowing the rows to it. */}
          <div className="sticky top-0 z-10 flex h-9 min-w-0 items-center gap-2 border-b bg-card pl-1 text-[11.5px] text-muted-foreground">
            <span className="shrink-0">서버 {rows.length}</span>
            {focus && (
              <button aria-pressed={mine} onClick={() => setMine(!mine)} title={focus.name}
                className={cn("inline-flex h-6 min-w-0 items-center gap-1 rounded-full border px-2 transition-colors", mine ? "border-primary bg-primary/15 text-foreground" : "hover:border-white/25 hover:text-foreground")}>
                <Laptop className="size-3 shrink-0" strokeWidth={1.9} /><span className="truncate">{focus.name}</span>
              </button>
            )}
          </div>
          {columns.map((n) => (
            <button key={n} aria-pressed={only === n} onClick={() => setOnly(only === n ? null : n)}
              className={cn("sticky top-0 z-10 flex h-9 min-w-0 items-center gap-1.5 border-b border-l bg-card px-2 text-[12px] font-medium transition-colors hover:bg-accent", only === n && "bg-accent", only && only !== n && "opacity-50")}
              style={{ color: NATURE[n].color }}>
              <NatureIcon n={n} className="size-3.5 shrink-0" /><span className="truncate">{NATURE[n].label}</span>
              <span className="tnum ml-auto text-[11px] font-normal text-muted-foreground">{count(n)}</span>
            </button>
          ))}
          <div className="sticky top-0 z-10 flex h-9 items-center justify-center gap-1 border-b border-l bg-card text-[11.5px] text-muted-foreground"><NatureIcon n="read" className="size-3.5" />읽기</div>

          {rows.map(({ s, tools }) => {
            const machines = new Set(s.installs.map((i) => i.client)).size;
            const calls = s.tools.reduce((a, t) => a + t.calls, 0);
            const reads = tools.filter((t) => t.nature === "read");
            return [
              <button key={`${s.name}-name`} onClick={() => onServer(s.name)} className="flex min-h-11 items-center gap-2 border-b border-white/5 py-1.5 pr-2 pl-1 text-left transition-colors hover:bg-accent">
                <ServiceLogo name={s.name} size={20} />
                <span className="min-w-0">
                  <span className="block truncate font-mono text-[12.5px] font-medium">{s.name}</span>
                  <span className="tnum flex items-center gap-1 text-[11px] text-muted-foreground"><Laptop className="size-3" strokeWidth={1.9} />{machines}<span className="opacity-40">·</span>호출 {num(calls)}</span>
                </span>
              </button>,
              ...columns.map((n) => (
                <div key={`${s.name}-${n}`} className={cn("flex min-h-11 flex-wrap content-center gap-1 border-b border-l border-white/5 p-1.5", only && only !== n && "opacity-40")}>
                  {tools.filter((t) => t.nature === n).sort((a, b) => b.calls - a.calls).map((t) => (
                    <Chip key={t.name} t={t} focus={focus} onClick={() => onTool(s.name, t.name)} />
                  ))}
                </div>
              )),
              <div key={`${s.name}-read`} className="tnum flex min-h-11 items-center justify-center border-b border-l border-white/5 text-[12px] text-muted-foreground">
                {reads.length ? reads.length : <i className="size-1 rounded-full bg-foreground/12" />}
              </div>,
            ];
          })}
        </div>
        {!rows.length && <Empty />}
      </div>
    </div>
  );
}

/** A tool: its name and calls, edged in the strongest verdict its calls got. */
function Chip({ t, focus, onClick }: { t: Tool; focus: Machine | null; onClick: () => void }) {
  const v = verdictOf(t), mine = focus ? callsBy(t, focus) : 0;
  const machines = new Set(t.users.map((u) => u.client)).size;
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <button onClick={onClick}
          className={cn("relative inline-flex h-6 max-w-full items-center gap-1.5 overflow-hidden rounded-md border pr-1.5 pl-2.5 transition-colors hover:border-white/30",
            v === "none" ? "border-dashed border-white/15 text-muted-foreground" : "border-transparent bg-secondary",
            mine > 0 && "ring-1 ring-primary")}>
          <i className="absolute inset-y-0 left-0 w-1" style={{ background: EDGE[v] }} />
          <span className="truncate font-mono text-[11.5px]">{t.name}</span>
          {t.calls > 0 && <span className="tnum shrink-0 text-[11px] text-muted-foreground">{num(t.calls)}</span>}
        </button>
      </TooltipTrigger>
      <TooltipContent className="max-w-[300px]">
        <div className="flex flex-col gap-1 py-0.5">
          <span className="font-mono font-medium">{t.name}</span>
          {t.description && <span className="opacity-80">{t.description}</span>}
          <span className="tnum">{NATURE[t.nature].label} · 호출 {num(t.calls)}{t.review ? ` · 확인 ${t.review}` : ""}{t.deny ? ` · 차단 ${t.deny}` : ""} · 기기 {machines}</span>
          {focus && mine > 0 && <span className="tnum">{focus.name} · {num(mine)}</span>}
        </div>
      </TooltipContent>
    </Tooltip>
  );
}
