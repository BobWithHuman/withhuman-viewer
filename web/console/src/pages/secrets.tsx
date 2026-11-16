// 민감 정보: the sensitive values the engine found in the sessions, followed to the Calls that
// used them. The flow carries the page (where values came in, what kind, how they ended); the
// bars beside it split them by kind, destination and machine; a band, node or bar pressed
// narrows the lists beneath, and a row opens its story on the right. A value is shown masked,
// as the engine keeps it, and a place it showed opens in 기록.

import { type ColumnDef } from "@tanstack/react-table";
import { ArrowRight, ArrowUpRight, FileText, Globe, Laptop, MessageSquareText, Search, Send, ShieldAlert, SquareTerminal, X } from "lucide-react";
import { useEffect, useMemo, useState, type ReactNode } from "react";
import { send } from "@/bridge";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Sheet, SheetContent, SheetDescription, SheetFooter, SheetHeader, SheetTitle } from "@/components/ui/sheet";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { DataTable, Empty, Legend, PageHeader, Panel, ServiceLogo, Stat, VerdictBadge, ago, clock, People, Person } from "@/kit";
import { cn } from "@/lib/utils";
import {
  CALL_VERDICT, CATEGORY, DEMO, FROM, STATUS, STATUSES, attempts, bars, byStatus, categoryLabel, fromOf, passes, machineName, placesOf, sessionName, short, sources, values, NO_USER, valueKey,
  type Attempt, type Bar, type Category, type Data, type Dim, type Filter, type Mention, type Source, type Status, type Trail, type Value,
} from "./secrets/data";
import { Flow } from "./secrets/flow";
import { SourceBars, parts } from "./secrets/sources";

const DAY = 24 * 3600e3;
const PERIODS: [string, string][] = [["1", "24시간"], ["7", "7일"], ["30", "30일"], ["all", "전체"]];
const RANK: Record<Status, number> = { out: 3, blocked: 2, used: 1, seen: 0 };
type Tab = "values" | "sessions" | "sources";
/** What a figure pressed leaves on the page: every value, those put in a prompt, those a session
 * tried to send out, or those that came out of a document or a tool result. */
type Focus = "all" | "prompt" | "tries" | "sources";
const FOCUS: Record<Focus, (t: Trail) => boolean> = {
  all: () => true,
  prompt: (t) => fromOf(t) === "user",
  tries: (t) => t.mentions.some((m) => m.out),
  sources: (t) => !!t.source,
};
type Open = { value: Value } | { trail: Trail } | { attempt: Attempt } | { source: Source } | null;
type Split = "kind" | "source" | "place" | "user" | "client";
const SPLITS: [Split, string][] = [["kind", "종류"], ["source", "출처"], ["place", "전송 대상"], ["user", "사용자"], ["client", "단말"]];

/** Asks the app to show a place a value was in on the route map of 기록. */
const openInRecord = (session: string, seq: number) => send("secrets", { type: "open", session, seq });

function StatusBadge({ s }: { s: Status }) {
  return <Badge variant="secondary" className={cn("gap-1 border-0 font-semibold", STATUS[s].cls)}><i className="size-1.5 rounded-full bg-current" />{STATUS[s].label}</Badge>;
}

/** A destination's mark: the service's logo for an MCP server, a globe for a host. */
function PlaceMark({ name, size = 18 }: { name: string; size?: number }) {
  // An MCP destination ends in its server, after whom the Call named: "bob@x.com · slack (MCP)".
  if (name.endsWith("(MCP)")) return <ServiceLogo name={name.split(" · ").pop()!.replace(" (MCP)", "")} size={size} />;
  return <span className="grid shrink-0 place-items-center rounded-full bg-secondary text-muted-foreground" style={{ width: size, height: size }}><Globe style={{ width: size * 0.62, height: size * 0.62 }} strokeWidth={1.9} /></span>;
}

function When({ at, now }: { at: number | null; now: number }) {
  return <span className="tnum text-muted-foreground" title={clock(at)}>{ago(at, now)}</span>;
}

/** The viewport's height, so the picture takes its share of a small window and of a large one. */
function useViewHeight() {
  const [h, setH] = useState(window.innerHeight);
  useEffect(() => {
    const on = () => setH(window.innerHeight);
    window.addEventListener("resize", on);
    return () => window.removeEventListener("resize", on);
  }, []);
  return h;
}

