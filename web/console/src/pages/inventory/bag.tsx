// What one machine holds and how it stands, all in view at once: four figures (calls stopped,
// sensitive values sent out, what waits for the team, shadow AI runs), its agents and MCP servers
// as small marks in one line, and its sessions by engine mode. Each opens where it is handled.
// The machine is picked here: its name opens a list to search.

import { Check, ChevronDown, ChevronUp, ChevronsUpDown, Inbox, KeyRound, Laptop, Search, ShieldCheck, ShieldOff, ShieldX, type LucideIcon } from "lucide-react";
import { useMemo, useState, type ReactNode } from "react";
import { Input } from "@/components/ui/input";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { Empty, ServiceLogo, ago } from "@/kit";
import { cn } from "@/lib/utils";
import { AgentMark } from "./marks";
import { isOn } from "./mcp/types";
import { STATE, agentLabel, type Machine, type View } from "./model";
import { KillSwitch } from "./kill";
import { riskOf, tint } from "./threats";


/** The machines to pick from, shadow AI first, then stopped, then running: those the engine knows,
 * and those only the EDR lists. */
export const machineOrder = (view: View) => [...view.machines]
  .filter((m) => m.state !== "unregistered" || m.running || m.edrOnly)
  .sort((a, b) => rank(a) - rank(b) || a.name.localeCompare(b.name));
const rank = (m: Machine) => (m.shadow ? 0 : m.stopped ? 1 : m.state === "running" ? 2 : 3);

/** The MCP servers the panel shows before the rest are counted. */
const MANY = 5;

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
  // Two groups: the machines that want a look (shadow AI, calls waiting for the team, or stopped), and the rest.
  // Three groups: stopped by the kill switch; to watch (shadow AI, or calls waiting for the team); and fine.
  const waiting = (x: Machine) => view.standing[x.id]?.pending ?? 0;
  const watch = (x: Machine) => !x.stopped && (x.shadow > 0 || waiting(x) > 0);
  const groups = [
    { key: "stopped", label: "멈춤", list: found.filter((x) => !!x.stopped) },
    { key: "watch", label: "주의", list: found.filter(watch) },
    { key: "fine", label: "정상", list: found.filter((x) => !x.stopped && !watch(x)) },
  ].filter((g) => g.list.length);
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
                <div className={cn("flex items-center gap-1.5 px-2 pt-1.5 pb-1 text-[11px] font-semibold", g.key === "stopped" ? "text-deny" : g.key === "watch" ? "text-catch" : "text-muted-foreground")}>
                  <i className={cn("size-1.5 rounded-full", g.key === "stopped" ? "bg-deny" : g.key === "watch" ? "bg-catch" : "bg-allow")} />
                  {g.label}<span className="tnum opacity-70">{g.list.length}</span>
                </div>
                {g.list.map((x) => {
                  const i = flat.indexOf(x);
                  return (
                    <button key={x.id} onClick={() => pick(x)} onMouseEnter={() => setAt(i)}
                      className={cn("flex h-10 w-full items-center gap-2.5 rounded-md px-2 text-left text-[12.5px] transition-colors", i === at ? "bg-accent" : "hover:bg-accent")}>
                      {dot(x)}
                      <span className="min-w-0 flex-1">
                        <span className="block truncate font-medium">{x.name}</span>
                        <span className={cn("block truncate text-[11px]", x.stopped ? "text-deny/80" : "text-muted-foreground")}>{x.stopped ? (x.stopped.reason || "사유 없음") : [x.agent && agentLabel(x.agent), x.last && ago(x.last, view.now)].filter(Boolean).join(" · ") || "신호 없음"}</span>
                      </span>
                      {x.stopped && <span className="shrink-0 rounded bg-deny/15 px-1.5 text-[11px] leading-[18px] font-semibold text-deny">멈춤</span>}
                      {!x.stopped && x.shadow > 0 && <span className="tnum inline-flex shrink-0 items-center gap-1 rounded bg-deny/15 px-1.5 text-[11px] leading-[18px] font-medium text-deny"><ShieldOff className="size-3" strokeWidth={2.2} />섀도 {x.shadow}</span>}
                      {!x.stopped && waiting(x) > 0 && <span className="tnum shrink-0 rounded bg-catch/15 px-1.5 text-[11px] leading-[18px] font-medium text-catch">대기 {waiting(x)}</span>}
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

/** One figure of how a machine stands: what it counts, the number, and what it is out of. */
function Tile({ icon: Icon, label, n, sub, tone, onClick, tip }: {
  icon: LucideIcon; label: string; n: number; sub?: ReactNode; tone: string; onClick?: () => void; tip: ReactNode;
}) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <button onClick={onClick} disabled={!onClick}
          className="flex min-w-0 flex-col gap-0.5 rounded-lg border bg-background/50 px-2.5 py-2 text-left transition-colors enabled:hover:border-white/30 enabled:hover:bg-accent"
          style={n ? { borderColor: `color-mix(in oklab, ${tone} 45%, transparent)`, background: `color-mix(in oklab, ${tone} 8%, transparent)` } : undefined}>
          <span className="flex items-center gap-1 text-[11px] text-muted-foreground"><Icon className="size-3.5 shrink-0" strokeWidth={1.9} style={n ? { color: tone } : undefined} /><span className="truncate">{label}</span></span>
          <span className="tnum text-[20px] leading-tight font-semibold" style={n ? { color: tone } : { color: "var(--muted-foreground)" }}>{n.toLocaleString()}</span>
          <span className="tnum truncate text-[10.5px] text-muted-foreground">{sub ?? "\u00a0"}</span>
        </button>
      </TooltipTrigger>
      <TooltipContent>{tip}</TooltipContent>
    </Tooltip>
  );
}

/** A small thing held: its mark, a sign in its corner, and its name under it. */
function Item({ children, name, corner, style, faint, tip, onClick }: {
  children: ReactNode; name: string; corner?: ReactNode; style?: React.CSSProperties; faint?: boolean; tip: ReactNode; onClick?: () => void;
}) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <button onClick={onClick} className="group flex w-[52px] shrink-0 flex-col items-center gap-1">
          <span className={cn("relative grid size-10 place-items-center rounded-lg border bg-background/70 transition-colors group-hover:border-white/30", faint && "border-dashed")} style={style}>
            <span className={cn(faint && "opacity-40")}>{children}</span>
            {corner && <span className="absolute -right-1 -bottom-1">{corner}</span>}
          </span>
          <span className="w-full truncate text-center text-[10.5px] leading-tight text-muted-foreground">{name}</span>
        </button>
      </TooltipTrigger>
      <TooltipContent>{tip}</TooltipContent>
    </Tooltip>
  );
}

