// What a picked row tells, in the sheet on the right. A machine: what it said at install, its projects and whether each is protected, the MCP servers set
// up on it, how its sessions were let run, and the switch that stops it.

import { BadgeCheck, Laptop, OctagonX, ShieldCheck, ShieldOff } from "lucide-react";
import { useState, type ReactNode } from "react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { SheetDescription, SheetHeader, SheetTitle } from "@/components/ui/sheet";
import { Legend, ServiceLogo, SplitBar, Warn, ago, clock, num } from "@/kit";
import { cn } from "@/lib/utils";
import { AgentMark, Chip, Dash, EngineBadge, PermBadge, StateBadge } from "./marks";
import { isOn } from "./mcp/types";
import { EDR_STATUS, PERM, PERM_KEYS, WHY, agentLabel, type Machine, type View } from "./model";

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

export function DeviceSheet({ m, view, onServer, onStop }: {
  m: Machine; view: View; onServer: (name: string) => void;
  /** Stops the machine, or lets it go again. */
  onStop: (stopped: boolean) => void;
}) {
  // Stopping asks twice: the first press arms it.
  const [armed, setArmed] = useState(false);
  const servers = view.serversOn(m);
  const projects = view.projects.filter((p) => p.device === m.id);
  // Projects a check found with no hooks, and agents the EDR saw running with no session open.
  const shadow = view.shadow.filter((a) => a.device === m.id && a.project);
  const seen = view.shadow.filter((a) => a.device === m.id && !a.project);
  const runs = view.sessions.filter((r) => r.device === m.id);
  const modes = PERM_KEYS.map((k) => ({ key: k, label: PERM[k].label, color: PERM[k].color, value: runs.filter((r) => r.permission_mode === k).length }));
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
          ["EDR", m.edr
            ? <span className="inline-flex items-center gap-1.5"><BadgeCheck className={cn("size-3.5", m.edr.status === "active" ? "text-info" : "text-muted-foreground")} strokeWidth={2} />{EDR_STATUS[m.edr.status ?? ""] ?? m.edr.status ?? "상태 모름"}<span className="font-mono text-[11.5px] text-muted-foreground">에이전트 {m.edr.id}</span></span>
            : view.edrKnown ? <Warn>EDR에 없음</Warn> : null],
          ["시리얼", m.serial && <span className="font-mono text-[12px]">{m.serial}</span>],
          ["OS", m.os],
          ["에이전트", m.agent && <AgentMark agent={m.agent} size={16} label />],
          ["훅 버전", m.hook_version],
          ["등록", m.registered_at ? clock(m.registered_at) : "등록 안 됨"],
          ["처음 들림", clock(m.first_seen)],
          ["마지막 신호", m.last && `${clock(m.last)} · ${ago(m.last, view.now)}`],
        ]} />

        {seen.length > 0 && (
          <Section title="섀도 AI" count={seen.length}>
            <div className="flex flex-col">
              {seen.map((a, i) => (
                <Line key={`e${i}`} className="bg-deny/[0.07]">
                  <ShieldOff className="size-4 shrink-0 text-deny" strokeWidth={1.9} />
                  <AgentMark agent={a.agent} size={16} label />
                  <Chip className="shrink-0 bg-deny/15 text-deny">{WHY[a.why ?? ""] ?? "훅 없음"}</Chip>
                  <When>{clock(a.last_seen)} · {ago(a.last_seen, view.now)}</When>
                </Line>
              ))}
            </div>
          </Section>
        )}

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
      </div>
    </>
  );
}

