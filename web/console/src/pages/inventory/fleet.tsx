// The picture the page opens on: which machine has what. A row is a machine; the columns are the
// agents and the MCP servers found anywhere, and a mark in a cell says that machine has it. An
// agent under the engine's hooks is marked protected, one found running without them is shadow
// AI; a server no tool of which was called from the machine is drawn faint. A machine or a server
// pressed opens its detail. A machine is marked when the EDR knows it; the machines the EDR lists
// that have no hooks come last.

import { BadgeCheck, BadgeX, Laptop, OctagonX, ShieldCheck, ShieldOff } from "lucide-react";
import { Fragment, type ReactNode } from "react";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { Empty, NATURE, NatureIcon, ServiceLogo, ago, type Nature } from "@/kit";
import { cn } from "@/lib/utils";
import { AgentMark } from "./marks";
import { isOn, type Server, type Tool } from "./mcp/types";
import { EDR_STATUS, ENGINE, STATE, agentLabel, type EdrOnly, type Machine, type Shadow, type View } from "./model";

/** A row: a machine the engine knows, or a place shadow AI was found on that it does not. */
type Row = { key: string; name: string; machine?: Machine; shadow: Shadow[]; edr?: EdrOnly };

const AGENT_ORDER = ["claude-code", "codex"];
/** What makes an MCP tool a threat, the worst first: what moves money or accounts, sends data
 * out, or deletes. */
const RISKY: Nature[] = ["money", "send", "delete"];
const worst = (tools: Tool[]) => RISKY.find((n) => tools.some((t) => t.nature === n)) ?? null;
/** A server's tools that are a threat, those most called first. */
const risky = (s: Server) => s.tools.filter((t) => RISKY.includes(t.nature)).sort((a, b) => RISKY.indexOf(a.nature) - RISKY.indexOf(b.nature) || b.calls - a.calls);
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
      {item(<i className="size-2.5 rounded-full ring-2 ring-send" />, "위험 도구 호출")}
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

        {/* The servers' tools that are a threat. */}
        <div className="sticky top-[54px] z-10 flex h-9 items-center border-b bg-card pl-2 text-[11.5px] text-muted-foreground">위험 도구</div>
        <div className="sticky top-[54px] z-10 h-9 border-b border-l bg-card" />
        <div className="sticky top-[54px] z-10 flex h-9 items-center border-b border-l bg-card px-2">
          {servers.map((s) => {
            const bad = risky(s), w = worst(bad);
            if (!w) return <span key={s.name} className="grid w-11 shrink-0 place-items-center"><i className="size-1 rounded-full bg-foreground/12" /></span>;
            const deny = s.tools.reduce((a, t) => a + t.deny, 0);
            return (
              <span key={s.name} className="grid w-11 shrink-0 place-items-center">
                {tip(
                  <button onClick={() => onServer(s.name)} className="tnum inline-flex h-6 items-center gap-0.5 rounded-md px-1 text-[11px] font-semibold transition-colors hover:bg-accent" style={{ color: NATURE[w].color }}>
                    <NatureIcon n={w} className="size-3.5" />{bad.length}
                    {deny > 0 && <i className="ml-0.5 size-1.5 rounded-full bg-deny" />}
                  </button>,
                  <div className="flex flex-col gap-1 py-0.5">
                    <span className="font-medium">{s.name}</span>
                    {bad.slice(0, 8).map((t) => (
                      <span key={t.name} className="tnum flex items-center gap-1.5">
                        <NatureIcon n={t.nature} className="size-3" />
                        <span className="font-mono">{t.name}</span>
                        <span className="opacity-70">{NATURE[t.nature].label} · 호출 {t.calls.toLocaleString()}{t.review ? ` · 확인 ${t.review}` : ""}{t.deny ? ` · 차단 ${t.deny}` : ""}</span>
                      </span>
                    ))}
                    {bad.length > 8 && <span className="opacity-70">+{bad.length - 8}</span>}
                  </div>,
                )}
              </span>
            );
          })}
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
                  // The tools that are a threat this machine called, and the worst of them.
                  const used = risky(s).filter((t) => t.users.some((u) => u.n > 0 && isOn(u.client, m)));
                  const w = worst(used);
                  return (
                    <button key={s.name} onClick={() => onServer(s.name)} className={cn(CELL, "rounded-md transition-colors hover:bg-accent")}>
                      {tip(<span className={cn("rounded-full", !n && "opacity-40 outline outline-1 outline-offset-2 outline-foreground/60 outline-dashed", w && "ring-2 ring-offset-2 ring-offset-card")} style={w ? { ["--tw-ring-color" as string]: NATURE[w].color } : undefined}><ServiceLogo name={s.name} size={20} /></span>,
                        <div className="flex flex-col gap-1 py-0.5">
                          <span>{s.name} · {n ? `호출 ${n.toLocaleString()}` : "호출 없음"} · {r.name}</span>
                          {used.map((t) => (
                            <span key={t.name} className="tnum flex items-center gap-1.5"><NatureIcon n={t.nature} className="size-3" /><span className="font-mono">{t.name}</span><span className="opacity-70">{t.users.filter((u) => isOn(u.client, m)).reduce((a, u) => a + u.n, 0).toLocaleString()}</span></span>
                          ))}
                        </div>)}
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
