// 인벤토리: the machines the engine protects and what is on them. On the left the MCP tools that
// are a threat, across every machine; the machine picked in the panel beside it is the one the
// panel and the lists under them show, and the tools it called are marked on the left: its sessions, the shadow AI seen
// on it and its MCP servers, each row in brief and opening in full. A machine's sheet is where it is stopped; the app does
// that (`src/admin/inventory.rs`) and answers in `data.reply`.

import type { ColumnDef } from "@tanstack/react-table";
import { ArrowUpRight, Check, Laptop, PackageOpen, PlugZap, Search, TriangleAlert, X } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Sheet, SheetContent } from "@/components/ui/sheet";
import { Skeleton } from "@/components/ui/skeleton";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { send } from "@/bridge";
import { DataTable, Empty, PageHeader, Panel, ServiceLogo, ago, clock, num } from "@/kit";
import { cn } from "@/lib/utils";
import { demo, demoAnswer } from "./inventory/demo";
import { Asks, type Ask } from "./inventory/dialogs";
import { AgentMark, Chip, Dash, EngineBadge, StateBadge } from "./inventory/marks";
import { Threats, ThreatsLegend } from "./inventory/threats";
import { ServerSheet, ToolSheet } from "./inventory/mcp/sheets";
import { isOn } from "./inventory/mcp/types";
import { END, ENGINE, derive, runName, type Data, type Run, type Shadow, type Tab } from "./inventory/model";
import { Bag, machineOrder } from "./inventory/bag";
import { DeviceSheet } from "./inventory/sheets";
import { StoppedMachines } from "./inventory/kill";

const TABS: [Tab, string][] = [["sessions", "세션"], ["shadow", "섀도 AI"], ["mcp", "MCP"]];
/** A tab the app named, when this page has it. */
const known = (t?: string | null) => TABS.find(([k]) => k === t)?.[0];
/** The engine mode the sessions are narrowed to. */
type Filter = { kind: "engine"; value: string };
type Open = { kind: "device"; id: string } | { kind: "server"; name: string } | { kind: "tool"; server: string; tool: string };

const mono = "font-mono text-[12px]";
const when = (ms: number | null, now: number) => (ms ? <span className="tnum text-muted-foreground">{ago(ms, now)}</span> : <Dash />);