export default function Page({ data }: { data: unknown | null }) {
  const d = (data as Data | null) ?? DEMO;
  const [period, setPeriod] = useState("7");
  const [filter, setFilter] = useState<Filter>({});
  const [tab, setTab] = useState<Tab>("values");
  const [focus, setFocus] = useState<Focus>("all");
  const [split, setSplit] = useState<Split>("kind");
  const [query, setQuery] = useState("");
  const [open, setOpen] = useState<Open>(null);
  const flowHeight = Math.round(Math.max(240, Math.min(useViewHeight() * 0.4 - 64, 360)));

  const days = period === "all" ? null : Number(period);
  const inPeriod = useMemo(() => d.trails.filter((t) => days === null || (t.last !== null && t.last >= d.now - days * DAY)), [d, days]);
  const before = useMemo(() => (days === null ? null : d.trails.filter((t) => t.last !== null && t.last < d.now - days * DAY && t.last >= d.now - 2 * days * DAY)), [d, days]);
  const delta = (count: (list: Trail[]) => number) => {
    const was = before ? count(before) : 0;
    return was ? Math.round(((count(inPeriod) - was) / was) * 100) : null;
  };
  const prompted = inPeriod.filter((t) => fromOf(t) === "user");
  const tries = useMemo(() => attempts(inPeriod), [inPeriod]);
  const from = useMemo(() => sources(inPeriod), [inPeriod]);
  // The picture, the bars and the lists show only what the figure pressed is about.
  const focused = useMemo(() => inPeriod.filter(FOCUS[focus]), [inPeriod, focus]);

  // The lists follow every pick; the flow and the bars each leave out their own, so what is
  // picked in one stays in view beside what was not.
  const q = query.trim().toLowerCase();
  const shown = useMemo(
    () => focused.filter((t) => passes(t, filter) && (!q || [t.text, t.kind, t.title, t.client, t.host, t.user, t.source, t.session].some((s) => s?.toLowerCase().includes(q)))),
    [focused, filter, q],
  );
  const flowTrails = useMemo(() => focused.filter((t) => passes(t, filter, ["from", "source", "category", "status"])), [focused, filter]);
  const ended = byStatus(flowTrails);
  const flowSources = useMemo(() => sources(flowTrails), [flowTrails]);
  const splitBars = useMemo(
    () => bars(focused.filter((t) => passes(t, filter, [split])), (t) => (split === "kind" ? [t.kind] : split === "client" ? [t.client] : split === "user" ? [t.user ?? NO_USER] : split === "source" ? [t.source ?? FROM[fromOf(t)].label] : placesOf(t))),
    [focused, filter, split],
  );
  const shownValues = useMemo(() => values(shown), [shown]);
  // Every session each value showed in, over all time, for "where else".
  const byValue = useMemo(() => {
    const m = new Map<string, Trail[]>();
    for (const t of d.trails) m.set(valueKey(t), [...(m.get(valueKey(t)) ?? []), t]);
    return m;
  }, [d.trails]);
  const shownTries = useMemo(() => attempts(shown), [shown]);
  const shownFrom = useMemo(() => sources(shown), [shown]);
  const chips = (Object.keys(filter) as Dim[]).filter((k) => filter[k] !== undefined);
  const chipLabel = (k: Dim) => (k === "from" ? FROM[filter.from!].label : k === "category" ? categoryLabel(filter.category!) : k === "status" ? STATUS[filter.status!].label : filter[k]!);
  const only = (f: Focus, to: Tab) => {
    setFocus(f);
    setFilter({});
    setTab(to);
  };

  return (
    <div className="flex h-full flex-col">
      <PageHeader title="민감 정보" meta={<>
        <Badge variant="secondary" className="gap-1.5 border-0 font-normal text-muted-foreground"><i className={cn("size-1.5 rounded-full", d.demo ? "bg-review" : "bg-allow")} />{d.store}</Badge>
        {d.loaded ? <span className="tnum">{clock(d.loaded)}</span> : <span>불러오는 중</span>}
        {d.error && <Badge variant="secondary" className="max-w-[420px] gap-1 border-0 bg-deny/15 text-deny" title={d.error}><ShieldAlert className="size-3" /><span className="truncate">{d.error}</span></Badge>}
      </>}>
        <ToggleGroup type="single" size="sm" variant="outline" spacing={0} value={period} onValueChange={(v) => v && setPeriod(v)}>
          {PERIODS.map(([v, l]) => <ToggleGroupItem key={v} value={v} className="px-3 text-[12.5px]">{l}</ToggleGroupItem>)}
        </ToggleGroup>
      </PageHeader>

      <div className="scroll flex min-h-0 flex-1 flex-col gap-4 overflow-auto px-6 pb-6">
        <div className="grid shrink-0 grid-cols-4 gap-4">
          <Stat label="탐지된 민감정보" icon={ShieldAlert} value={values(inPeriod).length} unit={`개 · 노출 ${inPeriod.length}회`} delta={delta((l) => values(l).length)}
            active={focus === "all"} onClick={() => only("all", "values")} />
          <Stat label="사용자 입력" icon={MessageSquareText} value={prompted.length}
            unit={`회 · 세션 ${new Set(prompted.map((t) => t.session)).size} · 단말 ${new Set(prompted.map((t) => t.client)).size}`}
            delta={delta((l) => l.filter((t) => fromOf(t) === "user").length)}
            active={focus === "prompt"} onClick={() => only("prompt", "values")} />
          <Stat label="유출 시도" icon={Send} value={tries.length} tone={tries.some((a) => a.ran > 0) ? "var(--deny)" : undefined}
            unit={`개 · 유출 ${tries.filter((a) => a.ran > 0).length} · 차단 ${tries.filter((a) => a.ran === 0).length}`}
            active={focus === "tries"} onClick={() => only("tries", "sessions")} />
          <Stat label="노출 출처" icon={FileText} value={from.length} unit={`곳 · 값 ${from.reduce((a, s) => a + s.trails.length, 0)}`}
            active={focus === "sources"} onClick={() => only("sources", "values")} />
        </div>

        <div className="grid shrink-0 grid-cols-[minmax(0,1fr)_340px] gap-4 min-[1600px]:grid-cols-[minmax(0,1fr)_400px]">
          {focus === "sources" ? (
            <Panel title="노출 출처" count={flowSources.length} aside={<Legend items={STATUSES.map((k) => ({ label: STATUS[k].label, color: STATUS[k].color, value: ended[k] }))} />}>
              <SourceBars sources={flowSources} picked={filter.source} height={flowHeight} onPick={(name) => setFilter({ ...filter, source: filter.source === name ? undefined : name })} />
            </Panel>
          ) : (
            <Panel title="값의 흐름" count={flowTrails.length} aside={<Legend items={STATUSES.map((s) => ({ label: STATUS[s].label, color: STATUS[s].color, value: ended[s] }))} />}>
              <Flow trails={flowTrails} filter={filter} onFilter={setFilter} height={flowHeight} />
            </Panel>
          )}
          <Panel title={
            <ToggleGroup type="single" size="sm" value={split} onValueChange={(v) => v && setSplit(v as Split)} className="-ml-1.5">
              {SPLITS.map(([v, l]) => <ToggleGroupItem key={v} value={v} className="px-2 text-[13px] text-muted-foreground data-[state=on]:text-foreground">{l}</ToggleGroupItem>)}
            </ToggleGroup>
          } aside={<span className="tnum">{splitBars.length}</span>}>
            <SplitBars bars={splitBars} split={split} picked={filter[split]} height={flowHeight}
              onPick={(key) => setFilter({ ...filter, [split]: filter[split] === key ? undefined : key })} />
          </Panel>
        </div>

        <Panel pad={false} className="min-h-[264px] flex-1 overflow-hidden [&>[data-slot=card-content]]:min-h-0 [&>[data-slot=card-content]]:flex-1 [&_[data-slot=table-container]]:h-full [&_[data-slot=table-container]]:overflow-auto"
          title={
            <Tabs value={tab} onValueChange={(v) => setTab(v as Tab)}>
              <TabsList className="-ml-1">
                {([["values", "값", shownValues.length], ["sessions", "유출 시도", shownTries.length], ["sources", "노출 출처", shownFrom.length]] as [Tab, string, number][]).map(([v, l, n]) => (
                  <TabsTrigger key={v} value={v} className="gap-1.5 px-2.5 text-[12.5px]">{l}<span className="tnum text-[11.5px] text-muted-foreground">{n}</span></TabsTrigger>
                ))}
              </TabsList>
            </Tabs>
          }
          aside={<>
            {chips.map((k) => (
              <button key={k} onClick={() => setFilter({ ...filter, [k]: undefined })} className="inline-flex h-6 max-w-[180px] items-center gap-1 rounded-md bg-primary/15 pr-1 pl-2 text-[12px] text-primary transition-colors hover:bg-primary/25">
                <span className="truncate">{chipLabel(k)}</span><X className="size-3 shrink-0" />
              </button>
            ))}
            {chips.length > 1 && <Button variant="ghost" size="xs" onClick={() => setFilter({})}>지우기</Button>}
            <label className="relative">
              <Search className="pointer-events-none absolute top-1/2 left-2 size-3.5 -translate-y-1/2 text-muted-foreground" />
              <Input value={query} onChange={(e) => setQuery(e.target.value)} placeholder="값·세션·사용자·단말" className="h-7 w-44 pl-7 text-[12.5px] md:text-[12.5px]" />
            </label>
          </>}>
          {tab === "values" && <DataTable dense columns={valueColumns(d.now, focus === "prompt")} rows={shownValues} onRow={(v) => setOpen(v.trails.length === 1 ? { trail: v.trails[0] } : { value: v })}
            picked={(v) => !!open && (("value" in open && open.value.key === v.key) || ("trail" in open && v.trails.some((t) => t.id === open.trail.id)))} />}
          {tab === "sessions" && <DataTable dense columns={attemptColumns(d.now)} rows={shownTries} onRow={(a) => setOpen({ attempt: a })} picked={(a) => !!open && "attempt" in open && open.attempt.session === a.session} />}
          {tab === "sources" && <DataTable dense columns={sourceColumns(d.now)} rows={shownFrom} onRow={(s) => setOpen({ source: s })} picked={(s) => !!open && "source" in open && open.source.name === s.name} />}
        </Panel>
      </div>

      <Sheet open={open !== null} onOpenChange={(on) => !on && setOpen(null)}>
        <SheetContent className="gap-0 data-[side=right]:w-[520px] data-[side=right]:sm:max-w-[520px]" aria-describedby={undefined} onOpenAutoFocus={(e) => e.preventDefault()}>
          {open && "value" in open && <ValueSheet v={open.value} onTrail={(t) => setOpen({ trail: t })} />}
          {open && "trail" in open && <TrailSheet t={open.trail} others={(byValue.get(valueKey(open.trail)) ?? []).filter((x) => x.id !== open.trail.id)} onTrail={(t) => setOpen({ trail: t })} />}
          {open && "attempt" in open && <AttemptSheet a={open.attempt} />}
          {open && "source" in open && <SourceSheet s={open.source} />}
        </SheetContent>
      </Sheet>
    </div>
  );
}

