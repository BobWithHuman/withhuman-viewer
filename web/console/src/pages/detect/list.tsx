// What the team has to do now, by what it calls for: the cases where something already ran (a
// leak, a held Call a person let run, one allowed though hard to undo) to answer for; the cases
// the engine stopped, to judge; and the machines to fix. A row opens its detail at the right,
// from where the Call opens in 기록.

import { type ColumnDef } from "@tanstack/react-table";
import { Download, Search } from "lucide-react";
import { useMemo, useRef, useState } from "react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Skeleton } from "@/components/ui/skeleton";
import { Sheet, SheetContent } from "@/components/ui/sheet";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { DataTable, Panel, ServiceLogo, clock } from "@/kit";
import { cn } from "@/lib/utils";
import { CaseView } from "./case";
import { CAUSE, type Blocked, type Call, type Case, type CaseGroup, DIM, type Group, MARK, type Mark, SEV, type Sev, SEVS, cases, happened, isOpen, named, stateOf, toolName, worst } from "./data";

/** What the page asks the app to do. */
export type Ask = (message:
  | { do: "open" | "case"; session: string; seq: number }
  | { do: "triage"; sessions: string[]; state: string | null }
  | { do: "source"; source: string; on: boolean; session: string; seq: number }
  | { do: "stop_device"; device: string; reason?: string; session: string; seq: number }
  | { do: "inventory" }
  | { do: "export"; name: string; text: string }
  | { do: "settings" }) => void;

type View = "ran" | "held" | "sources";
const NO_PROMPT = "프롬프트 없음";
/** How many rows a view lists; the newest or the most come first. */
const ROWS = 300;

export function SevBadge({ s }: { s: Sev }) {
  return <Badge variant="secondary" className={cn("border-0 font-semibold", SEV[s].cls)}>{SEV[s].label}</Badge>;
}
const When = ({ at }: { at: number | null }) => <span className="tnum text-[12px] text-muted-foreground">{clock(at)}</span>;

/** How many of some Calls were stopped, held and allowed though hard to undo. */
function Verdicts({ calls }: { calls: Call[] }) {
  const n = (v: string) => calls.filter((c) => c.verdict === v).length;
  return (
    <span className="tnum flex items-center gap-3 text-[12px]">
      {([["deny", "차단", "text-deny"], ["review", "확인", "text-review"], ["allow", "허용", "text-allow"]] as const).map(([v, label, cls]) => (
        <span key={v} title={label} className={cn("inline-flex items-center gap-1", n(v) ? cls : "opacity-25")}><i className="size-1.5 rounded-full bg-current" />{n(v)}</span>
      ))}
    </span>
  );
}
/** What a Call did, by name: an MCP tool by its own name (its server by its logo), a shell
 * command by its program, any other by its tool. */
