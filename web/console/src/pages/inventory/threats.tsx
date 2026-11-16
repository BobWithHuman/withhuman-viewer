// The MCP tools that are a threat. A row is a server, the riskiest first, with its tools as chips
// in the order of their risk (how often the engine stopped their calls), each marked by what it
// does in the engine's groups. A chip's red deepens with its risk, plain where the engine stopped
// nothing and dashed when never called; a row shows its riskiest few and opens to the rest. Hovering tells a tool's calls and machines; pressing
// opens it, and a server's name opens the server. A kind of tool pressed above narrows the chips to
// it, 위험만 to the tools at risk, and the machine picked beside to the tools it called.

import { Laptop } from "lucide-react";
import { useState, type ReactNode } from "react";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { Empty, NATURE, NatureIcon, ServiceLogo, num, type Nature } from "@/kit";
import { cn } from "@/lib/utils";
import { isOn, type Server, type Tool } from "./mcp/types";
import type { Machine } from "./model";

/** What a tool does, the most dangerous first. */
const KINDS: Nature[] = ["transact", "send", "delete", "execute", "access", "delegate", "change", "unknown", "read"];
/** The chips a row shows before it is opened. */
const FEW = 8;

/** A tool's risk, 0 to 100: its calls the engine blocked, and half those it held for the user,
 * over its calls and five more, so a tool called once and stopped once does not top the list. */
export const riskOf = (t: Pick<Tool, "calls" | "deny" | "review">) => Math.round((100 * (t.deny + t.review / 2)) / (t.calls + 5));
/** At risk: what 위험만 keeps. */
const atRisk = (t: Tool) => t.calls > 0 && riskOf(t) >= 10;
/** How red a risk is drawn, 0 to 1: full from 45. */
const heat = (risk: number) => Math.min(1, risk / 45);
/** A chip's or a badge's colours for a risk: a red that deepens with it, none at 0. */
const tint = (risk: number) => {
  const h = heat(risk);
  return h > 0
    ? { background: `color-mix(in oklab, var(--deny) ${Math.round(5 + 27 * h)}%, var(--secondary))`, borderColor: `color-mix(in oklab, var(--deny) ${Math.round(12 + 48 * h)}%, transparent)` }
    : undefined;
};
const ink = (risk: number) => `color-mix(in oklab, var(--deny) ${Math.round(45 + 55 * heat(risk))}%, var(--muted-foreground))`;
const callsBy = (t: Tool, m: Machine) => t.users.reduce((a, u) => a + (isOn(u.client, m) ? u.n : 0), 0);
/** The riskiest first, then what can do the most harm, then the most called. */
const order = (a: Tool, b: Tool) => riskOf(b) - riskOf(a) || Number(a.calls === 0) - Number(b.calls === 0) || KINDS.indexOf(a.nature) - KINDS.indexOf(b.nature) || b.calls - a.calls;

export function ThreatsLegend() {
  const item = (mark: ReactNode, label: string) => <span className="inline-flex items-center gap-1.5">{mark}{label}</span>;
  return (
    <div className="flex items-center gap-3.5 text-[12px] text-muted-foreground">
      <span className="inline-flex items-center gap-1.5">막은 비율
        <i className="h-2.5 w-20 rounded-sm" style={{ background: "linear-gradient(90deg, var(--secondary), color-mix(in oklab, var(--deny) 32%, var(--secondary)))" }} />
      </span>
      {item(<i className="size-2.5 rounded-sm border border-dashed border-foreground/40" />, "호출 없음")}
      {item(<i className="size-1.5 rounded-full bg-primary" />, "고른 기기")}
    </div>
  );
}

