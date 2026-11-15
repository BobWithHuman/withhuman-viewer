// What one person holds, laid out like a game's inventory: a slot for each machine, agent, MCP
// server, kind of session and key. A slot's corner tells how many or how it stands; a slot
// pressed opens what it holds. The person is the one picked in the picture beside it.

import { History, KeyRound, Laptop, ShieldCheck, ShieldOff, UserRound } from "lucide-react";
import type { ReactNode } from "react";
import { Button } from "@/components/ui/button";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { Empty, Person, ServiceLogo, ago, shortUser } from "@/kit";
import { cn } from "@/lib/utils";
import { AgentMark } from "./marks";
import { isOn } from "./mcp/types";
import { ENGINE, PERM, PERM_KEYS, STATE, agentLabel, type View } from "./model";


/** One slot: what it holds, its name under it, and a count or a mark in its corner. */
function Slot({ children, name, corner, tone, faint, tip, onClick }: {
  children: ReactNode; name: string; corner?: ReactNode; tone?: string; faint?: boolean; tip: ReactNode; onClick?: () => void;
}) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <button onClick={onClick} disabled={!onClick} className="group flex w-[56px] flex-col items-center gap-1 text-left">
          <span className={cn("relative grid size-[52px] place-items-center rounded-lg border bg-background/70 transition-colors group-enabled:group-hover:border-white/30 group-enabled:group-hover:bg-accent", faint && "border-dashed")}
            style={tone ? { borderColor: `color-mix(in oklab, ${tone} 55%, transparent)` } : undefined}>
            <span className={cn(faint && "opacity-40")}>{children}</span>
            {corner !== undefined && <span className="tnum absolute right-1 bottom-0.5 text-[10.5px] leading-none font-semibold" style={tone ? { color: tone } : undefined}>{corner}</span>}
          </span>
          <span className="w-full truncate text-center text-[10.5px] text-muted-foreground">{name}</span>
        </button>
      </TooltipTrigger>
      <TooltipContent>{tip}</TooltipContent>
    </Tooltip>
  );
}

/** A kind of thing held, in rows of `row` slots; a row not full ends in empty ones. */
function Shelf({ title, count, row, children }: { title: string; count: number; row: number; children: ReactNode[] }) {
  const empty = (row - (children.length % row)) % row || (children.length ? 0 : row);
  return (
    <section className={row > 3 ? "col-span-2" : undefined}>
      <h3 className="mb-1.5 flex items-center gap-1.5 text-[11.5px] text-muted-foreground">{title}<b className="tnum font-medium text-foreground/80">{count}</b></h3>
      <div className="flex flex-wrap gap-x-[10px] gap-y-2">
        {children}
        {Array.from({ length: empty }, (_, i) => <span key={i} className="mb-[18px] size-[52px] rounded-lg border border-white/5 bg-background/30" />)}
      </div>
    </section>
  );
}

