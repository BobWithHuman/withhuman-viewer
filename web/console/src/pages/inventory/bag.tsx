// What one machine holds, laid out like a game's inventory: a slot for each agent, MCP server,
// kind of project and kind of session. A slot's corner tells how many or how it stands; a slot
// pressed opens what it holds. The machine is picked here: its name opens a list to search.

import { Check, ChevronsUpDown, FolderGit2, History, Laptop, PanelRightOpen, Search, ShieldCheck, ShieldOff } from "lucide-react";
import { useMemo, useState, type ReactNode } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { Empty, ServiceLogo, ago } from "@/kit";
import { cn } from "@/lib/utils";
import { AgentMark } from "./marks";
import { isOn } from "./mcp/types";
import { ENGINE, PERM, PERM_KEYS, STATE, agentLabel, type Machine, type View } from "./model";


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

/** The machines in the order the picture has them: shadow AI first, then stopped, then running. */
export const machineOrder = (view: View) => [...view.machines]
  .filter((m) => m.state !== "unregistered" || m.running)
  .sort((a, b) => rank(a) - rank(b) || a.name.localeCompare(b.name));
const rank = (m: Machine) => (m.shadow ? 0 : m.stopped ? 1 : m.state === "running" ? 2 : 3);

/** The machine's name as a button that opens every machine to pick from, with a box to search
 * them by name, ID or serial. */
function Picker({ view, id, onPick }: { view: View; id: string | null; onPick: (id: string) => void }) {
  const [open, setOpen] = useState(false);
  const [q, setQ] = useState("");
  const all = useMemo(() => machineOrder(view), [view]);
  const needle = q.trim().toLowerCase();
  const found = all.filter((m) => !needle || [m.name, m.id, m.serial].some((t) => t?.toLowerCase().includes(needle)));
  const m = all.find((x) => x.id === id);
  const dot = (x: Machine) => <i className={cn("size-1.5 shrink-0 rounded-full", x.state === "running" ? "bg-allow" : x.state === "installed" ? "bg-catch" : "bg-foreground/35")} />;
  return (
    <Popover open={open} onOpenChange={(on) => { setOpen(on); if (!on) setQ(""); }}>
      <PopoverTrigger asChild>
        <button className="flex min-w-0 flex-1 items-center gap-3 rounded-lg py-1 pr-2 pl-1 text-left transition-colors hover:bg-accent aria-expanded:bg-accent">
          <span className="grid size-9 shrink-0 place-items-center rounded-lg bg-white/8"><Laptop className="size-[18px]" strokeWidth={1.75} /></span>
          <span className="min-w-0 flex-1">
            <span className="block truncate text-[14px] font-semibold">{m ? m.name : "기기 선택"}</span>
            {m && <span className="flex items-center gap-1.5 truncate text-[11.5px] text-muted-foreground">{dot(m)}{STATE[m.state].label}{m.last ? ` · ${ago(m.last, view.now)}` : ""}</span>}
          </span>
          <ChevronsUpDown className="size-3.5 shrink-0 text-muted-foreground" />
        </button>
      </PopoverTrigger>
      <PopoverContent align="start" className="w-[320px] gap-0 p-0">
        <label className="relative block border-b p-2">
          <Search className="pointer-events-none absolute top-1/2 left-4 size-3.5 -translate-y-1/2 text-muted-foreground" />
          <Input autoFocus value={q} onChange={(e) => setQ(e.target.value)} placeholder="이름·ID·시리얼"
            onKeyDown={(e) => { if (e.key === "Enter" && found[0]) { onPick(found[0].id); setOpen(false); } }}
            className="h-8 pl-7 text-[12.5px] md:text-[12.5px]" />
        </label>
        <div className="scroll max-h-[300px] overflow-y-auto p-1">
          {!found.length && <Empty />}
          {found.map((x) => (
            <button key={x.id} onClick={() => { onPick(x.id); setOpen(false); }}
              className={cn("flex h-10 w-full items-center gap-2.5 rounded-md px-2 text-left text-[12.5px] transition-colors hover:bg-accent", x.id === id && "bg-accent")}>
              <Laptop className="size-4 shrink-0 text-muted-foreground" strokeWidth={1.75} />
              <span className="min-w-0 flex-1">
                <span className="block truncate font-medium">{x.name}</span>
                <span className="flex items-center gap-1.5 truncate text-[11px] text-muted-foreground">{dot(x)}{[x.agent && agentLabel(x.agent), x.last && ago(x.last, view.now)].filter(Boolean).join(" · ")}</span>
              </span>
              {x.stopped && <span className="shrink-0 rounded bg-deny/15 px-1.5 text-[11px] leading-[18px] font-semibold text-deny">멈춤</span>}
              {x.shadow > 0 && <span className="tnum inline-flex shrink-0 items-center gap-1 rounded bg-deny/15 px-1.5 text-[11px] leading-[18px] font-medium text-deny"><ShieldOff className="size-3" strokeWidth={2.2} />{x.shadow}</span>}
              {x.id === id && <Check className="size-3.5 shrink-0 text-primary" />}
            </button>
          ))}
        </div>
      </PopoverContent>
    </Popover>
  );
}