// ---------------------------------------------------------------- the bars beside the flow
function SplitBars({ bars, split, picked, height, onPick }: { bars: Bar[]; split: Split; picked?: string; height: number; onPick: (key: string) => void }) {
  const max = Math.max(...bars.map((b) => b.total), 1);
  return (
    <div className="scroll -mx-2 overflow-auto" style={{ height }}>
      {!bars.length && <Empty />}
      {bars.map((b) => (
        <Tooltip key={b.key}>
          <TooltipTrigger asChild>
            <button onClick={() => onPick(b.key)} aria-pressed={picked === b.key}
              className={cn("grid h-8 w-full grid-cols-[minmax(0,11fr)_minmax(0,9fr)_24px] items-center gap-3 rounded-md px-2 text-left text-[12.5px] transition-colors hover:bg-accent",
                picked === b.key && "bg-accent ring-1 ring-ring ring-inset", picked !== undefined && picked !== b.key && "opacity-45")}>
              <span className="flex min-w-0 items-center gap-2">
                {split === "place" && <PlaceMark name={b.key} />}
                {split === "client" && <Laptop className="size-3.5 shrink-0 text-muted-foreground" strokeWidth={1.9} />}
                {split === "user" && b.key !== NO_USER && <Person name={b.key} size={18} />}
                {split === "source" && <FileText className="size-3.5 shrink-0 text-muted-foreground" strokeWidth={1.9} />}
                <span className={cn("truncate", split === "source" && "font-mono text-[12px]")}>{split === "place" ? b.key.replace(" (MCP)", "") : split === "source" ? short(b.key) : split === "user" ? b.key.replace(/@.*/, "") : b.key}</span>
              </span>
              <span className="flex h-2 overflow-hidden rounded-full bg-white/6">
                <span className="flex h-full gap-px" style={{ width: `${(b.total / max) * 100}%` }}>
                  {STATUSES.filter((s) => b.split[s] > 0).map((s) => <i key={s} className="h-full" style={{ flex: b.split[s], background: STATUS[s].color }} />)}
                </span>
              </span>
              <b className="tnum text-right font-medium">{b.total}</b>
            </button>
          </TooltipTrigger>
          <TooltipContent side="left" className="grid gap-1">
            <span className="max-w-[360px] font-medium break-all">{b.key}</span>
            {STATUSES.filter((s) => b.split[s] > 0).map((s) => (
              <span key={s} className="tnum flex items-center gap-1.5"><i className="size-2 rounded-full" style={{ background: STATUS[s].color }} />{STATUS[s].label}<b className="ml-auto pl-4 font-medium">{b.split[s]}</b></span>
            ))}
          </TooltipContent>
        </Tooltip>
      ))}
    </div>
  );
}

