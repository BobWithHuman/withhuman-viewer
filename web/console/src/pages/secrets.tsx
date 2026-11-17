// 민감 정보: the sensitive values the engine found in the sessions, followed to the Calls that
// used them. The flow carries the page (where values came in, what kind, how they ended); the
// bars beside it split them by kind, destination and machine; a band, node or bar pressed
// narrows the lists beneath, and a row opens its story on the right. A value is shown masked,
// as the engine keeps it, and a place it showed opens in 기록.

import { type ColumnDef } from "@tanstack/react-table";
import { ArrowRight, ArrowUpRight, Bot, FileText, Globe, Hash, History, Laptop, Mail, MessageSquareText, Search, Send, ShieldAlert, SquareTerminal, UserRound, Wrench, X } from "lucide-react";
import { useEffect, useMemo, useState, type ReactNode } from "react";
import { send } from "@/bridge";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Sheet, SheetContent, SheetDescription, SheetHeader, SheetTitle } from "@/components/ui/sheet";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { DataTable, Empty, Legend, PageHeader, Panel, ServiceLogo, SourceMeta, Stat, ago, clock } from "@/kit";
import { cn } from "@/lib/utils";
import { isShell } from "@/kit/code";
import {
  CATEGORY, DEMO, FROM, STATUS, STATUSES, WHY, WHYS, attempts, bars, byStatus, categoryLabel, fromOf, passes, machineName, placesOf, sessionName, short, sources, values, valueKey,
  type Attempt, type Bar, type Category, type Data, type Dim, type Filter, type Source, type Status, type Trail, type Value, type Why,
} from "./secrets/data";
import { Flow } from "./secrets/flow";
import { SourceBars, parts } from "./secrets/sources";

const DAY = 24 * 3600e3;
const PERIODS: [string, string][] = [["1", "24시간"], ["7", "7일"], ["30", "30일"], ["all", "전체"]];
const RANK: Record<Status, number> = { out: 3, blocked: 2, normal: 1, seen: 0 };
/** What a figure pressed leaves on the page: every value, those put in a prompt, those a session
 * tried to send out, or those that came out of a document or a tool result. */
type Focus = "all" | "prompt" | "tries" | "sources";
const FOCUS: Record<Focus, (t: Trail) => boolean> = {
  all: () => true,
  prompt: (t) => fromOf(t) === "user",
  tries: (t) => t.mentions.some((m) => m.out),
  sources: (t) => !!t.source,
};
type Open = { value: Value } | { input: Trail } | { attempt: Attempt } | { source: Source } | null;
type Split = "kind" | "source" | "place" | "client";
const SPLITS: [Split, string][] = [["kind", "종류"], ["source", "출처"], ["place", "전송 대상"], ["client", "단말"]];
/** What each figure's list is of and its bars split by first: the values; each value a person
 * put in a prompt, by machine; the sessions that tried to send them, by where to; the sources. */
const LIST: Record<Focus, { label: string; split: Split }> = {
  all: { label: "값", split: "kind" },
  prompt: { label: "입력", split: "client" },
  tries: { label: "유출 시도", split: "place" },
  sources: { label: "출처", split: "kind" },
};

/** Asks the app to show a place a value was in on the route map of 기록. */
const openInRecord = (session: string, seq: number) => send("secrets", { type: "open", session, seq });

function StatusBadge({ s }: { s: Status }) {
  return <Badge variant="secondary" className={cn("gap-1 border-0 font-semibold", STATUS[s].cls)}><i className="size-1.5 rounded-full bg-current" />{STATUS[s].label}</Badge>;
}

