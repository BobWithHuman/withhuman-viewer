// What a picked row tells, in the sheet on the right. A machine: what it said at install, its
// projects and whether each is protected, the MCP servers set up on it, how its sessions were
// let run, its latest sessions, and its person's kill switch. A person: their machines, their keys, each revocable, and the
// record of keys issued, revoked and refused.

import { ArrowUpRight, BadgeCheck, Ban, KeyRound, Laptop, OctagonX, ShieldCheck, ShieldOff, ShieldX } from "lucide-react";
import { useState, type ReactNode } from "react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { SheetDescription, SheetFooter, SheetHeader, SheetTitle } from "@/components/ui/sheet";
import { Legend, Person, ServiceLogo, SplitBar, Warn, ago, clock, num } from "@/kit";
import { cn } from "@/lib/utils";
import type { Ask } from "./dialogs";
import { AgentMark, Chip, Dash, EngineBadge, PermBadge, StateBadge } from "./marks";
import { isOn } from "./mcp/types";
import { EDR_STATUS, EVENT, PERM, PERM_KEYS, agentLabel, type Machine, type PersonRow, type View } from "./model";

/** The owner picker's choice for no one. */
const NO_OWNER = "-";

export function Section({ title, count, aside, children }: { title: string; count?: number; aside?: ReactNode; children: ReactNode }) {
  return (
    <section className="flex flex-col gap-2">
      <h3 className="flex items-center gap-2 text-[12px] font-medium text-muted-foreground">
        {title}{count !== undefined && <span className="tnum rounded-full bg-secondary px-1.5 text-[11px]">{count}</span>}
        {aside && <span className="ml-auto flex items-center gap-2 font-normal">{aside}</span>}
      </h3>
      {children}
    </section>
  );
}

function Facts({ rows }: { rows: [string, ReactNode][] }) {
  return (
    <dl className="grid grid-cols-2 gap-x-4 gap-y-3 rounded-lg bg-white/[0.04] p-3.5">
      {rows.map(([k, v]) => (
        <div key={k} className="flex min-w-0 flex-col gap-0.5">
          <dt className="text-[11.5px] text-muted-foreground">{k}</dt>
          <dd className="tnum min-w-0 truncate text-[13px]">{v || <Dash />}</dd>
        </div>
      ))}
    </dl>
  );
}

export const Line = ({ onClick, children, className }: { onClick?: () => void; children: ReactNode; className?: string }) => (
  <button disabled={!onClick} onClick={onClick}
    className={cn("group flex h-9 w-full min-w-0 items-center gap-2.5 rounded-md px-2.5 text-left text-[13px] transition-colors", onClick && "hover:bg-white/6", className)}>
    {children}
  </button>
);
export const When = ({ children }: { children: ReactNode }) => <span className="tnum ml-auto shrink-0 text-[12px] text-muted-foreground">{children}</span>;

