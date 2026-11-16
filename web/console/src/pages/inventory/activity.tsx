// What each machine is doing and has done. On the left every machine, with its last two weeks of
// calls as a sparkline; on the right the machine picked: what is set up on it, its calls by day,
// and the tools, sessions and latest calls behind them. A day, a tool or a session pressed narrows
// the others to it; a session or a call opens its record.

import { ArrowUpRight, FilePen, FilePlus2, FileText, FolderGit2, Globe, Laptop, OctagonX, PanelRightOpen, Search, ShieldCheck, ShieldOff, SquareTerminal, Wrench, X, type LucideIcon } from "lucide-react";
import { useMemo, useState, type ReactNode } from "react";
import { Bar, BarChart, Cell, XAxis } from "recharts";
import { Button } from "@/components/ui/button";
import { ChartContainer, ChartTooltip, ChartTooltipContent, type ChartConfig } from "@/components/ui/chart";
import { Input } from "@/components/ui/input";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { Empty, Legend, ServiceLogo, VERDICT, VerdictBadge, ago, clock, num, type Verdict } from "@/kit";
import { cn } from "@/lib/utils";
import { AgentMark, EngineBadge, PermBadge } from "./marks";
import { DAY, agentLabel, type Activity, type Machine, type View } from "./model";

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

/** A machine's calls each day, oldest first. */
const perDay = (a: Activity | undefined, days: number) => {
  const out = Array.from({ length: days }, () => [0, 0, 0] as Counts);
  for (const c of a?.cells ?? []) add(out[c[0]], c);
  return out;
};

/** The machines in the order they want a look: running first, then the busiest, then the latest heard. */
export function machineOrder(view: View) {
  const calls = (m: Machine) => (view.activity[m.id]?.cells ?? []).reduce((n, c) => n + c[3] + c[4] + c[5], 0);
  return view.machines
    .filter((m) => m.state !== "unregistered" || m.running)
    .map((m) => ({ m, n: calls(m) }))
    .sort((a, b) => Number(b.m.running) - Number(a.m.running) || b.n - a.n || (b.m.last ?? 0) - (a.m.last ?? 0))
    .map((x) => x.m);
}

/** Two weeks of calls as small bars, the blocked part of each in red. */
function Spark({ days }: { days: Counts[] }) {
  const most = Math.max(1, ...days.map(total));
  return (
    <span className="flex h-5 w-[70px] shrink-0 items-end gap-px" aria-hidden>
      {days.map((d, i) => {
        const h = total(d) ? Math.max(2, (total(d) / most) * 20) : 1;
        return (
          <i key={i} className="flex w-1 flex-col-reverse overflow-hidden rounded-[1px]" style={{ height: h }}>
            <span className="block bg-foreground/40" style={{ height: `${(d[0] / Math.max(1, total(d))) * 100}%` }} />
            <span className="block bg-review" style={{ height: `${(d[1] / Math.max(1, total(d))) * 100}%` }} />
            <span className="block bg-deny" style={{ height: `${(d[2] / Math.max(1, total(d))) * 100}%` }} />
            {!total(d) && <span className="block h-px bg-foreground/15" />}
          </i>
        );
      })}
    </span>
  );
}

const dot = (m: Machine) => <i className={cn("size-1.5 shrink-0 rounded-full", m.state === "running" ? "animate-pulse bg-allow" : m.state === "installed" ? "bg-catch" : "bg-foreground/35")} />;