export function Bag({ view, id, onPick, onDevice, onServer, onSessions }: {
  view: View; id: string | null; onPick: (id: string) => void;
  onDevice: (id: string) => void; onServer: (name: string) => void;
  /** Lists the machine's sessions, those let run one way when a mode is named. */
  onSessions: (id: string, mode?: string) => void;
}) {
  const m = id ? view.machines.find((x) => x.id === id) : undefined;
  if (!m) return <div className="flex h-full flex-col"><div className="flex shrink-0 pb-3"><Picker view={view} id={null} onPick={onPick} /></div><Empty>기기 선택</Empty></div>;
  const shadow = view.shadow.filter((a) => a.device === m.id);
  const hookless = shadow.filter((a) => a.project);
  const projects = view.projects.filter((p) => p.device === m.id);
  const sessions = view.sessions.filter((r) => r.device === m.id);
  const servers = view.serversOn(m)
    .map((s) => ({ s, calls: s.tools.reduce((a, t) => a + t.users.reduce((b, u) => b + (isOn(u.client, m) ? u.n : 0), 0), 0) }))
    .sort((a, b) => b.calls - a.calls);
  const guarded = m.agent && m.state !== "unregistered" ? [m.agent] : [];
  const wild = [...new Set(shadow.map((a) => a.agent))];
  const modes = PERM_KEYS.map((k) => ({ k, n: sessions.filter((r) => r.permission_mode === k).length })).filter((x) => x.n);
  const e = m.engine_mode ? ENGINE[m.engine_mode] : undefined;

  return (
    <div className="flex h-full flex-col">
      <div className="flex shrink-0 items-center gap-2 pb-3">
        <Picker view={view} id={m.id} onPick={onPick} />
        {m.stopped && <span className="rounded bg-deny/15 px-1.5 text-[11px] leading-5 font-semibold text-deny">멈춤</span>}
        {e && <Tooltip><TooltipTrigger asChild><e.icon className="size-4 shrink-0" strokeWidth={1.9} style={{ color: e.color }} /></TooltipTrigger><TooltipContent>엔진 {e.label}</TooltipContent></Tooltip>}
        <Tooltip><TooltipTrigger asChild><Button size="icon-sm" variant="secondary" aria-label="상세" onClick={() => onDevice(m.id)}><PanelRightOpen /></Button></TooltipTrigger><TooltipContent>상세 · 기기 멈춤</TooltipContent></Tooltip>
      </div>
      <div className="scroll -mr-2 grid min-h-0 flex-1 grid-cols-2 content-start gap-x-4 gap-y-2.5 overflow-y-auto pr-2">
        <Shelf title="에이전트" count={guarded.length + wild.length} row={3}>
          {[
            ...guarded.map((a) => (
              <Slot key={`g${a}`} name={agentLabel(a)} corner={<ShieldCheck className="size-3 text-allow" strokeWidth={2.2} />} tip={<>{agentLabel(a)} · 보호됨{m.hook_version ? ` · 훅 ${m.hook_version}` : ""}</>}>
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
        <Shelf title="프로젝트" count={projects.length + hookless.length} row={3}>
          {[
            ...(projects.length ? [<Slot key="p" name="보호" corner={projects.length} onClick={() => onDevice(m.id)} tip={<>보호된 프로젝트 {projects.length}</>}><FolderGit2 className="size-6 text-allow" strokeWidth={1.6} /></Slot>] : []),
            ...(hookless.length ? [<Slot key="s" name="섀도" tone="var(--deny)" corner={hookless.length} onClick={() => onDevice(m.id)} tip={<>훅 없이 돈 프로젝트 {hookless.length}</>}><FolderGit2 className="size-6 text-deny" strokeWidth={1.6} /></Slot>] : []),
          ]}
        </Shelf>
        <Shelf title="MCP 서버" count={servers.length} row={6}>
          {servers.map(({ s, calls }) => (
            <Slot key={s.name} name={s.name} onClick={() => onServer(s.name)} faint={!calls} corner={calls ? calls.toLocaleString() : undefined}
              tip={<>{s.name} · {calls ? `호출 ${calls.toLocaleString()}` : "호출 없음"}</>}>
              <ServiceLogo name={s.name} size={26} />
            </Slot>
          ))}
        </Shelf>
        <Shelf title="세션" count={sessions.length} row={6}>
          {modes.map(({ k, n }) => (
            <Slot key={k} name={PERM[k].label} onClick={() => onSessions(m.id, k)} corner={n.toLocaleString()} tone={k === "bypassPermissions" ? "var(--deny)" : undefined}
              tip={<>{PERM[k].label} · 세션 {n.toLocaleString()}</>}>
              <History className="size-6" strokeWidth={1.6} style={{ color: PERM[k].color }} />
            </Slot>
          ))}
        </Shelf>
      </div>
    </div>
  );
}