// ---------------------------------------------------------------- the lists
const Masked = ({ children, className }: { children: ReactNode; className?: string }) => <code className={cn("font-mono text-[12px]", className)}>{children}</code>;
const Client = ({ name }: { name: string }) => <span className="inline-flex min-w-0 max-w-full items-center gap-1.5 text-muted-foreground"><Laptop className="size-3.5 shrink-0" strokeWidth={1.9} /><span className="truncate">{name}</span></span>;

/** How far something spread, marked when it is in more than one session. */
function Spread({ sessions, clients }: { sessions: number; clients: number }) {
  return <span className={cn("tnum whitespace-nowrap", sessions > 1 ? "font-medium" : "text-muted-foreground")}>세션 {sessions} · 단말 {clients}</span>;
}

/** Whose machine something happened on: the person when the inventory knows, and the machine. */
function Who({ t }: { t: { user?: string | null; host?: string | null; client: string } }) {
  return (
    <span className="flex min-w-0 items-center gap-2">
      {t.user ? <Person name={t.user} size={20} label /> : <span className="text-muted-foreground">사용자 모름</span>}
      <Client name={machineName(t)} />
    </span>
  );
}

function Places({ names }: { names: string[] }) {
  return (
    <span className="flex min-w-0 items-center gap-2">
      {names.slice(0, 3).map((p) => <span key={p} className="inline-flex min-w-0 items-center gap-1.5" title={p}><PlaceMark name={p} size={16} /><span className="truncate">{p.replace(" (MCP)", "")}</span></span>)}
      {names.length > 3 && <span className="tnum shrink-0 text-muted-foreground">+{names.length - 3}</span>}
    </span>
  );
}

/** What a person typed a value in, else the session's name. */
const typedIn = (t: Trail) => t.mentions.find((m) => m.said)?.said ?? sessionName(t);

/** A value over every session it showed in: how it ended at its worst, where it was sent, the
 * Call that carried it, where it came from, and how far it spread. */
const valueColumns = (now: number, typed: boolean): ColumnDef<Value>[] => [
  { id: "status", header: "결과", size: 132, accessorFn: (v) => RANK[v.status], cell: ({ row: { original: v } }) => <StatusBadge s={v.status} /> },
  { id: "text", header: "값", size: 300, accessorFn: (v) => v.text, cell: ({ row: { original: v } }) => {
    const Icon = CATEGORY[v.category as Category]?.icon;
    return (
      <span className="flex min-w-0 items-center gap-2" title={v.text}>
        {Icon && <Icon className="size-3.5 shrink-0 text-muted-foreground" strokeWidth={1.9} />}
        <Masked className="min-w-0 truncate">{v.text}</Masked>
        <span className="shrink-0 text-[12px] text-muted-foreground">{v.kind}</span>
      </span>
    );
  } },
  { id: "places", header: "전송 대상", size: 200, accessorFn: (v) => v.places.join(" "), cell: ({ row: { original: v } }) => (v.places.length ? <Places names={v.places} /> : <span className="text-muted-foreground/60">–</span>) },
  { id: "carrier", header: "전송 호출", accessorFn: (v) => v.carrier ?? "", cell: ({ row: { original: v } }) => (v.carrier ? <Masked className="block truncate text-muted-foreground" >{v.carrier}</Masked> : <span className="text-muted-foreground/60">–</span>) },
  typed
    // Every value here was typed by a person: the prompt it was typed in tells more than where from.
    ? { id: "typed", header: "입력 프롬프트", size: 250, accessorFn: (v) => typedIn(v.trails[0]), cell: ({ row: { original: v } }) => (
      <span className="flex min-w-0 items-center gap-1.5" title={v.trails.map(typedIn).join("\n")}>
        <span className="truncate">{typedIn(v.trails[0])}</span>{v.sessions > 1 && <span className="tnum shrink-0 text-muted-foreground">+{v.sessions - 1}</span>}
      </span>
    ) }
    : { id: "from", header: "출처", size: 210, accessorFn: (v) => `${v.froms.join(" ")} ${v.sources.join(" ")}`, cell: ({ row: { original: v } }) => (
      <span className="flex min-w-0 items-center gap-1.5" title={v.sources.join("\n") || undefined}>
        {v.sources.length
          ? <><Masked className="truncate">{short(v.sources[0])}</Masked>{v.sources.length > 1 && <span className="tnum shrink-0 text-muted-foreground">+{v.sources.length - 1}</span>}</>
          : <span className="truncate">{v.froms.map((f) => FROM[f].label).join(" · ")}</span>}
      </span>
    ) },
  { id: "users", header: "사용자", size: 120, accessorFn: (v) => v.users.join(" "), cell: ({ row: { original: v } }) => (
    v.users.length === 1 ? <Person name={v.users[0]} size={20} label /> : v.users.length ? <People names={v.users} max={4} /> : <span className="text-muted-foreground/60">–</span>
  ) },
  { id: "spread", header: "영향 범위", size: 132, accessorFn: (v) => v.sessions * 1000 + v.clients.length, cell: ({ row: { original: v } }) => <Spread sessions={v.sessions} clients={v.clients.length} /> },
  { id: "last", header: "마지막", size: 88, accessorFn: (v) => v.last ?? 0, cell: ({ row: { original: v } }) => <When at={v.last} now={now} /> },
  // A value that went out opens in 기록 at the Call that sent it, without the detail between.
  { id: "open", header: "", size: 44, enableSorting: false, cell: ({ row: { original: v } }) => {
    const t = v.trails.find((x) => x.status === "out"), m = t?.mentions.find((x) => x.out && !x.held) ?? t?.mentions.find((x) => x.out);
    return t && m ? (
      <button aria-label="기록에서 열기" title="기록에서 열기" onClick={(e) => { e.stopPropagation(); openInRecord(t.session, m.seq); }}
        className="grid size-6 place-items-center rounded-md text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"><ArrowUpRight className="size-3.5" /></button>
    ) : null;
  } },
];

