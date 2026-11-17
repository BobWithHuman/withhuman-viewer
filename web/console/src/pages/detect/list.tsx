// What the team has to do now, by what it calls for: the cases where something already ran (a
// leak, a held Call a person let run, one allowed though hard to undo) to answer for; the cases
// the engine stopped, to judge; and the machines to fix. A row opens its detail at the right,
// from where the Call opens in 기록.

import { type ColumnDef } from "@tanstack/react-table";
import { Download, Search } from "lucide-react";
import { useMemo, useRef, useState, type ReactNode } from "react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Skeleton } from "@/components/ui/skeleton";
import { Sheet, SheetContent } from "@/components/ui/sheet";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { DataTable, Panel, clock } from "@/kit";
import { cn } from "@/lib/utils";
import { CaseView } from "./case";
import { type Alert, type Blocked, type Call, type Case, type CaseGroup, DIM, type Group, MARK, type Mark, SEV, type Sev, cases, happened, isOpen, named, stateOf, toolName, worst } from "./data";

/** What the page asks the app to do. */
export type Ask = (message:
  | { do: "open" | "case"; session: string; seq: number }
  | { do: "triage"; sessions: string[]; state: string | null }
  | { do: "source"; source: string; on: boolean; session: string; seq: number }
  | { do: "stop_device"; device: string; session: string; seq: number }
  | { do: "inventory" }
  | { do: "export"; name: string; text: string }
  | { do: "settings" }) => void;

type View = "ran" | "held" | "devices";
const NO_PROMPT = "프롬프트 없음";
/** How many rows a view lists; the newest or the most come first. */
const ROWS = 300;

