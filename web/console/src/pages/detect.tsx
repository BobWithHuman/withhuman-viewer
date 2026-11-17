// 보안 현황, the security team's main page, in three parts, top to bottom. Over time: the Calls the engine
// stopped, held or allowed though hard to undo, day by day (dragging across the days makes them
// the period), under their counts.
// The riskiest: the machines, sessions, tools or kinds of action ranked by the weight of the
// Calls worth a look on each, beside it. Under them, the list of what to look at first, a session a row. Pressing a band, a figure, a machine or a row
// narrows the rest; what is applied shows as chips, each removable. The counting is done here,
// over the Calls the app hands over once.

import { LoaderCircle, Settings, X } from "lucide-react";
import { useMemo, useState } from "react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { Skeleton } from "@/components/ui/skeleton";
import { Legend, PageHeader, Panel, VERDICT, Warn } from "@/kit";
import { cn } from "@/lib/utils";
import { send } from "../bridge";
import { type RankBy, Ranking } from "./detect/breakdown";
import { type Call, DIM, DIMS, type Data, type Dim, type Filter, MODE, type Period, VERDICTS, coverage, demo, named, passes, rangeLabel, span, within } from "./detect/data";
import { type Ask, type Told, Watch } from "./detect/list";
import { Trend } from "./detect/trend";

const PERIODS: [number, string][] = [[1, "24시간"], [7, "7일"], [30, "30일"], [0, "전체"]];
/** How a figure moved against the stretch before, in percent; nothing when there is no before. */
const moved = (now: number, before: number | null) => (before ? Math.round(((now - before) / before) * 100) : null);
const hm = (ms: number) => { const d = new Date(ms), z = (n: number) => String(n).padStart(2, "0"); return `${z(d.getHours())}:${z(d.getMinutes())}`; };
const FILL = "[&>[data-slot=card-content]]:flex [&>[data-slot=card-content]]:min-h-0 [&>[data-slot=card-content]]:flex-1 [&>[data-slot=card-content]]:flex-col";