export function Bag({ view, who, onDevice, onServer, onPerson, onSessions }: {
  view: View; who: string | null;
  onDevice: (id: string) => void; onServer: (name: string) => void; onPerson: (email: string) => void;
  /** Lists the person's sessions, those let run one way when a mode is named. */
  onSessions: (email: string, mode?: string) => void;
}) {
  if (!who) return <Empty>사용자 선택</Empty>;
  const machines = view.machines.filter((m) => m.user === who);
  const mine = new Set(machines.map((m) => m.id));
  const shadow = view.shadow.filter((a) => a.user === who || (a.device && mine.has(a.device)));
  const person = view.people.find((p) => p.email === who);
  const sessions = view.sessions.filter((r) => r.user === who || (r.device && mine.has(r.device)));
  const keys = view.keys.filter((k) => k.email === who);
  const servers = (view.mcp?.servers ?? [])
    .map((s) => ({ s, on: machines.filter((m) => s.installs.some((i) => isOn(i.client, m))) }))
    .filter((x) => x.on.length)
    .map(({ s, on }) => ({ s, on, calls: s.tools.reduce((a, t) => a + t.users.reduce((b, u) => b + (on.some((m) => isOn(u.client, m)) ? u.n : 0), 0), 0) }))
    .sort((a, b) => b.calls - a.calls);
  // An agent under the hooks on some machine, and the agents found without them.
  const guarded = [...new Set(machines.filter((m) => m.agent && m.state !== "unregistered").map((m) => m.agent!))];
  const wild = [...new Set(shadow.map((a) => a.agent))];
  const modes = PERM_KEYS.map((k) => ({ k, n: sessions.filter((r) => r.permission_mode === k).length })).filter((x) => x.n);
  const live = keys.filter((k) => !k.revoked_at);

  return (
    <div className="flex h-full flex-col">
      <div className="flex shrink-0 items-center gap-3 pb-3">
        <Person name={who} size={36} />
        <div className="min-w-0 flex-1">
          <div className="truncate text-[14px] font-semibold">{shortUser(who)}</div>
          <div className="truncate text-[11.5px] text-muted-foreground">{[machines[0]?.team, who.includes("@") ? who.replace(/^.*@/, "@") : null].filter(Boolean).join(" · ")}</div>
        </div>
        {person?.stopped && <span className="rounded bg-deny/15 px-1.5 text-[11px] leading-5 font-semibold text-deny">멈춤</span>}
        {person && <Button size="sm" variant="secondary" onClick={() => onPerson(who)}><UserRound />상세</Button>}
      </div>
      <div className="scroll -mr-2 grid min-h-0 flex-1 grid-cols-2 content-start gap-x-4 gap-y-2.5 overflow-y-auto pr-2">
        <Shelf title="기기" count={machines.length} row={3}>
          {machines.map((m) => {
            const e = m.engine_mode ? ENGINE[m.engine_mode] : undefined;
            return (
              <Slot key={m.id} name={m.name} onClick={() => onDevice(m.id)} tone={m.state === "running" ? "var(--allow)" : undefined} faint={m.state === "unregistered"}
                corner={e && <e.icon className="size-3" strokeWidth={2.2} style={{ color: e.color }} />}
                tip={<>{m.name} · {STATE[m.state].label}{e ? ` · 엔진 ${e.label}` : ""}{m.last ? ` · ${ago(m.last, view.now)}` : ""}</>}>
                <Laptop className="size-6 text-foreground/80" strokeWidth={1.6} />
              </Slot>
            );
          })}
        </Shelf>
        <Shelf title="에이전트" count={guarded.length + wild.length} row={3}>
          {[
            ...guarded.map((a) => (
              <Slot key={`g${a}`} name={agentLabel(a)} corner={<ShieldCheck className="size-3 text-allow" strokeWidth={2.2} />} tip={<>{agentLabel(a)} · 보호됨 · 기기 {machines.filter((m) => m.agent === a).length}</>}>
                <AgentMark agent={a} size={26} />
              </Slot>
            )),
            ...wild.map((a) => (
              <Slot key={`w${a}`} name={agentLabel(a)} tone="var(--deny)" corner={<ShieldOff className="size-3" strokeWidth={2.2} />} tip={<>섀도 AI · {agentLabel(a)} · {shadow.filter((x) => x.agent === a).length}곳</>}>
                <AgentMark agent={a} size={26} />
              </Slot>
            )),
          ]}
        </Shelf>
        <Shelf title="MCP 서버" count={servers.length} row={6}>
          {servers.map(({ s, on, calls }) => (
            <Slot key={s.name} name={s.name} onClick={() => onServer(s.name)} faint={!calls} corner={calls ? calls.toLocaleString() : undefined}
              tip={<>{s.name} · 기기 {on.length} · {calls ? `호출 ${calls.toLocaleString()}` : "호출 없음"}</>}>
              <ServiceLogo name={s.name} size={26} />
            </Slot>
          ))}
        </Shelf>
        <Shelf title="세션" count={sessions.length} row={3}>
          {modes.map(({ k, n }) => (
            <Slot key={k} name={PERM[k].label} onClick={() => onSessions(who, k)} corner={n.toLocaleString()} tone={k === "bypassPermissions" ? "var(--deny)" : undefined}
              tip={<>{PERM[k].label} · 세션 {n.toLocaleString()}</>}>
              <History className="size-6" strokeWidth={1.6} style={{ color: PERM[k].color }} />
            </Slot>
          ))}
        </Shelf>
        <Shelf title="키" count={keys.length} row={3}>
          {[
            ...(live.length ? [<Slot key="live" name="사용 중" onClick={person ? () => onPerson(who) : undefined} corner={live.length} tip={<>사용 중인 키 {live.length}</>}><KeyRound className="size-6 text-allow" strokeWidth={1.6} /></Slot>] : []),
            ...(keys.length > live.length ? [<Slot key="gone" name="회수됨" faint onClick={person ? () => onPerson(who) : undefined} corner={keys.length - live.length} tip={<>회수된 키 {keys.length - live.length}</>}><KeyRound className="size-6" strokeWidth={1.6} /></Slot>] : []),
          ]}
        </Shelf>
      </div>
    </div>
  );
}