export function Threats({ servers, focus, onServer, onTool }: {
  servers: Server[]; focus: Machine | null;
  onServer: (name: string) => void; onTool: (server: string, tool: string) => void;
}) {
  const [kind, setKind] = useState<Nature | null>(null);
  const [mine, setMine] = useState(false);
  const [risky, setRisky] = useState(false);
  const [opened, setOpened] = useState<Set<string>>(new Set());
  if (!servers.length) return <Empty>MCP 호출 없음</Empty>;

  const shown = (t: Tool) => (!kind || t.nature === kind) && (!mine || !focus || callsBy(t, focus) > 0) && (!risky || atRisk(t));
  const rows = servers
    .map((s) => ({ s, risk: Math.max(0, ...s.tools.map(riskOf)), tools: s.tools.filter(shown).sort(order) }))
    .filter((r) => r.tools.length)
    .sort((a, b) => b.risk - a.risk || order(a.tools[0], b.tools[0]) || a.s.name.localeCompare(b.s.name));
  const kinds = KINDS.filter((n) => servers.some((s) => s.tools.some((t) => t.nature === n)));
  const toggle = (name: string) => setOpened((o) => { const n = new Set(o); if (n.has(name)) n.delete(name); else n.add(name); return n; });
  const pill = (on: boolean) => cn("inline-flex h-7 shrink-0 items-center gap-1.5 rounded-full border px-2.5 transition-colors", on ? "border-primary bg-primary/15 text-foreground" : "text-muted-foreground hover:border-white/25 hover:text-foreground");

  return (
    <div className="flex h-full min-h-0 flex-col gap-2">
      {/* What the tools can do, the tools at risk, and the machine picked: each narrows the chips. */}
      <div className="flex shrink-0 flex-wrap items-center gap-1.5 text-[12px]">
        <button aria-pressed={risky} onClick={() => setRisky(!risky)}
          className={cn("inline-flex h-7 shrink-0 items-center rounded-full border px-2.5 font-medium transition-colors", risky ? "border-deny/60 bg-deny/15 text-deny" : "text-muted-foreground hover:border-white/25 hover:text-foreground")}>위험만</button>
        <i className="mx-0.5 h-4 w-px bg-border" />
        {kinds.map((n) => (
          <button key={n} aria-pressed={kind === n} onClick={() => setKind(kind === n ? null : n)} className={pill(kind === n)}>
            <span style={{ color: NATURE[n].color }}><NatureIcon n={n} className="size-3.5" /></span>{NATURE[n].label}
          </button>
        ))}
        {focus && (
          <button aria-pressed={mine} onClick={() => setMine(!mine)} className={cn(pill(mine), "ml-auto max-w-[180px]")}>
            <Laptop className="size-3.5 shrink-0" strokeWidth={1.9} /><span className="truncate">{focus.name}</span>
          </button>
        )}
      </div>

      <div className="scroll -mr-1 min-h-0 flex-1 overflow-y-auto pr-1">
        {!rows.length && <Empty />}
        {rows.map(({ s, risk, tools }) => {
          const machines = new Set(s.installs.map((i) => i.client)).size;
          const calls = s.tools.reduce((a, t) => a + t.calls, 0);
          const open = opened.has(s.name), rest = tools.length - FEW;
          return (
            <div key={s.name} className="grid grid-cols-[minmax(140px,170px)_minmax(0,1fr)] border-b border-white/5">
              <button onClick={() => onServer(s.name)} className="flex min-h-11 items-center gap-2 py-1.5 pr-2 pl-1 text-left transition-colors hover:bg-accent">
                <ServiceLogo name={s.name} size={20} />
                <span className="min-w-0">
                  <span className="flex min-w-0 items-center gap-1.5">
                    <span className="truncate font-mono text-[12.5px] font-medium">{s.name}</span>
                    {risk >= 10 && <span className="tnum shrink-0 rounded border px-1 text-[10.5px] leading-4 font-semibold" style={{ ...tint(risk), color: ink(risk) }}>{risk}</span>}
                  </span>
                  <span className="tnum flex items-center gap-1 text-[11px] text-muted-foreground"><Laptop className="size-3" strokeWidth={1.9} />{machines}<span className="opacity-40">·</span>호출 {num(calls)}</span>
                </span>
              </button>
              <div className="flex min-w-0 flex-wrap content-center items-center gap-1 py-1.5 pl-2">
                {(open ? tools : tools.slice(0, FEW)).map((t) => <Chip key={t.name} t={t} focus={focus} onClick={() => onTool(s.name, t.name)} />)}
                {rest > 0 && (
                  <button onClick={() => toggle(s.name)} className="tnum h-6 rounded-md px-1.5 text-[11.5px] text-muted-foreground transition-colors hover:bg-accent hover:text-foreground">
                    {open ? "접기" : `+${rest}`}
                  </button>
                )}
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
}

/** A tool: what it does, its name, and its risk or calls, in a red that deepens with its risk. */
function Chip({ t, focus, onClick }: { t: Tool; focus: Machine | null; onClick: () => void }) {
  const risk = riskOf(t), mine = focus ? callsBy(t, focus) : 0;
  const machines = new Set(t.users.map((u) => u.client)).size;
  const stopped = t.calls ? Math.round((100 * (t.deny + t.review)) / t.calls) : 0;
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <button onClick={onClick} style={t.calls ? tint(risk) : undefined}
          className={cn("relative inline-flex h-6 max-w-full items-center gap-1.5 rounded-md border px-2 transition-colors hover:border-white/40",
            t.calls === 0 ? "border-dashed border-white/15 text-muted-foreground" : "border-transparent bg-secondary text-foreground/85",
            heat(risk) > 0.55 && "font-semibold text-foreground")}>
          {mine > 0 && <i className="absolute -top-0.5 -left-0.5 size-1.5 rounded-full bg-primary" />}
          <span className="shrink-0" style={{ color: NATURE[t.nature].color }}><NatureIcon n={t.nature} className="size-3" /></span>
          <span className="truncate font-mono text-[11.5px]">{t.name}</span>
          {atRisk(t)
            ? <span className="tnum shrink-0 text-[11px] font-semibold" style={{ color: ink(risk) }}>{risk}</span>
            : t.calls > 0 && <span className="tnum shrink-0 text-[11px] font-normal text-muted-foreground">{num(t.calls)}</span>}
        </button>
      </TooltipTrigger>
      <TooltipContent className="max-w-[300px]">
        <div className="flex flex-col gap-1 py-0.5">
          <span className="font-mono font-medium">{t.name}</span>
          {t.description && <span className="opacity-80">{t.description}</span>}
          <span className="tnum">{NATURE[t.nature].label}{t.calls ? ` · 위험도 ${risk} · 막은 비율 ${stopped}%` : " · 호출 없음"}</span>
          <span className="tnum">호출 {num(t.calls)}{t.review ? ` · 확인 ${t.review}` : ""}{t.deny ? ` · 차단 ${t.deny}` : ""} · 기기 {machines}</span>
          {focus && mine > 0 && <span className="tnum">{focus.name} · {num(mine)}</span>}
        </div>
      </TooltipContent>
    </Tooltip>
  );
}