export default function Page({ data }: { data: unknown | null }) {
  // Opened on its own, the page shows the demo and answers its own requests.
  const standalone = data === null;
  const [local, setLocal] = useState(() => demo());
  const d = (data as Data | null) ?? local;

  const [windowMin, setWindowMin] = useState(10);
  const [tab, setTab] = useState<Tab>(known(d.tab) ?? "sessions");
  const [filter, setFilter] = useState<Filter | null>(null);
  // The machine the panel and the lists show: the first in the picture until one is picked.
  const [focus, setFocus] = useState<string | null>(null);
  const [q, setQ] = useState("");
  const [open, setOpen] = useState<Open | null>(null);
  const [ask, setAsk] = useState<Ask | null>(null);
  const [pending, setPending] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [toast, setToast] = useState<string | null>(null);
  const req = useRef(Date.now());
  const lists = useRef<HTMLDivElement>(null);

  const view = useMemo(() => derive(d, windowMin), [d, windowMin]);

  const request = (message: Record<string, unknown>) => {
    const id = ++req.current;
    setPending(id);
    setError(null);
    send("inventory", { ...message, req: id });
    if (standalone) setTimeout(() => setLocal((l) => demoAnswer(l, message, id)), 400);
  };
  // The app's answer to what was asked: a key to show, a note that it is done, or why it failed.
  useEffect(() => {
    const r = d.reply;
    if (!r || r.req !== pending) return;
    setPending(null);
    if (!standalone) send("inventory", { type: "ack" });
    if (!r.ok) setError(r.error ?? "실패");
    else {
      setAsk(null);
      setToast(r.done === "device_stopped" ? "기기 멈춤" : r.done === "device_released" ? "기기 멈춤 풀림" : "관리 API 저장됨");
    }
  }, [d.reply, pending, standalone]);
  // The app names the tab to open on; it changes the tab again only when it names another.
  useEffect(() => {
    const t = known(d.tab);
    if (t) setTab(t);
  }, [d.tab]);
  useEffect(() => {
    if (!toast) return;
    const t = setTimeout(() => setToast(null), 4000);
    return () => clearTimeout(t);
  }, [toast]);

  const openSession = (id: string) => send("inventory", { type: "open_session", id });
  const closeAsk = () => { setAsk(null); setError(null); setPending(null); };

  const has = (...parts: (string | null | undefined)[]) => !q.trim() || parts.some((p) => p?.toLowerCase().includes(q.trim().toLowerCase()));
  const current = view.machines.find((m) => m.id === focus) ?? machineOrder(view)[0];
  const mine = <T extends { device: string | null }>(list: T[]) => (current ? list.filter((x) => x.device === current.id) : []);
  const allSessions = mine(view.sessions), allShadow = mine(view.shadow);
  const modeOf = (r: Run) => view.sessionCalls[r.id]?.mode ?? null;
  const sessions = allSessions.filter((r) => (!filter || modeOf(r) === filter.value) && has(r.title, r.folder, r.id));
  const shadow = allShadow.filter((a) => has(a.project, a.agent, a.command));
  const servers = view.mcp?.servers ?? [];
  // The machine's MCP servers, with the calls of their tools made from it and the latest.
  const serverRows = useMemo(() => (current ? view.serversOn(current) : []).map((s) => {
    const calls = s.tools.reduce((a, t) => a + t.users.reduce((b, u) => b + (isOn(u.client, current!) ? u.n : 0), 0), 0);
    const last = Math.max(0, ...s.tools.flatMap((t) => t.recent.filter((r) => isOn(r.client, current!)).map((r) => r.at)));
    return { s, calls, last: last || null };
  }).sort((a, b) => b.calls - a.calls), [view, current]);
  const shownServers = serverRows.filter((x) => has(x.s.name, ...x.s.tools.map((t) => t.name)));
  const sessionCols = useMemo<ColumnDef<Run>[]>(() => [
    { id: "state", header: "상태", size: 96, accessorFn: (r) => Number(view.going(r)), cell: ({ row }) => (
      view.going(row.original) ? <StateBadge state="running" /> : <span className="text-muted-foreground">{row.original.ended_at ? "끝남" : "신호 끊김"}</span>
    ) },
    { id: "what", header: "작업", accessorFn: (r) => r.title ?? r.id, cell: ({ row: { original: r } }) => (
      <span className="flex min-w-0 items-center gap-1.5" title={r.title ?? r.id}>
        {r.folder && <span className="shrink-0 text-muted-foreground">{r.folder}</span>}
        <span className={cn("truncate", !r.title && cn(mono, "text-muted-foreground"))}>{r.title ?? r.id.slice(0, 8)}</span>
        {r.opens && <ArrowUpRight className="size-3.5 shrink-0 text-muted-foreground" />}
      </span>
    ) },
    { id: "calls", header: "호출", size: 160, accessorFn: (r) => view.sessionCalls[r.id]?.n ?? 0, cell: ({ row: { original: r } }) => {
      const n = view.sessionCalls[r.id];
      return n ? (
        <span className="tnum flex items-center gap-2"><b className="font-medium">{num(n.n)}</b>
          {n.review > 0 && <span className="text-[12px] text-review">확인 {n.review}</span>}
          {n.deny > 0 && <span className="text-[12px] text-deny">차단 {n.deny}</span>}
        </span>
      ) : <Dash />;
    } },
    { id: "engine", header: "엔진 모드", size: 136, accessorFn: (r) => modeOf(r) ?? "", cell: ({ row }) => <EngineBadge mode={modeOf(row.original)} /> },
    { id: "end", header: "끝", size: 168, accessorFn: (r) => r.ended_at ?? 0, cell: ({ row: { original: r } }) => (
      r.ended_at ? <span className="tnum truncate text-muted-foreground">{clock(r.ended_at)} · {END[r.end_reason ?? ""] ?? "끝남"}</span> : <Dash />
    ) },
  ], [view]);

  const shadowCols = useMemo<ColumnDef<Shadow>[]>(() => [
    { id: "agent", header: "에이전트", size: 152, accessorFn: (a) => a.agent, cell: ({ row }) => <AgentMark agent={row.original.agent} label /> },
    { id: "session", header: "세션", size: 340, accessorFn: (a) => a.command ?? "", cell: ({ row: { original: a } }) => {
      const n = runName(a.command);
      const resumed = n?.resume && n.resume !== true ? view.sessions.find((r) => r.id === n.resume) : undefined;
      return (
        <span className="flex min-w-0 items-center gap-1.5" title={a.command ?? undefined}>
          {n?.prompt ? <span className="truncate">{n.prompt}</span>
            : n?.resume ? <><Chip className="shrink-0">이어 하기</Chip><span className={cn("truncate", !resumed?.title && cn(mono, "text-muted-foreground"))}>{resumed?.title ?? (n.resume === true ? "" : n.resume.slice(0, 8))}</span></>
            : <span className="text-muted-foreground">{a.command ? "프롬프트 없음" : "명령줄 없음"}</span>}
        </span>
      );
    } },
    { id: "first", header: "처음 본 때", size: 140, accessorFn: (a) => a.first ?? a.last_seen ?? 0, cell: ({ row: { original: a } }) => <span className="tnum text-muted-foreground">{clock(a.first ?? a.last_seen)}</span> },
    { id: "last", header: "마지막 본 때", size: 140, accessorFn: (a) => a.last_seen ?? 0, cell: ({ row }) => when(row.original.last_seen, view.now) },
    { id: "scans", header: "스캔", size: 80, accessorFn: (a) => a.scans ?? 1, cell: ({ row }) => <span className="tnum">{row.original.scans ?? 1}</span> },
  ], [view]);

  type ServerRow = (typeof serverRows)[number];
  const serverCols = useMemo<ColumnDef<ServerRow>[]>(() => [
    { id: "name", header: "서버", accessorFn: (x) => x.s.name, cell: ({ row: { original: x } }) => (
      <span className="flex min-w-0 items-center gap-2"><ServiceLogo name={x.s.name} size={18} /><span className="truncate font-mono text-[12.5px]">{x.s.name}</span><Chip className="shrink-0">{x.s.transport === "local" ? "로컬" : "원격"}</Chip></span>
    ) },
    { id: "tools", header: "도구", size: 96, accessorFn: (x) => x.s.tools.length, cell: ({ row }) => <span className="tnum">{row.original.s.tools.length}</span> },
    { id: "calls", header: "호출", size: 112, accessorFn: (x) => x.calls, cell: ({ row }) => <span className={cn("tnum", !row.original.calls && "text-muted-foreground")}>{num(row.original.calls)}</span> },
    { id: "last", header: "마지막 호출", size: 140, accessorFn: (x) => x.last ?? 0, cell: ({ row }) => when(row.original.last, view.now) },
  ], [view.now]);

  const header = (
    <PageHeader title="인벤토리" meta={<>
      <span className="inline-flex items-center gap-1.5"><i className={cn("size-1.5 rounded-full", d.source.demo ? "bg-catch" : "bg-allow")} />{d.source.label}</span>
      {d.status !== "loading" && <span className="tnum">{clock(d.now)}</span>}
    </>}>
      <Select value={String(windowMin)} onValueChange={(v) => setWindowMin(Number(v))}>
        <SelectTrigger size="sm" className="gap-1.5 text-[12.5px]"><span className="text-muted-foreground">활동 기준</span><SelectValue /></SelectTrigger>
        <SelectContent align="end">{[10, 30, 60].map((m) => <SelectItem key={m} value={String(m)}>최근 {m}분</SelectItem>)}</SelectContent>
      </Select>
      <StoppedMachines machines={view.machines} now={view.now} onPick={setFocus}
        onRelease={(id) => request({ type: "stop_device", device: id, stopped: false, actor: d.actor, reason: "" })} />
      <Button variant="outline" size="sm" onClick={() => setAsk({ kind: "admin" })}>
        <PlugZap />관리 API
        <i className={cn("size-1.5 rounded-full", d.source.demo ? "bg-catch" : d.admin.has_token ? "bg-allow" : "bg-white/25")} />
        <span className="text-muted-foreground">{d.source.demo ? "데모" : d.admin.has_token ? "연결됨" : "연결 안 됨"}</span>
      </Button>
    </PageHeader>
  );
  const dialogs = ask && (
    <Asks admin={d.admin} demo={d.source.demo} busy={pending !== null} error={error} onSend={request} onClose={closeAsk} />
  );

  if (d.status !== "ready") {
    return (
      <div className="flex h-full flex-col">
        {header}
        {d.status === "loading" ? (
          <div className="flex min-h-0 flex-1 flex-col gap-4 px-6 pb-6">
            <div className="grid h-[352px] shrink-0 grid-cols-12 gap-4"><Skeleton className="col-span-8 rounded-xl" /><Skeleton className="col-span-4 rounded-xl" /></div>
            <Skeleton className="min-h-0 flex-1 rounded-xl" />
          </div>
        ) : (
          <div className="grid min-h-0 flex-1 place-items-center px-6 pb-16">
            <div className="flex max-w-[480px] flex-col items-center gap-3 text-center">
              <span className={cn("grid size-11 place-items-center rounded-xl", d.status === "failed" ? "bg-deny/15 text-deny" : "bg-white/8 text-muted-foreground")}>
                {d.status === "failed" ? <TriangleAlert className="size-5" /> : <PackageOpen className="size-5" />}
              </span>
              <span className="text-[15px] font-semibold">{d.status === "failed" ? "불러오기 실패" : "인벤토리 없음"}</span>
              {d.status === "failed"
                ? <code className="font-mono text-[12px] break-all text-muted-foreground">{d.error}</code>
                : <div className="flex flex-wrap justify-center gap-1.5"><Chip>기기 등록</Chip><Chip>훅 설치</Chip><Chip>PostgreSQL 엔진</Chip></div>}
            </div>
          </div>
        )}
        {dialogs}
      </div>
    );
  }

  const machine = open?.kind === "device" ? view.machines.find((m) => m.id === open.id) : undefined;
  const server = open?.kind === "server" || open?.kind === "tool" ? servers.find((s) => s.name === (open.kind === "server" ? open.name : open.server)) : undefined;
  const tool = open?.kind === "tool" ? server?.tools.find((t) => t.name === open.tool) : undefined;
  const openServer = (name: string) => setOpen({ kind: "server", name });
  const openTool = (s: string, t: string) => setOpen({ kind: "tool", server: s, tool: t });
  const openDevice = (id: string) => setOpen({ kind: "device", id });
  const chip = filter && `엔진 ${ENGINE[filter.value]?.label ?? filter.value}`;
  const counts: Record<Tab, number> = {
    sessions: allSessions.length, shadow: allShadow.length, mcp: serverRows.length,
  };
  const shownCount = sessions.length;

  return (
    <div className="flex h-full flex-col">
      {header}
      <div className="scroll flex min-h-0 flex-1 flex-col gap-4 overflow-y-auto px-6 pb-6">
        <div className="grid shrink-0 grid-cols-12 gap-4">
          <Panel className="col-span-8 min-w-0" title="MCP 위협" aside={<>{view.mcp?.from_calls && <Chip>호출 기준</Chip>}<ThreatsLegend /></>}>
            <div className="h-[360px]">
              <Threats servers={servers} focus={current ?? null} onServer={openServer} onTool={openTool} />
            </div>
          </Panel>
          <Panel className="col-span-4 min-w-0" title="인벤토리">
            <div className="h-[360px]">
              <Bag view={view} id={current?.id ?? null} onPick={setFocus} onDevice={openDevice} onServer={openServer}
                onSessions={(id, mode) => { setFocus(id); setFilter(mode ? { kind: "engine", value: mode } : null); setTab("sessions"); }}
                onShadow={() => setTab("shadow")} onGo={(to) => send("inventory", { type: "go", to })} onSession={openSession}
                onStop={(id, stopped, reason) => request({ type: "stop_device", device: id, stopped, actor: d.actor, reason })} />
            </div>
          </Panel>
        </div>

        <Card ref={lists} className="flex min-h-[340px] flex-1 flex-col gap-0 overflow-hidden py-0 shadow-none">
          <div className="flex h-12 shrink-0 items-center gap-3 border-b px-4">
            {current && <span className="flex shrink-0 items-center gap-1.5 text-[13px] font-semibold"><Laptop className="size-4 text-muted-foreground" strokeWidth={1.8} />{current.name}</span>}
            <Tabs value={tab} onValueChange={(v) => setTab(v as Tab)}>
              <TabsList>
                {TABS.map(([key, label]) => (
                  <TabsTrigger key={key} value={key} className="gap-1.5">
                    {label}<span className={cn("tnum rounded-full bg-secondary px-1.5 text-[11px] font-normal text-muted-foreground", key === "shadow" && counts.shadow > 0 && "bg-deny/15 text-deny")}>{num(counts[key])}</span>
                  </TabsTrigger>
                ))}
              </TabsList>
            </Tabs>
            {chip && (
              <Badge variant="secondary" className="h-6 gap-1 border-0 bg-primary/15 pr-1 font-medium text-primary">
                {chip}<span className="tnum opacity-70">{shownCount}</span>
                <button aria-label="필터 지우기" onClick={() => setFilter(null)} className="grid size-4 place-items-center rounded-full hover:bg-primary/20"><X className="size-3" /></button>
              </Badge>
            )}
            {tab === "shadow" && !view.shadowKnown && <Chip>미수집</Chip>}
            {tab === "mcp" && !view.mcp && <Chip>미수집</Chip>}
            <div className="ml-auto flex items-center gap-3">
              <div className="relative">
                <Search className="pointer-events-none absolute top-1/2 left-2.5 size-3.5 -translate-y-1/2 text-muted-foreground" />
                <Input value={q} onChange={(e) => setQ(e.target.value)} placeholder="검색" className="h-7 w-48 pl-8 text-[12.5px]" />
              </div>
            </div>
          </div>
          <div className="min-h-0 flex-1 text-[13px] [&_[data-slot=table-container]]:h-full [&_[data-slot=table-container]]:overflow-auto [&_[data-slot=table-container]]:[scrollbar-color:#4a4a50_transparent] [&_[data-slot=table-container]]:[scrollbar-width:thin] [&_td:first-child]:pl-4 [&_th:first-child]:pl-4">
            {tab === "sessions" && <DataTable key="sessions" columns={sessionCols} rows={sessions.slice(0, 300)} onRow={(r) => { if (r.opens) openSession(r.id); }} />}
            {tab === "shadow" && <DataTable key="shadow" columns={shadowCols} rows={shadow} onRow={() => current && openDevice(current.id)} />}
            {tab === "mcp" && (view.mcp ? <DataTable key="mcp" columns={serverCols} rows={shownServers} onRow={(x) => openServer(x.s.name)} picked={(x) => open?.kind === "server" && open.name === x.s.name} /> : <Empty />)}
          </div>
        </Card>
      </div>

      <Sheet open={!!(machine || server)} onOpenChange={(o) => { if (!o && !ask) setOpen(null); }}>
        <SheetContent aria-describedby={undefined} onOpenAutoFocus={(e) => e.preventDefault()} className="w-[480px] gap-0 data-[side=right]:sm:max-w-[480px]">
          {machine && <DeviceSheet m={machine} view={view} onServer={openServer}
            onStop={(stopped, reason) => request({ type: "stop_device", device: machine.id, stopped, actor: d.actor, reason })} />}
          {server && (tool
            ? <ToolSheet server={server} tool={tool} view={view} onServer={openServer} onDevice={openDevice} />
            : <ServerSheet server={server} view={view} onTool={openTool} onDevice={openDevice} />)}
        </SheetContent>
      </Sheet>
      {dialogs}
      {toast && (
        <div className="pointer-events-none fixed inset-x-0 bottom-6 z-[60] flex justify-center">
          <span className="inline-flex items-center gap-2 rounded-full border bg-popover px-3.5 py-2 text-[12.5px] shadow-xl animate-in fade-in-0 slide-in-from-bottom-2"><Check className="size-3.5 text-allow" />{toast}</span>
        </div>
      )}
    </div>
  );
}