export function DeviceList({ view, picked, onPick }: { view: View; picked: string | null; onPick: (id: string) => void }) {
  const [q, setQ] = useState("");
  const all = useMemo(() => machineOrder(view), [view]);
  const needle = q.trim().toLowerCase();
  const found = all.filter((m) => !needle || [m.name, m.id, m.serial].some((t) => t?.toLowerCase().includes(needle)));
  return (
    <div className="flex h-full min-h-0 flex-col">
      <label className="relative mb-2 block shrink-0">
        <Search className="pointer-events-none absolute top-1/2 left-2.5 size-3.5 -translate-y-1/2 text-muted-foreground" />
        <Input value={q} onChange={(e) => setQ(e.target.value)} placeholder="이름·ID·시리얼" className="h-8 pl-8 text-[12.5px] md:text-[12.5px]" />
      </label>
      <div className="scroll -mx-2 min-h-0 flex-1 overflow-y-auto px-2">
        {!found.length && <Empty />}
        {found.map((m) => (
          <button key={m.id} onClick={() => onPick(m.id)} aria-pressed={m.id === picked}
            className={cn("flex h-11 w-full items-center gap-2 rounded-md px-2 text-left text-[12.5px] transition-colors hover:bg-accent", m.id === picked && "bg-accent ring-1 ring-ring ring-inset")}>
            {dot(m)}
            <span className="min-w-0 flex-1">
              <span className="block truncate font-medium">{m.name}</span>
              <span className="flex items-center gap-1 truncate text-[11px] text-muted-foreground">
                {m.agent && <AgentMark agent={m.agent} size={11} />}{m.last ? ago(m.last, view.now) : "신호 없음"}
              </span>
            </span>
            {m.stopped && <OctagonX className="size-3.5 shrink-0 text-deny" strokeWidth={2.2} />}
            {m.shadow > 0 && <span className="tnum inline-flex shrink-0 items-center gap-0.5 text-[11px] font-medium text-deny"><ShieldOff className="size-3" strokeWidth={2.2} />{m.shadow}</span>}
            <Spark days={perDay(view.activity[m.id], view.days)} />
          </button>
        ))}
      </div>
    </div>
  );
}

/** A chip saying what the lists are narrowed to, with a cross to let it go. */
function Narrow({ children, onClear }: { children: ReactNode; onClear: () => void }) {
  return (
    <span className="inline-flex h-6 max-w-[220px] items-center gap-1.5 rounded-full bg-primary/15 pr-1 pl-2 text-[12px] font-medium text-primary">
      <span className="flex min-w-0 items-center gap-1 truncate">{children}</span>
      <button aria-label="지우기" onClick={onClear} className="grid size-4 shrink-0 place-items-center rounded-full hover:bg-primary/20"><X className="size-3" /></button>
    </span>
  );
}

