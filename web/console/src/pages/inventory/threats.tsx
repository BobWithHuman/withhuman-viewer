// The MCP tools that are a threat, across every machine, by how often the engine stopped them.
// On the left every tool called as a dot: across, the share of its calls the engine blocked or held
// for the user; up, how many calls; its size, how many machines; its colour, what it can do. The
// dots up and to the right are the tools most used and most stopped. On the right the tools in the
// order of their risk. A dot and its row light together; either opens the tool; a kind of tool
// pressed narrows both to it, and the machine picked beside rings the tools it called.

import { Laptop } from "lucide-react";
import { useMemo, useState } from "react";
import { CartesianGrid, Cell, Scatter, ScatterChart, Tooltip as ChartTip, XAxis, YAxis, ZAxis } from "recharts";
import { ChartContainer } from "@/components/ui/chart";
import { Empty, NATURE, NatureIcon, ServiceLogo, num, type Nature } from "@/kit";
import { cn } from "@/lib/utils";
import { isOn, type Server, type Tool } from "./mcp/types";
import type { Machine } from "./model";

/** What a tool can do, the most dangerous first. */
const KINDS: Nature[] = ["money", "send", "delete", "write", "unknown", "read"];

/** A tool's risk, 0 to 100: its calls the engine blocked, and half those it held for the user,
 * over its calls and five more, so a tool called once and stopped once does not top the list. */
export const riskOf = (t: Pick<Tool, "calls" | "deny" | "review">) => Math.round((100 * (t.deny + t.review / 2)) / (t.calls + 5));
const LEVEL = (r: number) => (r >= 25 ? { label: "높음", cls: "bg-deny/15 text-deny" } : r >= 10 ? { label: "중간", cls: "bg-review/15 text-review" } : { label: "낮음", cls: "bg-white/8 text-muted-foreground" });

type Row = { key: string; server: string; t: Tool; risk: number; stopped: number; machines: number; mine: number };