export function SevBadge({ s }: { s: Sev }) {
  return <Badge variant="secondary" className={cn("border-0 font-semibold", SEV[s].cls)}>{SEV[s].label}</Badge>;
}
const Mono = ({ children }: { children: ReactNode }) => <span className="font-mono text-[12px]">{children}</span>;
const When = ({ at }: { at: number | null }) => <span className="tnum text-[12px] text-muted-foreground">{clock(at)}</span>;

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
const RAN = {
  leak: { label: "유출", cls: "bg-deny/15 text-deny" },
  approved: { label: "사용자 승인", cls: "bg-chart-1/15 text-chart-1" },
  allowed: { label: "고위험 허용", cls: "bg-review/15 text-review" },
} as const;
type Ran = keyof typeof RAN;
/** Why a case calls for an answer: what ran in it. */
const whyRan = (g: CaseGroup): Ran[] => {
  const did = g.calls.filter(happened);
  return ([
    g.leaked.length > 0 && "leak",
    did.some((c) => !c.leak && c.verdict === "review") && "approved",
    did.some((c) => !c.leak && c.verdict !== "review") && "allowed",
  ] as const).filter((w): w is Ran => !!w);
};
const caseColumns = (marks: Record<string, Mark>, ran: boolean): ColumnDef<CaseGroup>[] => [
  { id: "mark", header: "처리", size: 92, accessorFn: (g) => stateOf(g, marks) ?? "", cell: ({ row }) => {
    const st = stateOf(row.original, marks);
    if (st === "mixed") return <span className="rounded bg-white/8 px-1.5 text-[11.5px] leading-[20px] font-semibold text-foreground/80">일부</span>;
    return st ? <span className={cn("rounded px-1.5 text-[11.5px] leading-[20px] font-semibold", MARK[st].cls)}>{MARK[st].label}</span> : <span className="inline-flex items-center gap-1.5 text-[12px] text-muted-foreground"><i className="size-1.5 rounded-full bg-primary" />미처리</span>;
  } },
  // What already ran in it, worst first: a value sent out, a held Call a person let run, one
  // allowed though risky. Only in the cases where something ran.
  ...(ran ? [{ id: "why", header: "실행됨", size: 170, accessorFn: (g: CaseGroup) => whyRan(g).join(" "), cell: ({ row }: { row: { original: CaseGroup } }) => (
    <span className="flex min-w-0 items-center gap-1">
      {whyRan(row.original).map((w) => <span key={w} className={cn("shrink-0 rounded px-1.5 text-[11.5px] leading-[20px] font-semibold", RAN[w].cls)} title={w === "leak" ? row.original.leaked.join(", ") : undefined}>{RAN[w].label}</span>)}
    </span>
  ) } as ColumnDef<CaseGroup>] : []),
  { id: "sev", header: "심각도", size: 88, accessorFn: (g) => g.risk, cell: ({ row }) => <SevBadge s={worst(row.original)} /> },
  { id: "case", header: "사건", accessorFn: (g) => g.cause ?? named(g.calls[0]) ?? "", cell: ({ row }) => {
    const g = row.original;
    return g.cause
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
  const [view, setView] = useState<View>("ran");
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
  const listed = useMemo(() => (every ? allCases : allCases.filter((g) => g.sessions.some((s) => isOpen(marks[s])))), [allCases, every, marks]);
  // Those where something already ran, to answer for; those the engine stopped, to judge.
  const done = useMemo(() => listed.filter((g) => g.ran > 0), [listed]);
  const held = useMemo(() => listed.filter((g) => g.ran === 0), [listed]);
  const sessions = view === "held" ? held : done;
  const columns = useMemo(() => caseColumns(marks, view !== "held"), [marks, view]);
  const same = (c: Call) => !!picked && c.session === picked.session && c.seq === picked.seq;
  // The Call open as last seen, kept while a search or a period hides it from the list.
  const last = useRef<Call | null>(null);
  const call = picked ? newest.find(same) ?? (last.current && same(last.current) ? last.current : null) : null;
  last.current = call;
  const group = call ? allCases.find((g) => g.calls.some(same)) ?? cases([call])[0] : undefined;
  const open = (c: Call) => { setPicked({ session: c.session, seq: c.seq }); ask({ do: "case", session: c.session, seq: c.seq }); };
  // What is listed now, as a file: the cases with their Calls.
  const exported = () => ask({
    do: "export", name: `withhuman-${view}-${new Date().toISOString().slice(0, 10)}.json`,
    text: JSON.stringify(sessions.map((g) => ({ cause: g.cause, state: stateOf(g, marks), sessions: g.sessions.map((s) => ({ session: s, mark: marks[s] ?? null })), machines: g.machines, already_ran: g.ran, calls: g.calls })), null, 2),
  });

  const tab = (v: View, label: string, n: number) => (
    <TabsTrigger value={v} className="gap-1.5 px-2.5 text-[13px]">{label}<span className="tnum text-[11.5px] text-muted-foreground">{n.toLocaleString()}</span></TabsTrigger>
  );
  return (
    <Panel
      pad={false} className={cn("min-h-0 overflow-hidden [&>[data-slot=card-content]]:flex [&>[data-slot=card-content]]:min-h-0 [&>[data-slot=card-content]]:flex-1 [&>[data-slot=card-content]]:flex-col", className)}
      title={<Tabs value={view} onValueChange={(v) => setView(v as View)}><TabsList>{tab("ran", "이미 실행됨", done.length)}{tab("held", "막힘", held.length)}{tab("devices", "기기", alerts.length)}</TabsList></Tabs>}
      aside={<>
        {saved && <span className="max-w-[320px] truncate text-[12px]" title={saved}>{saved}</span>}
        {view !== "devices" && (
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
        {loading && <div className="flex flex-col gap-2 p-4">{Array.from({ length: 8 }, (_, i) => <Skeleton key={i} className="h-8 rounded-md" />)}</div>}
        {!loading && view !== "devices" && <DataTable columns={columns} rows={sessions.slice(0, ROWS)} onRow={(g) => open(first(g))} picked={(g) => g.calls.some(same)} dense />}
        {!loading && view === "devices" && <DataTable columns={DEVICES} rows={alerts} onRow={() => ask({ do: "inventory" })} dense />}
      </div>

      <Sheet open={!!picked} onOpenChange={(on) => !on && setPicked(null)}>
        <SheetContent className="gap-0 data-[side=right]:w-[min(1040px,92vw)] data-[side=right]:sm:max-w-[min(1040px,92vw)]" onOpenAutoFocus={(e) => e.preventDefault()}>
          {call && group && <CaseView call={call} group={group} data={caseData} marks={marks} blocked={blocked} ask={ask} onCall={open} />}
        </SheetContent>
      </Sheet>
    </Panel>
  );
}
