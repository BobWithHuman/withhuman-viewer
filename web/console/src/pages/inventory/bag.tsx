// What one machine holds, laid out like a game's inventory: a slot for each agent (protected, and
// the shadow runs seen of it), its projects, each MCP server (in the colour of its riskiest tool)
// and each mode the engine ran its sessions in. A slot names what it holds and how many under it;
// a slot pressed opens what it holds; a shelf with nothing says so. The machine is picked here:
// its name opens a list to search.

import { Check, ChevronDown, ChevronUp, ChevronsUpDown, FolderGit2, History, Laptop, PanelRightOpen, Search, ShieldCheck, ShieldOff } from "lucide-react";
import { useMemo, useState, type ReactNode } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { Empty, ServiceLogo, ago } from "@/kit";
import { cn } from "@/lib/utils";
import { AgentMark } from "./marks";
import { isOn } from "./mcp/types";
import { ENGINE, ENGINE_KEYS, STATE, agentLabel, type Machine, type View } from "./model";
import { ink, riskOf, tint } from "./threats";


/** One slot: what it holds, a mark in its corner, and under it its name and a count. */
function Slot({ children, name, sub, corner, style, faint, tip, onClick }: {
  children: ReactNode; name: string; sub?: ReactNode; corner?: ReactNode; style?: React.CSSProperties; faint?: boolean; tip: ReactNode; onClick?: () => void;
}) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <button onClick={onClick} disabled={!onClick} className="group flex w-[68px] flex-col items-center gap-1 text-left">
          <span className={cn("relative grid size-[52px] place-items-center rounded-lg border bg-background/70 transition-colors group-enabled:group-hover:border-white/30", faint && "border-dashed")} style={style}>
            <span className={cn(faint && "opacity-40")}>{children}</span>
            {corner !== undefined && <span className="absolute -right-1 -bottom-1">{corner}</span>}
          </span>
          <span className="w-full truncate text-center text-[11px] leading-tight text-foreground/85" title={name}>{name}</span>
          {sub !== undefined && <span className="tnum -mt-0.5 w-full truncate text-center text-[10.5px] leading-tight text-muted-foreground">{sub}</span>}
        </button>
      </TooltipTrigger>
      <TooltipContent>{tip}</TooltipContent>
    </Tooltip>
  );
}

/** A kind of thing held; one with nothing says so in a line. */
function Shelf({ title, count, wide, children }: { title: string; count: number; wide?: boolean; children: ReactNode[] }) {
  return (
    <section className={wide ? "col-span-2" : undefined}>
      <h3 className="mb-1.5 flex items-center gap-1.5 text-[11.5px] text-muted-foreground">{title}<b className="tnum font-medium text-foreground/80">{count}</b></h3>
      {children.length ? <div className="flex flex-wrap gap-x-1.5 gap-y-2">{children}</div> : <span className="text-[12px] text-muted-foreground/70">없음</span>}
    </section>
  );
}

/** The machines to pick from, shadow AI first, then stopped, then running: those the engine knows,
 * and those only the EDR lists. */
export const machineOrder = (view: View) => [...view.machines]
  .filter((m) => m.state !== "unregistered" || m.running || m.edrOnly)
  .sort((a, b) => rank(a) - rank(b) || a.name.localeCompare(b.name));
const rank = (m: Machine) => (m.shadow ? 0 : m.stopped ? 1 : m.state === "running" ? 2 : 3);

/** The groups the list of machines is in, in this order. */
const GROUPS: { key: string; label: string; of: (m: Machine) => boolean }[] = [
  { key: "shadow", label: "섀도 AI", of: (m) => m.shadow > 0 },
  { key: "stopped", label: "멈춤", of: (m) => !!m.stopped },
  { key: "running", label: "활동 중", of: (m) => m.state === "running" },
  { key: "edr", label: "미보호", of: (m) => !!m.edrOnly },
  { key: "rest", label: "쉬는 중", of: () => true },
];

const dot = (x: Machine) => <i className={cn("size-1.5 shrink-0 rounded-full", x.state === "running" ? "animate-pulse bg-allow" : x.state === "installed" ? "bg-catch" : x.edrOnly ? "bg-review" : "bg-foreground/35")} />;