export default function Page({ data }: { data: unknown | null }) {
  // The moment the data came is "now" for every period, so the counts hold still between reads.
  const now = useMemo(() => Date.now(), [data]);
  const d = useMemo(() => (data as Data | null) ?? demo(now), [data, now]);
  const [period, setPeriod] = useState<Period>({ days: 7, range: null });
  const [filter, setFilter] = useState<Filter>({});
  const [rankBy, setRankBy] = useState<RankBy>("machine");
  const ask: Ask = (message) => send("detect", message);

  const toggle = (dim: Dim, key: string) => setFilter((f) => ({ ...f, [dim]: f[dim] === key ? undefined : key }));
  const { from, to, before } = span(period, now);
  const inPeriod = useMemo(() => d.calls.filter((c) => within(c, from, to)), [d, from, to]);
  const shown = useMemo(() => inPeriod.filter((c) => passes(c, filter)), [inPeriod, filter]);
  // Each figure and the machines count with their own choice left out, so it stays a choice.
  const byVerdict = useMemo(() => inPeriod.filter((c) => passes(c, filter, "verdict")), [inPeriod, filter]);
  const ranked = useMemo(() => inPeriod.filter((c) => passes(c, filter, rankBy)), [inPeriod, filter, rankBy]);
  const earlier = useMemo(() => (before === null ? null : d.calls.filter((c) => within(c, before, from) && passes(c, filter, "verdict"))), [d, before, from, filter]);
  const count = (list: Call[] | null, v: string) => (list ? list.filter((c) => c.verdict === v).length : null);
  const machine = filter.machine === undefined ? null : d.machines.indexOf(filter.machine);
  const cover = useMemo(() => coverage(d.cover, from, to, machine), [d, from, to, machine]);
  // The chart's stretch: all time starts at the first Call worth a look.
  const first = useMemo(() => d.calls.reduce((a, c) => (c.at !== null && c.at < a ? c.at : a), now), [d, now]);
  const mode = MODE[cover.mode] ?? MODE.none;
  const blank = d.loading && !d.calls.length;
  const marks = d.triage ?? {}, blocked = d.blocked ?? [];
  // The machines to do something about: those with Calls nothing decided, set up yet never
  // heard, heard yet never set up, newly registered, or gone quiet.
  const alerts = useMemo<Told[]>(() => {
    const quiet = new Set<number>();
    for (const [hour, m, , , undecided] of d.cover) if (undecided > 0 && hour + 3_600_000 > from && hour < to) quiet.add(m);
    const of = (list: typeof d.alerts extends infer A ? A extends { new: infer L } ? L : never : never, why: Told["why"]) => (list ?? []).map((a) => ({ ...a, why }));
    return [
      ...[...quiet].map((m) => ({ id: d.machines[m], host: null, user: null, at: null, edr: null, why: "undecided" as const })),
      ...of(d.alerts?.installed ?? [], "installed"), ...of(d.alerts?.unregistered ?? [], "unregistered"),
      ...of(d.alerts?.new ?? [], "new"), ...of(d.alerts?.silent ?? [], "silent"),
    ];
  }, [d, from, to]);
  // The sessions of the period the team marked an attack.
  const attacks = useMemo(() => new Set(inPeriod.filter((c) => marks[c.session]?.state === "attack").map((c) => c.session)).size, [inPeriod, marks]);
  const chips: { key: string; label: string; value: string; drop: () => void }[] = [
    ...(period.range ? [{ key: "range", label: "기간", value: rangeLabel(period.range), drop: () => setPeriod((p) => ({ ...p, range: null })) }] : []),
    ...DIMS.filter((dim) => filter[dim] !== undefined).map((dim) => ({ key: dim, label: DIM[dim].label, value: dim === "session" ? named(d.calls.find((c) => c.session === filter.session)) ?? DIM.session.name(filter.session!) : DIM[dim].name(filter[dim]!), drop: () => setFilter((f) => ({ ...f, [dim]: undefined })) })),
  ];

  return (
    <div className="flex h-full min-w-0 flex-col">
      <PageHeader
        title="보안 현황"
        meta={<>
          <Badge variant="secondary" className={`gap-1 border-0 font-semibold ${mode.cls}`}><i className="size-1.5 rounded-full bg-current" />{mode.label}</Badge>
          {d.failed && <Tooltip><TooltipTrigger asChild><span><Warn>불러오기 실패</Warn></span></TooltipTrigger><TooltipContent className="max-w-80 break-words">{d.failed}</TooltipContent></Tooltip>}
          {d.refreshed && <span className="tnum">갱신 {hm(d.refreshed)}</span>}
          {d.loading && <span className="inline-flex items-center gap-1.5"><LoaderCircle className="size-3.5 animate-spin" />불러오는 중</span>}
          {chips.map((c) => (
            <span key={c.key} className="inline-flex h-6 max-w-72 items-center gap-1.5 rounded-full bg-primary/15 pr-1 pl-2.5 text-[12px] text-primary">
              <span className="shrink-0 opacity-75">{c.label}</span>
              <span className="min-w-0 truncate font-medium">{c.value}</span>
              <button onClick={c.drop} aria-label={`${c.label} 지우기`} className="grid size-4 shrink-0 place-items-center rounded-full transition-colors hover:bg-primary/25"><X className="size-3" /></button>
            </span>
          ))}
          {chips.length > 1 && <Button variant="ghost" size="xs" className="text-muted-foreground" onClick={() => { setFilter({}); setPeriod((p) => ({ ...p, range: null })); }}>모두 지우기</Button>}
        </>}
      >
        <ToggleGroup type="single" variant="outline" size="sm" spacing={0} value={period.range ? "" : String(period.days)} onValueChange={(v) => v && setPeriod({ days: Number(v), range: null })}>
          {PERIODS.map(([days, label]) => <ToggleGroupItem key={days} value={String(days)} className="px-3 text-[12.5px]">{label}</ToggleGroupItem>)}
        </ToggleGroup>
        <Tooltip>
          <TooltipTrigger asChild><Button variant="ghost" size="icon-sm" aria-label="연결 설정" onClick={() => ask({ do: "settings" })}><Settings /></Button></TooltipTrigger>
          <TooltipContent>연결 설정</TooltipContent>
        </Tooltip>
      </PageHeader>

      <div className="scroll min-h-0 flex-1 overflow-y-auto px-6 pb-6">
        <div className="flex min-h-full flex-col gap-3">
          <div className="grid h-[clamp(360px,42vh,480px)] shrink-0 grid-cols-[minmax(0,2.4fr)_minmax(0,1fr)] gap-3">
            <Panel
              className={`h-full ${FILL}`}
              title={
                <div className="flex items-baseline gap-5 font-normal">
                  {VERDICTS.map((v) => {
                    const n = count(byVerdict, v)!, delta = moved(n, count(earlier, v));
                    return (
                      <button key={v} onClick={() => toggle("verdict", v)} aria-pressed={filter.verdict === v}
                        className={cn("flex items-baseline gap-1.5 rounded-md px-1.5 py-0.5 transition-colors hover:bg-accent", filter.verdict === v && "bg-accent ring-1 ring-ring", filter.verdict !== undefined && filter.verdict !== v && "opacity-45")}>
                        <span className="text-[12.5px] text-muted-foreground">{DIM.verdict.name(v)}</span>
                        <span className="tnum text-[20px] leading-none font-semibold tracking-[-0.01em]" style={{ color: VERDICT[v].color }}>{blank ? "–" : n.toLocaleString()}</span>
                        {delta !== null && delta !== 0 && <span className="tnum text-[11.5px] text-muted-foreground">{delta > 0 ? "+" : "−"}{Math.abs(delta)}%</span>}
                      </button>
                    );
                  })}
                  <span className="flex items-baseline gap-1.5 px-1.5">
                    <span className="text-[12.5px] text-muted-foreground">확인된 공격</span>
                    <span className={cn("tnum text-[20px] leading-none font-semibold tracking-[-0.01em]", attacks ? "text-deny" : "text-muted-foreground")}>{attacks}</span>
                  </span>
                </div>
              }
            >
              <div className="min-h-0 flex-1">
                {blank
                  ? <Skeleton className="h-full w-full rounded-lg" />
                  : <Trend calls={shown} from={Number.isFinite(from) ? from : first} to={Number.isFinite(to) ? to : now + 1} onRange={(range) => setPeriod((p) => ({ ...p, range }))} />}
              </div>
              <div className="flex shrink-0 items-center pt-3">
                <Legend items={VERDICTS.map((v) => ({ label: DIM.verdict.name(v), color: VERDICT[v].color }))} />
                <span className="tnum ml-auto text-[12px] text-muted-foreground">{period.range ? rangeLabel(period.range) : PERIODS.find((p) => p[0] === period.days)?.[1]}</span>
              </div>
            </Panel>

            <Panel className={`h-full ${FILL}`} pad={false}
              title="위험 순위"
              aside={(
                <ToggleGroup type="single" size="sm" value={rankBy} onValueChange={(v) => v && setRankBy(v as RankBy)}>
                  {([["machine", "단말"], ["session", "세션"], ["tool", "도구"]] as [RankBy, string][]).map(([v, l]) => <ToggleGroupItem key={v} value={v} className="px-2.5 text-[12.5px]">{l}</ToggleGroupItem>)}
                </ToggleGroup>
              )}>
              <div className="scroll min-h-0 flex-1 overflow-y-auto px-2 py-2">
                {blank
                  ? <div className="flex flex-col gap-2 px-2 py-1">{Array.from({ length: 6 }, (_, i) => <Skeleton key={i} className="h-9 rounded-lg" />)}</div>
                  : <Ranking calls={ranked} by={rankBy} picked={filter[rankBy]} onPick={(key) => toggle(rankBy, key)} />}
              </div>
            </Panel>
          </div>
          <Watch calls={shown} ask={ask} loading={blank} marks={marks} caseData={d.case} saved={d.saved} blocked={blocked} alerts={alerts} className="h-[clamp(420px,58vh,760px)] shrink-0" />
        </div>
      </div>
    </div>
  );
}
