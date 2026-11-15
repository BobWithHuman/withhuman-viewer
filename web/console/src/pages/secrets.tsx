// 민감 정보: the sensitive values the engine found in the sessions, followed to the Calls that
// used them. The flow carries the page (where values came in, what kind, how they ended); the
// bars beside it split them by kind, destination and machine; a band, node or bar pressed
// narrows the lists beneath, and a row opens its story on the right. A value is shown masked,
// as the engine keeps it, and a place it showed opens in 기록.

import { type ColumnDef } from "@tanstack/react-table";
import { ArrowUpRight, FileText, Globe, Laptop, MessageSquareText, Search, Send, ShieldAlert, X } from "lucide-react";
import { useEffect, useMemo, useState, type ReactNode } from "react";
import { send } from "@/bridge";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Sheet, SheetContent, SheetDescription, SheetFooter, SheetHeader, SheetTitle } from "@/components/ui/sheet";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { DataTable, Empty, Legend, PageHeader, Panel, ServiceLogo, Stat, VerdictBadge, ago, clock } from "@/kit";
import { cn } from "@/lib/utils";
import {
  CALL_VERDICT, CATEGORY, DEMO, FROM, STATUS, STATUSES, attempts, bars, byStatus, categoryLabel, fromOf, passes, placesOf, sessionName, short, sources,
  type Attempt, type Bar, type Category, type Data, type Dim, type Filter, type Mention, type Source, type Status, type Trail,
} from "./secrets/data";
import { Flow } from "./secrets/flow";
import { SourceBars } from "./secrets/sources";

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
type Open = { trail: Trail } | { attempt: Attempt } | { source: Source } | null;
type Split = "kind" | "place" | "client";
const SPLITS: [Split, string][] = [["kind", "종류"], ["place", "보낸 곳"], ["client", "단말"]];

/** Asks the app to show a place a value was in on the route map of 기록. */
const openInRecord = (session: string, seq: number) => send("secrets", { type: "open", session, seq });

function StatusBadge({ s }: { s: Status }) {
  return <Badge variant="secondary" className={cn("gap-1 border-0 font-semibold", STATUS[s].cls)}><i className="size-1.5 rounded-full bg-current" />{STATUS[s].label}</Badge>;
}

