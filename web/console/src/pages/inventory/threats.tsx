// The MCP tools that are a threat. A row is a server, the riskiest first; the columns are what its
// tools can do, from moving money or accounts to deleting, writing, and what the registry cannot
// tell, with reading only counted. A tool is a chip coloured by its risk: how often the engine
// stopped its calls. Hovering tells its calls and machines; pressing opens it, and a server's name
// opens the server. A column's head narrows the rows to the servers with such tools, 위험만 to the
// tools at risk, and the machine picked beside to the tools it called.

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
/** A tool's risk, 0 to 100: its calls the engine blocked, and half those it held for the user,
 * over its calls and five more, so a tool called once and stopped once does not top the list. */
export const riskOf = (t: Pick<Tool, "calls" | "deny" | "review">) => Math.round((100 * (t.deny + t.review / 2)) / (t.calls + 5));
type Level = "high" | "mid" | "low" | "none";
const levelOf = (t: Tool): Level => (t.calls === 0 ? "none" : riskOf(t) >= 25 ? "high" : riskOf(t) >= 10 ? "mid" : "low");
const LEVEL: Record<Level, { label: string; chip: string; badge: string }> = {
  high: { label: "높음", chip: "border-deny/60 bg-deny/20 text-foreground font-semibold", badge: "bg-deny/20 text-deny" },
  mid: { label: "중간", chip: "border-review/50 bg-review/15 text-foreground", badge: "bg-review/15 text-review" },
  low: { label: "낮음", chip: "border-transparent bg-secondary text-foreground/80", badge: "bg-white/8 text-muted-foreground" },
  none: { label: "호출 없음", chip: "border-dashed border-white/15 text-muted-foreground", badge: "" },
};
const callsBy = (t: Tool, m: Machine) => t.users.reduce((a, u) => a + (isOn(u.client, m) ? u.n : 0), 0);

/** How a server ranks: by its riskiest tool, then its most dangerous tool that was called. */
function score(s: Server) {
  const called = s.tools.filter((t) => t.calls > 0);
  return { risk: Math.max(0, ...called.map(riskOf)), worst: Math.min(...called.map((t) => rank(t.nature)), RISK.length) };
}

