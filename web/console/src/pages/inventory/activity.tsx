// What a machine is doing and has done: its calls by day, and the tools, sessions
// and latest calls behind them. A day, a tool or a session pressed narrows the others to it; a
// session or a call opens its record.

import { ArrowUpRight, FilePen, FilePlus2, FileText, Globe, Search, SquareTerminal, Wrench, X, type LucideIcon } from "lucide-react";
import { useState, type ReactNode } from "react";
import { Bar, BarChart, Cell, XAxis } from "recharts";
import { ChartContainer, ChartTooltip, ChartTooltipContent, type ChartConfig } from "@/components/ui/chart";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { Empty, Legend, ServiceLogo, VERDICT, VerdictBadge, ago, clock, num, type Verdict } from "@/kit";
import { cn } from "@/lib/utils";
import { DAY, type Activity, type Machine, type View } from "./model";

const VERDICTS: Verdict[] = ["allow", "review", "deny"];
type Counts = [number, number, number];
const total = (c: Counts) => c[0] + c[1] + c[2];
const add = (c: Counts, cell: Activity["cells"][number]) => { c[0] += cell[3]; c[1] += cell[4]; c[2] += cell[5]; };
const verdictOf = (d: string | null): Verdict => (d === "DENY" ? "deny" : d === "REVIEW" ? "review" : "allow");

/** A tool by its server's logo when it is an MCP tool, else by what it does. */
const BUILTIN: Record<string, LucideIcon> = { Bash: SquareTerminal, Read: FileText, Edit: FilePen, MultiEdit: FilePen, Write: FilePlus2, Grep: Search, Glob: Search, WebFetch: Globe, WebSearch: Globe };
const mcpOf = (tool: string) => /^mcp__(.+?)__(.+)$/.exec(tool);
function ToolMark({ tool, size = 16 }: { tool: string; size?: number }) {
  const mcp = mcpOf(tool);
  if (mcp) return <ServiceLogo name={mcp[1]} size={size} />;
  const Icon = BUILTIN[tool] ?? Wrench;
  return <Icon className="shrink-0 text-muted-foreground" style={{ width: size - 2, height: size - 2 }} strokeWidth={1.8} />;
}
const toolLabel = (tool: string) => mcpOf(tool)?.[2] ?? tool;

/** A chip saying what the lists are narrowed to, with a cross to let it go. */
function Narrow({ children, onClear }: { children: ReactNode; onClear: () => void }) {
  return (
    <span className="inline-flex h-6 max-w-[220px] items-center gap-1.5 rounded-full bg-primary/15 pr-1 pl-2 text-[12px] font-medium text-primary">
      <span className="flex min-w-0 items-center gap-1 truncate">{children}</span>
      <button aria-label="지우기" onClick={onClear} className="grid size-4 shrink-0 place-items-center rounded-full hover:bg-primary/20"><X className="size-3" /></button>
    </span>
  );
}

/** A list of rows under a small title, scrolling past a few. */
function Column({ title, count, children }: { title: string; count: number; children: ReactNode }) {
  return (
    <section className="flex min-w-0 flex-col">
      <h3 className="mb-1 flex shrink-0 items-center gap-1.5 text-[12px] font-medium text-muted-foreground">{title}<span className="tnum rounded-full bg-secondary px-1.5 text-[11px]">{count}</span></h3>
      <div className="scroll -mx-1 max-h-[260px] overflow-y-auto px-1">{count ? children : <Empty />}</div>
    </section>
  );
}

/** Allowed, held and blocked as one bar the length of the count. */
function Split({ c, most }: { c: Counts; most: number }) {
  return (
    <span className="flex h-1.5 min-w-0 flex-1 overflow-hidden rounded-full bg-white/5">
      <span className="flex h-full" style={{ width: `${(total(c) / Math.max(1, most)) * 100}%` }}>
        {VERDICTS.map((v, i) => c[i] > 0 && <i key={v} className="block h-full" style={{ flex: c[i], background: v === "allow" ? "color-mix(in oklab, var(--foreground) 45%, transparent)" : VERDICT[v].color }} />)}
      </span>
    </span>
  );
}
const Held = ({ c }: { c: Counts }) => (
  <span className="tnum flex w-14 shrink-0 justify-end gap-1.5 text-[11px] font-medium">
    {c[1] > 0 && <span className="text-review">{c[1]}</span>}
    {c[2] > 0 && <span className="text-deny">{c[2]}</span>}
  </span>
);