/** How many of each kind, the most first: "API 키 2 · IBAN 1". */
function Kinds({ trails }: { trails: Trail[] }) {
  const n = new Map<string, number>();
  for (const t of trails) n.set(t.kind, (n.get(t.kind) ?? 0) + 1);
  const list = [...n.entries()].sort((a, b) => b[1] - a[1]);
  return (
    <span className="flex min-w-0 items-center gap-1 overflow-hidden" title={list.map(([k, c]) => `${k} ${c}`).join(" · ")}>
      {list.slice(0, 2).map(([k, c]) => <span key={k} className="shrink-0 rounded bg-secondary px-1.5 text-[11.5px] leading-[19px] text-foreground/85">{k}{c > 1 && <b className="tnum ml-1 font-medium">{c}</b>}</span>)}
      {list.length > 2 && <span className="tnum shrink-0 text-[11.5px] text-muted-foreground">+{list.length - 2}</span>}
    </span>
  );
}

/** How the Calls sending values out ended: those that ran, those kept back. */
function Sent({ ran, held }: { ran: number; held: number }) {
  return (
    <span className="tnum flex items-center gap-2.5 text-[12px]">
      <span className={cn("inline-flex items-center gap-1", ran ? "text-deny" : "opacity-25")}><i className="size-1.5 rounded-full bg-current" />유출 {ran}</span>
      <span className={cn("inline-flex items-center gap-1", held ? "text-review" : "opacity-25")}><i className="size-1.5 rounded-full bg-current" />차단 {held}</span>
    </span>
  );
}

/** A session that tried to send values out: what it is, where to, the Call that carried them,
 * which values, and how its sends ended. */
const attemptColumns = (now: number): ColumnDef<Attempt>[] => [
  { id: "status", header: "결과", size: 132, accessorFn: (a) => Number(a.ran > 0), cell: ({ row: { original: a } }) => <StatusBadge s={a.ran > 0 ? "out" : "blocked"} /> },
  { id: "session", header: "세션", size: 250, accessorFn: (a) => sessionName(a), cell: ({ row: { original: a } }) => <span className="block truncate" title={sessionName(a)}>{sessionName(a)}</span> },
  { id: "places", header: "전송 대상", size: 200, accessorFn: (a) => a.places.join(" "), cell: ({ row: { original: a } }) => <Places names={a.places} /> },
  { id: "carrier", header: "전송 호출", accessorFn: (a) => a.carrier ?? "", cell: ({ row: { original: a } }) => (a.carrier ? <Masked className="block truncate text-muted-foreground">{a.carrier}</Masked> : <span className="text-muted-foreground/60">–</span>) },
  { id: "values", header: "값", size: 190, accessorFn: (a) => a.trails.length, cell: ({ row: { original: a } }) => <Kinds trails={a.trails} /> },
  { id: "sent", header: "전송", size: 132, accessorFn: (a) => a.ran * 1000 + a.held, cell: ({ row: { original: a } }) => <Sent ran={a.ran} held={a.held} /> },
  { id: "client", header: "단말", size: 150, accessorFn: (a) => a.client, cell: ({ row: { original: a } }) => <Client name={a.client} /> },
  { id: "last", header: "마지막", size: 88, accessorFn: (a) => a.last ?? 0, cell: ({ row: { original: a } }) => <When at={a.last} now={now} /> },
];

/** A document, command or tool the agents read values from: what it is, how many values and how
 * they ended, which kinds, and how far they spread. */
const sourceColumns = (now: number): ColumnDef<Source>[] => [
  { id: "name", header: "노출 출처", accessorFn: (s) => s.name, cell: ({ row: { original: s } }) => {
    const p = parts(s.name);
    return (
      <span className="flex min-w-0 items-center gap-2" title={s.name}>
        {p.server ? <ServiceLogo name={p.server} size={18} /> : p.command ? <SquareTerminal className="size-4 shrink-0 text-muted-foreground" strokeWidth={1.9} /> : <FileText className="size-4 shrink-0 text-muted-foreground" strokeWidth={1.9} />}
        <Masked className="min-w-12 truncate">{p.main}</Masked>
        {p.rest && <span className="min-w-0 shrink-[1000] truncate text-[11.5px] text-muted-foreground">{p.rest}</span>}
      </span>
    );
  } },
  { id: "values", header: "값", size: 190, accessorFn: (s) => s.trails.length, cell: ({ row: { original: s } }) => <Kinds trails={s.trails} /> },
  { id: "ended", header: "처리 결과", size: 250, accessorFn: (s) => { const n = byStatus(s.trails); return n.out * 1e6 + n.blocked * 1e3 + s.trails.length; }, cell: ({ row: { original: s } }) => {
    const n = byStatus(s.trails);
    return (
      <span className="flex items-center gap-2.5">
        <span className="flex h-2 w-[92px] shrink-0 gap-px overflow-hidden rounded-full">
          {STATUSES.filter((k) => n[k] > 0).map((k) => <i key={k} className="h-full" style={{ flex: `${n[k]} 0 4px`, background: STATUS[k].color, opacity: k === "seen" ? 0.35 : 1 }} />)}
        </span>
        <Sent ran={n.out} held={n.blocked} />
      </span>
    );
  } },
  { id: "spread", header: "영향 범위", size: 132, accessorFn: (s) => s.sessions * 1000 + s.clients, cell: ({ row: { original: s } }) => <Spread sessions={s.sessions} clients={s.clients} /> },
  { id: "last", header: "마지막", size: 88, accessorFn: (s) => s.last ?? 0, cell: ({ row: { original: s } }) => <When at={s.last} now={now} /> },
];

