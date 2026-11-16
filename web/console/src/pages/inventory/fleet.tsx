// The picture the page opens on: which machine has what. A row is a machine; the columns are the
// agents and the MCP servers found anywhere, and a mark in a cell says that machine has it. An
// agent under the engine's hooks is marked protected, one found running without them is shadow
// AI; a server no tool of which was called from the machine is drawn faint. A machine or a server
// pressed opens its detail. A machine is marked when the EDR knows it; the machines the EDR lists
// that have no hooks come last.

import { BadgeCheck, BadgeX, Laptop, OctagonX, ShieldCheck, ShieldOff, Zap } from "lucide-react";
import { Fragment, type ReactNode } from "react";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { Empty, ServiceLogo, ago } from "@/kit";
import { cn } from "@/lib/utils";
import { AgentMark } from "./marks";
import { isOn, type Server } from "./mcp/types";
import { EDR_STATUS, ENGINE, PERM, STATE, agentLabel, type EdrOnly, type Machine, type Shadow, type View } from "./model";

/** A row: a machine the engine knows, or a place shadow AI was found on that it does not. */
type Row = { key: string; name: string; machine?: Machine; shadow: Shadow[]; edr?: EdrOnly };

const AGENT_ORDER = ["claude-code", "codex"];
const CELL = "grid h-10 w-11 shrink-0 place-items-center";
const tip = (mark: ReactNode, text: ReactNode) => <Tooltip><TooltipTrigger asChild>{mark}</TooltipTrigger><TooltipContent>{text}</TooltipContent></Tooltip>;

/** What the marks mean, for the panel's title row. */
export function FleetLegend() {
  const item = (mark: ReactNode, label: string) => <span className="inline-flex items-center gap-1.5">{mark}{label}</span>;
  return (
    <div className="flex items-center gap-3.5 text-[12px] text-muted-foreground">
      {item(<ShieldCheck className="size-3.5 text-allow" strokeWidth={2} />, "보호됨")}
      {item(<ShieldOff className="size-3.5 text-deny" strokeWidth={2} />, "섀도 AI")}
      {item(<BadgeCheck className="size-3.5 text-info" strokeWidth={2} />, "EDR 확인")}
      {item(<i className="size-2.5 rounded-full bg-foreground/70" />, "사용")}
      {item(<i className="size-2.5 rounded-full border border-dashed border-foreground/50" />, "설치만")}
    </div>
  );
}