export function Threats({ servers, focus, onTool }: {
  servers: Server[]; focus: Machine | null; onTool: (server: string, tool: string) => void;
}) {
  const [kind, setKind] = useState<Nature | null>(null);
  const [mine, setMine] = useState(false);
  const [hover, setHover] = useState<string | null>(null);

  const all = useMemo<Row[]>(() => servers.flatMap((s) => s.tools.filter((t) => t.calls > 0).map((t) => ({
    key: `${s.name}\u0000${t.name}`, server: s.name, t, risk: riskOf(t),
    stopped: Math.round((100 * (t.deny + t.review)) / t.calls),
    machines: new Set(t.users.map((u) => u.client)).size,
    mine: focus ? t.users.reduce((a, u) => a + (isOn(u.client, focus) ? u.n : 0), 0) : 0,
  }))), [servers, focus]);
  if (!all.length) return <Empty>MCP 호출 없음</Empty>;

  const rows = all.filter((r) => (!kind || r.t.nature === kind) && (!mine || r.mine > 0));
  const ranked = [...rows].sort((a, b) => b.risk - a.risk || b.t.deny - a.t.deny || b.t.calls - a.t.calls);
  const kinds = KINDS.filter((n) => all.some((r) => r.t.nature === n));
  const dots = rows.map((r) => ({ ...r, x: r.stopped, y: r.t.calls, z: r.machines }));

  return (
    <div className="flex h-full min-h-0 flex-col gap-2">
      {/* What the tools can do, each narrowing both sides to it; and the machine picked. */}
      <div className="flex shrink-0 flex-wrap items-center gap-1.5 text-[12px]">
        {kinds.map((n) => (
          <button key={n} aria-pressed={kind === n} onClick={() => setKind(kind === n ? null : n)}
            className={cn("inline-flex h-7 items-center gap-1.5 rounded-full border px-2.5 transition-colors", kind === n ? "border-primary bg-primary/15 text-foreground" : "text-muted-foreground hover:border-white/25 hover:text-foreground", kind && kind !== n && "opacity-50")}>
            <i className="size-2 rounded-full" style={{ background: NATURE[n].color }} />{NATURE[n].label}
            <span className="tnum text-[11px] opacity-70">{all.filter((r) => r.t.nature === n).length}</span>
          </button>
        ))}
        {focus && (
          <button aria-pressed={mine} onClick={() => setMine(!mine)}
            className={cn("ml-auto inline-flex h-7 max-w-[200px] items-center gap-1.5 rounded-full border px-2.5 transition-colors", mine ? "border-primary bg-primary/15 text-foreground" : "text-muted-foreground hover:border-white/25 hover:text-foreground")}>
            <Laptop className="size-3.5 shrink-0" strokeWidth={1.9} /><span className="truncate">{focus.name}</span>
          </button>
        )}
      </div>

      <div className="grid min-h-0 flex-1 grid-cols-[minmax(0,1fr)_minmax(0,1fr)] gap-4">
        {/* Every tool called: the share stopped across, the calls up. */}
        <div className="relative min-h-0 rounded-lg bg-white/[0.03]">
          <span className="pointer-events-none absolute top-1.5 right-3 text-[11px] text-deny/80">위험</span>
          <span className="pointer-events-none absolute top-1.5 left-11 text-[10.5px] text-muted-foreground">호출</span>
          <span className="pointer-events-none absolute right-3 bottom-7 text-[10.5px] text-muted-foreground">막은 비율</span>
          <ChartContainer config={{}} className="aspect-auto h-full w-full">
            <ScatterChart margin={{ top: 22, right: 16, bottom: 4, left: 0 }}>
              <CartesianGrid stroke="rgb(255 255 255 / 0.06)" />
              <XAxis type="number" dataKey="x" domain={[0, 100]} ticks={[0, 25, 50, 75, 100]} unit="%" tickLine={false} axisLine={false} tick={{ fontSize: 10.5 }} height={22} />
              <YAxis type="number" dataKey="y" scale="log" domain={[1, (max: number) => Math.max(10, max * 1.6)]} allowDataOverflow tickLine={false} axisLine={false} tick={{ fontSize: 10.5 }} width={38}
                tickFormatter={(v: number) => num(v)} />
              <ZAxis type="number" dataKey="z" range={[36, 320]} />
              <ChartTip cursor={false} content={({ payload }) => {
                const r = payload?.[0]?.payload as (Row & { x: number }) | undefined;
                return r ? <Detail r={r} focus={focus} /> : null;
              }} />
              <Scatter data={dots} isAnimationActive={false} className="cursor-pointer"
                onClick={(d) => { const r = d as unknown as Row; onTool(r.server, r.t.name); }}
                onMouseEnter={(d) => setHover((d as unknown as Row).key)} onMouseLeave={() => setHover(null)}>
                {dots.map((d) => (
                  <Cell key={d.key} fill={NATURE[d.t.nature].color} fillOpacity={hover && hover !== d.key ? 0.25 : 0.8}
                    stroke={d.mine > 0 ? "var(--primary)" : hover === d.key ? "var(--foreground)" : "none"} strokeWidth={2} />
                ))}
              </Scatter>
            </ScatterChart>
          </ChartContainer>
        </div>

        {/* The tools in the order of their risk. */}
        <div className="scroll -mr-1 min-h-0 overflow-y-auto pr-1">
          {!ranked.length && <Empty />}
          {ranked.map((r, i) => {
            const level = LEVEL(r.risk);
            return (
              <button key={r.key} onClick={() => onTool(r.server, r.t.name)} onMouseEnter={() => setHover(r.key)} onMouseLeave={() => setHover(null)}
                className={cn("flex h-10 w-full items-center gap-2 rounded-md px-1.5 text-left transition-colors hover:bg-accent", hover === r.key && "bg-accent", r.mine > 0 && "ring-1 ring-primary ring-inset")}>
                <span className="tnum w-5 shrink-0 text-right text-[11px] text-muted-foreground">{i + 1}</span>
                <ServiceLogo name={r.server} size={18} />
                <span className="min-w-0 flex-1">
                  <span className="flex min-w-0 items-center gap-1.5">
                    <NatureIcon n={r.t.nature} className="size-3 shrink-0" />
                    <span className="truncate font-mono text-[12px]">{r.t.name}</span>
                  </span>
                  {/* The calls blocked, held and let through, as one bar. */}
                  <span className="mt-1 flex h-1.5 overflow-hidden rounded-full bg-white/8">
                    <i className="h-full bg-deny" style={{ width: `${(100 * r.t.deny) / r.t.calls}%` }} />
                    <i className="h-full bg-review" style={{ width: `${(100 * r.t.review) / r.t.calls}%` }} />
                  </span>
                </span>
                <span className="tnum w-10 shrink-0 text-right text-[11.5px] text-muted-foreground">{num(r.t.calls)}</span>
                <span className={cn("tnum inline-flex w-[58px] shrink-0 items-center justify-center gap-1 rounded px-1 text-[11px] leading-5 font-semibold", level.cls)}>{level.label} {r.risk}</span>
              </button>
            );
          })}
        </div>
      </div>
    </div>
  );
}

/** A tool as a dot tells it: what it can do, its calls and how many the engine stopped. */
function Detail({ r, focus }: { r: Row; focus: Machine | null }) {
  return (
    <div className="grid min-w-[200px] gap-1 rounded-lg border bg-popover px-3 py-2 text-[12px] shadow-xl">
      <span className="flex items-center gap-1.5 font-medium"><ServiceLogo name={r.server} size={14} /><span className="font-mono">{r.t.name}</span></span>
      <span className="flex items-center gap-1.5 text-muted-foreground"><NatureIcon n={r.t.nature} className="size-3" />{NATURE[r.t.nature].label} · 기기 {r.machines}</span>
      <span className="tnum">호출 {num(r.t.calls)} · 막은 비율 {r.stopped}%</span>
      <span className="tnum text-muted-foreground">차단 {r.t.deny} · 사용자 확인 {r.t.review} · 위험도 {r.risk}</span>
      {focus && r.mine > 0 && <span className="tnum">{focus.name} · {num(r.mine)}</span>}
    </div>
  );
}