// ---------------------------------------------------------------- the sheet
const time = (ms: number | null) => (ms ? new Date(ms).toTimeString().slice(0, 8) : "");

function Facts({ rows }: { rows: [string, ReactNode][] }) {
  return (
    <dl className="grid grid-cols-[84px_minmax(0,1fr)] gap-x-3 gap-y-2 text-[12.5px]">
      {rows.map(([k, v]) => <div key={k} className="contents"><dt className="text-muted-foreground">{k}</dt><dd className="min-w-0">{v}</dd></div>)}
    </dl>
  );
}

function Section({ title, count, children }: { title: string; count?: number; children: ReactNode }) {
  return (
    <section className="grid gap-2">
      <h3 className="flex items-center gap-2 text-[12px] font-medium text-muted-foreground">{title}{count !== undefined && <span className="tnum rounded-full bg-secondary px-1.5 text-[11px]">{count}</span>}</h3>
      {children}
    </section>
  );
}

/** One place a value showed in its session; pressed, it opens there in 기록. */
function MentionRow({ m, session }: { m: Mention; session: string }) {
  const tone = m.out ? (m.held ? "var(--review)" : "var(--deny)") : m.call ? "var(--write)" : "var(--info)";
  return (
    <button onClick={() => openInRecord(session, m.seq)} className="group relative grid w-full grid-cols-[14px_58px_minmax(0,1fr)_auto] items-center gap-x-2 rounded-md py-1.5 pr-2 text-left text-[12.5px] transition-colors hover:bg-accent">
      <i className="z-10 mx-auto size-2 rounded-full ring-4 ring-popover group-hover:ring-accent" style={{ background: tone }} />
      <span className="tnum text-[12px] text-muted-foreground">{time(m.at)}</span>
      <span className="flex min-w-0 items-center gap-2">
        <span className="shrink-0">{m.where}</span>
        {m.tool && <span className="max-w-32 shrink-0 truncate rounded-md bg-secondary px-1.5 py-0.5 text-[11px] text-muted-foreground">{m.tool.replace(/^mcp__/, "").replace(/__/g, " · ")}</span>}
        {m.place && <span className="inline-flex shrink-0 items-center gap-1" style={{ color: tone }}><PlaceMark name={m.place} size={15} />{m.place.replace(" (MCP)", "")}</span>}
        {m.what && <Masked className="truncate text-muted-foreground">{m.what}</Masked>}
      </span>
      <span className="flex items-center gap-1.5">
        {m.verdict && <VerdictBadge v={CALL_VERDICT[m.verdict].v}>{CALL_VERDICT[m.verdict].label}</VerdictBadge>}
        <ArrowUpRight className="size-3.5 text-muted-foreground opacity-0 transition-opacity group-hover:opacity-100" />
      </span>
    </button>
  );
}

/** What a person typed, with the value marked where it is (both masked alike). */
function Said({ text, value }: { text: string; value: string }) {
  const at = value ? text.indexOf(value) : -1;
  if (at < 0) return <span className="text-[13px] leading-relaxed break-words whitespace-pre-wrap">{text}</span>;
  return (
    <span className="text-[13px] leading-relaxed break-words whitespace-pre-wrap">
      {text.slice(0, at)}<mark className="rounded bg-deny/25 px-0.5 font-mono text-[12px] text-foreground">{value}</mark>{text.slice(at + value.length)}
    </span>
  );
}

/** A list of places down a line, as the value went through them. */
function Line({ children }: { children: ReactNode }) {
  return <div className="relative before:absolute before:top-3 before:bottom-3 before:left-[6.5px] before:w-px before:bg-border">{children}</div>;
}

/** A value named in a sheet's list; pressed, it opens where it first showed in 기록. */
function ValueRow({ t, client }: { t: Trail; client?: boolean }) {
  const first = t.mentions[0];
  return (
    <button onClick={() => first && openInRecord(t.session, first.seq)} className="group flex w-full min-w-0 items-center gap-2 rounded-md bg-background/60 px-2.5 py-2 text-left text-[12.5px] transition-colors hover:bg-accent">
      <Masked className="min-w-0 flex-1 truncate">{t.text}</Masked>
      <span className="shrink-0 text-muted-foreground">{t.kind}</span>
      {client && <span className="max-w-[120px] shrink-0 truncate text-muted-foreground">{t.client}</span>}
      <StatusBadge s={t.status} />
      <ArrowUpRight className="size-3.5 shrink-0 text-muted-foreground opacity-0 transition-opacity group-hover:opacity-100" />
    </button>
  );
}

function OpenButton({ session, seq }: { session: string; seq: number | undefined }) {
  return (
    <SheetFooter className="border-t">
      <Button disabled={seq === undefined} onClick={() => seq !== undefined && openInRecord(session, seq)}><ArrowUpRight />기록에서 열기</Button>
    </SheetFooter>
  );
}

