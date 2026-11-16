// 인벤토리: the machines, people and sessions the engine protects. The page opens on the picture
// of who has what (each person's machines by the agents and MCP servers on them), and beside it
// what the person picked holds, slot by slot. Under it the machines, people,
// sessions and shadow AI as lists, and the MCP servers set up on the machines as a board of
// their tools by what the engine did with the calls; a machine, a person, a server or a tool
// opens in a sheet, where keys are issued and revoked and a person is stopped. The app does
// those (`src/admin/inventory.rs`) and answers in `data.reply`.

import type { ColumnDef } from "@tanstack/react-table";
import { ArrowUpRight, Check, KeyRound, Laptop, PackageOpen, PlugZap, Search, ShieldOff, TriangleAlert, X } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { Checkbox } from "@/components/ui/checkbox";
import { Input } from "@/components/ui/input";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Sheet, SheetContent } from "@/components/ui/sheet";
import { Skeleton } from "@/components/ui/skeleton";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { send } from "@/bridge";
import { DataTable, Empty, NATURE, NatureIcon, PageHeader, Panel, Person, Warn, ago, clock, shortUser, type Nature } from "@/kit";
import { cn } from "@/lib/utils";
import { demo, demoAnswer } from "./inventory/demo";
import { Asks, type Ask } from "./inventory/dialogs";
import { AgentMark, Chip, Dash, EngineBadge, PermBadge, StateBadge } from "./inventory/marks";
import { Fleet, FleetLegend } from "./inventory/fleet";
import { Band } from "./inventory/mcp/band";
import { Board } from "./inventory/mcp/board";
import { ServerSheet, ToolSheet } from "./inventory/mcp/sheets";
import { NATURES, toolKey, type Server, type Tool } from "./inventory/mcp/types";
import { END, PERM, SOURCE, derive, type Data, type Machine, type PersonRow, type Run, type Shadow, type Tab } from "./inventory/model";
import { Bag } from "./inventory/bag";
import { DeviceSheet, PersonSheet } from "./inventory/sheets";

const TABS: [Tab, string][] = [["devices", "기기"], ["people", "사용자"], ["sessions", "세션"], ["shadow", "섀도 AI"], ["mcp", "MCP"]];
/** The way agents are let run that the lists are narrowed to. */
type Filter = { kind: "perm"; value: string };
type Open = { kind: "device"; id: string } | { kind: "person"; email: string } | { kind: "server"; name: string } | { kind: "tool"; server: string; tool: string };

const mono = "font-mono text-[12px]";
const when = (ms: number | null, now: number) => (ms ? <span className="tnum text-muted-foreground">{ago(ms, now)}</span> : <Dash />);
const who = (email: string | null) => (email ? <Person name={email} size={20} label /> : <Dash />);

