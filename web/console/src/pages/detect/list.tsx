// What to look at now, as one list seen three ways: the sessions with Calls worth a look, the
// riskiest first, each with its machine, its tools and what was stopped or held in it; those
// Calls one by one; and the Calls a person let run after the engine asked. A row opens its
// detail at the right, from where the Call opens in 기록 or in 감사's approvals.

import { type ColumnDef } from "@tanstack/react-table";
import { Download, Search, Zap } from "lucide-react";
import { useMemo, useRef, useState, type ReactNode } from "react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Skeleton } from "@/components/ui/skeleton";
import { Sheet, SheetContent } from "@/components/ui/sheet";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { DataTable, Panel, VerdictBadge, Warn, clock } from "@/kit";
import { cn } from "@/lib/utils";
import { CaseView } from "./case";
import { ACTION, type Alert, type Blocked, type Call, type Case, type CaseGroup, DIM, type Group, MARK, type Mark, SEV, SEVS, type Sev, cases, isOpen, named, stateOf, toolName, worst } from "./data";

/** What the page asks the app to do. */
export type Ask = (message:
  | { do: "open" | "case"; session: string; seq: number }
  | { do: "triage"; sessions: string[]; state: string | null }
  | { do: "source"; source: string; on: boolean; session: string; seq: number }
  | { do: "stop_device"; device: string; session: string; seq: number }
  | { do: "inventory" }
  | { do: "export"; name: string; text: string }
  | { do: "settings" }) => void;

type View = "sessions" | "calls" | "approved" | "devices";
const NO_PROMPT = "프롬프트 없음";
/** How many rows a view lists; the newest or the most come first. */
const ROWS = 300;

export function SevBadge({ s }: { s: Sev }) {
  return <Badge variant="secondary" className={cn("border-0 font-semibold", SEV[s].cls)}>{SEV[s].label}</Badge>;
}
const Mono = ({ children }: { children: ReactNode }) => <span className="font-mono text-[12px]">{children}</span>;
const What = ({ c }: { c: Call }) => (
  <span className="flex min-w-0 items-center gap-2">
    {c.tool && <span className="max-w-32 shrink-0 truncate rounded-md bg-secondary px-1.5 py-0.5 text-[11px] text-muted-foreground">{toolName(c.tool)}</span>}
    <code className="min-w-0 truncate font-mono text-[12px]">{c.target ?? c.tool}</code>
  </span>
);
const When = ({ at }: { at: number | null }) => <span className="tnum text-[12px] text-muted-foreground">{clock(at)}</span>;

/** A Call's verdict, and for one held whether the person let it run. */
const Verdict = ({ c }: { c: Call }) => <VerdictBadge v={c.verdict}>{c.verdict === "review" ? (c.ran ? "확인 · 승인" : "확인 · 반려") : c.verdict === "allow" ? "허용" : undefined}</VerdictBadge>;
/** How many of some Calls were stopped, held and allowed though hard to undo. */
function Verdicts({ calls }: { calls: Call[] }) {
  const n = (v: string) => calls.filter((c) => c.verdict === v).length;
  return (
    <span className="tnum flex items-center gap-2.5 text-[12px]">
      {([["deny", "차단", "text-deny"], ["review", "확인", "text-review"], ["allow", "허용", "text-allow"]] as const).map(([v, label, cls]) => (
        <span key={v} className={cn("inline-flex items-center gap-1", n(v) ? cls : "opacity-25")}><i className="size-1.5 rounded-full bg-current" />{label} {n(v)}</span>
      ))}
    </span>
  );
}
/** The tools some Calls used, the most used first. */
function Tools({ calls }: { calls: Call[] }) {
  const n = new Map<string, number>();
  for (const c of calls) if (c.tool) n.set(c.tool, (n.get(c.tool) ?? 0) + 1);
  const list = [...n.entries()].sort((a, b) => b[1] - a[1]);
  return (
    <span className="flex min-w-0 items-center gap-1 overflow-hidden" title={list.map(([t, c]) => `${toolName(t)} ${c}`).join(" · ")}>
      {list.slice(0, 2).map(([t, c]) => <span key={t} className="max-w-28 shrink-0 truncate rounded-md bg-secondary px-1.5 py-0.5 text-[11px] text-foreground/85">{toolName(t)}{c > 1 && <b className="tnum ml-1 font-medium">{c}</b>}</span>)}
      {list.length > 2 && <span className="tnum shrink-0 text-[11.5px] text-muted-foreground">+{list.length - 2}</span>}
    </span>
  );
}
/** The worst Call of a session, the newest of those. */
const first = (g: Group) => { const w = worst(g); return g.calls.find((c) => c.sev === w) ?? g.calls[0]; };