const didOf = (c: Call): { name: string; server?: string } => {
  const mcp = c.tool ? /^mcp__(.+?)__(.+)$/.exec(c.tool) : null;
  if (mcp) return { name: mcp[2], server: mcp[1] };
  if (c.program) return { name: c.program };
  return { name: c.tool ? toolName(c.tool) : "–" };
};
/** The Calls in a case that the engine caught, by what they did, the most first. */
function Hits({ calls }: { calls: Call[] }) {
  const n = new Map<string, { did: ReturnType<typeof didOf>; n: number }>();
  for (const c of calls) {
    const did = didOf(c), key = `${did.server ?? ""}:${did.name}`;
    const had = n.get(key);
    if (had) had.n += 1; else n.set(key, { did, n: 1 });
  }
  const list = [...n.values()].sort((a, b) => b.n - a.n);
  return (
    <span className="flex min-w-0 items-center gap-1 overflow-hidden" title={list.map((x) => `${x.did.name} ${x.n}`).join(" · ")}>
      {list.slice(0, 1).map(({ did, n: k }) => (
        <span key={`${did.server}:${did.name}`} className="inline-flex min-w-0 items-center gap-1 rounded-md bg-secondary px-1.5 py-0.5 font-mono text-[11px] text-foreground/85">
          {did.server && <span className="shrink-0"><ServiceLogo name={did.server} size={12} /></span>}
          <span className="min-w-0 truncate">{did.name}</span>{k > 1 && <b className="tnum shrink-0 font-sans font-medium">{k}</b>}
        </span>
      ))}
      {list.length > 1 && <span className="tnum shrink-0 text-[11.5px] text-muted-foreground">+{list.length - 1}</span>}
    </span>
  );
}
/** What told the agent to act, by kind and short name; its whole in the tooltip. */
function CauseChip({ c }: { c: Call }) {
  if (!c.cause || !c.cause_kind) return <span className="text-muted-foreground">–</span>;
  const server = c.cause_kind === "tool" && c.cause.startsWith("mcp__") ? c.cause.split("__")[1] : undefined;
  return (
    <span className="flex min-w-0 items-center gap-1.5" title={c.cause}>
      <span className="shrink-0 rounded bg-catch/15 px-1.5 text-[11px] leading-[19px] font-medium text-catch">{CAUSE[c.cause_kind]}</span>
      {server && <ServiceLogo name={server} size={13} />}
      <span className="truncate font-mono text-[12px]">{c.cause_name ?? c.cause}</span>
    </span>
  );
}
/** Some machines: the first by name, the rest by how many. */
const Machines = ({ list }: { list: string[] }) => (
  <span className="flex min-w-0 items-center gap-1.5" title={list.join("\n")}>
    <span className="truncate font-mono text-[12px]">{list[0]}</span>
    {list.length > 1 && <span className="tnum shrink-0 text-[11.5px] text-muted-foreground">+{list.length - 1}</span>}
  </span>
);
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
  ...(ran ? [{ id: "why", header: "실행됨", size: 152, accessorFn: (g: CaseGroup) => whyRan(g).join(" "), cell: ({ row }: { row: { original: CaseGroup } }) => (
    <span className="flex min-w-0 items-center gap-1">
      {whyRan(row.original).map((w) => <span key={w} className={cn("shrink-0 rounded px-1.5 text-[11.5px] leading-[20px] font-semibold", RAN[w].cls)} title={w === "leak" ? row.original.leaked.join(", ") : undefined}>{RAN[w].label}</span>)}
    </span>
  ) } as ColumnDef<CaseGroup>] : []),
  { id: "sev", header: "심각도", size: 88, accessorFn: (g) => g.risk, cell: ({ row }) => <SevBadge s={worst(row.original)} /> },
  // What the person had it do: the session's short name.
  { id: "work", header: "작업", size: 260, accessorFn: (g) => named(first(g)) ?? "", cell: ({ row }) => {
    const c = first(row.original), name = named(c);
    return <span className={cn("block truncate", !name && "text-muted-foreground")} title={c.prompt ?? undefined}>{name ?? NO_PROMPT}</span>;
  } },
  { id: "cause", header: "원인", size: 170, accessorFn: (g) => g.calls.find((c) => c.cause)?.cause_name ?? "", cell: ({ row }) => <CauseChip c={row.original.calls.find((c) => c.cause) ?? row.original.calls[0]} /> },
  { id: "machine", header: "단말", size: 140, accessorFn: (g) => g.machines[0] ?? "", cell: ({ row }) => <Machines list={row.original.machines} /> },
  { id: "sessions", header: "세션", size: 64, accessorFn: (g) => g.sessions.length, cell: ({ row }) => <span className="tnum">{row.original.sessions.length}</span> },
  { id: "hits", header: "걸린 호출", size: 280, accessorFn: (g) => didOf(first(g)).name, cell: ({ row }) => <Hits calls={row.original.calls} /> },
  { id: "found", header: "차단 · 확인 · 허용", size: 130, accessorFn: (g) => g.total, cell: ({ row }) => <Verdicts calls={row.original.calls} /> },
  { id: "at", header: "최근", size: 104, accessorFn: (g) => g.calls[0].at ?? 0, cell: ({ row }) => <When at={row.original.calls[0].at} /> },
];
/** A source that told agents to act: the cases it made, over how many sessions and machines. */
type Source = { key: string; call: Call; cases: CaseGroup[]; sessions: string[]; machines: string[]; ran: number; held: number; risk: number; at: number };
function sources(list: CaseGroup[]): Source[] {
  const by = new Map<string, Source>();
  for (const g of list) {
    const c = g.calls.find((x) => x.cause && x.cause_kind);
    if (!c) continue;
    const key = `${c.cause_kind}:${c.cause_name ?? c.cause}`;
    const s = by.get(key) ?? { key, call: c, cases: [], sessions: [], machines: [], ran: 0, held: 0, risk: 0, at: 0 };
    s.cases.push(g);
    s.sessions = [...new Set([...s.sessions, ...g.sessions])];
    s.machines = [...new Set([...s.machines, ...g.machines])];
    if (g.ran > 0) s.ran += 1; else s.held += 1;
    s.risk = Math.max(s.risk, g.risk);
    s.at = Math.max(s.at, g.calls[0].at ?? 0);
    by.set(key, s);
  }
  // The worst first, then the farthest reaching, the newest.
  return [...by.values()].sort((a, b) => SEVS.indexOf(worstOf(a)) - SEVS.indexOf(worstOf(b)) || b.sessions.length - a.sessions.length || b.cases.length - a.cases.length || b.at - a.at);
}
const worstOf = (s: Source): Sev => SEVS.find((v) => s.cases.some((g) => worst(g) === v)) ?? "after";
const SOURCES: ColumnDef<Source>[] = [
  { id: "sev", header: "심각도", size: 88, accessorFn: (s) => -SEVS.indexOf(worstOf(s)), cell: ({ row }) => <SevBadge s={worstOf(row.original)} /> },
  { id: "cause", header: "원인", accessorFn: (s) => s.call.cause_name ?? "", cell: ({ row }) => <CauseChip c={row.original.call} /> },
  // What already ran from it, or that it was all stopped.
  { id: "why", header: "결과", size: 170, accessorFn: (s) => s.ran * 1000 + s.held, cell: ({ row }) => {
    const why = [...new Set(row.original.cases.flatMap(whyRan))];
    return (
      <span className="flex min-w-0 items-center gap-1">
        {why.length
          ? why.map((w) => <span key={w} className={cn("shrink-0 rounded px-1.5 text-[11.5px] leading-[20px] font-semibold", RAN[w].cls)}>{RAN[w].label}</span>)
          : <span className="rounded bg-white/8 px-1.5 text-[11.5px] leading-[20px] font-semibold text-foreground/70">막힘</span>}
      </span>
    );
  } },
  { id: "sessions", header: "세션", size: 64, accessorFn: (s) => s.sessions.length, cell: ({ row }) => <span className="tnum">{row.original.sessions.length}</span> },
  { id: "machine", header: "단말", size: 160, accessorFn: (s) => s.machines.length, cell: ({ row }) => <Machines list={row.original.machines} /> },
  { id: "hits", header: "걸린 호출", size: 280, accessorFn: (s) => s.cases.reduce((a, g) => a + g.total, 0), cell: ({ row }) => <Hits calls={row.original.cases.flatMap((g) => g.calls)} /> },
  { id: "found", header: "차단 · 확인 · 허용", size: 130, accessorFn: (s) => s.cases.reduce((a, g) => a + g.total, 0), cell: ({ row }) => <Verdicts calls={row.original.cases.flatMap((g) => g.calls)} /> },
  { id: "at", header: "최근", size: 104, accessorFn: (s) => s.at, cell: ({ row }) => <When at={row.original.at || null} /> },
];