export function ThreatsLegend() {
  const item = (mark: ReactNode, label: string) => <span className="inline-flex items-center gap-1.5">{mark}{label}</span>;
  return (
    <div className="flex items-center gap-3.5 text-[12px] text-muted-foreground">
      <span>위험도</span>
      {item(<i className="size-2.5 rounded-sm border border-deny/60 bg-deny/20" />, "높음")}
      {item(<i className="size-2.5 rounded-sm border border-review/50 bg-review/15" />, "중간")}
      {item(<i className="size-2.5 rounded-sm bg-secondary" />, "낮음")}
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
  const [risky, setRisky] = useState(false);
  if (!servers.length) return <Empty>MCP 호출 없음</Empty>;

  // Only what some tool can do has a column.
  const columns = RISK.filter((n) => servers.some((s) => s.tools.some((t) => t.nature === n)));
  const shown = (t: Tool) => (!mine || !focus || callsBy(t, focus) > 0) && (!risky || ["high", "mid"].includes(levelOf(t)));
  const rows = servers
    .map((s) => ({ s, sc: score(s), tools: s.tools.filter(shown) }))
    .filter((r) => r.tools.length && (!only || r.tools.some((t) => t.nature === only)))
    .sort((a, b) => b.sc.risk - a.sc.risk || a.sc.worst - b.sc.worst || a.s.name.localeCompare(b.s.name));
  const count = (n: Nature) => servers.reduce((a, s) => a + s.tools.filter((t) => t.nature === n && shown(t)).length, 0);
  // Reading tools are only counted, unless one is at risk.
  const atRisk = (t: Tool) => levelOf(t) === "high" || levelOf(t) === "mid";
  const riskyReads = servers.some((s) => s.tools.some((t) => t.nature === "read" && atRisk(t)));
  const grid = { gridTemplateColumns: `minmax(140px,170px) ${columns.map(() => "minmax(0,1fr)").join(" ")} ${riskyReads ? "minmax(0,0.8fr)" : "52px"}` };

  return (
    <div className="flex h-full min-h-0 flex-col">
      <div className="scroll min-h-0 flex-1 overflow-y-auto">
        <div className="grid text-[12.5px]" style={grid}>
          {/* What the tools can do, each head narrowing the rows to it. */}
          <div className="sticky top-0 z-10 flex h-9 min-w-0 items-center gap-2 border-b bg-card pl-1 text-[11.5px] text-muted-foreground">
            <span className="shrink-0">서버 {rows.length}</span>
            <button aria-pressed={risky} onClick={() => setRisky(!risky)}
              className={cn("inline-flex h-6 shrink-0 items-center rounded-full border px-2 transition-colors", risky ? "border-deny/60 bg-deny/15 text-deny" : "hover:border-white/25 hover:text-foreground")}>위험만</button>
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

          {rows.map(({ s, sc, tools }) => {
            const machines = new Set(s.installs.map((i) => i.client)).size;
            const calls = s.tools.reduce((a, t) => a + t.calls, 0);
            const reads = tools.filter((t) => t.nature === "read");
            const loud = reads.filter(atRisk).sort((a, b) => riskOf(b) - riskOf(a)), quiet = reads.length - loud.length;
            return [
              <button key={`${s.name}-name`} onClick={() => onServer(s.name)} className="flex min-h-11 items-center gap-2 border-b border-white/5 py-1.5 pr-2 pl-1 text-left transition-colors hover:bg-accent">
                <ServiceLogo name={s.name} size={20} />
                <span className="min-w-0">
                  <span className="flex min-w-0 items-center gap-1.5">
                    <span className="truncate font-mono text-[12.5px] font-medium">{s.name}</span>
                    {sc.risk >= 10 && <span className={cn("tnum shrink-0 rounded px-1 text-[10.5px] leading-4 font-semibold", LEVEL[sc.risk >= 25 ? "high" : "mid"].badge)}>{sc.risk}</span>}
                  </span>
                  <span className="tnum flex items-center gap-1 text-[11px] text-muted-foreground"><Laptop className="size-3" strokeWidth={1.9} />{machines}<span className="opacity-40">·</span>호출 {num(calls)}</span>
                </span>
              </button>,
              ...columns.map((n) => (
                <div key={`${s.name}-${n}`} className={cn("flex min-h-11 flex-wrap content-center gap-1 border-b border-l border-white/5 p-1.5", only && only !== n && "opacity-40")}>
                  {tools.filter((t) => t.nature === n).sort((a, b) => riskOf(b) - riskOf(a) || b.calls - a.calls).map((t) => (
                    <Chip key={t.name} t={t} focus={focus} onClick={() => onTool(s.name, t.name)} />
                  ))}
                </div>
              )),
              <div key={`${s.name}-read`} className={cn("tnum flex min-h-11 flex-wrap content-center items-center gap-1 border-b border-l border-white/5 p-1.5 text-[12px] text-muted-foreground", !loud.length && "justify-center", only && "opacity-40")}>
                {loud.map((t) => <Chip key={t.name} t={t} focus={focus} onClick={() => onTool(s.name, t.name)} />)}
                {quiet > 0 ? <span className="px-1">{loud.length ? `+${quiet}` : quiet}</span> : !loud.length && <i className="size-1 rounded-full bg-foreground/12" />}
              </div>,
            ];
          })}
        </div>
        {!rows.length && <Empty />}
      </div>
    </div>
  );
}

/** A tool: its name and calls, coloured by its risk. */
function Chip({ t, focus, onClick }: { t: Tool; focus: Machine | null; onClick: () => void }) {
  const level = levelOf(t), mine = focus ? callsBy(t, focus) : 0;
  const machines = new Set(t.users.map((u) => u.client)).size;
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <button onClick={onClick}
          className={cn("inline-flex h-6 max-w-full items-center gap-1.5 rounded-md border px-2 transition-colors hover:border-white/40", LEVEL[level].chip, mine > 0 && "ring-1 ring-primary")}>
          <span className="truncate font-mono text-[11.5px]">{t.name}</span>
          {(level === "high" || level === "mid") && <span className={cn("tnum shrink-0 text-[11px] font-semibold", level === "high" ? "text-deny" : "text-review")}>{riskOf(t)}</span>}
          {level === "low" && <span className="tnum shrink-0 text-[11px] text-muted-foreground">{num(t.calls)}</span>}
        </button>
      </TooltipTrigger>
      <TooltipContent className="max-w-[300px]">
        <div className="flex flex-col gap-1 py-0.5">
          <span className="font-mono font-medium">{t.name}</span>
          {t.description && <span className="opacity-80">{t.description}</span>}
          <span className="tnum">위험도 {LEVEL[level].label}{t.calls ? ` ${riskOf(t)}` : ""} · {NATURE[t.nature].label}</span>
          <span className="tnum">호출 {num(t.calls)}{t.review ? ` · 확인 ${t.review}` : ""}{t.deny ? ` · 차단 ${t.deny}` : ""} · 기기 {machines}</span>
          {focus && mine > 0 && <span className="tnum">{focus.name} · {num(mine)}</span>}
        </div>
      </TooltipContent>
    </Tooltip>
  );
}