/** A column of rows under a small title. */
function Column({ title, count, children }: { title: string; count: number; children: ReactNode }) {
  return (
    <section className="flex min-h-0 min-w-0 flex-col">
      <h3 className="mb-1.5 flex shrink-0 items-center gap-1.5 px-1 text-[12px] font-medium text-muted-foreground">{title}<span className="tnum rounded-full bg-secondary px-1.5 text-[11px]">{count}</span></h3>
      <div className="scroll -mr-1 min-h-0 flex-1 overflow-y-auto pr-1">{count ? children : <Empty />}</div>
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

export function DeviceActivity({ m, view, onDetail, onSession, onServer }: {
  m: Machine; view: View; onDetail: () => void; onSession: (id: string) => void; onServer: (name: string) => void;
}) {
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

  const servers = view.serversOn(m);
  const shadow = view.shadow.filter((x) => x.device === m.id);
  const projects = view.projects.filter((p) => p.device === m.id);
  const guarded = m.agent && m.state !== "unregistered" ? m.agent : null;
  const wild = [...new Set(shadow.map((x) => x.agent))];
  const tip = (mark: ReactNode, text: ReactNode) => <Tooltip><TooltipTrigger asChild>{mark}</TooltipTrigger><TooltipContent>{text}</TooltipContent></Tooltip>;

  return (
    <div className="flex h-full min-h-0 flex-col gap-3">
      {/* The machine and what is set up on it. */}
      <div className="flex shrink-0 items-center gap-3">
        <span className="grid size-10 shrink-0 place-items-center rounded-lg bg-white/8"><Laptop className="size-5" strokeWidth={1.7} /></span>
        <div className="flex min-w-0 flex-col">
          <span className="flex items-center gap-2 text-[15px] font-semibold">{m.name}{dot(m)}</span>
          <span className="truncate font-mono text-[11px] text-muted-foreground">{m.id}</span>
        </div>
        <div className="ml-3 flex min-w-0 items-center gap-1.5">
          {guarded && tip(<span className="relative"><AgentMark agent={guarded} size={22} /><ShieldCheck className="absolute -right-1.5 -bottom-1.5 size-3.5 rounded-full bg-card p-px text-allow" strokeWidth={2.2} /></span>, <>{agentLabel(guarded)} · 보호됨</>)}
          {wild.map((x) => tip(<span key={x} className="relative rounded-full outline-2 outline-offset-2 outline-deny"><AgentMark agent={x} size={22} /><ShieldOff className="absolute -right-1.5 -bottom-1.5 size-3.5 rounded-full bg-card p-px text-deny" strokeWidth={2.2} /></span>, <>섀도 AI · {agentLabel(x)}</>))}
          {servers.length > 0 && <i className="mx-1 h-5 w-px bg-border" />}
          {servers.slice(0, 8).map((s) => tip(<button key={s.name} onClick={() => onServer(s.name)} className="rounded-full transition-opacity hover:opacity-80"><ServiceLogo name={s.name} size={20} /></button>, s.name))}
          {servers.length > 8 && <span className="tnum text-[11.5px] text-muted-foreground">+{servers.length - 8}</span>}
          {(projects.length > 0 || shadow.length > 0) && <i className="mx-1 h-5 w-px bg-border" />}
          {projects.length > 0 && tip(<span className="tnum inline-flex items-center gap-1 text-[12px] text-allow"><FolderGit2 className="size-3.5" strokeWidth={1.9} />{projects.length}</span>, <>보호된 프로젝트 {projects.length}</>)}
          {shadow.length > 0 && tip(<span className="tnum inline-flex items-center gap-1 text-[12px] text-deny"><FolderGit2 className="size-3.5" strokeWidth={1.9} />{shadow.length}</span>, <>훅 없는 프로젝트 {shadow.length}</>)}
        </div>
        <div className="ml-auto flex shrink-0 items-center gap-2 text-[12.5px]">
          {m.stopped && <span className="inline-flex items-center gap-1 rounded bg-deny/15 px-1.5 text-[11.5px] leading-5 font-semibold text-deny"><OctagonX className="size-3" strokeWidth={2.2} />멈춤</span>}
          <EngineBadge mode={m.engine_mode} />
          {m.permission_mode && <PermBadge mode={m.permission_mode} />}
          <Button size="sm" variant="secondary" onClick={onDetail}><PanelRightOpen />상세</Button>
        </div>
      </div>

      {/* Its calls by day. */}
      <div className="flex shrink-0 flex-col gap-1 rounded-lg bg-white/[0.03] px-3 pt-2.5 pb-1">
        <div className="flex h-6 items-center gap-2 text-[12px]">
          <span className="font-medium text-muted-foreground">호출</span>
          <span className="tnum text-[13px] font-semibold">{num(total(all))}</span>
          {day !== null && <Narrow onClear={() => setDay(null)}>{dayName(day)}</Narrow>}
          {tool !== null && <Narrow onClear={() => setTool(null)}><ToolMark tool={a!.tools[tool]} size={14} />{toolLabel(a!.tools[tool])}</Narrow>}
          {session !== null && <Narrow onClear={() => setSession(null)}>{titleOf(a!.sessions[session])}</Narrow>}
          <span className="ml-auto"><Legend items={[{ label: "허용", color: "color-mix(in oklab, var(--foreground) 45%, transparent)", value: all[0] }, { label: "사용자 확인", color: "var(--review)", value: all[1] }, { label: "차단", color: "var(--deny)", value: all[2] }]} /></span>
        </div>
        <ChartContainer config={config} className="aspect-auto h-[112px] w-full">
          <BarChart data={chart} margin={{ top: 4, right: 4, bottom: 0, left: 4 }} barCategoryGap={3}>
            <XAxis dataKey="day" interval={0} tickLine={false} axisLine={false} tickMargin={4} height={18} tick={{ fontSize: 10.5 }} />
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
      {!a ? <div className="grid min-h-0 flex-1 place-items-center"><Empty>호출 없음</Empty></div> : (
        <div className="grid min-h-0 flex-1 grid-cols-3 gap-4">
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