/** A value that showed in several sessions: where it went and each session it showed in. */
function ValueSheet({ v, onTrail }: { v: Value; onTrail: (t: Trail) => void }) {
  const Icon = CATEGORY[v.category as Category]?.icon;
  return (
    <>
      <SheetHeader className="gap-3 border-b pr-12">
        <SheetTitle className="flex items-center gap-2 text-[15px]">{Icon && <Icon className="size-4 text-muted-foreground" strokeWidth={1.9} />}{v.kind}<StatusBadge s={v.status} /></SheetTitle>
        <SheetDescription asChild><Masked className="block rounded-lg border bg-background px-3 py-2.5 text-[13px] break-all text-foreground">{v.text}</Masked></SheetDescription>
      </SheetHeader>
      <div className="scroll grid min-h-0 flex-1 content-start gap-5 overflow-auto p-4">
        <Facts rows={[
          ["사용자", v.users.length ? <span className="flex flex-wrap gap-x-3 gap-y-1">{v.users.map((u) => <Person key={u} name={u} size={20} label />)}</span> : <span className="text-muted-foreground">모름</span>],
          ["영향 범위", <span className="tnum">세션 {v.sessions} · 단말 {v.clients.length}</span>],
          ["전송 대상", v.places.length ? <Places names={v.places} /> : <span className="text-muted-foreground">–</span>],
          ["전송 호출", v.carrier ? <Masked className="break-all">{v.carrier}</Masked> : <span className="text-muted-foreground">–</span>],
          ["출처", <span className="break-all">{v.sources.length ? v.sources.map(short).join(", ") : v.froms.map((f) => FROM[f].label).join(" · ")}</span>],
          ["마지막", <span className="tnum">{clock(v.last)}</span>],
        ]} />
        <Section title="경로" count={v.trails.length}>
          <div className="grid gap-1.5">
            {v.trails.map((t) => {
              // Where it came in (the first mention), and where it left or was used last.
              const first = t.mentions[0];
              const end = [...t.mentions].reverse().find((m) => m.out) ?? [...t.mentions].reverse().find((m) => m.call);
              const step = (m: Mention | undefined, tone: string) => m && (
                <button onClick={() => openInRecord(t.session, m.seq)} className="flex min-w-0 items-center gap-1.5 rounded-md px-1.5 py-1 text-left transition-colors hover:bg-accent" title="기록에서 열기">
                  <i className="size-2 shrink-0 rounded-full" style={{ background: tone }} />
                  <span className="shrink-0 text-muted-foreground">{m.where}</span>
                  {m.tool && <span className="max-w-28 shrink-0 truncate rounded bg-secondary px-1 text-[11px]">{m.tool.replace(/^mcp__/, "").replace(/__/g, " · ")}</span>}
                  {m.place ? <span className="truncate">{m.place.replace(" (MCP)", "")}</span> : m.what && <Masked className="truncate text-muted-foreground">{m.what}</Masked>}
                </button>
              );
              return (
                <div key={t.id} className="grid gap-0.5 rounded-md bg-background/60 px-1.5 py-1.5 text-[12.5px]">
                  <span className="flex min-w-0 items-center gap-2 px-1.5 text-[12px] text-muted-foreground"><span className="truncate">{sessionName(t)}</span><span className="ml-auto shrink-0"><Who t={t} /></span></span>
                  <div className="grid grid-cols-[minmax(0,1fr)_14px_minmax(0,1fr)] items-center">
                    {step(first, "var(--info)") ?? <span />}
                    <ArrowRight className="size-3.5 text-muted-foreground" />
                    {end && end !== first ? step(end, end.out ? (end.held ? "var(--review)" : "var(--deny)") : "var(--write)") : <span className="px-1.5 text-muted-foreground">–</span>}
                  </div>
                </div>
              );
            })}
          </div>
        </Section>
        <Section title="드러난 세션" count={v.trails.length}>
          <div className="grid gap-1">
            {v.trails.map((t) => (
              <button key={t.id} onClick={() => onTrail(t)} className="flex w-full min-w-0 items-center gap-2 rounded-md bg-background/60 px-2.5 py-2 text-left text-[12.5px] transition-colors hover:bg-accent">
                <span className="grid min-w-0 flex-1 gap-1">
                  <span className="truncate">{sessionName(t)}</span>
                  <span className="text-[12px]"><Who t={t} /></span>
                </span>
                <StatusBadge s={t.status} />
              </button>
            ))}
          </div>
        </Section>
      </div>
    </>
  );
}