/** The machine picked, as a field that opens every machine to pick from, grouped by what wants a
 * look and searched by name, ID or serial (arrows and Enter pick too); beside it the previous and
 * next machine. */
function Picker({ view, id, onPick }: { view: View; id: string | null; onPick: (id: string) => void }) {
  const [open, setOpen] = useState(false);
  const [q, setQ] = useState("");
  const [at, setAt] = useState(0);
  const all = useMemo(() => machineOrder(view), [view]);
  const needle = q.trim().toLowerCase();
  const found = all.filter((m) => !needle || [m.name, m.id, m.serial].some((t) => t?.toLowerCase().includes(needle)));
  // Each machine in its first group, the groups in order: the list as it is drawn and walked.
  const groups = GROUPS.map((g, gi) => ({ ...g, list: found.filter((m) => g.of(m) && GROUPS.findIndex((x) => x.of(m)) === gi) })).filter((g) => g.list.length);
  const flat = groups.flatMap((g) => g.list);
  const m = all.find((x) => x.id === id);
  const index = all.findIndex((x) => x.id === id);
  const pick = (x: Machine) => { onPick(x.id); setOpen(false); };
  const step = (by: number) => all.length && onPick(all[(index + by + all.length) % all.length].id);
  return (
    <div className="flex min-w-0 flex-1 items-center gap-1.5">
      <Popover open={open} onOpenChange={(on) => { setOpen(on); setQ(""); setAt(Math.max(0, flat.findIndex((x) => x.id === id))); }}>
        <PopoverTrigger asChild>
          <button className="flex h-12 min-w-0 flex-1 items-center gap-2.5 rounded-lg border bg-background/60 pr-2.5 pl-2 text-left transition-colors hover:border-white/30 hover:bg-accent aria-expanded:border-primary aria-expanded:bg-accent">
            <span className="grid size-8 shrink-0 place-items-center rounded-md bg-white/8"><Laptop className="size-4" strokeWidth={1.75} /></span>
            <span className="min-w-0 flex-1">
              <span className="block truncate text-[14px] font-semibold">{m ? m.name : "기기 선택"}</span>
              {m && <span className="flex items-center gap-1.5 truncate text-[11.5px] text-muted-foreground">{dot(m)}{STATE[m.state].label}{m.last ? ` · ${ago(m.last, view.now)}` : ""}</span>}
            </span>
            <span className="tnum shrink-0 text-[11.5px] text-muted-foreground">{index + 1}/{all.length}</span>
            <ChevronsUpDown className="size-4 shrink-0 text-muted-foreground" />
          </button>
        </PopoverTrigger>
        <PopoverContent align="start" className="w-[var(--radix-popover-trigger-width)] min-w-[300px] gap-0 p-0">
          <label className="relative block border-b p-2">
            <Search className="pointer-events-none absolute top-1/2 left-4 size-3.5 -translate-y-1/2 text-muted-foreground" />
            <Input autoFocus value={q} placeholder="이름·ID·시리얼"
              onChange={(e) => { setQ(e.target.value); setAt(0); }}
              onKeyDown={(e) => {
                if (e.key === "ArrowDown") { e.preventDefault(); setAt((a) => Math.min(flat.length - 1, a + 1)); }
                if (e.key === "ArrowUp") { e.preventDefault(); setAt((a) => Math.max(0, a - 1)); }
                if (e.key === "Enter" && flat[at]) pick(flat[at]);
              }}
              className="h-9 pl-7 text-[13px] md:text-[13px]" />
          </label>
          <div className="scroll max-h-[340px] overflow-y-auto p-1">
            {!flat.length && <Empty />}
            {groups.map((g) => (
              <div key={g.key} className="pb-1">
                <div className="flex items-center gap-1.5 px-2 pt-1.5 pb-1 text-[11px] font-medium text-muted-foreground">
                  {g.key === "shadow" && <ShieldOff className="size-3 text-deny" strokeWidth={2.2} />}{g.label}<span className="tnum opacity-70">{g.list.length}</span>
                </div>
                {g.list.map((x) => {
                  const i = flat.indexOf(x);
                  return (
                    <button key={x.id} onClick={() => pick(x)} onMouseEnter={() => setAt(i)}
                      className={cn("flex h-10 w-full items-center gap-2.5 rounded-md px-2 text-left text-[12.5px] transition-colors", i === at ? "bg-accent" : "hover:bg-accent")}>
                      {dot(x)}
                      <span className="min-w-0 flex-1">
                        <span className="block truncate font-medium">{x.name}</span>
                        <span className="block truncate text-[11px] text-muted-foreground">{[x.agent && agentLabel(x.agent), x.last && ago(x.last, view.now)].filter(Boolean).join(" · ") || "신호 없음"}</span>
                      </span>
                      {x.shadow > 0 && <span className="tnum inline-flex shrink-0 items-center gap-1 rounded bg-deny/15 px-1.5 text-[11px] leading-[18px] font-medium text-deny"><ShieldOff className="size-3" strokeWidth={2.2} />{x.shadow}</span>}
                      {x.id === id && <Check className="size-3.5 shrink-0 text-primary" />}
                    </button>
                  );
                })}
              </div>
            ))}
          </div>
        </PopoverContent>
      </Popover>
      <div className="flex shrink-0 flex-col gap-0.5">
        <button aria-label="이전 기기" onClick={() => step(-1)} className="grid h-[23px] w-7 place-items-center rounded-md border bg-background/60 text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"><ChevronUp className="size-3.5" /></button>
        <button aria-label="다음 기기" onClick={() => step(1)} className="grid h-[23px] w-7 place-items-center rounded-md border bg-background/60 text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"><ChevronDown className="size-3.5" /></button>
      </div>
    </div>
  );
}