/** A destination's mark: the service's logo for an MCP server, a globe for a host. */
function PlaceMark({ name, size = 18 }: { name: string; size?: number }) {
  if (name.endsWith("(MCP)")) return <ServiceLogo name={name} size={size} />;
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
    () => focused.filter((t) => passes(t, filter) && (!q || [t.text, t.kind, t.title, t.client, t.source, t.session].some((s) => s?.toLowerCase().includes(q)))),
    [focused, filter, q],
  );
  const flowTrails = useMemo(() => focused.filter((t) => passes(t, filter, ["from", "source", "category", "status"])), [focused, filter]);
  const ended = byStatus(flowTrails);
  const flowSources = useMemo(() => sources(flowTrails), [flowTrails]);
  const splitBars = useMemo(
    () => bars(focused.filter((t) => passes(t, filter, [split])), (t) => (split === "kind" ? [t.kind] : split === "client" ? [t.client] : placesOf(t))),
    [focused, filter, split],
  );
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
          <Stat label="드러난 값" icon={ShieldAlert} value={inPeriod.length} unit="개" delta={delta((l) => l.length)}
            active={focus === "all"} onClick={() => only("all", "values")} />
          <Stat label="프롬프트에 넣음" icon={MessageSquareText} value={prompted.length}
            unit={`회 · 세션 ${new Set(prompted.map((t) => t.session)).size} · 단말 ${new Set(prompted.map((t) => t.client)).size}`}
            delta={delta((l) => l.filter((t) => fromOf(t) === "user").length)}
            active={focus === "prompt"} onClick={() => only("prompt", "values")} />
          <Stat label="유출 시도 세션" icon={Send} value={tries.length} tone={tries.some((a) => a.ran > 0) ? "var(--deny)" : undefined}
            unit={`개 · 나감 ${tries.filter((a) => a.ran > 0).length} · 막힘 ${tries.filter((a) => a.ran === 0).length}`}
            active={focus === "tries"} onClick={() => only("tries", "sessions")} />
          <Stat label="값을 읽어 온 곳" icon={FileText} value={from.length} unit={`곳 · 값 ${from.reduce((a, s) => a + s.trails.length, 0)}`}
            active={focus === "sources"} onClick={() => only("sources", "values")} />
        </div>

        <div className="grid shrink-0 grid-cols-[minmax(0,1fr)_340px] gap-4 min-[1600px]:grid-cols-[minmax(0,1fr)_400px]">
          {focus === "sources" ? (
            <Panel title="값을 읽어 온 곳" count={flowSources.length} aside={<Legend items={STATUSES.map((k) => ({ label: STATUS[k].label, color: STATUS[k].color, value: ended[k] }))} />}>
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
                {([["values", "값", shown.length], ["sessions", "유출 시도 세션", shownTries.length], ["sources", "읽어 온 곳", shownFrom.length]] as [Tab, string, number][]).map(([v, l, n]) => (
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
              <Input value={query} onChange={(e) => setQuery(e.target.value)} placeholder="값·세션·단말" className="h-7 w-44 pl-7 text-[12.5px] md:text-[12.5px]" />
            </label>
          </>}>
          {tab === "values" && <DataTable dense columns={valueColumns(d.now)} rows={shown} onRow={(t) => setOpen({ trail: t })} picked={(t) => !!open && "trail" in open && open.trail.id === t.id} />}
          {tab === "sessions" && <DataTable dense columns={attemptColumns(d.now)} rows={shownTries} onRow={(a) => setOpen({ attempt: a })} picked={(a) => !!open && "attempt" in open && open.attempt.session === a.session} />}
          {tab === "sources" && <DataTable dense columns={sourceColumns(d.now)} rows={shownFrom} onRow={(s) => setOpen({ source: s })} picked={(s) => !!open && "source" in open && open.source.name === s.name} />}
        </Panel>
      </div>

      <Sheet open={open !== null} onOpenChange={(on) => !on && setOpen(null)}>
        <SheetContent className="gap-0 data-[side=right]:w-[520px] data-[side=right]:sm:max-w-[520px]" aria-describedby={undefined} onOpenAutoFocus={(e) => e.preventDefault()}>
          {open && "trail" in open && <TrailSheet t={open.trail} />}
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
                <span className="truncate">{split === "place" ? b.key.replace(" (MCP)", "") : b.key}</span>
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
            <span className="font-medium">{b.key}</span>
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

function Places({ names }: { names: string[] }) {
  return (
    <span className="flex min-w-0 items-center gap-2">
      {names.slice(0, 3).map((p) => <span key={p} className="inline-flex min-w-0 items-center gap-1.5" title={p}><PlaceMark name={p} size={16} /><span className="truncate">{p.replace(" (MCP)", "")}</span></span>)}
      {names.length > 3 && <span className="tnum shrink-0 text-muted-foreground">+{names.length - 3}</span>}
    </span>
  );
}

const valueColumns = (now: number): ColumnDef<Trail>[] => [
  { id: "text", header: "값", size: 232, accessorFn: (t) => t.text, cell: ({ row: { original: t } }) => {
    const Icon = CATEGORY[t.category as Category]?.icon;
    return <span className="flex min-w-0 items-center gap-2" title={t.text}>{Icon && <Icon className="size-3.5 shrink-0 text-muted-foreground" strokeWidth={1.9} />}<Masked className="truncate">{t.text}</Masked></span>;
  } },
  { id: "kind", header: "종류", size: 118, accessorFn: (t) => t.kind },
  { id: "from", header: "들어온 곳", accessorFn: (t) => `${fromOf(t)} ${t.source ?? ""}`, cell: ({ row: { original: t } }) => (
    <span className="flex min-w-0 items-center gap-1.5" title={t.source ?? undefined}>
      <span className="shrink-0">{FROM[fromOf(t)].label}</span>
      {t.source && <Masked className="truncate text-muted-foreground">{short(t.source)}</Masked>}
    </span>
  ) },
  { id: "session", header: "세션", accessorFn: (t) => sessionName(t), cell: ({ row: { original: t } }) => <span className="block truncate text-muted-foreground" title={sessionName(t)}>{sessionName(t)}</span> },
  { id: "calls", header: "호출", size: 64, accessorFn: (t) => t.calls, cell: ({ row: { original: t } }) => <span className={cn("tnum", !t.calls && "text-muted-foreground")}>{t.calls}</span> },
  { id: "status", header: "결과", size: 132, accessorFn: (t) => RANK[t.status], cell: ({ row: { original: t } }) => <StatusBadge s={t.status} /> },
  { id: "client", header: "단말", size: 168, accessorFn: (t) => t.client, cell: ({ row: { original: t } }) => <Client name={t.client} /> },
  { id: "last", header: "마지막", size: 88, accessorFn: (t) => t.last ?? 0, cell: ({ row: { original: t } }) => <When at={t.last} now={now} /> },
];

const attemptColumns = (now: number): ColumnDef<Attempt>[] => [
  { id: "status", header: "결과", size: 132, accessorFn: (a) => Number(a.ran > 0), cell: ({ row: { original: a } }) => <StatusBadge s={a.ran > 0 ? "out" : "blocked"} /> },
  { id: "session", header: "세션", accessorFn: (a) => sessionName(a), cell: ({ row: { original: a } }) => <span className="block truncate" title={sessionName(a)}>{sessionName(a)}</span> },
  { id: "client", header: "단말", size: 168, accessorFn: (a) => a.client, cell: ({ row: { original: a } }) => <Client name={a.client} /> },
  { id: "values", header: "값", size: 56, accessorFn: (a) => a.trails.length, cell: ({ row: { original: a } }) => <span className="tnum">{a.trails.length}</span> },
  { id: "ran", header: "나감", size: 64, accessorFn: (a) => a.ran, cell: ({ row: { original: a } }) => <span className={cn("tnum", a.ran ? "text-deny" : "text-muted-foreground")}>{a.ran}</span> },
  { id: "held", header: "막힘", size: 64, accessorFn: (a) => a.held, cell: ({ row: { original: a } }) => <span className={cn("tnum", a.held ? "text-review" : "text-muted-foreground")}>{a.held}</span> },
  { id: "places", header: "보내려 한 곳", accessorFn: (a) => a.places.join(" "), cell: ({ row: { original: a } }) => <Places names={a.places} /> },
  { id: "last", header: "마지막", size: 88, accessorFn: (a) => a.last ?? 0, cell: ({ row: { original: a } }) => <When at={a.last} now={now} /> },
];

const sourceColumns = (now: number): ColumnDef<Source>[] => [
  { id: "name", header: "문서·출처", accessorFn: (s) => s.name, cell: ({ row: { original: s } }) => (
    <span className="flex min-w-0 items-baseline gap-2" title={s.name}><Masked className="shrink-0">{short(s.name).slice(0, 48)}</Masked>{short(s.name) !== s.name && <span className="truncate text-[12px] text-muted-foreground">{s.name}</span>}</span>
  ) },
  { id: "tool", header: "읽은 도구", size: 190, accessorFn: (s) => s.tool, cell: ({ row: { original: s } }) => <span className="block truncate text-muted-foreground" title={s.tool}>{s.tool}</span> },
  { id: "values", header: "값", size: 56, accessorFn: (s) => s.trails.length, cell: ({ row: { original: s } }) => <span className="tnum">{s.trails.length}</span> },
  { id: "categories", header: "유형", size: 232, enableSorting: false, cell: ({ row: { original: s } }) => (
    <span className="flex items-center gap-1 overflow-hidden">{s.categories.map((c) => <Badge key={c} variant="secondary" className="border-0 font-normal">{categoryLabel(c)}</Badge>)}</span>
  ) },
  { id: "sessions", header: "세션", size: 64, accessorFn: (s) => s.sessions, cell: ({ row: { original: s } }) => <span className="tnum">{s.sessions}</span> },
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

function TrailSheet({ t }: { t: Trail }) {
  const Icon = CATEGORY[t.category as Category]?.icon;
  return (
    <>
      <SheetHeader className="gap-3 border-b pr-12">
        <SheetTitle className="flex items-center gap-2 text-[15px]">{Icon && <Icon className="size-4 text-muted-foreground" strokeWidth={1.9} />}{t.kind}<StatusBadge s={t.status} /></SheetTitle>
        <SheetDescription asChild><Masked className="block rounded-lg border bg-background px-3 py-2.5 text-[13px] break-all text-foreground">{t.text}</Masked></SheetDescription>
      </SheetHeader>
      <div className="scroll grid min-h-0 flex-1 content-start gap-5 overflow-auto p-4">
        <div className="grid grid-cols-3 gap-2">
          {([["호출에 넣음", t.calls, "var(--write)"], ["밖으로 나감", t.outRan, "var(--deny)"], ["막힘", t.outStopped, "var(--review)"]] as [string, number, string][]).map(([l, n, c]) => (
            <div key={l} className="rounded-lg border px-3 py-2">
              <div className="text-[12px] text-muted-foreground">{l}</div>
              <div className="tnum text-[20px] leading-tight font-semibold" style={{ color: n ? c : "var(--muted-foreground)" }}>{n}</div>
            </div>
          ))}
        </div>
        <Facts rows={[
          ["유형", categoryLabel(t.category)],
          ["들어온 곳", FROM[fromOf(t)].label],
          ...(t.source ? [["출처", <Masked className="break-all">{t.source}</Masked>] as [string, ReactNode]] : []),
          ...(placesOf(t).length ? [["보낸 곳", <Places names={placesOf(t)} />] as [string, ReactNode]] : []),
          ["세션", <span className="line-clamp-2">{sessionName(t)}</span>],
          ["단말", <Client name={t.client} />],
          ["마지막", <span className="tnum">{clock(t.last)}</span>],
        ]} />
        <Section title="지나간 곳" count={t.mentions.length}>
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
          ["호출", <span className="tnum flex gap-3"><span>나감 <b className={cn("font-medium", a.ran > 0 && "text-deny")}>{a.ran}</b></span><span>막힘 <b className={cn("font-medium", a.held > 0 && "text-review")}>{a.held}</b></span></span>],
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