export function DeviceSheet({ m, view, onPerson, onSession, onServer, onAsk, onOwner, onStop }: {
  m: Machine; view: View; onPerson: (email: string) => void; onSession: (id: string) => void; onServer: (name: string) => void; onAsk: (ask: Ask) => void;
  /** Names the machine's owner, or takes the name off. */
  onOwner: (email: string | null) => void;
  /** Stops the machine, or lets it go again. */
  onStop: (stopped: boolean) => void;
}) {
  // Stopping asks twice: the first press arms it.
  const [armed, setArmed] = useState(false);
  const servers = view.serversOn(m);
  const projects = view.projects.filter((p) => p.device === m.id);
  const shadow = view.shadow.filter((a) => a.device === m.id);
  const runs = view.sessions.filter((r) => r.device === m.id);
  const latest = [...runs].sort((a, b) => (b.last_seen_at ?? b.created_at ?? 0) - (a.last_seen_at ?? a.created_at ?? 0)).slice(0, 8);
  const modes = PERM_KEYS.map((k) => ({ key: k, label: PERM[k].label, color: PERM[k].color, value: runs.filter((r) => r.permission_mode === k).length }));
  const person = m.user ? view.people.find((p) => p.email === m.user) : undefined;
  const user = m.user;
  return (
    <>
      <SheetHeader className="gap-2 border-b p-5 pr-12">
        <div className="flex items-center gap-2.5">
          <span className="grid size-9 shrink-0 place-items-center rounded-lg bg-white/8"><Laptop className="size-[18px]" strokeWidth={1.75} /></span>
          <div className="flex min-w-0 flex-col">
            <SheetTitle className="truncate text-[15px] font-semibold">{m.name}</SheetTitle>
            <SheetDescription className="truncate font-mono text-[11.5px]">{m.id}</SheetDescription>
          </div>
        </div>
        <div className="flex flex-wrap items-center gap-x-3 gap-y-1.5 text-[12.5px]">
          <StateBadge state={m.state} />
          <EngineBadge mode={m.engine_mode} />
          {m.permission_mode && <PermBadge mode={m.permission_mode} />}
          {m.shadow > 0 && <Badge variant="secondary" className="gap-1 border-0 bg-deny/15 font-semibold text-deny"><ShieldOff />섀도 AI {m.shadow}</Badge>}
          {m.stopped && <Badge variant="secondary" className="gap-1 border-0 bg-deny/15 font-semibold text-deny"><OctagonX />멈춤{m.stopped.denied ? ` · 거부 ${m.stopped.denied}` : ""}</Badge>}
          {m.state !== "unregistered" && (m.stopped
            ? <Button size="xs" variant="outline" className="ml-auto" onClick={() => onStop(false)}>멈춤 풀기</Button>
            : <Button size="xs" variant="outline" className={cn("ml-auto", armed && "border-deny bg-deny/15 text-deny hover:bg-deny/25 hover:text-deny")} onBlur={() => setArmed(false)}
                onClick={() => { if (armed) { onStop(true); setArmed(false); } else setArmed(true); }}><OctagonX />{armed ? "한 번 더" : "기기 멈춤"}</Button>)}
        </div>
      </SheetHeader>
      <div className="scroll flex min-h-0 flex-1 flex-col gap-5 overflow-y-auto p-5">
        <Facts rows={[
          ["담당자", (
            <Select value={m.owner ?? NO_OWNER} onValueChange={(v) => onOwner(v === NO_OWNER ? null : v)}>
              <SelectTrigger size="sm" className="h-7 w-full max-w-[260px] text-[12.5px]"><SelectValue /></SelectTrigger>
              <SelectContent>
                <SelectItem value={NO_OWNER} className="text-[12.5px] text-muted-foreground">지정 안 함</SelectItem>
                {[...new Set([...view.people.map((p) => p.email), ...(m.owner ? [m.owner] : [])])].sort().map((email) => <SelectItem key={email} value={email} className="text-[12.5px]">{email}</SelectItem>)}
              </SelectContent>
            </Select>
          )],
          ["키 사용자", user && (person
            ? <button className="max-w-full rounded hover:underline" onClick={() => onPerson(user)}><Person name={user} size={18} label /></button>
            : <Person name={user} size={18} label />)],
          ...(m.owner && user && m.owner !== user ? [["", <Warn>담당자와 키 사용자가 다름</Warn>] as [string, ReactNode]] : []),
          ["EDR", m.edr
            ? <span className="inline-flex items-center gap-1.5"><BadgeCheck className={cn("size-3.5", m.edr.status === "active" ? "text-info" : "text-muted-foreground")} strokeWidth={2} />{EDR_STATUS[m.edr.status ?? ""] ?? m.edr.status ?? "상태 모름"}<span className="font-mono text-[11.5px] text-muted-foreground">에이전트 {m.edr.id}</span></span>
            : view.edrKnown ? <Warn>EDR에 없음</Warn> : null],
          ["시리얼", m.serial && <span className="font-mono text-[12px]">{m.serial}</span>],
          ["팀", m.team],
          ["OS · 계정", [m.os, m.os_user].filter(Boolean).join(" · ")],
          ["에이전트", m.agent && <AgentMark agent={m.agent} size={16} label />],
          ["훅 버전", m.hook_version],
          ["등록", m.registered_at ? clock(m.registered_at) : "등록 안 됨"],
          ["처음 들림", clock(m.first_seen)],
          ["마지막 신호", m.last && `${clock(m.last)} · ${ago(m.last, view.now)}`],
        ]} />

        <Section title="프로젝트" count={projects.length + shadow.length}
          aside={<Legend items={[{ label: "보호", color: "var(--allow)", value: projects.length }, { label: "섀도", color: "var(--deny)", value: shadow.length }]} />}>
          {projects.length + shadow.length === 0 && <span className="px-2.5 text-[12.5px] text-muted-foreground">없음</span>}
          <div className="flex flex-col">
            {shadow.map((a, i) => (
              <Line key={`s${i}`} className="bg-deny/[0.07]">
                <ShieldOff className="size-4 shrink-0 text-deny" strokeWidth={1.9} />
                <code className="min-w-0 truncate font-mono text-[12px]" title={a.project ?? ""}>{a.project ?? "–"}</code>
                <When>{agentLabel(a.agent)} · {ago(a.last_seen, view.now)}</When>
              </Line>
            ))}
            {projects.map((p) => (
              <Line key={p.project}>
                <ShieldCheck className="size-4 shrink-0 text-allow" strokeWidth={1.9} />
                <code className="min-w-0 truncate font-mono text-[12px]" title={p.project}>{p.project}</code>
                <When>{clock(p.installed_at)}</When>
              </Line>
            ))}
          </div>
        </Section>

        {view.mcp && (
          <Section title="MCP 서버" count={servers.length}>
            {servers.length === 0 && <span className="px-2.5 text-[12.5px] text-muted-foreground">없음</span>}
            <div className="flex flex-col">
              {servers.map((s) => {
                const calls = s.tools.reduce((a, t) => a + (t.users.find((u) => isOn(u.client, m))?.n ?? 0), 0);
                return (
                  <Line key={s.name} onClick={() => onServer(s.name)}>
                    <ServiceLogo name={s.name} size={18} />
                    <span className="min-w-0 truncate font-mono text-[12px]">{s.name}</span>
                    <Chip className="shrink-0">{s.transport === "local" ? "로컬" : "원격"}</Chip>
                    <When>도구 {s.tools.length} · 호출 {num(calls)}</When>
                  </Line>
                );
              })}
            </div>
          </Section>
        )}

        <Section title="권한 모드" count={runs.length}>
          {runs.length === 0 ? <span className="px-2.5 text-[12.5px] text-muted-foreground">없음</span> : (
            <div className="flex flex-col gap-2 px-0.5">
              <SplitBar parts={modes} />
              <Legend items={modes.filter((x) => x.value > 0)} />
            </div>
          )}
        </Section>

        <Section title="최근 세션" count={latest.length}>
          {latest.length === 0 && <span className="px-2.5 text-[12.5px] text-muted-foreground">없음</span>}
          <div className="flex flex-col">
            {latest.map((r) => (
              <Line key={r.id} onClick={r.opens ? () => onSession(r.id) : undefined}>
                <i className={cn("size-2 shrink-0 rounded-full", view.going(r) ? "animate-pulse bg-allow" : "bg-white/20")} />
                <span className={cn("min-w-0 truncate", !r.title && "font-mono text-[12px] text-muted-foreground")}>{r.title ?? r.id.slice(0, 8)}</span>
                {r.permission_mode === "bypassPermissions" && <Badge variant="secondary" className="shrink-0 border-0 bg-deny/15 font-semibold text-deny">확인 없이</Badge>}
                <When>{ago(r.last_seen_at ?? r.created_at, view.now)}</When>
                {r.opens && <ArrowUpRight className="size-3.5 shrink-0 text-muted-foreground opacity-0 transition-opacity group-hover:opacity-100" />}
              </Line>
            ))}
          </div>
        </Section>
      </div>
      {user && (
        <SheetFooter className="flex-row items-center gap-3 border-t p-4">
          <div className="flex min-w-0 flex-col gap-0.5">
            <span className="text-[11.5px] text-muted-foreground">킬 스위치</span>
            <span className="truncate text-[13px]">{user}</span>
          </div>
          {person?.stopped
            ? <Badge variant="secondary" className="ml-auto border-0 bg-deny/15 font-semibold text-deny">멈춤</Badge>
            : <Button variant="destructive" className="ml-auto" onClick={() => onAsk({ kind: "stop", email: user, keys: person?.active_keys ?? 0 })}><OctagonX />사용자 멈추기</Button>}
        </SheetFooter>
      )}
    </>
  );
}