export function Bag({ view, id, onPick, onDevice, onServer, onSessions }: {
  view: View; id: string | null; onPick: (id: string) => void;
  onDevice: (id: string) => void; onServer: (name: string) => void;
  /** Lists the machine's sessions, those the engine ran in one mode when a mode is named. */
  onSessions: (id: string, mode?: string) => void;
}) {
  const m = id ? view.machines.find((x) => x.id === id) : undefined;
  if (!m) return <div className="flex h-full flex-col"><div className="flex shrink-0 pb-3"><Picker view={view} id={null} onPick={onPick} /></div><Empty>기기 선택</Empty></div>;
  const shadow = view.shadow.filter((a) => a.device === m.id);
  const hookless = shadow.filter((a) => a.project);
  const projects = view.projects.filter((p) => p.device === m.id);
  const sessions = view.sessions.filter((r) => r.device === m.id);
  // Each server with the calls of its tools from this machine, and the risk of its riskiest tool it called.
  const servers = view.serversOn(m)
    .map((s) => {
      const used = s.tools.map((t) => ({ t, n: t.users.reduce((b, u) => b + (isOn(u.client, m) ? u.n : 0), 0) })).filter((x) => x.n);
      return { s, calls: used.reduce((a, x) => a + x.n, 0), risk: Math.max(0, ...used.map((x) => riskOf(x.t))) };
    })
    .sort((a, b) => b.risk - a.risk || b.calls - a.calls);
  // Each agent once: protected when the hooks are on it, with the shadow runs seen of it.
  const agents = [...new Set([...(m.agent && m.state !== "unregistered" ? [m.agent] : []), ...shadow.map((a) => a.agent)])]
    .map((a) => ({ a, guarded: a === m.agent && m.state !== "unregistered", runs: shadow.filter((x) => x.agent === a && !x.project).length }));
  const modes = ENGINE_KEYS.map((k) => ({ k, n: sessions.filter((r) => view.sessionCalls[r.id]?.mode === k).length })).filter((x) => x.n);
  const e = m.engine_mode ? ENGINE[m.engine_mode] : undefined;

  return (
    <div className="flex h-full flex-col">
      <div className="flex shrink-0 items-center gap-2 pb-3">
        <Picker view={view} id={m.id} onPick={onPick} />
        {m.edrOnly
          ? <span className="shrink-0 rounded bg-review/15 px-1.5 text-[11px] leading-5 font-medium text-review">미보호</span>
          : <span className="shrink-0 rounded bg-allow/15 px-1.5 text-[11px] leading-5 font-medium text-allow">보호됨</span>}
        {m.stopped && <span className="rounded bg-deny/15 px-1.5 text-[11px] leading-5 font-semibold text-deny">멈춤</span>}
        {e && <Tooltip><TooltipTrigger asChild><e.icon className="size-4 shrink-0" strokeWidth={1.9} style={{ color: e.color }} /></TooltipTrigger><TooltipContent>엔진 {e.label}</TooltipContent></Tooltip>}
        <Tooltip><TooltipTrigger asChild><Button size="icon-sm" variant="secondary" aria-label="상세" onClick={() => onDevice(m.id)}><PanelRightOpen /></Button></TooltipTrigger><TooltipContent>상세 · 기기 멈춤</TooltipContent></Tooltip>
      </div>
      <div className="scroll -mr-2 grid min-h-0 flex-1 grid-cols-2 content-start gap-x-4 gap-y-2.5 overflow-y-auto pr-2">
        <Shelf title="에이전트" count={agents.length}>
          {agents.map(({ a, guarded, runs }) => (
            <Slot key={a} name={agentLabel(a)} onClick={() => onDevice(m.id)}
              sub={[guarded && "보호됨", runs > 0 && `섀도 ${runs}`].filter(Boolean).join(" · ") || "미보호"}
              style={runs ? { borderColor: "color-mix(in oklab, var(--deny) 55%, transparent)" } : undefined}
              corner={guarded
                ? <ShieldCheck className={cn("size-4 rounded-full bg-card p-px", runs ? "text-review" : "text-allow")} strokeWidth={2.2} />
                : <ShieldOff className="size-4 rounded-full bg-card p-px text-deny" strokeWidth={2.2} />}
              tip={<>{agentLabel(a)}{guarded ? ` · 보호됨${m.hook_version ? ` · 훅 ${m.hook_version}` : ""}` : ""}{runs ? ` · 섀도 실행 ${runs}` : ""}</>}>
              <AgentMark agent={a} size={26} />
            </Slot>
          ))}
        </Shelf>
        <Shelf title="프로젝트" count={projects.length + hookless.length}>
          {[
            ...(projects.length ? [<Slot key="p" name="보호" sub={`${projects.length}개`} onClick={() => onDevice(m.id)} tip={<>보호된 프로젝트 {projects.length}</>}><FolderGit2 className="size-6 text-allow" strokeWidth={1.6} /></Slot>] : []),
            ...(hookless.length ? [<Slot key="s" name="미보호" sub={`${hookless.length}개`} style={{ borderColor: "color-mix(in oklab, var(--deny) 55%, transparent)" }} onClick={() => onDevice(m.id)} tip={<>미보호 프로젝트 {hookless.length}</>}><FolderGit2 className="size-6 text-deny" strokeWidth={1.6} /></Slot>] : []),
          ]}
        </Shelf>
        <Shelf title="MCP 서버" count={servers.length} wide>
          {servers.map(({ s, calls, risk }) => (
            <Slot key={s.name} name={s.name} onClick={() => onServer(s.name)} faint={!calls} style={calls ? tint(risk) : undefined}
              sub={calls ? <>호출 {calls.toLocaleString()}{risk >= 10 && <b className="ml-1 font-semibold" style={{ color: ink(risk) }}>{risk}</b>}</> : "안 씀"}
              tip={<>{s.name} · {calls ? `호출 ${calls.toLocaleString()} · 위험도 ${risk}` : "설치만 됨 · 호출 없음"}</>}>
              <ServiceLogo name={s.name} size={26} />
            </Slot>
          ))}
        </Shelf>
        <Shelf title="세션" count={sessions.length} wide>
          {modes.map(({ k, n }) => (
            <Slot key={k} name={`엔진 ${ENGINE[k].label}`} sub={`${n.toLocaleString()}개`} onClick={() => onSessions(m.id, k)}
              tip={<>엔진 {ENGINE[k].label} · 세션 {n.toLocaleString()}</>}>
              <History className="size-6" strokeWidth={1.6} style={{ color: ENGINE[k].color }} />
            </Slot>
          ))}
        </Shelf>
      </div>
    </div>
  );
}