export function Bag({ view, id, onPick, onDevice, onServer, onSessions, onShadow, onGo, onSession, onStop }: {
  view: View; id: string | null; onPick: (id: string) => void;
  onDevice: (id: string) => void; onServer: (name: string) => void;
  /** Opens the machine's shadow AI runs; opens 메인 or 민감 정보. */
  onShadow: () => void; onGo: (to: "main" | "secrets") => void;
  /** Opens a session's record. */
  onSession: (id: string) => void;
  /** Stops the machine, or lets it go again. */
  onStop: (id: string, stopped: boolean, reason: string) => void;
  /** Lists the machine's sessions, those the engine ran in one mode when a mode is named. */
  onSessions: (id: string, mode?: string) => void;
}) {
  const m = id ? view.machines.find((x) => x.id === id) : undefined;
  if (!m) return <div className="flex h-full flex-col"><div className="flex shrink-0 pb-3"><Picker view={view} id={null} onPick={onPick} /></div><Empty>기기 선택</Empty></div>;
  const shadow = view.shadow.filter((a) => a.device === m.id);
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
  const st = view.standing[m.id] ?? { calls: 0, deny: 0, review: 0, worth: 0, pending: 0, out: 0, stopped: 0 };
  // Its latest sessions, those going first.
  const latest = [...sessions].sort((a, b) => Number(view.going(b)) - Number(view.going(a)) || (b.last_seen_at ?? b.created_at ?? 0) - (a.last_seen_at ?? a.created_at ?? 0)).slice(0, 4);
  const runs = shadow.filter((a) => !a.project).length;

  return (
    <div className="flex h-full flex-col">
      <div className="flex shrink-0 items-center gap-2 pb-3">
        <Picker view={view} id={m.id} onPick={onPick} />
        <KillSwitch key={m.id} m={m} now={view.now} onStop={(stopped, reason) => onStop(m.id, stopped, reason)} />
      </div>
      {/* Its background opens the machine's sheet; what is pressed inside opens its own. */}
      <div role="button" tabIndex={0} aria-label="상세"
        onClick={(e) => { if (!(e.target as HTMLElement).closest("button, a, input")) onDevice(m.id); }}
        onKeyDown={(e) => { if (e.key === "Enter" && e.target === e.currentTarget) onDevice(m.id); }}
        className="-mx-2 -mb-2 flex min-h-0 flex-1 cursor-pointer flex-col gap-3 rounded-lg px-2 pb-2 transition-colors hover:bg-white/[0.025]">
        {/* How it stands: four figures, each opening where it is handled. */}
        <div className="grid shrink-0 grid-cols-4 gap-2">
          <Tile icon={ShieldX} label="차단" n={st.deny + st.review} tone="var(--deny)" onClick={() => onSessions(m.id)}
            sub={<>호출 {st.calls.toLocaleString()}</>} tip={<>호출 {st.calls.toLocaleString()} · 차단 {st.deny} · 사용자 확인 {st.review}</>} />
          <Tile icon={KeyRound} label="민감 정보" n={st.out} tone="var(--deny)" onClick={() => onGo("secrets")}
            sub={<>막음 {st.stopped}</>} tip={<>민감한 값을 밖으로 보낸 호출 {st.out} · 막힌 호출 {st.stopped}</>} />
          <Tile icon={Inbox} label="처리 대기" n={st.pending} tone="var(--catch)" onClick={() => onGo("main")}
            sub={<>탐지 {st.worth}</>} tip={<>탐지 {st.worth} · 처리 대기 {st.pending}</>} />
          <Tile icon={ShieldOff} label="섀도 AI" n={runs} tone="var(--deny)" onClick={runs ? onShadow : undefined}
            sub="실행" tip={<>보호 밖에서 돈 실행 {runs}</>} />
        </div>

        {/* What it holds: its agents, then its MCP servers in the colour of their riskiest tool. */}
        <section className="min-w-0 shrink-0">
          <h3 className="mb-1.5 flex items-center gap-1.5 text-[11.5px] text-muted-foreground">에이전트 <b className="tnum font-medium text-foreground/80">{agents.length}</b><span className="opacity-40">·</span>MCP <b className="tnum font-medium text-foreground/80">{servers.length}</b></h3>
          <div className="flex items-start gap-1 overflow-hidden">
            {agents.map(({ a, guarded, runs: n }) => (
              <Item key={a} name={agentLabel(a).replace(" Code", "")} onClick={() => onDevice(m.id)}
                style={n ? { borderColor: "color-mix(in oklab, var(--deny) 55%, transparent)" } : undefined}
                corner={guarded
                  ? <ShieldCheck className={cn("size-3.5 rounded-full bg-card p-px", n ? "text-review" : "text-allow")} strokeWidth={2.2} />
                  : <ShieldOff className="size-3.5 rounded-full bg-card p-px text-deny" strokeWidth={2.2} />}
                tip={<>{agentLabel(a)}{guarded ? " · 보호됨" : " · 미보호"}{n ? ` · 섀도 실행 ${n}` : ""}</>}>
                <AgentMark agent={a} size={22} />
              </Item>
            ))}
            {agents.length > 0 && servers.length > 0 && <i className="mx-1 mt-2 h-6 w-px shrink-0 bg-border" />}
            {servers.slice(0, MANY).map(({ s, calls, risk }) => (
              <Item key={s.name} name={s.name} onClick={() => onServer(s.name)} faint={!calls} style={calls ? tint(risk) : undefined}
                tip={<>{s.name} · {calls ? `호출 ${calls.toLocaleString()} · 위험도 ${risk}` : "호출 없음"}</>}>
                <ServiceLogo name={s.name} size={22} />
              </Item>
            ))}
            {servers.length > MANY && (
              <button onClick={() => onDevice(m.id)} className="tnum mt-2.5 shrink-0 rounded-md px-1.5 text-[12px] text-muted-foreground hover:bg-accent hover:text-foreground">+{servers.length - MANY}</button>
            )}
            {!agents.length && !servers.length && <span className="text-[12px] text-muted-foreground/70">없음</span>}
          </div>
        </section>

        {/* What it was asked to do lately: its latest sessions by the prompt that began them. */}
        <section className="flex min-h-0 flex-1 flex-col">
          <h3 className="mb-1 flex items-center gap-1.5 text-[11.5px] text-muted-foreground">최근 작업 <b className="tnum font-medium text-foreground/80">{sessions.length}</b></h3>
          <div className="min-h-0 flex-1 overflow-hidden">
            {!latest.length && <span className="text-[12px] text-muted-foreground/70">없음</span>}
            {latest.map((r) => (
              <button key={r.id} onClick={() => onSession(r.id)}
                className="flex h-8 w-full items-center gap-2 rounded-md px-1 text-left transition-colors hover:bg-accent">
                <i className={cn("size-1.5 shrink-0 rounded-full", view.going(r) ? "animate-pulse bg-allow" : "bg-white/20")} />
                <span className={cn("min-w-0 flex-1 truncate text-[12.5px]", !r.title && "font-mono text-[12px] text-muted-foreground")}>{r.title ?? r.id.slice(0, 8)}</span>
                {r.folder && <span className="max-w-[90px] shrink-0 truncate text-[11.5px] text-muted-foreground">{r.folder}</span>}
                {(view.sessionCalls[r.id]?.deny ?? 0) > 0 && <span className="tnum shrink-0 rounded bg-deny/15 px-1.5 text-[11px] leading-[18px] font-medium text-deny">차단 {view.sessionCalls[r.id]!.deny}</span>}
                <span className="tnum shrink-0 text-[11px] text-muted-foreground">{view.going(r) ? "진행 중" : ago(r.last_seen_at ?? r.created_at, view.now)}</span>
              </button>
            ))}
          </div>
        </section>
      </div>
    </div>
  );
}