export function DeviceActivity({ m, view, onSession, wide }: { m: Machine; view: View; onSession: (id: string) => void; wide?: boolean }) {
  const a = view.activity[m.id];
  const days = view.days;
  const [day, setDay] = useState<number | null>(null);
  const [tool, setTool] = useState<number | null>(null);
  const [session, setSession] = useState<number | null>(null);

  // Each part counts the calls the other two let through, so its own pick never empties it.
  const cells = a?.cells ?? [];
  const pass = (c: Activity["cells"][number], skip: "day" | "tool" | "session") =>
    (skip === "day" || day === null || c[0] === day) && (skip === "tool" || tool === null || c[1] === tool) && (skip === "session" || session === null || c[2] === session);
  const byDay = Array.from({ length: days }, () => [0, 0, 0] as Counts);
  const byTool = (a?.tools ?? []).map(() => [0, 0, 0] as Counts);
  const bySession = (a?.sessions ?? []).map(() => [0, 0, 0] as Counts);
  const all: Counts = [0, 0, 0];
  for (const c of cells) {
    if (pass(c, "day")) add(byDay[c[0]], c);
    if (pass(c, "tool")) add(byTool[c[1]], c);
    if (pass(c, "session")) add(bySession[c[2]], c);
    if (pass(c, "day") && (day === null || c[0] === day)) add(all, c);
  }

  const start = new Date(view.now); start.setHours(0, 0, 0, 0);
  const dayStart = (i: number) => start.getTime() - (days - 1 - i) * DAY;
  const dayName = (i: number) => (i === days - 1 ? "오늘" : (() => { const d = new Date(dayStart(i)); return `${d.getMonth() + 1}/${d.getDate()}`; })());
  const chart = byDay.map((c, i) => ({ day: dayName(i), allow: c[0], review: c[1], deny: c[2] }));
  const config: ChartConfig = { allow: { label: "허용", color: "color-mix(in oklab, var(--foreground) 40%, transparent)" }, review: { label: "사용자 확인", color: "var(--review)" }, deny: { label: "차단", color: "var(--deny)" } };

  const tools = (a?.tools ?? []).map((name, i) => ({ i, name, c: byTool[i] })).filter((t) => total(t.c)).sort((x, y) => total(y.c) - total(x.c));
  const runs = new Map(view.sessions.map((r) => [r.id, r]));
  const sessions = (a?.sessions ?? []).map((id, i) => ({ i, id, c: bySession[i], r: runs.get(id) })).filter((s) => total(s.c))
    .sort((x, y) => Number(!!y.r && view.going(y.r)) - Number(!!x.r && view.going(x.r)) || (y.r?.last_seen_at ?? 0) - (x.r?.last_seen_at ?? 0));
  const recent = (a?.recent ?? []).filter((c) =>
    (day === null || (c.at != null && c.at >= dayStart(day) && c.at < dayStart(day) + DAY))
    && (tool === null || c.tool === a!.tools[tool]) && (session === null || c.session === a!.sessions[session])).slice(0, 60);
  const titleOf = (id: string) => runs.get(id)?.title ?? id.slice(0, 8);

  const tip = (mark: ReactNode, text: ReactNode) => <Tooltip><TooltipTrigger asChild>{mark}</TooltipTrigger><TooltipContent>{text}</TooltipContent></Tooltip>;

  return (
    <div className="flex flex-col gap-4">
      {/* Its calls by day. */}
      <div className="flex shrink-0 flex-col gap-1 rounded-lg bg-white/[0.03] px-3 pt-2.5 pb-1">
        <div className="flex min-h-6 flex-wrap items-center gap-x-2 gap-y-1 text-[12px]">
          <span className="font-medium text-muted-foreground">호출</span>
          <span className="tnum text-[13px] font-semibold">{num(total(all))}</span>
          {day !== null && <Narrow onClear={() => setDay(null)}>{dayName(day)}</Narrow>}
          {tool !== null && <Narrow onClear={() => setTool(null)}><ToolMark tool={a!.tools[tool]} size={14} />{toolLabel(a!.tools[tool])}</Narrow>}
          {session !== null && <Narrow onClear={() => setSession(null)}>{titleOf(a!.sessions[session])}</Narrow>}
          <span className="ml-auto"><Legend items={[{ label: "허용", color: "color-mix(in oklab, var(--foreground) 45%, transparent)", value: all[0] }, { label: "사용자 확인", color: "var(--review)", value: all[1] }, { label: "차단", color: "var(--deny)", value: all[2] }]} /></span>
        </div>
        <ChartContainer config={config} className="aspect-auto h-[112px] w-full">
          <BarChart data={chart} margin={{ top: 4, right: 4, bottom: 0, left: 4 }} barCategoryGap={3}>
            <XAxis dataKey="day" interval={1} tickLine={false} axisLine={false} tickMargin={4} height={18} tick={{ fontSize: 10.5 }} />
            <ChartTooltip cursor={{ fill: "rgb(255 255 255 / 0.06)" }} content={<ChartTooltipContent />} />
            {(["allow", "review", "deny"] as const).map((k, n) => (
              <Bar key={k} dataKey={k} stackId="calls" fill={`var(--color-${k})`} radius={n === 2 ? [2, 2, 0, 0] : 0} isAnimationActive={false} className="cursor-pointer" onClick={(_, i) => setDay(day === i ? null : i)}>
                {chart.map((_, i) => <Cell key={i} fillOpacity={day === null || day === i ? 1 : 0.25} />)}
              </Bar>
            ))}
          </BarChart>
        </ChartContainer>
      </div>

      {/* What was behind them. */}
      {a && (
        <div className={wide ? "grid grid-cols-3 gap-5" : "flex flex-col gap-4"}>
          <Column title="도구" count={tools.length}>
            {tools.map((t) => (
              <button key={t.name} onClick={() => setTool(tool === t.i ? null : t.i)} aria-pressed={tool === t.i}
                className={cn("flex h-8 w-full items-center gap-2 rounded-md px-1.5 text-left text-[12.5px] transition-[background-color,opacity] hover:bg-accent", tool === t.i && "bg-accent", tool !== null && tool !== t.i && "opacity-45")}>
                <ToolMark tool={t.name} />
                <span className="w-[104px] shrink-0 truncate font-mono text-[12px]" title={t.name}>{toolLabel(t.name)}</span>
                <Split c={t.c} most={total(tools[0].c)} />
                <span className="tnum w-9 shrink-0 text-right font-medium">{num(total(t.c))}</span>
                <Held c={t.c} />
              </button>
            ))}
          </Column>
          <Column title="세션" count={sessions.length}>
            {sessions.map((s) => {
              const live = !!s.r && view.going(s.r);
              return (
                <div key={s.id} className={cn("group flex h-10 w-full items-center gap-2 rounded-md px-1.5 transition-[background-color,opacity] hover:bg-accent", session === s.i && "bg-accent", session !== null && session !== s.i && "opacity-45")}>
                  <button onClick={() => setSession(session === s.i ? null : s.i)} aria-pressed={session === s.i} className="flex min-w-0 flex-1 items-center gap-2 text-left">
                    <i className={cn("size-1.5 shrink-0 rounded-full", live ? "animate-pulse bg-allow" : "bg-white/20")} />
                    <span className="min-w-0 flex-1">
                      <span className={cn("block truncate text-[12.5px]", !s.r?.title && "font-mono text-[12px] text-muted-foreground")}>{titleOf(s.id)}</span>
                      <span className="flex items-center gap-1.5 truncate text-[11px] text-muted-foreground">
                        {s.r?.folder && <span className="truncate">{s.r.folder}</span>}
                        <span className="shrink-0">{live ? "진행 중" : ago(s.r?.last_seen_at ?? null, view.now)}</span>
                        {s.r?.permission_mode === "bypassPermissions" && <span className="shrink-0 text-deny">확인 없이</span>}
                      </span>
                    </span>
                    <span className="tnum shrink-0 text-[12px] font-medium">{num(total(s.c))}</span>
                    <Held c={s.c} />
                  </button>
                  {s.r?.opens && tip(<button aria-label="기록" onClick={() => onSession(s.id)} className="grid size-6 shrink-0 place-items-center rounded text-muted-foreground opacity-0 transition-opacity group-hover:opacity-100 hover:bg-white/10 hover:text-foreground"><ArrowUpRight className="size-3.5" /></button>, "기록")}
                </div>
              );
            })}
          </Column>
          <Column title="최근 호출" count={recent.length}>
            {recent.map((c) => {
              const v = verdictOf(c.decision);
              return (
                <button key={`${c.session}-${c.seq}`} onClick={() => onSession(c.session)}
                  className="flex h-8 w-full items-center gap-2 rounded-md px-1.5 text-left transition-colors hover:bg-accent">
                  <span className="tnum w-10 shrink-0 text-[11.5px] text-muted-foreground">{clock(c.at).slice(-5)}</span>
                  {c.tool && <ToolMark tool={c.tool} size={14} />}
                  <code className="min-w-0 flex-1 truncate font-mono text-[12px]" title={c.target ?? ""}>{c.target ?? (c.tool ? toolLabel(c.tool) : "–")}</code>
                  {v !== "allow" && <VerdictBadge v={v}>{v === "review" ? (c.ran ? "확인 후 실행" : "확인 대기") : undefined}</VerdictBadge>}
                </button>
              );
            })}
          </Column>
        </div>
      )}
    </div>
  );
}