/** A case: how it stands, whether something already ran, how bad, what caused it (or the
 * request, when no cause is known), how far it reached, and its worst Call. */
const caseColumns = (marks: Record<string, Mark>): ColumnDef<CaseGroup>[] => [
  { id: "mark", header: "처리", size: 92, accessorFn: (g) => stateOf(g, marks) ?? "", cell: ({ row }) => {
    const st = stateOf(row.original, marks);
    if (st === "mixed") return <span className="rounded bg-white/8 px-1.5 text-[11.5px] leading-[20px] font-semibold text-foreground/80">일부</span>;
    return st ? <span className={cn("rounded px-1.5 text-[11.5px] leading-[20px] font-semibold", MARK[st].cls)}>{MARK[st].label}</span> : <span className="inline-flex items-center gap-1.5 text-[12px] text-muted-foreground"><i className="size-1.5 rounded-full bg-primary" />미처리</span>;
  } },
  { id: "ran", header: "실행", size: 88, accessorFn: (g) => g.ran, cell: ({ row }) => (row.original.ran
    ? <span className="tnum inline-flex items-center gap-1 rounded bg-deny/15 px-1.5 text-[11.5px] leading-[20px] font-semibold text-deny"><Zap className="size-3" />실행됨 {row.original.ran}</span>
    : <span className="text-[12px] text-muted-foreground">막음</span>) },
  { id: "sev", header: "심각도", size: 88, accessorFn: (g) => g.risk, cell: ({ row }) => <SevBadge s={worst(row.original)} /> },
  { id: "case", header: "사건", accessorFn: (g) => g.cause ?? named(g.calls[0]) ?? "", cell: ({ row }) => {
    const g = row.original;
    const leaked = g.leaked.length > 0 && (
      <span className="shrink-0 rounded bg-deny/15 px-1.5 text-[11px] leading-[19px] font-semibold text-deny" title={g.leaked.join(", ")}>유출 · {g.leaked[0]}{g.leaked.length > 1 ? ` +${g.leaked.length - 1}` : ""}</span>
    );
    return leaked ? <span className="flex min-w-0 items-center gap-2">{leaked}<span className="truncate">{g.cause ?? named(g.calls[0]) ?? NO_PROMPT}</span></span> : g.cause
      ? <span className="flex min-w-0 items-center gap-2" title={g.cause}><span className="shrink-0 rounded bg-catch/15 px-1.5 text-[11px] leading-[19px] font-medium text-catch">원인</span><Mono>{g.cause.length > 80 ? `${g.cause.slice(0, 80)}…` : g.cause}</Mono></span>
      : <span className={cn("block truncate", !named(g.calls[0]) && "text-muted-foreground")} title={g.calls[0].prompt ?? undefined}>{named(g.calls[0]) ?? NO_PROMPT}</span>;
  } },
  { id: "reach", header: "범위", size: 150, accessorFn: (g) => g.sessions.length * 1000 + g.machines.length, cell: ({ row }) => {
    const g = row.original;
    return g.sessions.length > 1
      ? <span className="tnum font-medium whitespace-nowrap">세션 {g.sessions.length} · 단말 {g.machines.length}</span>
      : <span className="block truncate font-mono text-[12px] text-muted-foreground">{g.machines[0]}</span>;
  } },
  { id: "tools", header: "도구", size: 180, accessorFn: (g) => g.calls[0].tool ?? "", cell: ({ row }) => <Tools calls={row.original.calls} /> },
  { id: "found", header: "탐지", size: 200, accessorFn: (g) => g.total, cell: ({ row }) => <Verdicts calls={row.original.calls} /> },
  { id: "target", header: "가장 심각한 호출", accessorFn: (g) => first(g).target ?? "", cell: ({ row }) => <code className="block truncate font-mono text-[12px] text-muted-foreground">{first(row.original).target ?? first(row.original).tool}</code> },
  { id: "at", header: "최근", size: 104, accessorFn: (g) => g.calls[0].at ?? 0, cell: ({ row }) => <When at={row.original.calls[0].at} /> },
];
const CALLS: ColumnDef<Call>[] = [
  { id: "sev", header: "심각도", size: 96, accessorFn: (c) => -SEVS.indexOf(c.sev), cell: ({ row }) => <SevBadge s={row.original.sev} /> },
  { id: "verdict", header: "판정", size: 116, accessorFn: (c) => c.verdict, cell: ({ row }) => <Verdict c={row.original} /> },
  { id: "target", header: "호출", accessorFn: (c) => c.target ?? "", cell: ({ row }) => <What c={row.original} /> },
  { id: "action", header: "행동", size: 132, accessorFn: (c) => ACTION[c.action]?.label ?? c.action, cell: ({ getValue }) => <span className="text-muted-foreground">{getValue<string>()}</span> },
  { id: "kind", header: "위반 유형", size: 196, accessorFn: (c) => DIM.kind.name(DIM.kind.of(c)) },
  { id: "source", header: "유발 원인", size: 100, accessorFn: (c) => DIM.source.name(c.source), cell: ({ getValue }) => <span className="text-muted-foreground">{getValue<string>()}</span> },
  { id: "machine", header: "단말", size: 152, accessorKey: "machine", cell: ({ row }) => <Mono>{row.original.machine}</Mono> },
  { id: "at", header: "시각", size: 104, accessorFn: (c) => c.at ?? 0, cell: ({ row }) => <When at={row.original.at} /> },
];
const APPROVED: ColumnDef<Call>[] = [
  { id: "hard", header: "되돌리기", size: 132, accessorFn: (c) => (c.hard ? 1 : 0), cell: ({ row }) => (row.original.hard ? <Warn>어려움</Warn> : <span className="text-muted-foreground">가능</span>) },
  { id: "target", header: "승인한 호출", accessorFn: (c) => c.target ?? "", cell: ({ row }) => <What c={row.original} /> },
  { id: "kind", header: "유형", size: 208, accessorFn: (c) => DIM.kind.name(DIM.kind.of(c)) },
  { id: "machine", header: "단말", size: 152, accessorKey: "machine", cell: ({ row }) => <Mono>{row.original.machine}</Mono> },
  { id: "at", header: "시각", size: 104, accessorFn: (c) => c.at ?? 0, cell: ({ row }) => <When at={row.original.at} /> },
];
/** A machine to tell the team about, with why. */
export type Told = Alert & { why: "new" | "silent" | "installed" | "unregistered" | "undecided" };
const WHY: Record<Told["why"], { label: string; cls: string }> = {
  new: { label: "새 기기", cls: "bg-info/15 text-info" },
  silent: { label: "연결 끊김", cls: "bg-review/15 text-review" },
  installed: { label: "설치만 됨", cls: "bg-catch/15 text-catch" },
  unregistered: { label: "등록 안 됨", cls: "bg-white/8 text-foreground/80" },
  undecided: { label: "판정 없는 호출", cls: "bg-deny/15 text-deny" },
};
const DEVICES: ColumnDef<Told>[] = [
  { id: "why", header: "알림", size: 110, accessorFn: (a) => a.why, cell: ({ row }) => <span className={cn("rounded px-1.5 text-[11.5px] leading-[20px] font-semibold", WHY[row.original.why].cls)}>{WHY[row.original.why].label}</span> },
  { id: "host", header: "기기", accessorFn: (a) => a.host ?? a.id, cell: ({ row }) => <span className="flex min-w-0 items-baseline gap-2"><Mono>{row.original.host ?? row.original.id}</Mono>{row.original.host && <span className="truncate font-mono text-[11px] text-muted-foreground">{row.original.id}</span>}</span> },
  { id: "edr", header: "EDR", size: 120, accessorFn: (a) => a.edr ?? "", cell: ({ row }) => <span className="text-muted-foreground">{row.original.edr === "active" ? "연결됨" : row.original.edr === "disconnected" ? "끊김" : row.original.edr ?? "–"}</span> },
  { id: "at", header: "시각", size: 104, accessorFn: (a) => a.at ?? 0, cell: ({ row }) => <When at={row.original.at} /> },
];