/** A target's mark, by how it is written: a mail address, a channel, a host, or an account or ID. */
function TargetIcon({ target, size = 16 }: { target: string; size?: number }) {
  const Icon = target.includes("@") ? Mail : target.startsWith("#") ? Hash : HOSTLIKE.test(target) ? Globe : UserRound;
  return <span className="grid shrink-0 place-items-center rounded-full bg-secondary text-muted-foreground" style={{ width: size, height: size }}><Icon style={{ width: size * 0.62, height: size * 0.62 }} strokeWidth={1.9} /></span>;
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
    () => focused.filter((t) => passes(t, filter) && (!q || [t.text, t.kind, t.title, t.client, t.host, t.source, t.session].some((s) => s?.toLowerCase().includes(q)))),
    [focused, filter, q],
  );
  const flowTrails = useMemo(() => focused.filter((t) => passes(t, filter, ["from", "source", "category", "status"])), [focused, filter]);
  const ended = byStatus(flowTrails);
  const flowSources = useMemo(() => sources(flowTrails), [flowTrails]);
  const splitBars = useMemo(
    () => bars(focused.filter((t) => passes(t, filter, [split])), (t) => (split === "kind" ? [t.kind] : split === "client" ? [t.client] : split === "source" ? [t.source ?? FROM[fromOf(t)].label] : placesOf(t))),
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
  const only = (f: Focus) => {
    setFocus(f);
    setFilter({});
    setSplit(LIST[f].split);
    setOpen(null);
  };
  // The bars' splits worth having: every value a person typed came from them, so not by source.
  const splits = SPLITS.filter(([v]) => !(focus === "prompt" && v === "source"));
  const shownInputs = useMemo(() => [...shown].sort((a, b) => RANK[b.status] - RANK[a.status] || (b.last ?? 0) - (a.last ?? 0)), [shown]);
  const listed = { all: shownValues.length, prompt: shownInputs.length, tries: shownTries.length, sources: shownFrom.length }[focus];

  return (
    <div className="flex h-full flex-col">
      <PageHeader title="민감 정보" meta={<>
        <SourceMeta label={d.store} demo={d.demo} at={d.loaded} />
        {!d.loaded && <span>불러오는 중</span>}
        {d.error && <Badge variant="secondary" className="max-w-[420px] gap-1 border-0 bg-deny/15 text-deny" title={d.error}><ShieldAlert className="size-3" /><span className="truncate">{d.error}</span></Badge>}
      </>}>
        <ToggleGroup type="single" size="sm" variant="outline" spacing={0} value={period} onValueChange={(v) => v && setPeriod(v)}>
          {PERIODS.map(([v, l]) => <ToggleGroupItem key={v} value={v} className="px-3 text-[12.5px]">{l}</ToggleGroupItem>)}
        </ToggleGroup>
      </PageHeader>

      <div className="scroll flex min-h-0 flex-1 flex-col gap-4 overflow-auto px-6 pb-6">
        <div className="grid shrink-0 grid-cols-4 gap-4">
          <Stat label="탐지된 민감정보" icon={ShieldAlert} value={values(inPeriod).length} unit={`개 · 노출 ${inPeriod.length}회`} delta={delta((l) => values(l).length)}
            active={focus === "all"} onClick={() => only("all")} />
          <Stat label="사용자 입력" icon={MessageSquareText} value={prompted.length}
            unit={`회 · 세션 ${new Set(prompted.map((t) => t.session)).size} · 단말 ${new Set(prompted.map((t) => t.client)).size}`}
            delta={delta((l) => l.filter((t) => fromOf(t) === "user").length)}
            active={focus === "prompt"} onClick={() => only("prompt")} />
          <Stat label="유출 시도" icon={Send} value={tries.length} tone={tries.some((a) => a.ran > 0) ? "var(--deny)" : undefined}
            unit={`개 · 유출 ${tries.filter((a) => a.ran > 0).length} · 차단 ${tries.filter((a) => a.ran === 0).length}`}
            active={focus === "tries"} onClick={() => only("tries")} />
          <Stat label="출처" icon={FileText} value={from.length} unit={`곳 · 값 ${from.reduce((a, s) => a + s.trails.length, 0)}`}
            active={focus === "sources"} onClick={() => only("sources")} />
        </div>

        <div className="grid shrink-0 grid-cols-[minmax(0,1fr)_340px] gap-4 min-[1600px]:grid-cols-[minmax(0,1fr)_400px]">
          {focus === "sources" ? (
            <Panel title="출처" count={flowSources.length} aside={<Legend items={STATUSES.map((k) => ({ label: STATUS[k].label, color: STATUS[k].color, value: ended[k] }))} />}>
              <SourceBars sources={flowSources} picked={filter.source} height={flowHeight} onPick={(name) => setFilter({ ...filter, source: filter.source === name ? undefined : name })} />
            </Panel>
          ) : (
            <Panel title="값의 흐름" count={flowTrails.length} aside={<Legend items={STATUSES.map((s) => ({ label: STATUS[s].label, color: STATUS[s].color, value: ended[s] }))} />}>
              <Flow trails={flowTrails} filter={filter} onFilter={setFilter} height={flowHeight} />
            </Panel>
          )}
          <Panel title={
            <ToggleGroup type="single" size="sm" value={split} onValueChange={(v) => v && setSplit(v as Split)} className="-ml-1.5">
              {splits.map(([v, l]) => <ToggleGroupItem key={v} value={v} className="px-2 text-[13px] text-muted-foreground data-[state=on]:text-foreground">{l}</ToggleGroupItem>)}
            </ToggleGroup>
          } aside={<span className="tnum">{splitBars.length}</span>}>
            <SplitBars bars={splitBars} split={split} picked={filter[split]} height={flowHeight}
              onPick={(key) => setFilter({ ...filter, [split]: filter[split] === key ? undefined : key })} />
          </Panel>
        </div>

        <Panel pad={false} className="min-h-[264px] flex-1 overflow-hidden [&>[data-slot=card-content]]:min-h-0 [&>[data-slot=card-content]]:flex-1 [&_[data-slot=table-container]]:h-full [&_[data-slot=table-container]]:overflow-auto"
          title={LIST[focus].label} count={listed}
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
          {focus === "all" && <DataTable dense columns={valueColumns(d.now)} rows={shownValues} onRow={(v) => setOpen({ value: v })}
            picked={(v) => !!open && "value" in open && open.value.key === v.key} />}
          {focus === "prompt" && <DataTable dense columns={inputColumns(d.now)} rows={shownInputs} onRow={(t) => setOpen({ input: t })}
            picked={(t) => !!open && "input" in open && open.input.id === t.id} />}
          {focus === "tries" && <DataTable dense columns={attemptColumns(d.now)} rows={shownTries} onRow={(a) => setOpen({ attempt: a })} picked={(a) => !!open && "attempt" in open && open.attempt.session === a.session} />}
          {focus === "sources" && <DataTable dense columns={sourceColumns(d.now)} rows={shownFrom} onRow={(s) => setOpen({ source: s })} picked={(s) => !!open && "source" in open && open.source.name === s.name} />}
        </Panel>
      </div>

      <Sheet open={open !== null} onOpenChange={(on) => !on && setOpen(null)}>
        <SheetContent className="gap-0 data-[side=right]:w-[520px] data-[side=right]:sm:max-w-[520px]" aria-describedby={undefined} onOpenAutoFocus={(e) => e.preventDefault()}>
          {open && "value" in open && <ValueSheet v={open.value} all={byValue.get(valueKey(open.value.trails[0])) ?? open.value.trails} now={d.now} />}
          {open && "input" in open && <ValueSheet v={values([open.input])[0]} all={byValue.get(valueKey(open.input)) ?? [open.input]} now={d.now} />}
          {open && "attempt" in open && <AttemptSheet a={open.attempt} now={d.now} />}
          {open && "source" in open && <SourceSheet s={open.source} now={d.now} />}
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

                {split === "client" && <Laptop className="size-3.5 shrink-0 text-muted-foreground" strokeWidth={1.9} />}
                {split === "source" && <FileText className="size-3.5 shrink-0 text-muted-foreground" strokeWidth={1.9} />}
                {split === "place" ? <Dest place={b.key} /> : <span className={cn("truncate", split === "source" && "font-mono text-[12px]")}>{split === "source" ? short(b.key) : b.key}</span>}
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

/** Where a value was sent: whom, by its mark, with the MCP server that carried it; or, when the
 * Call named no one, 대상 모름 with the server or tool that sent it. */
/** Where a value was sent: a place or a person, never the tool that sent it. The service (its
 * logo and name), and whom or where in it when the Call names one (`slack → #dev-alerts`,
 * `git → origin`); a host as itself. The tool it went through shows on hover. */
function Dest({ place }: { place: string }) {
  const d = targetOf(place);
  const title = (d.target ? (d.server ? `${d.server} → ${d.target}` : d.target) : d.server ?? "") + (d.tool ? `\n도구 · ${d.tool}` : "");
  if (d.server) {
    return (
      <span className="inline-flex min-w-0 items-center gap-1.5" title={title}>
        <ServiceLogo name={d.server} size={14} />
        <span className={cn("shrink-0", d.target && "text-muted-foreground")}>{d.server}</span>
        {d.target && <><ArrowRight className="size-3 shrink-0 text-muted-foreground" strokeWidth={2} /><span className="truncate">{d.target}</span></>}
      </span>
    );
  }
  if (d.target) return <span className="inline-flex min-w-0 items-center gap-1.5" title={title}><TargetIcon target={d.target} /><span className="truncate">{d.target}</span></span>;
  // Only the tool is known, which is not a place.
  return <span className="text-muted-foreground" title={d.tool ? `도구 · ${d.tool}` : place}>확인 불가</span>;
}

/** A Call that sent a value out: where to, and the tool and target it ran with. */
type Send = { place: string; tool?: string | null; what?: string | null; program?: string | null };
const sendsOf = (trails: Trail[]): Send[] => trails.flatMap((t) => t.mentions.filter((m) => m.out && m.place).map((m) => ({ place: m.place!, tool: m.tool, what: m.what, program: m.program })));
/** The tool a value went through: an MCP tool as its server and the tool of it (`agentdojo ·
 * send_email`), a command line tool by its service, a shell command by its program, else the tool. */
const toolOf = (s: Send) => {
  const d = targetOf(s.place);
  if (d.server) return d.tool ? `${d.server} · ${d.tool}` : d.server;
  // The program the app read off the command, past comments (`# …`), settings and wrappers.
  if (isShell(s.tool)) return s.program ?? toolName(s.tool);
  return d.tool ?? (s.tool ? toolName(s.tool) : null);
};
function UsedTool({ s }: { s: Send }) {
  const d = targetOf(s.place), name = toolOf(s);
  if (!name) return <span className="text-muted-foreground">–</span>;
  const Icon = isShell(s.tool) ? SquareTerminal : Wrench;
  return (
    <span className="inline-flex min-w-0 items-center gap-1.5" title={name}>
      {d.server ? <ServiceLogo name={d.server} size={14} /> : <Icon className="size-3.5 shrink-0 text-muted-foreground" strokeWidth={1.9} />}
      <Masked className="truncate">{name}</Masked>
    </span>
  );
}
/** Whom or where a value went, written out: a mail address, a channel, an account, a host. */
function Whom({ target, wrap }: { target: string; wrap?: boolean }) {
  return (
    <span className="inline-flex min-w-0 items-center gap-1.5" title={target}>
      <TargetIcon target={target} />
      <span className={wrap ? "min-w-0 break-all" : "truncate"}>{target}</span>
    </span>
  );
}
/** The first of a list, the rest counted and named on hover. */
function FirstOf({ list, show }: { list: string[]; show: (x: string) => ReactNode }) {
  if (!list.length) return <span className="text-muted-foreground">–</span>;
  return (
    <span className="flex min-w-0 items-center gap-1.5">
      {show(list[0])}
      {list.length > 1 && <span className="tnum shrink-0 text-[11.5px] text-muted-foreground" title={list.slice(1).join("\n")}>+{list.length - 1}</span>}
    </span>
  );
}
/** The tools values went through, each once. */
const Tools = ({ sends }: { sends: Send[] }) => {
  const byTool = new Map<string, Send>();
  for (const s of sends) { const k = toolOf(s); if (k && !byTool.has(k)) byTool.set(k, s); }
  return <FirstOf list={[...byTool.keys()]} show={(k) => <UsedTool s={byTool.get(k)!} />} />;
};
/** Whom values went to, each once. */
const Whoms = ({ names }: { names: string[] }) => (
  <FirstOf list={[...new Set(names.map((p) => targetOf(p).target).filter((x): x is string => !!x))]} show={(x) => <Whom target={x} />} />
);
/** The two columns of where a value went: the tool, and whom. */
function sendColumns<T>(trails: (row: T) => Trail[]): ColumnDef<T>[] {
  const places = (r: T) => [...new Set(sendsOf(trails(r)).map((s) => s.place))];
  return [
    { id: "tool", header: "사용된 도구", size: 190, accessorFn: (r) => sendsOf(trails(r)).map(toolOf).join(" "), cell: ({ row }) => <Tools sends={sendsOf(trails(row.original))} /> },
    { id: "whom", header: "유출한 곳", size: 230, accessorFn: (r) => places(r).map((p) => targetOf(p).target ?? "").join(" "), cell: ({ row }) => <Whoms names={places(row.original)} /> },
  ];
}

/** The first of where values were sent, the rest counted. */
function Places({ names, empty }: { names: string[]; empty?: ReactNode }) {
  const list = dests(names);
  if (!list.length) return <>{empty ?? null}</>;
  return (
    <span className="flex min-w-0 items-center gap-1.5">
      <Dest place={list[0]} />
      {list.length > 1 && <span className="tnum shrink-0 text-[11.5px] text-muted-foreground" title={list.slice(1).join("\n")}>+{list.length - 1}</span>}
    </span>
  );
}

const HOSTLIKE = /^(https?:\/\/)?[a-z0-9-]+(\.[a-z0-9-]+)+(:\d+)?(\/\S*)?$/i;
/** Whom or where a value was sent to, apart from what carried it: "bob@x.com · slack (MCP)" is
 * bob@x.com by slack; "slack:post_message (MCP)" is no one named, the value put into slack's post_message; a host is itself; anything else
 * is the tool that sent it, naming no one. */
const targetOf = (place: string): { target: string | null; server: string | null; tool: string | null } => {
  // An MCP server, or a command line tool (`origin · git (CLI)`): whom or where, by which service.
  if (place.endsWith("(MCP)") || place.endsWith("(CLI)")) {
    const parts = place.replace(/ \((MCP|CLI)\)$/, "").split(" · ");
    // The server, and the tool of it the value went into (`slack:post_message`).
    const [server, tool] = parts[parts.length - 1].split(/:(.*)/s);
    return { target: parts.length > 1 ? parts[0] : null, server, tool: tool || null };
  }
  if (HOSTLIKE.test(place)) return { target: place, server: null, tool: null };
  return { target: null, server: null, tool: place === "알 수 없음" ? null : place };
};
/** Where values were sent, each once, those naming whom first. */
const dests = (places: string[]) => [...new Set(places)].sort((a, b) => Number(!targetOf(a).target) - Number(!targetOf(b).target));

/** A document or tool a value came out of, as a cell names it: an MCP tool by its server's logo
 * and its own name, a path by its file name. */
function SourceName({ name }: { name: string }) {
  const mcp = /^mcp__(.+?)__(.+)$/.exec(name);
  if (mcp) return <span className="flex min-w-0 items-center gap-1.5" title={name}><ServiceLogo name={mcp[1]} size={16} /><Masked className="truncate">{mcp[2]}</Masked></span>;
  return <Masked className="truncate" >{short(name)}</Masked>;
}

/** What a person typed a value in, else the session's name. */
const typedIn = (t: Trail) => t.mentions.find((m) => m.said)?.said ?? sessionName(t);

/** Why the sends that ran got out, each with how many: the engine's leave, no decision, the
 * person's leave, or the engine's mode as it names it. */
function Whys({ whys }: { whys: Partial<Record<Why, number>> }) {
  const shown = WHYS.filter((w) => (whys[w] ?? 0) > 0);
  if (!shown.length) return null;
  return (
    <span className="flex min-w-0 flex-wrap items-center gap-1">
      {shown.map((w) => (
        <span key={w} className={cn("tnum inline-flex shrink-0 items-center gap-1 rounded px-1.5 text-[11px] leading-[19px] font-medium", WHY[w].cls, WHY[w].mono && "font-mono")}>
          {WHY[w].label}{(whys[w] ?? 0) > 1 && <span className="opacity-70">{whys[w]}</span>}
        </span>
      ))}
    </span>
  );
}

/** Where a value came in, as a cell names it: a person, the document or tool, or not known. */
function OriginName({ origin }: { origin: string }) {
  if (origin === "user") return <span className="flex min-w-0 items-center gap-1.5"><MessageSquareText className="size-3.5 shrink-0 text-muted-foreground" strokeWidth={1.9} /><span className="truncate">사용자 입력</span></span>;
  if (origin === "agent") return <span className="flex min-w-0 items-center gap-1.5 text-muted-foreground"><Bot className="size-3.5 shrink-0" strokeWidth={1.9} /><span className="truncate">에이전트 작성</span></span>;
  if (origin === "earlier") return <span className="flex min-w-0 items-center gap-1.5"><History className="size-3.5 shrink-0 text-muted-foreground" strokeWidth={1.9} /><span className="truncate">이전 세션</span></span>;
  const p = parts(origin);
  return (
    <span className="flex min-w-0 items-center gap-1.5" title={origin}>
      {!p.server && (p.command ? <SquareTerminal className="size-3.5 shrink-0 text-muted-foreground" strokeWidth={1.9} /> : <FileText className="size-3.5 shrink-0 text-muted-foreground" strokeWidth={1.9} />)}
      <SourceName name={origin} />
    </span>
  );
}
const originOf = (t: Trail) => (fromOf(t) === "user" ? "user" : t.source ?? (fromOf(t) === "earlier" ? "earlier" : "agent"));

/** A value over every session it showed in: how it ended at its worst, where it was sent, the
 * Call that carried it, where it came from, and how far it spread. */
const valueColumns = (now: number): ColumnDef<Value>[] => [
  { id: "status", header: "결과", size: 92, accessorFn: (v) => RANK[v.status], cell: ({ row: { original: v } }) => <StatusBadge s={v.status} /> },
  ...sendColumns<Value>((v) => v.trails),
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
  { id: "from", header: "유입 경로", size: 280, accessorFn: (v) => `${v.froms.join(" ")} ${v.sources.join(" ")}`, cell: ({ row: { original: v } }) => {
      const typed = v.froms.includes("user"), from = typed ? "user" : v.froms[0];
      return (
        <span className="flex min-w-0 items-center gap-1.5" title={v.sources.join("\n") || undefined}>
          <span className="shrink-0 rounded bg-secondary px-1.5 text-[11.5px] leading-[19px] text-foreground/85">{FROM[from].label}</span>
          {!typed && v.sources.length > 0 && <><SourceName name={v.sources[0]} />{v.sources.length > 1 && <span className="tnum shrink-0 text-muted-foreground">+{v.sources.length - 1}</span>}</>}
        </span>
      );
    } },
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

/** A value a person put in a prompt, once a session: how it ended, the value, the prompt, and
 * the machine. */
const inputColumns = (now: number): ColumnDef<Trail>[] => [
  { id: "status", header: "결과", size: 92, accessorFn: (t) => RANK[t.status], cell: ({ row: { original: t } }) => <StatusBadge s={t.status} /> },
  ...sendColumns<Trail>((t) => [t]),
  { id: "text", header: "값", size: 250, accessorFn: (t) => t.text, cell: ({ row: { original: t } }) => {
    const Icon = CATEGORY[t.category as Category]?.icon;
    return (
      <span className="flex min-w-0 items-center gap-2" title={t.text}>
        {Icon && <Icon className="size-3.5 shrink-0 text-muted-foreground" strokeWidth={1.9} />}
        <Masked className="min-w-0 truncate">{t.text}</Masked>
        <span className="shrink-0 text-[12px] text-muted-foreground">{t.kind}</span>
      </span>
    );
  } },
  { id: "typed", header: "입력 프롬프트", size: 320, accessorFn: typedIn, cell: ({ row: { original: t } }) => <span className="block truncate" title={typedIn(t)}>{typedIn(t)}</span> },
  { id: "client", header: "단말", size: 150, accessorFn: machineName, cell: ({ row: { original: t } }) => <Client name={machineName(t)} /> },
  { id: "last", header: "마지막", size: 88, accessorFn: (t) => t.last ?? 0, cell: ({ row: { original: t } }) => <When at={t.last} now={now} /> },
  // The prompt it was typed in, in 기록.
  { id: "open", header: "", size: 44, enableSorting: false, cell: ({ row: { original: t } }) => {
    const m = t.mentions.find((x) => x.said) ?? t.mentions[0];
    return m ? (
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
  { id: "status", header: "결과", size: 200, accessorFn: (a) => Number(a.ran > 0), cell: ({ row: { original: a } }) => <span className="flex min-w-0 items-center gap-1.5"><StatusBadge s={a.ran > 0 ? "out" : "blocked"} /><Whys whys={a.whys} /></span> },
  { id: "from", header: "출처", size: 200, accessorFn: (a) => a.trails.map(originOf).join(" "), cell: ({ row: { original: a } }) => {
    const from = [...new Set(a.trails.map(originOf))].sort((x, y) => Number(x === "agent") - Number(y === "agent"));
    return <FirstOf list={from} show={(o) => <OriginName origin={o} />} />;
  } },
  ...sendColumns<Attempt>((a) => a.trails),
  { id: "session", header: "세션", size: 220, accessorFn: (a) => sessionName(a), cell: ({ row: { original: a } }) => <span className="block truncate" title={sessionName(a)}>{sessionName(a)}</span> },
  { id: "values", header: "값", size: 170, accessorFn: (a) => a.trails.length, cell: ({ row: { original: a } }) => <Kinds trails={a.trails} /> },
  { id: "sent", header: "전송", size: 124, accessorFn: (a) => a.ran * 1000 + a.held, cell: ({ row: { original: a } }) => <Sent ran={a.ran} held={a.held} /> },
  { id: "client", header: "단말", size: 150, accessorFn: (a) => a.client, cell: ({ row: { original: a } }) => <Client name={a.client} /> },
  { id: "last", header: "마지막", size: 88, accessorFn: (a) => a.last ?? 0, cell: ({ row: { original: a } }) => <When at={a.last} now={now} /> },
];

/** A document, command or tool the agents read values from: what it is, how many values and how
 * they ended, which kinds, and how far they spread. */
const sourceColumns = (now: number): ColumnDef<Source>[] => [
  { id: "name", header: "출처", accessorFn: (s) => s.name, cell: ({ row: { original: s } }) => {
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
  ...sendColumns<Source>((s) => s.trails),
  { id: "spread", header: "영향 범위", size: 132, accessorFn: (s) => s.sessions * 1000 + s.clients, cell: ({ row: { original: s } }) => <Spread sessions={s.sessions} clients={s.clients} /> },
  { id: "last", header: "마지막", size: 88, accessorFn: (s) => s.last ?? 0, cell: ({ row: { original: s } }) => <When at={s.last} now={now} /> },
];

// ---------------------------------------------------------------- the sheet
function Section({ title, count, children }: { title: string; count?: number; children: ReactNode }) {
  return (
    <section className="grid gap-2">
      <h3 className="flex items-center gap-2 text-[12px] font-medium text-muted-foreground">{title}{count !== undefined && <span className="tnum rounded-full bg-secondary px-1.5 text-[11px]">{count}</span>}</h3>
      {children}
    </section>
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

const toolName = (tool: string | null | undefined) => (tool ?? "").replace(/^mcp__/, "").replace(/__/g, " · ");
const Tool = ({ name }: { name?: string | null }) => (name ? <span className="max-w-36 shrink-0 truncate rounded-md bg-secondary px-1.5 py-0.5 text-[11px] text-muted-foreground">{toolName(name)}</span> : null);

/** Where a value came in, said plainly: by what way, and the document, tool or prompt itself. */
function Origin({ t }: { t: Trail }) {
  const said = t.mentions.find((m) => m.said);
  const from = fromOf(t), Icon = FROM[from].icon;
  return (
    <button onClick={() => { const m = said ?? t.mentions[0]; if (m) openInRecord(t.session, m.seq); }}
      className="group grid min-w-0 gap-1.5 rounded-lg border bg-background px-3 py-2.5 text-left transition-colors hover:bg-accent/60">
      <span className="flex items-center gap-1.5 text-[12px] font-medium"><Icon className="size-3.5 text-muted-foreground" strokeWidth={1.9} />{FROM[from].label}<Tool name={said ? null : t.sourceTool} /><ArrowUpRight className="ml-auto size-3.5 text-muted-foreground opacity-0 transition-opacity group-hover:opacity-100" /></span>
      {said ? <Said text={said.said!} value={t.text} />
        : t.source ? <span className="grid gap-0.5"><Masked className="text-[13px] font-medium text-foreground">{short(t.source)}</Masked>{short(t.source) !== t.source && <Masked className="text-[11.5px] break-all text-muted-foreground">{t.source}</Masked>}</span>
          : <span className="text-[12.5px] text-muted-foreground">어디서 왔는지 모름</span>}
    </button>
  );
}

/** How a value went, as two ends joined by an arrow: how it came in, and whom it was sent to
 * with the tool that sent it. The arrow carries how it ended. */
function Path({ trails, status }: { trails: Trail[]; status: Status }) {
  const first = trails[0], from = fromOf(first), was = first.before;
  const Icon = from === "earlier" ? FROM.external.icon : FROM[from].icon;
  const typed = trails.some((t) => fromOf(t) === "user");
  // A value come in in an earlier session names that session under how it came in there.
  const earlier = was && <span className="flex min-w-0 items-center gap-1 text-[11.5px] text-muted-foreground" title={was.title ?? was.session}><History className="size-3 shrink-0" strokeWidth={1.9} /><span className="shrink-0">이전 세션</span><span className="truncate">· {was.title ?? was.session.slice(0, 8)}</span></span>;
  // The Calls that sent it, the first that got out leading.
  const sends = trails.flatMap((t) => t.mentions.filter((m) => m.out && m.place)).sort((a, b) => Number(a.held) - Number(b.held) || Number(!targetOf(a.place!).target) - Number(!targetOf(b.place!).target));
  const lead = sends[0];
  const where = lead ? targetOf(lead.place!) : null;
  const others = new Set(sends.map((m) => m.place)).size - 1;
  const tone = status === "out" ? "var(--deny)" : status === "blocked" ? "var(--review)" : "var(--border)";
  const end = "flex min-w-0 flex-col gap-1 rounded-lg border bg-background px-3 py-2.5";
  return (
    <div className="grid grid-cols-[minmax(0,1fr)_64px_minmax(0,1fr)] items-center">
      <div className={cn(end, from === "agent" && !typed && "border-dashed")}>
        <span className="text-[11px] text-muted-foreground">유입 경로</span>
        {typed ? <span className="flex items-center gap-1.5 text-[13px] font-medium"><MessageSquareText className="size-3.5 text-muted-foreground" strokeWidth={1.9} />사용자 입력</span>
          : first.source ? (
            <>
              <span className="flex min-w-0 items-center gap-1.5 text-[13px] font-medium"><Icon className="size-3.5 shrink-0 text-muted-foreground" strokeWidth={1.9} /><SourceName name={first.source} /></span>
              {earlier || <span className="truncate text-[11.5px] text-muted-foreground" title={first.source}>{[first.sourceTool && toolName(first.sourceTool), short(first.source) !== first.source && first.source].filter(Boolean).join(" · ")}</span>}
            </>
          ) : was ? (
            <>
              <span className="flex items-center gap-1.5 text-[13px] font-medium"><MessageSquareText className="size-3.5 text-muted-foreground" strokeWidth={1.9} />사용자 입력</span>
              {earlier}
            </>
          ) : <span className="flex items-center gap-1.5 text-[13px] text-muted-foreground"><Icon className="size-3.5" strokeWidth={1.9} />에이전트 작성</span>}
      </div>
      <div className="flex flex-col items-center gap-1 px-1.5">
        {status !== "seen" && <span className="text-[11px] font-semibold" style={{ color: tone }}>{STATUS[status].label}</span>}
        <span className="relative h-0.5 w-full rounded-full" style={{ background: tone }}><ArrowRight className="absolute -top-[7px] -right-1.5 size-4" style={{ color: tone }} /></span>
      </div>
      <div className={end} style={status !== "seen" ? { borderColor: `color-mix(in oklab, ${tone} 55%, transparent)`, background: `color-mix(in oklab, ${tone} 8%, var(--background))` } : undefined}>
        {where ? (
          <>
            <span className="text-[11px] text-muted-foreground">유출한 곳</span>
            <span className="flex min-w-0 items-start gap-1.5 text-[13px] font-medium">
              {where.target ? <Whom target={where.target} wrap /> : <span className="font-normal text-muted-foreground">대상 없음</span>}
              {others > 0 && <span className="tnum shrink-0 text-[11.5px] font-normal text-muted-foreground" title={[...new Set(sends.map((m) => m.place))].slice(1).join("\n")}>+{others}</span>}
            </span>
            <span className="mt-1 text-[11px] text-muted-foreground">사용된 도구</span>
            <span className="flex min-w-0 items-center text-[12px]"><UsedTool s={{ place: lead!.place!, tool: lead!.tool, what: lead!.what, program: lead!.program }} /></span>
          </>
        ) : <><span className="text-[11px] text-muted-foreground">유출한 곳</span><span className="text-[13px] text-muted-foreground">밖으로 안 나감</span></>}
      </div>
    </div>
  );
}

/** The machines a value showed on: each with how many sessions, how it ended at worst, and when
 * last; pressed, its worst session opens in 기록 where it mattered most. */
function Machines({ trails, now }: { trails: Trail[]; now: number }) {
  const by = new Map<string, Trail[]>();
  for (const t of trails) by.set(machineName(t), [...(by.get(machineName(t)) ?? []), t]);
  const rows = [...by.entries()].map(([machine, list]) => {
    const worst = list.reduce((a, t) => (RANK[t.status] > RANK[a.status] ? t : a), list[0]);
    const latest = list.reduce((a, t) => ((t.last ?? 0) > (a.last ?? 0) ? t : a), list[0]);
    return { machine, list, worst, latest };
  }).sort((a, b) => RANK[b.worst.status] - RANK[a.worst.status] || (b.latest.last ?? 0) - (a.latest.last ?? 0));
  return (
    <div className="grid gap-1">
      {rows.map(({ machine, list, worst, latest }) => {
        const m = worst.mentions.find((x) => x.out && !x.held) ?? worst.mentions.find((x) => x.out) ?? worst.mentions[0];
        return (
          <button key={machine} onClick={() => m && openInRecord(worst.session, m.seq)} className="group flex min-w-0 items-center gap-3 rounded-md bg-background/60 px-2.5 py-2 text-left text-[12.5px] transition-colors hover:bg-accent">
            <span className="min-w-0 flex-1"><Client name={machine} /></span>
            <span className="tnum shrink-0 text-[12px] text-muted-foreground">세션 {list.length}</span>
            <StatusBadge s={worst.status} />
            <span className="w-14 shrink-0 text-right"><When at={latest.last} now={now} /></span>
          </button>
        );
      })}
    </div>
  );
}

/** A value: how it went, from where to where; where it came in, plainly; and who it was. */
function ValueSheet({ v, all, now }: { v: Value; all: Trail[]; now: number }) {
  const Icon = CATEGORY[v.category as Category]?.icon;
  // What a person typed the value in, each prompt once; a document or tool is named in the path.
  const typedOrigins = all.filter((t) => t.mentions.some((m) => m.said)).filter((t, i, list) => list.findIndex((x) => (x.source ?? x.mentions.find((m) => m.said)?.said ?? fromOf(x)) === (t.source ?? t.mentions.find((m) => m.said)?.said ?? fromOf(t))) === i);
  return (
    <>
      <SheetHeader className="gap-2 border-b pr-12">
        <SheetTitle className="flex items-center gap-2 text-[15px]">{Icon && <Icon className="size-4 text-muted-foreground" strokeWidth={1.9} />}<Masked className="truncate text-[14px]">{v.text}</Masked><span className="shrink-0 text-[12.5px] font-normal text-muted-foreground">{v.kind}</span><span className="ml-1"><StatusBadge s={v.status} /></span></SheetTitle>
        <SheetDescription className="sr-only">{v.kind}</SheetDescription>
      </SheetHeader>
      <div className="scroll grid min-h-0 flex-1 content-start gap-5 overflow-auto p-4">
        <Path trails={all} status={v.status} />
        {typedOrigins.length > 0 && (
          <Section title="입력 프롬프트" count={typedOrigins.length > 1 ? typedOrigins.length : undefined}>
            {typedOrigins.slice(0, 3).map((t) => <Origin key={t.id} t={t} />)}
          </Section>
        )}
        <Section title="단말" count={new Set(all.map(machineName)).size}>
          <Machines trails={all} now={now} />
        </Section>
      </div>
    </>
  );
}

/** A session that tried to send values out: how they went, what the person asked, and which values. */
function AttemptSheet({ a, now }: { a: Attempt; now: number }) {
  const said = a.trails.flatMap((t) => t.mentions).find((m) => m.said)?.said ?? a.title;
  return (
    <>
      <SheetHeader className="gap-2 border-b pr-12">
        <SheetTitle className="flex items-center gap-2 text-[15px]">유출 시도<span className="ml-1"><StatusBadge s={a.ran > 0 ? "out" : "blocked"} /></span><Whys whys={a.whys} /></SheetTitle>
        <SheetDescription className="sr-only">유출 시도</SheetDescription>
      </SheetHeader>
      <div className="scroll grid min-h-0 flex-1 content-start gap-5 overflow-auto p-4">
        <Path trails={a.trails} status={a.ran > 0 ? "out" : "blocked"} />
        <Section title="사용자 요청">
          <div className="rounded-lg border bg-background px-3 py-2.5 text-[13px] leading-relaxed break-words whitespace-pre-wrap">{said ?? <span className="text-muted-foreground">프롬프트 없음</span>}</div>
        </Section>
        <Section title="실려 간 값" count={a.trails.length}>
          <div className="flex flex-wrap gap-1.5">
            {a.trails.map((t) => <span key={t.id} className="inline-flex items-center gap-1.5 rounded-md bg-secondary px-2 py-1 text-[12px]"><Masked>{t.text}</Masked><span className="text-muted-foreground">{t.kind}</span></span>)}
          </div>
        </Section>
        <Section title="단말">
          <Machines trails={a.trails.slice(0, 1)} now={now} />
        </Section>
      </div>
    </>
  );
}

/** A document or tool values came out of: what came out and how it ended, and who read it. */
function SourceSheet({ s, now }: { s: Source; now: number }) {
  const n = byStatus(s.trails);
  const readers = [...new Map(s.trails.map((t) => [t.session, t] as const)).values()].sort((a, b) => (b.last ?? 0) - (a.last ?? 0));
  return (
    <>
      <SheetHeader className="gap-3 border-b pr-12">
        <SheetTitle className="flex items-center gap-2 text-[15px]"><FileText className="size-4 shrink-0 text-muted-foreground" strokeWidth={1.9} /><span className="truncate">{short(s.name)}</span><Tool name={s.tool} /></SheetTitle>
        <SheetDescription asChild><Masked className="block text-[12px] break-all text-muted-foreground">{s.name}</Masked></SheetDescription>
        <div className="flex items-center gap-3 text-[12.5px]">
          <span className="tnum">값 <b className="font-semibold">{s.trails.length}</b></span>
          <Sent ran={n.out} held={n.blocked} />
          <span className="ml-auto"><Spread sessions={s.sessions} clients={s.clients} /></span>
        </div>
      </SheetHeader>
      <div className="scroll grid min-h-0 flex-1 content-start gap-5 overflow-auto p-4">
        <Section title="여기서 나온 값" count={s.trails.length}>
          <div className="grid gap-1">
            {[...s.trails].sort((a, b) => RANK[b.status] - RANK[a.status]).map((t) => (
              <div key={t.id} className="flex min-w-0 items-center gap-2 rounded-md bg-background/60 px-2.5 py-2 text-[12.5px]">
                <Masked className="min-w-0 truncate">{t.text}</Masked>
                <span className="shrink-0 text-muted-foreground">{t.kind}</span>
                <span className="ml-auto flex shrink-0 items-center gap-2">{t.status !== "seen" && <Places names={placesOf(t)} />}<StatusBadge s={t.status} /></span>
              </div>
            ))}
          </div>
        </Section>
        <Section title="읽은 단말" count={new Set(readers.map(machineName)).size}>
          <Machines trails={readers} now={now} />
        </Section>
      </div>
    </>
  );
}