function TrailSheet({ t, others, onTrail }: { t: Trail; others: Trail[]; onTrail: (t: Trail) => void }) {
  const Icon = CATEGORY[t.category as Category]?.icon;
  return (
    <>
      <SheetHeader className="gap-3 border-b pr-12">
        <SheetTitle className="flex items-center gap-2 text-[15px]">{Icon && <Icon className="size-4 text-muted-foreground" strokeWidth={1.9} />}{t.kind}<StatusBadge s={t.status} /></SheetTitle>
        <SheetDescription asChild><Masked className="block rounded-lg border bg-background px-3 py-2.5 text-[13px] break-all text-foreground">{t.text}</Masked></SheetDescription>
      </SheetHeader>
      <div className="scroll grid min-h-0 flex-1 content-start gap-5 overflow-auto p-4">
        <div className="grid grid-cols-3 gap-2">
          {([["호출 사용", t.calls, "var(--write)"], ["유출", t.outRan, "var(--deny)"], ["차단", t.outStopped, "var(--review)"]] as [string, number, string][]).map(([l, n, c]) => (
            <div key={l} className="rounded-lg border px-3 py-2">
              <div className="text-[12px] text-muted-foreground">{l}</div>
              <div className="tnum text-[20px] leading-tight font-semibold" style={{ color: n ? c : "var(--muted-foreground)" }}>{n}</div>
            </div>
          ))}
        </div>
        {t.mentions.filter((m) => m.said).slice(0, 3).map((m, k) => (
          <Section key={`${m.seq}-${k}`} title={k ? "재입력 프롬프트" : "입력 프롬프트"}>
            <button onClick={() => openInRecord(t.session, m.seq)} className="group grid gap-1.5 rounded-lg border bg-background px-3 py-2.5 text-left transition-colors hover:bg-accent/60">
              <Said text={m.said!} value={t.text} />
              <span className="tnum flex items-center gap-1 text-[11.5px] text-muted-foreground">{time(m.at)}<ArrowUpRight className="size-3 opacity-0 transition-opacity group-hover:opacity-100" /></span>
            </button>
          </Section>
        ))}
        <Facts rows={[
          ["유형", categoryLabel(t.category)],
          ["유입 경로", FROM[fromOf(t)].label],
          ...(t.source ? [["출처", <Masked className="break-all">{t.source}</Masked>] as [string, ReactNode]] : []),
          ...(placesOf(t).length ? [["전송 대상", <Places names={placesOf(t)} />] as [string, ReactNode]] : []),
          ["사용자", t.user ? <Person name={t.user} size={20} label /> : <span className="text-muted-foreground">모름</span>],
          ["단말", <span className="flex min-w-0 items-center gap-2"><Client name={machineName(t)} />{t.host && t.host !== t.client && <Masked className="truncate text-muted-foreground">{t.client}</Masked>}</span>],
          ["세션", <span className="line-clamp-2">{sessionName(t)}</span>],
          ["마지막", <span className="tnum">{clock(t.last)}</span>],
        ]} />
        {others.length > 0 && (
          <Section title="동일 값 사용 세션" count={others.length}>
            <div className="grid gap-1">
              {others.map((o) => (
                <button key={o.id} onClick={() => onTrail(o)} className="flex w-full min-w-0 items-center gap-2 rounded-md bg-background/60 px-2.5 py-2 text-left text-[12.5px] transition-colors hover:bg-accent">
                  <span className="grid min-w-0 flex-1 gap-1">
                    <span className="truncate">{sessionName(o)}</span>
                    <span className="text-[12px]"><Who t={o} /></span>
                  </span>
                  <StatusBadge s={o.status} />
                </button>
              ))}
            </div>
          </Section>
        )}
        <Section title="이동 경로" count={t.mentions.length}>
          <Line>{t.mentions.map((m, i) => <MentionRow key={`${m.seq}-${i}`} m={m} session={t.session} />)}</Line>
        </Section>
      </div>
      <OpenButton session={t.session} seq={t.mentions[0]?.seq} />
    </>
  );
}

function AttemptSheet({ a }: { a: Attempt }) {
  return (
    <>
      <SheetHeader className="gap-2 border-b pr-12">
        <SheetTitle className="line-clamp-2 text-[15px]">{sessionName(a)}</SheetTitle>
        <div className="flex items-center gap-2 text-[12.5px]"><StatusBadge s={a.ran > 0 ? "out" : "blocked"} /><Client name={a.client} /></div>
      </SheetHeader>
      <div className="scroll grid min-h-0 flex-1 content-start gap-5 overflow-auto p-4">
        <Facts rows={[
          ["보내려 한 곳", <span className="flex flex-wrap gap-x-3 gap-y-1">{a.places.map((p) => <span key={p} className="inline-flex items-center gap-1.5"><PlaceMark name={p} size={16} />{p}</span>)}</span>],
          ["호출", <span className="tnum flex gap-3"><span>유출 <b className={cn("font-medium", a.ran > 0 && "text-deny")}>{a.ran}</b></span><span>차단 <b className={cn("font-medium", a.held > 0 && "text-review")}>{a.held}</b></span></span>],
          ["마지막", <span className="tnum">{clock(a.last)}</span>],
        ]} />
        <Section title="보내려 한 값" count={a.trails.length}>
          {a.trails.map((t) => (
            <div key={t.id} className="grid gap-1">
              <ValueRow t={t} />
              <Line>{t.mentions.filter((m) => m.out).map((m, i) => <MentionRow key={`${m.seq}-${i}`} m={m} session={t.session} />)}</Line>
            </div>
          ))}
        </Section>
      </div>
      <OpenButton session={a.session} seq={a.latest} />
    </>
  );
}

function SourceSheet({ s }: { s: Source }) {
  const first = s.trails[0];
  return (
    <>
      <SheetHeader className="gap-3 border-b pr-12">
        <SheetTitle className="flex items-center gap-2 text-[15px]"><FileText className="size-4 shrink-0 text-muted-foreground" strokeWidth={1.9} /><span className="truncate">{short(s.name)}</span></SheetTitle>
        <SheetDescription asChild><Masked className="block rounded-lg border bg-background px-3 py-2.5 break-all text-foreground">{s.name}</Masked></SheetDescription>
      </SheetHeader>
      <div className="scroll grid min-h-0 flex-1 content-start gap-5 overflow-auto p-4">
        <Facts rows={[
          ["읽은 도구", <span className="break-all">{s.tool}</span>],
          ["유형", <span className="flex flex-wrap gap-1">{s.categories.map((c) => <Badge key={c} variant="secondary" className="border-0 font-normal">{categoryLabel(c)}</Badge>)}</span>],
          ["세션", <span className="tnum">{s.sessions}</span>],
          ["마지막", <span className="tnum">{clock(s.last)}</span>],
        ]} />
        <Section title="여기서 나온 값" count={s.trails.length}>
          <div className="grid gap-1">{s.trails.map((t) => <ValueRow key={t.id} t={t} client />)}</div>
        </Section>
      </div>
      <OpenButton session={first?.session ?? ""} seq={first?.mentions[0]?.seq} />
    </>
  );
}