export function Watch({ calls, ask, loading, marks, caseData, saved, blocked, className }: {
  calls: Call[]; ask: Ask; loading?: boolean; className?: string;
  /** The sources the team distrusts. */
  blocked: Blocked[];
  /** What the team marked each session as, the case the app read, and what the last save said. */
  marks: Record<string, Mark>; caseData?: Case | null; saved?: string | null;
}) {
  const [view, setView] = useState<View>("ran");
  const [query, setQuery] = useState("");
  // The Call open, by its session and number: the page's data is sent again (when the case
  // arrives, a mark changes), and the Calls in it are new objects each time.
  const [picked, setPicked] = useState<{ session: string; seq: number } | null>(null);
  // Which cases are listed: those still to handle, those handled, or all. 미처리 pressed again
  // turns to 처리됨, and back.
  const [shownBy, setShownBy] = useState<"open" | "done" | "all">("open");

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
  const listed = useMemo(() => {
    const open = (g: CaseGroup) => g.sessions.some((s) => isOpen(marks[s]));
    return shownBy === "all" ? allCases : allCases.filter((g) => open(g) === (shownBy === "open"));
  }, [allCases, shownBy, marks]);
  // Those where something already ran, to answer for; those the engine stopped, to judge.
  const done = useMemo(() => listed.filter((g) => g.ran > 0), [listed]);
  const held = useMemo(() => listed.filter((g) => g.ran === 0), [listed]);
  const causes = useMemo(() => sources(listed), [listed]);
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
    text: JSON.stringify((view === "sources" ? causes.flatMap((x) => x.cases) : sessions).map((g) => ({ cause: g.cause, state: stateOf(g, marks), sessions: g.sessions.map((s) => ({ session: s, mark: marks[s] ?? null })), machines: g.machines, already_ran: g.ran, calls: g.calls })), null, 2),
  });

  const tab = (v: View, label: string, n: number) => (
    <TabsTrigger value={v} className="gap-1.5 px-2.5 text-[13px]">{label}<span className="tnum text-[11.5px] text-muted-foreground">{n.toLocaleString()}</span></TabsTrigger>
  );
  return (
    <Panel
      pad={false} className={cn("min-h-0 overflow-hidden [&>[data-slot=card-content]]:flex [&>[data-slot=card-content]]:min-h-0 [&>[data-slot=card-content]]:flex-1 [&>[data-slot=card-content]]:flex-col", className)}
      title={<Tabs value={view} onValueChange={(v) => setView(v as View)}><TabsList>{tab("ran", "이미 실행됨", done.length)}{tab("held", "막힘", held.length)}{tab("sources", "원인", causes.length)}</TabsList></Tabs>}
      aside={<>
        {saved && <span className="max-w-[320px] truncate text-[12px]" title={saved}>{saved}</span>}
        {(
          <ToggleGroup type="single" size="sm" value={shownBy === "all" ? "all" : "mine"}
            onValueChange={(v) => setShownBy(v === "all" ? "all" : v === "mine" ? "open" : shownBy === "open" ? "done" : "open")}>
            <ToggleGroupItem value="mine" className="px-2.5 text-[12.5px]">{shownBy === "done" ? "처리됨" : "미처리"}</ToggleGroupItem>
            <ToggleGroupItem value="all" className="px-2.5 text-[12.5px]">전체</ToggleGroupItem>
          </ToggleGroup>
        )}
        <Button variant="outline" size="sm" onClick={exported}><Download />내보내기</Button>
        <span className="relative">
          <Search className="pointer-events-none absolute top-1/2 left-2 size-3.5 -translate-y-1/2 text-muted-foreground" />
          <Input value={query} onChange={(e) => setQuery(e.target.value)} placeholder="검색" className="h-7 w-52 pl-7 text-[12.5px] md:text-[12.5px]" />
        </span>
      </>}
    >
      <div className="min-h-0 flex-1 border-t [&_[data-slot=table-container]]:scroll [&_[data-slot=table-container]]:h-full [&_[data-slot=table-container]]:overflow-y-auto [&_td:first-child]:pl-4 [&_td:last-child]:pr-4 [&_th:first-child]:pl-4 [&_th:last-child]:pr-4">
        {loading && <div className="flex flex-col gap-2 p-4">{Array.from({ length: 8 }, (_, i) => <Skeleton key={i} className="h-8 rounded-md" />)}</div>}
        {!loading && view !== "sources" && <DataTable columns={columns} rows={sessions.slice(0, ROWS)} onRow={(g) => open(first(g))} picked={(g) => g.calls.some(same)} dense />}
        {!loading && view === "sources" && <DataTable columns={SOURCES} rows={causes.slice(0, ROWS)} onRow={(s) => open(first(s.cases[0]))} picked={(s) => s.cases.some((g) => g.calls.some(same))} dense />}
      </div>

      <Sheet open={!!picked} onOpenChange={(on) => !on && setPicked(null)}>
        <SheetContent className="gap-0 data-[side=right]:w-[min(1040px,92vw)] data-[side=right]:sm:max-w-[min(1040px,92vw)]" onOpenAutoFocus={(e) => e.preventDefault()}>
          {call && group && <CaseView call={call} group={group} data={caseData} marks={marks} blocked={blocked} saved={saved} ask={ask} onCall={open} />}
        </SheetContent>
      </Sheet>
    </Panel>
  );
}