const EVENT_ICON = { issued: KeyRound, revoked: Ban, denied: ShieldX } as const;

export function PersonSheet({ p, view, onDevice, onAsk }: {
  p: PersonRow; view: View; onDevice: (id: string) => void; onAsk: (ask: Ask) => void;
}) {
  const keys = view.keys.filter((k) => k.email === p.email).sort((a, b) => Number(!!a.revoked_at) - Number(!!b.revoked_at) || (b.created_at ?? 0) - (a.created_at ?? 0));
  const events = view.events.filter((e) => e.email === p.email).slice(0, 12);
  const machines = p.devices.map((id) => view.machines.find((m) => m.id === id) ?? id);
  return (
    <>
      <SheetHeader className="gap-2 border-b p-5 pr-12">
        <div className="flex items-center gap-2.5">
          <Person name={p.email} size={36} />
          <div className="flex min-w-0 flex-col">
            <SheetTitle className="truncate text-[15px] font-semibold">{p.email}</SheetTitle>
            <SheetDescription className="text-[12px]">{p.kind === "service" ? "서비스 계정" : "사람"}</SheetDescription>
          </div>
        </div>
        <div className="flex flex-wrap items-center gap-1.5">
          {p.stopped
            ? <Badge variant="secondary" className="border-0 bg-deny/15 font-semibold text-deny">멈춤</Badge>
            : <Badge variant="secondary" className="border-0 bg-allow/15 font-medium text-allow">활성 키 {p.active_keys}</Badge>}
          {p.revoked_keys > 0 && <Chip>회수 {p.revoked_keys}</Chip>}
          {p.devices.length > 3 && <Warn>기기 많음</Warn>}
        </div>
      </SheetHeader>
      <div className="scroll flex min-h-0 flex-1 flex-col gap-5 overflow-y-auto p-5">
        <Facts rows={[
          ["기기", `${p.devices.length}대`],
          ["세션", `${p.sessions}개`],
          ["마지막 사용", p.last_used && `${clock(p.last_used)} · ${ago(p.last_used, view.now)}`],
          ["회수 뒤 거부", <span className={cn(p.denied > 0 && "text-deny")}>{p.denied}건</span>],
        ]} />

        <Section title="기기" count={machines.length}>
          {machines.length === 0 && <span className="px-2.5 text-[12.5px] text-muted-foreground">없음</span>}
          <div className="flex flex-col">
            {machines.map((m) => typeof m === "string" ? (
              <Line key={m}><Laptop className="size-4 shrink-0 text-muted-foreground" strokeWidth={1.75} /><code className="truncate font-mono text-[12px]">{m}</code></Line>
            ) : (
              <Line key={m.id} onClick={() => onDevice(m.id)}>
                <Laptop className="size-4 shrink-0 text-muted-foreground" strokeWidth={1.75} />
                <span className="min-w-0 truncate">{m.name}</span>
                <StateBadge state={m.state} />
                {m.shadow > 0 && <Badge variant="secondary" className="shrink-0 border-0 bg-deny/15 font-semibold text-deny">섀도 {m.shadow}</Badge>}
                <When>{m.last ? ago(m.last, view.now) : "–"}</When>
              </Line>
            ))}
          </div>
        </Section>

        <Section title="키" count={keys.length}>
          {keys.length === 0 && <span className="px-2.5 text-[12.5px] text-muted-foreground">없음</span>}
          <div className="flex flex-col gap-2">
            {keys.map((k) => (
              <div key={k.id} className={cn("flex items-start gap-3 rounded-lg border px-3 py-2.5", k.revoked_at && "border-transparent bg-white/[0.04]")}>
                <KeyRound className={cn("mt-0.5 size-4 shrink-0", k.revoked_at ? "text-muted-foreground/60" : "text-allow")} strokeWidth={1.9} />
                <div className="flex min-w-0 flex-1 flex-col gap-1 text-[12px] text-muted-foreground">
                  <code className={cn("truncate font-mono text-[12.5px] text-foreground", k.revoked_at && "text-muted-foreground line-through")}>{k.id}</code>
                  <span className="tnum truncate">발급 {clock(k.created_at)}{k.created_by && ` · ${k.created_by}`}{k.note && ` · ${k.note}`}</span>
                  {k.revoked_at ? (
                    <>
                      <span className="tnum truncate text-deny" title={k.revoke_reason ?? ""}>회수 {clock(k.revoked_at)}{k.revoked_by && ` · ${k.revoked_by}`}{k.revoke_reason && ` · ${k.revoke_reason}`}</span>
                      {k.first_denied_at && <span className="tnum truncate">첫 거부 {clock(k.first_denied_at)} · 거부 {k.denied_count ?? 0}건</span>}
                    </>
                  ) : <span className="tnum truncate">마지막 사용 {k.last_used_at ? ago(k.last_used_at, view.now) : "–"}</span>}
                </div>
                {!k.revoked_at && <Button variant="destructive" size="sm" onClick={() => onAsk({ kind: "revoke", key_id: k.id, email: k.email })}>회수</Button>}
              </div>
            ))}
          </div>
        </Section>

        {events.length > 0 && (
          <Section title="기록" count={events.length}>
            <div className="flex flex-col">
              {events.map((e, i) => {
                const Icon = EVENT_ICON[e.kind as keyof typeof EVENT_ICON] ?? KeyRound;
                return (
                  <div key={i} className="flex h-8 min-w-0 items-center gap-2.5 px-2.5 text-[12.5px]">
                    <Icon className={cn("size-3.5 shrink-0", e.kind === "issued" ? "text-allow" : "text-deny")} strokeWidth={1.9} />
                    <span className="shrink-0">{EVENT[e.kind] ?? "기록"}</span>
                    <span className="min-w-0 truncate text-muted-foreground" title={e.reason ?? ""}>{[e.actor ?? "엔진", e.reason].filter(Boolean).join(" · ")}</span>
                    <When>{clock(e.at)}</When>
                  </div>
                );
              })}
            </div>
          </Section>
        )}
      </div>
      <SheetFooter className="flex-row justify-end gap-2 border-t p-4">
        <Button variant="secondary" onClick={() => onAsk({ kind: "issue", email: p.email })}><KeyRound />키 발급</Button>
        {p.active_keys > 0 && <Button variant="destructive" onClick={() => onAsk({ kind: "stop", email: p.email, keys: p.active_keys })}><OctagonX />사용자 멈추기</Button>}
      </SheetFooter>
    </>
  );
}