export default function Page({ data }: { data: unknown | null }) {
  // Opened on its own, the page shows the demo and answers its own requests.
  const standalone = data === null;
  const [local, setLocal] = useState(() => demo());
  const d = (data as Data | null) ?? local;

  const [windowMin, setWindowMin] = useState(10);
  const [tab, setTab] = useState<Tab>(d.tab ?? "devices");
  const [nature, setNature] = useState<Nature | null>(null);
  const [all, setAll] = useState(false);
  const [filter, setFilter] = useState<Filter | null>(null);
  // The person whose holdings are shown beside the picture: the first in it until one is picked
  // from the panel's own list.
  const [holder, setHolder] = useState<string | null>(null);
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
    else if (r.done === "issued") setAsk({ kind: "issued", email: r.email ?? "", key_id: r.key_id ?? "", key: r.key ?? "" });
    else {
      setAsk(null);
      setToast(r.done === "revoked" ? `키 회수됨 · ${shortUser(r.email ?? "")} · ${r.key_id}`
        : r.done === "stopped" ? `멈춤 · ${shortUser(r.email ?? "")} · 키 ${r.count ?? 0}개 회수` : r.done === "owner" ? "담당자 저장됨" : "관리 API 저장됨");
    }
  }, [d.reply, pending, standalone]);
  // The app names the tab to open on; it changes the tab again only when it names another.
  useEffect(() => {
    if (d.tab) setTab(d.tab);
  }, [d.tab]);
  // The board is as tall as its servers, so its tab comes up to the top of the page.
  useEffect(() => {
    if (tab === "mcp") lists.current?.scrollIntoView({ block: "start" });
  }, [tab, d.status]);
  useEffect(() => {
    if (!toast) return;
    const t = setTimeout(() => setToast(null), 4000);
    return () => clearTimeout(t);
  }, [toast]);

  const openSession = (id: string) => send("inventory", { type: "open_session", id });
  const closeAsk = () => { setAsk(null); setError(null); setPending(null); };

  const has = (...parts: (string | null | undefined)[]) => !q.trim() || parts.some((p) => p?.toLowerCase().includes(q.trim().toLowerCase()));
  // The lists show only the person picked in the panel's own list, until that is let go.
  const mine = holder ? new Set(view.machines.filter((m) => m.user === holder).map((m) => m.id)) : null;
  const ofHolder = (user: string | null, device?: string | null) => !holder || user === holder || (!!device && mine!.has(device));
  const everyone = view.machines.filter((m) => all || m.state !== "unregistered" || m.running);
  const base = everyone.filter((m) => ofHolder(m.user));
  const machines = base.filter((m) =>
    (!filter || m.permission_mode === filter.value)
    && has(m.name, m.id, m.user, m.team, m.agent));
  const people = view.people.filter((p) => (!holder || p.email === holder) && has(p.email));
  const sessions = view.sessions.filter((r) =>
    ofHolder(r.user, r.device) && (!filter || r.permission_mode === filter.value)
    && has(r.host, r.device, r.user, r.title, r.folder, r.id, r.model));
  const shadow = view.shadow.filter((a) => ofHolder(a.user, a.device) && has(a.host, a.device, a.user, a.project, a.agent));
  const servers = view.mcp?.servers ?? [];
  // The servers the lists show: those on the picked person's machines.
  const listed = mine ? servers.filter((sv) => view.machines.some((m) => mine.has(m.id) && view.serversOn(m).includes(sv))) : servers;
  const lit = (s: Server, t: Tool) => (!nature || t.nature === nature) && has(s.name, t.name);
  const sifted = nature !== null || q.trim() !== "";

  const deviceCols = useMemo<ColumnDef<Machine>[]>(() => [
    { id: "name", header: "기기", accessorFn: (m) => m.name, cell: ({ row: { original: m } }) => (
      <span className="flex min-w-0 items-center gap-2">
        <Laptop className="size-4 shrink-0 text-muted-foreground" strokeWidth={1.75} />
        <span className="truncate font-medium" title={m.id}>{m.name}</span>
        {m.shadow > 0 && <Badge variant="secondary" className="shrink-0 gap-1 border-0 bg-deny/15 font-semibold text-deny"><ShieldOff />섀도 {m.shadow}</Badge>}
      </span>
    ) },
    { id: "user", header: "사용자", size: 160, accessorFn: (m) => m.user ?? "", cell: ({ row }) => who(row.original.user) },
    { id: "team", header: "팀", size: 96, accessorFn: (m) => m.team ?? "", cell: ({ row }) => (row.original.team ? <span className="truncate text-muted-foreground">{row.original.team}</span> : <Dash />) },
    { id: "agent", header: "에이전트", size: 144, accessorFn: (m) => m.agent ?? "", cell: ({ row }) => <AgentMark agent={row.original.agent} label /> },
    { id: "perm", header: "권한 모드", size: 136, accessorFn: (m) => m.permission_mode ?? "", cell: ({ row }) => <PermBadge mode={row.original.permission_mode} /> },
    { id: "engine", header: "엔진 모드", size: 112, accessorFn: (m) => m.engine_mode ?? "", cell: ({ row }) => <EngineBadge mode={row.original.engine_mode} /> },
    { id: "state", header: "상태", size: 112, accessorFn: (m) => ["running", "idle", "installed", "unregistered"].indexOf(m.state), cell: ({ row }) => <StateBadge state={row.original.state} /> },
    { id: "projects", header: "프로젝트", size: 96, accessorFn: (m) => m.projects, cell: ({ row }) => <span className="tnum">{row.original.projects}</span> },
    { id: "sessions", header: "세션", size: 72, accessorFn: (m) => m.sessions, cell: ({ row }) => <span className="tnum">{row.original.sessions}</span> },
    { id: "last", header: "마지막 신호", size: 108, accessorFn: (m) => m.last ?? 0, cell: ({ row }) => when(row.original.last, view.now) },
  ], [view.now]);

  const peopleCols = useMemo<ColumnDef<PersonRow>[]>(() => [
    { id: "email", header: "사용자", accessorFn: (p) => p.email, cell: ({ row: { original: p } }) => (
      <span className="flex min-w-0 items-center gap-2">
        <Person name={p.email} size={20} />
        <span className="truncate font-medium">{p.email}</span>
        {p.kind === "service" && <Chip>서비스</Chip>}
        {p.devices.length > 3 && <Warn>기기 많음</Warn>}
      </span>
    ) },
    { id: "devices", header: "기기", size: 88, accessorFn: (p) => p.devices.length, cell: ({ row }) => <span className="tnum">{row.original.devices.length}</span> },
    { id: "sessions", header: "세션", size: 88, accessorFn: (p) => p.sessions, cell: ({ row }) => <span className="tnum">{row.original.sessions}</span> },
    { id: "keys", header: "키", size: 184, accessorFn: (p) => (p.stopped ? -1 : p.active_keys), cell: ({ row: { original: p } }) => (
      <span className="flex items-center gap-1.5">
        {p.stopped ? <Badge variant="secondary" className="border-0 bg-deny/15 font-semibold text-deny">멈춤</Badge>
          : <Badge variant="secondary" className="border-0 bg-allow/15 font-medium text-allow">활성 {p.active_keys}</Badge>}
        {p.revoked_keys > 0 && <span className="tnum text-[12px] text-muted-foreground">회수 {p.revoked_keys}</span>}
      </span>
    ) },
    { id: "last", header: "마지막 사용", size: 120, accessorFn: (p) => p.last_used ?? 0, cell: ({ row }) => when(row.original.last_used, view.now) },
    { id: "denied", header: "거부", size: 88, accessorFn: (p) => p.denied, cell: ({ row }) => <span className={cn("tnum", row.original.denied > 0 ? "font-medium text-deny" : "text-muted-foreground")}>{row.original.denied}</span> },
  ], [view.now]);

  const sessionCols = useMemo<ColumnDef<Run>[]>(() => [
    { id: "state", header: "상태", size: 96, accessorFn: (r) => Number(view.going(r)), cell: ({ row }) => (
      view.going(row.original) ? <StateBadge state="running" /> : <span className="text-muted-foreground">{row.original.ended_at ? "끝남" : "신호 끊김"}</span>
    ) },
    { id: "device", header: "기기", size: 152, accessorFn: (r) => r.host ?? r.device ?? "", cell: ({ row: { original: r } }) => (
      r.device ? <span className="truncate" title={r.device}>{r.host ?? r.device}</span> : <Badge variant="secondary" className="border-0 bg-catch/15 font-semibold text-catch">ID 없음</Badge>
    ) },
    { id: "user", header: "사용자", size: 128, accessorFn: (r) => r.user ?? "", cell: ({ row }) => who(row.original.user) },
    { id: "what", header: "작업", accessorFn: (r) => r.title ?? r.id, cell: ({ row: { original: r } }) => (
      <span className="flex min-w-0 items-center gap-1.5" title={r.title ?? r.id}>
        {r.folder && <span className="shrink-0 text-muted-foreground">{r.folder}</span>}
        <span className={cn("truncate", !r.title && cn(mono, "text-muted-foreground"))}>{r.title ?? r.id.slice(0, 8)}</span>
        {r.opens && <ArrowUpRight className="size-3.5 shrink-0 text-muted-foreground" />}
      </span>
    ) },
    { id: "start", header: "시작", size: 176, accessorFn: (r) => r.created_at ?? 0, cell: ({ row: { original: r } }) => (
      <span className="tnum truncate text-muted-foreground">{[clock(r.created_at), r.source && SOURCE[r.source]].filter(Boolean).join(" · ") || "–"}</span>
    ) },
    { id: "perm", header: "권한 모드", size: 136, accessorFn: (r) => r.permission_mode ?? "", cell: ({ row }) => <PermBadge mode={row.original.permission_mode} /> },
    { id: "model", header: "모델", size: 136, accessorFn: (r) => r.model ?? "", cell: ({ row }) => <span className={cn(mono, "truncate text-muted-foreground")}>{row.original.model ?? "–"}</span> },
    { id: "end", header: "끝", size: 168, accessorFn: (r) => r.ended_at ?? 0, cell: ({ row: { original: r } }) => (
      r.ended_at ? <span className="tnum truncate text-muted-foreground">{clock(r.ended_at)} · {END[r.end_reason ?? ""] ?? "끝남"}</span> : <Dash />
    ) },
  ], [view]);

  const shadowCols = useMemo<ColumnDef<Shadow>[]>(() => [
    { id: "agent", header: "에이전트", size: 152, accessorFn: (a) => a.agent, cell: ({ row }) => <AgentMark agent={row.original.agent} label /> },
    { id: "device", header: "기기", size: 184, accessorFn: (a) => a.host ?? a.device ?? "", cell: ({ row: { original: a } }) => <span className="truncate font-medium" title={a.device ?? ""}>{a.host ?? a.device ?? "–"}</span> },
    { id: "user", header: "사용자", size: 168, accessorFn: (a) => a.user ?? "", cell: ({ row }) => who(row.original.user) },
    { id: "project", header: "프로젝트", accessorFn: (a) => a.project ?? "", cell: ({ row }) => <code className={cn(mono, "truncate")} title={row.original.project ?? ""}>{row.original.project ?? "–"}</code> },
    { id: "last", header: "마지막으로 보임", size: 128, accessorFn: (a) => a.last_seen ?? 0, cell: ({ row }) => when(row.original.last_seen, view.now) },
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
      <Button variant="outline" size="sm" onClick={() => setAsk({ kind: "admin" })}>
        <PlugZap />관리 API
        <i className={cn("size-1.5 rounded-full", d.source.demo ? "bg-catch" : d.admin.has_token ? "bg-allow" : "bg-white/25")} />
        <span className="text-muted-foreground">{d.source.demo ? "데모" : d.admin.has_token ? "연결됨" : "연결 안 됨"}</span>
      </Button>
    </PageHeader>
  );
  const dialogs = ask && (
    <Asks key={ask.kind + ("email" in ask ? ask.email : "")} ask={ask} actor={d.actor} admin={d.admin} demo={d.source.demo}
      busy={pending !== null} error={error} onSend={request} onCopy={(text) => send("inventory", { type: "copy", text })} onClose={closeAsk} />
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
                : <div className="flex flex-wrap justify-center gap-1.5"><Chip>기기 등록</Chip><Chip>사용자 키</Chip><Chip>PostgreSQL 엔진</Chip></div>}
            </div>
          </div>
        )}
        {dialogs}
      </div>
    );
  }

  const picked = open?.kind === "device" ? open.id : null;
  const machine = open?.kind === "device" ? view.machines.find((m) => m.id === open.id) : undefined;
  const person = open?.kind === "person" ? view.people.find((p) => p.email === open.email) : undefined;
  const server = open?.kind === "server" || open?.kind === "tool" ? servers.find((s) => s.name === (open.kind === "server" ? open.name : open.server)) : undefined;
  const tool = open?.kind === "tool" ? server?.tools.find((t) => t.name === open.tool) : undefined;
  const openServer = (name: string) => setOpen({ kind: "server", name });
  const openTool = (s: string, t: string) => setOpen({ kind: "tool", server: s, tool: t });
  const openDevice = (id: string) => setOpen({ kind: "device", id });
  const chip = filter && PERM[filter.value].label;
  const owner = holder ?? everyone.find((m) => m.user)?.user ?? null;
  const scoped = <T,>(list: T[], of: (t: T) => boolean) => (holder ? list.filter(of).length : list.length);
  const counts: Record<Tab, number> = {
    devices: base.length, people: scoped(view.people, (p) => p.email === holder),
    sessions: scoped(view.sessions, (r) => ofHolder(r.user, r.device)), shadow: scoped(view.shadow, (a) => ofHolder(a.user, a.device)), mcp: listed.length,
  };
  const shownCount = { devices: machines.length, people: people.length, sessions: sessions.length, shadow: shadow.length, mcp: listed.filter((s) => s.tools.some((t) => lit(s, t))).length }[tab];

  return (
    <div className="flex h-full flex-col">
      {header}
      <div className="scroll flex min-h-0 flex-1 flex-col gap-4 overflow-y-auto px-6 pb-6">
        <div className="grid shrink-0 grid-cols-12 gap-4">
          <Panel className="col-span-8 min-w-0" title="사용자 · 기기별 에이전트와 MCP" aside={<FleetLegend />}>
            <div className="h-[360px]">
              <Fleet view={view} machines={everyone} servers={servers} picked={picked} onDevice={openDevice} onServer={openServer} />
            </div>
          </Panel>
          <Panel className="col-span-4 min-w-0" title="인벤토리">
            <div className="h-[360px]">
              <Bag view={view} who={owner} onWho={setHolder}
                onIssue={(email) => setAsk({ kind: "issue", email })} onStop={(email, keys) => setAsk({ kind: "stop", email, keys })}
                onDevice={openDevice} onServer={openServer} onPerson={(email) => setOpen({ kind: "person", email })}
                onSessions={(email, mode) => { setQ(shortUser(email)); setFilter(mode ? { kind: "perm", value: mode } : null); setTab("sessions"); }} />
            </div>
          </Panel>
        </div>

        <Card ref={lists} className={cn("flex min-h-[300px] flex-col gap-0 py-0 shadow-none", tab === "mcp" ? "shrink-0 overflow-clip" : "flex-1 overflow-hidden")}>
          <div className={cn("flex h-12 shrink-0 items-center gap-3 border-b px-4", tab === "mcp" && "sticky top-0 z-20 bg-card")}>
            <Tabs value={tab} onValueChange={(v) => setTab(v as Tab)}>
              <TabsList>
                {TABS.map(([key, label]) => (
                  <TabsTrigger key={key} value={key} className="gap-1.5">
                    {label}<span className={cn("tnum rounded-full bg-secondary px-1.5 text-[11px] font-normal text-muted-foreground", key === "shadow" && counts.shadow > 0 && "bg-deny/15 text-deny")}>{counts[key]}</span>
                  </TabsTrigger>
                ))}
              </TabsList>
            </Tabs>
            {holder && (
              <Badge variant="secondary" className="h-6 gap-1.5 border-0 bg-primary/15 pr-1 font-medium text-primary">
                <Person name={holder} size={16} />{shortUser(holder)}
                <button aria-label="사용자 지우기" onClick={() => setHolder(null)} className="grid size-4 place-items-center rounded-full hover:bg-primary/20"><X className="size-3" /></button>
              </Badge>
            )}
            {chip && (
              <Badge variant="secondary" className="h-6 gap-1 border-0 bg-primary/15 pr-1 font-medium text-primary">
                {chip}<span className="tnum opacity-70">{shownCount}</span>
                <button aria-label="필터 지우기" onClick={() => setFilter(null)} className="grid size-4 place-items-center rounded-full hover:bg-primary/20"><X className="size-3" /></button>
              </Badge>
            )}
            {tab === "shadow" && !view.shadowKnown && <Chip>미수집</Chip>}
            {tab === "mcp" && !view.mcp && <Chip>미수집</Chip>}
            <div className="ml-auto flex items-center gap-3">
              {tab === "mcp" && servers.length > 0 && (
                <div className="flex items-center gap-1.5">
                  {NATURES.map((n) => (
                    <button key={n} aria-pressed={nature === n} onClick={() => setNature(nature === n ? null : n)}
                      className={cn("inline-flex h-7 shrink-0 items-center gap-1.5 rounded-full border px-2.5 text-[12.5px] transition-colors", nature === n ? "border-primary bg-primary/15 text-foreground" : "text-muted-foreground hover:border-white/25 hover:text-foreground")}>
                      <NatureIcon n={n} className="size-3.5" />{NATURE[n].label}
                      <span className="tnum text-[11.5px] opacity-70">{listed.reduce((a, s) => a + s.tools.filter((t) => t.nature === n).length, 0)}</span>
                    </button>
                  ))}
                </div>
              )}
              {tab === "devices" && view.unregistered > 0 && (
                <label className="flex cursor-pointer items-center gap-2 text-[12.5px] text-muted-foreground select-none hover:text-foreground">
                  <Checkbox checked={all} onCheckedChange={(v) => setAll(v === true)} />등록 안 된 ID <span className="tnum">{view.unregistered}</span>
                </label>
              )}
              {tab === "people" && <Button size="sm" variant="secondary" onClick={() => setAsk({ kind: "issue", email: "" })}><KeyRound />키 발급</Button>}
              <div className="relative">
                <Search className="pointer-events-none absolute top-1/2 left-2.5 size-3.5 -translate-y-1/2 text-muted-foreground" />
                <Input value={q} onChange={(e) => setQ(e.target.value)} placeholder="검색" className="h-7 w-48 pl-8 text-[12.5px]" />
              </div>
            </div>
          </div>
          <div className="min-h-0 flex-1 text-[13px] [&_[data-slot=table-container]]:h-full [&_[data-slot=table-container]]:overflow-auto [&_[data-slot=table-container]]:[scrollbar-color:#4a4a50_transparent] [&_[data-slot=table-container]]:[scrollbar-width:thin] [&_td:first-child]:pl-4 [&_th:first-child]:pl-4">
            {tab === "devices" && <DataTable key="devices" columns={deviceCols} rows={machines} onRow={(m) => setOpen({ kind: "device", id: m.id })} picked={(m) => m.id === picked} />}
            {tab === "people" && <DataTable key="people" columns={peopleCols} rows={people} onRow={(p) => setOpen({ kind: "person", email: p.email })} picked={(p) => open?.kind === "person" && p.email === open.email} />}
            {tab === "sessions" && <DataTable key="sessions" columns={sessionCols} rows={sessions.slice(0, 300)} onRow={(r) => { if (r.opens) openSession(r.id); else if (r.device) setOpen({ kind: "device", id: r.device }); }} />}
            {tab === "shadow" && <DataTable key="shadow" columns={shadowCols} rows={shadow} onRow={(a) => { if (a.device) setOpen({ kind: "device", id: a.device }); }} />}
            {tab === "mcp" && (view.mcp && servers.length > 0 ? (
              <>
                <Band mcp={view.mcp} now={view.now} nature={nature} onNature={(n) => setNature(nature === n ? null : n)} />
                <Board servers={listed} lit={lit} filtered={sifted} now={view.now}
                  picked={open?.kind === "tool" ? toolKey(open.server, open.tool) : null} pickedServer={open?.kind === "server" ? open.name : null}
                  onTool={openTool} onServer={openServer} />
              </>
            ) : <Empty />)}
          </div>
        </Card>
      </div>

      <Sheet open={!!(machine || person || server)} onOpenChange={(o) => { if (!o && !ask) setOpen(null); }}>
        <SheetContent aria-describedby={undefined} onOpenAutoFocus={(e) => e.preventDefault()} className="w-[480px] gap-0 data-[side=right]:sm:max-w-[480px]">
          {machine && <DeviceSheet m={machine} view={view} onPerson={(email) => setOpen({ kind: "person", email })} onSession={openSession} onServer={openServer} onAsk={setAsk}
            onOwner={(email) => request({ type: "owner", device: machine.id, email: email ?? "", actor: d.actor })} />}
          {person && <PersonSheet p={person} view={view} onDevice={openDevice} onAsk={setAsk} />}
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