export function Fleet({ view, machines, servers, picked, onDevice, onServer }: {
  view: View; machines: Machine[]; servers: Server[]; picked: string | null;
  onDevice: (id: string) => void; onServer: (name: string) => void;
}) {
  const shown = new Set(machines.map((m) => m.id));
  const rows: Row[] = machines.map((m) => ({ key: m.id, name: m.name, machine: m, shadow: view.shadow.filter((a) => a.device === m.id) }));
  // Shadow AI found where the engine knows no machine: a row for each place.
  const loose = new Map<string, Row>();
  for (const a of view.shadow) {
    if (a.device && shown.has(a.device)) continue;
    // A machine the EDR lists without hooks has its own row below.
    if (a.edr && view.edrOnly.some((e) => e.id === a.edr)) continue;
    const key = `?${a.device ?? a.host ?? ""}`;
    const row = loose.get(key) ?? { key, name: a.host ?? a.device ?? "기기 모름", shadow: [] };
    row.shadow.push(a);
    loose.set(key, row);
  }
  rows.push(...loose.values());
  // The machines the EDR lists with no hooks on them: no one's yet.
  rows.push(...view.edrOnly.map((e) => ({ key: `edr${e.id}`, name: e.name, shadow: view.shadow.filter((a) => a.edr === e.id), edr: e })));

  // Machines in the order of what needs a look: shadow AI first, then stopped, then running.
  const rank = (r: Row) => (r.shadow.length ? 0 : r.machine?.stopped ? 1 : r.machine?.state === "running" ? 2 : r.edr ? 4 : 3);
  rows.sort((a, b) => rank(a) - rank(b) || a.name.localeCompare(b.name));

  const found = new Set([...machines.map((m) => m.agent), ...view.shadow.map((a) => a.agent), ...view.edrOnly.map((e) => e.agent)].filter((a): a is string => !!a));
  const agents = [...AGENT_ORDER.filter((a) => found.has(a)), ...[...found].filter((a) => !AGENT_ORDER.includes(a)).sort()];
  /** The calls of a server's tools made from a machine over the 30 days. */
  const callsFrom = (s: Server, m: Machine) => s.tools.reduce((a, t) => a + t.users.reduce((b, u) => b + (isOn(u.client, m) ? u.n : 0), 0), 0);

  if (!rows.length) return <Empty />;
  return (
    <div className="scroll h-full overflow-auto">
      <div className="grid w-max min-w-full text-[12.5px]" style={{ gridTemplateColumns: "minmax(220px,1fr) auto auto" }}>
        {/* The columns' names stay at the top. */}
        <div className="sticky top-0 z-10 flex h-[54px] items-end border-b bg-card pb-2 text-[11.5px] text-muted-foreground">
          <span className="pl-2">기기 <b className="tnum font-medium text-foreground/80">{rows.length}</b></span>
        </div>
        <div className="sticky top-0 z-10 flex h-[54px] items-end border-b border-l bg-card px-2 pb-1.5">
          {agents.map((a) => (
            <span key={a} className="flex w-11 shrink-0 flex-col items-center gap-1">
              <AgentMark agent={a} size={18} />
              <span className="w-full truncate text-center text-[10.5px] text-muted-foreground">{agentLabel(a).replace(" Code", "")}</span>
            </span>
          ))}
        </div>
        <div className="sticky top-0 z-10 flex h-[54px] items-end border-b border-l bg-card px-2 pb-1.5">
          {!servers.length && <span className="pb-1 text-[11.5px] text-muted-foreground">MCP 미수집</span>}
          {servers.map((s) => (
            <button key={s.name} onClick={() => onServer(s.name)} className="flex w-11 shrink-0 flex-col items-center gap-1 rounded-md py-0.5 transition-colors hover:bg-accent">
              <ServiceLogo name={s.name} size={18} />
              <span className="w-full truncate px-0.5 text-center font-mono text-[10px] text-muted-foreground">{s.name}</span>
            </button>
          ))}
        </div>

        {rows.map((r) => {
          const m = r.machine;
          const line = "border-b border-white/5";
          return (
            <Fragment key={r.key}>
              <button disabled={!m} onClick={() => m && onDevice(m.id)} aria-pressed={m?.id === picked}
                className={cn("flex h-10 min-w-0 items-center gap-2 rounded-md px-2 text-left transition-colors enabled:hover:bg-accent", line, m?.id === picked && "bg-accent ring-1 ring-ring ring-inset")}>
                <Laptop className="size-4 shrink-0 text-muted-foreground" strokeWidth={1.8} />
                <span className="truncate font-mono text-[12px]">{r.name}</span>
                {m && tip(<i className={cn("size-1.5 shrink-0 rounded-full", m.state === "running" ? "animate-pulse bg-allow" : m.state === "installed" ? "bg-catch" : "bg-foreground/35")} />, <>{STATE[m.state].label}{m.last ? ` · ${ago(m.last, view.now)}` : ""}</>)}
                <span className="ml-auto flex shrink-0 items-center gap-1.5">
                  {m?.stopped && tip(<span className="inline-flex items-center gap-1 rounded bg-deny/15 px-1.5 text-[11px] leading-[18px] font-semibold text-deny"><OctagonX className="size-3" strokeWidth={2.2} />멈춤</span>, <>멈춤 · {m.stopped.by ?? ""} · {ago(m.stopped.at, view.now)}{m.stopped.denied ? ` · 거부 ${m.stopped.denied}` : ""}</>)}
                  {m?.edr && tip(<BadgeCheck className={cn("size-3.5", m.edr.status === "active" ? "text-info" : "text-muted-foreground")} strokeWidth={2} />, <>EDR 확인 · {EDR_STATUS[m.edr.status ?? ""] ?? m.edr.status ?? "상태 모름"} · 에이전트 {m.edr.id}</>)}
                  {m && !m.edr && view.edrKnown && m.state !== "unregistered" && tip(<BadgeX className="size-3.5 text-review" strokeWidth={2} />, "EDR에 없는 기기")}
                  {r.edr && <span className="rounded bg-review/15 px-1.5 text-[11px] leading-[18px] font-medium text-review">훅 없음</span>}
                  {m?.permission_mode === "bypassPermissions" && tip(<Zap className="size-3.5 text-deny" strokeWidth={2} />, PERM.bypassPermissions.label)}
                  {m?.engine_mode && ENGINE[m.engine_mode] && (() => {
                    const e = ENGINE[m.engine_mode!], Icon = e.icon;
                    return tip(<Icon className="size-3.5" strokeWidth={1.9} style={{ color: e.color }} />, `엔진 ${e.label}`);
                  })()}
                  {!m && !r.edr && <span className="rounded bg-white/8 px-1.5 text-[11px] leading-[18px] text-muted-foreground">등록 안 됨</span>}
                </span>
              </button>
              <div className={cn("flex border-l px-2", line)}>
                {agents.map((a) => {
                  const guarded = m?.agent === a && m.state !== "unregistered";
                  const wild = r.shadow.filter((x) => x.agent === a);
                  if (r.edr?.agent === a) return <span key={a} className={CELL}>{tip(<span className="relative rounded-full outline-2 outline-offset-2 outline-deny"><AgentMark agent={a} size={20} /><ShieldOff className="absolute -right-2 -bottom-2 size-3.5 rounded-full bg-card p-px text-deny" strokeWidth={2.2} /></span>, <>섀도 AI · {agentLabel(a)} · EDR이 실행을 봄</>)}</span>;
                  return (
                    <span key={a} className={CELL}>
                      {wild.length > 0
                        ? tip(<span className="relative rounded-full outline-2 outline-offset-2 outline-deny"><AgentMark agent={a} size={20} />
                            <ShieldOff className="absolute -right-2 -bottom-2 size-3.5 rounded-full bg-card p-px text-deny" strokeWidth={2.2} /></span>,
                          <>섀도 AI · {agentLabel(a)} · {wild.length}곳{guarded ? " · 보호된 것도 있음" : ""}</>)
                        : guarded
                          ? tip(<span className="relative"><AgentMark agent={a} size={20} />
                              <ShieldCheck className="absolute -right-2 -bottom-2 size-3.5 rounded-full bg-card p-px text-allow" strokeWidth={2.2} /></span>,
                            <>{agentLabel(a)} · 보호됨{m?.hook_version ? ` · 훅 ${m.hook_version}` : ""}</>)
                          : <i className="size-1 rounded-full bg-foreground/12" />}
                    </span>
                  );
                })}
              </div>
              <div className={cn("flex border-l px-2", line)}>
                {servers.map((s) => {
                  if (!m || !s.installs.some((x) => isOn(x.client, m))) return <span key={s.name} className={CELL}><i className="size-1 rounded-full bg-foreground/12" /></span>;
                  const n = callsFrom(s, m);
                  return (
                    <button key={s.name} onClick={() => onServer(s.name)} className={cn(CELL, "rounded-md transition-colors hover:bg-accent")}>
                      {tip(<span className={cn("rounded-full", !n && "opacity-40 outline outline-1 outline-offset-2 outline-foreground/60 outline-dashed")}><ServiceLogo name={s.name} size={20} /></span>,
                        <>{s.name} · {n ? `호출 ${n.toLocaleString()}` : "호출 없음"} · {r.name}</>)}
                    </button>
                  );
                })}
              </div>
            </Fragment>
          );
        })}
      </div>
    </div>
  );
}