export function Watch({ calls, ask, loading, marks, caseData, saved, blocked, alerts, className }: {
  calls: Call[]; ask: Ask; loading?: boolean; className?: string;
  /** The sources the team distrusts, and the machines to tell it about. */
  blocked: Blocked[]; alerts: Told[];
  /** What the team marked each session as, the case the app read, and what the last save said. */
  marks: Record<string, Mark>; caseData?: Case | null; saved?: string | null;
}) {
  const [view, setView] = useState<View>("sessions");
  const [query, setQuery] = useState("");
  // The Call open, by its session and number: the page's data is sent again (when the case
  // arrives, a mark changes), and the Calls in it are new objects each time.
  const [picked, setPicked] = useState<{ session: string; seq: number } | null>(null);
  // Whether the sessions already marked are listed too.
  const [every, setEvery] = useState(false);

  const found = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return calls;
    return calls.filter((c) => [c.target, c.title, c.prompt, c.machine, c.tool, c.folder, c.reason, DIM.kind.name(DIM.kind.of(c))].some((t) => t?.toLowerCase().includes(q)));
  }, [calls, query]);
  const newest = useMemo(() => [...found].sort((a, b) => (b.at ?? 0) - (a.at ?? 0) || b.seq - a.seq), [found]);
  // The sessions, the riskiest first; each one's Calls stay the newest first.
  // The cases: the Calls one cause made, over every session it reached. One still to handle
  // has a session not marked yet, or being looked at.
  const allCases = useMemo(() => cases(newest), [newest]);
  const sessions = useMemo(() => (every ? allCases : allCases.filter((g) => g.sessions.some((s) => isOpen(marks[s])))), [allCases, every, marks]);
  const columns = useMemo(() => caseColumns(marks), [marks]);
  const same = (c: Call) => !!picked && c.session === picked.session && c.seq === picked.seq;
  // The Call open as last seen, kept while a search or a period hides it from the list.
  const last = useRef<Call | null>(null);
  const call = picked ? newest.find(same) ?? (last.current && same(last.current) ? last.current : null) : null;
  last.current = call;
  const group = call ? allCases.find((g) => g.calls.some(same)) ?? cases([call])[0] : undefined;
  const open = (c: Call) => { setPicked({ session: c.session, seq: c.seq }); ask({ do: "case", session: c.session, seq: c.seq }); };
  // What is listed now, as a file: the sessions with their Calls, or the Calls alone.
  const exported = () => ask({
    do: "export", name: `withhuman-${view}-${new Date().toISOString().slice(0, 10)}.json`,
    text: JSON.stringify(view === "sessions"
      ? sessions.map((g) => ({ cause: g.cause, state: stateOf(g, marks), sessions: g.sessions.map((s) => ({ session: s, mark: marks[s] ?? null })), machines: g.machines, already_ran: g.ran, calls: g.calls }))
      : view === "calls" ? newest : approved, null, 2),
  });
  // The hardest to undo first, then the newest.
  // Those in a session already marked are left out with the marked sessions.
  const approved = useMemo(() => newest.filter((c) => c.verdict === "review" && c.ran && (every || isOpen(marks[c.session]))).sort((a, b) => Number(b.hard) - Number(a.hard)), [newest, every, marks]);

  const tab = (v: View, label: string, n: number) => (
    <TabsTrigger value={v} className="gap-1.5 px-2.5 text-[13px]">{label}<span className="tnum text-[11.5px] text-muted-foreground">{n.toLocaleString()}</span></TabsTrigger>
  );
  return (
    <Panel
      pad={false} className={cn("min-h-0 overflow-hidden [&>[data-slot=card-content]]:flex [&>[data-slot=card-content]]:min-h-0 [&>[data-slot=card-content]]:flex-1 [&>[data-slot=card-content]]:flex-col", className)}
      title={<Tabs value={view} onValueChange={(v) => setView(v as View)}><TabsList>{tab("sessions", every ? "사건" : "처리 대기", sessions.length)}{tab("calls", "호출", newest.length)}{tab("approved", "사용자 승인", approved.length)}{tab("devices", "기기 알림", alerts.length)}</TabsList></Tabs>}
      aside={<>
        {saved && <span className="max-w-[320px] truncate text-[12px]" title={saved}>{saved}</span>}
        {(view === "sessions" || view === "approved") && (
          <ToggleGroup type="single" size="sm" value={every ? "all" : "open"} onValueChange={(v) => v && setEvery(v === "all")}>
            <ToggleGroupItem value="open" className="px-2.5 text-[12.5px]">미처리</ToggleGroupItem>
            <ToggleGroupItem value="all" className="px-2.5 text-[12.5px]">전체</ToggleGroupItem>
          </ToggleGroup>
        )}
        {view === "devices"
          ? <Button variant="outline" size="sm" onClick={() => ask({ do: "inventory" })}>인벤토리</Button>
          : <Button variant="outline" size="sm" onClick={exported}><Download />내보내기</Button>}
        <span className="relative">
          <Search className="pointer-events-none absolute top-1/2 left-2 size-3.5 -translate-y-1/2 text-muted-foreground" />
          <Input value={query} onChange={(e) => setQuery(e.target.value)} placeholder="검색" className="h-7 w-52 pl-7 text-[12.5px] md:text-[12.5px]" />
        </span>
      </>}
    >
      <div className="min-h-0 flex-1 border-t [&_[data-slot=table-container]]:scroll [&_[data-slot=table-container]]:h-full [&_[data-slot=table-container]]:overflow-y-auto [&_td:first-child]:pl-4 [&_td:last-child]:pr-4 [&_th:first-child]:pl-4 [&_th:last-child]:pr-4">
        {!loading && view === "calls" && <DataTable columns={CALLS} rows={newest.slice(0, ROWS)} onRow={open} picked={same} dense />}
        {loading && <div className="flex flex-col gap-2 p-4">{Array.from({ length: 8 }, (_, i) => <Skeleton key={i} className="h-8 rounded-md" />)}</div>}
        {!loading && view === "sessions" && <DataTable columns={columns} rows={sessions.slice(0, ROWS)} onRow={(g) => open(first(g))} picked={(g) => g.calls.some(same)} dense />}
        {!loading && view === "devices" && <DataTable columns={DEVICES} rows={alerts} onRow={() => ask({ do: "inventory" })} dense />}
        {!loading && view === "approved" && <DataTable columns={APPROVED} rows={approved.slice(0, ROWS)} onRow={open} picked={same} dense />}
      </div>

      <Sheet open={!!picked} onOpenChange={(on) => !on && setPicked(null)}>
        <SheetContent className="gap-0 data-[side=right]:w-[min(1040px,92vw)] data-[side=right]:sm:max-w-[min(1040px,92vw)]" onOpenAutoFocus={(e) => e.preventDefault()}>
          {call && group && <CaseView call={call} group={group} data={caseData} marks={marks} blocked={blocked} ask={ask} onCall={open} />}
        </SheetContent>
      </Sheet>
    </Panel>
  );
}
