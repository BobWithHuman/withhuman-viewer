// 보안 현황, the security team's main page, in three parts. Over time: the Calls the engine
// stopped, held or allowed though hard to undo, day by day (dragging across the days makes them
// the period), under their counts.
// The riskiest: the machines, sessions, tools or kinds of action ranked by the weight of the
// Calls worth a look on each. And the list of what to look at first, a session a row. Pressing a band, a figure, a machine or a row
// narrows the rest; what is applied shows as chips, each removable. The counting is done here,
// over the Calls the app hands over once.

import { Download, LoaderCircle, Settings, X } from "lucide-react";
import { useMemo, useState } from "react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { Skeleton } from "@/components/ui/skeleton";
import { Legend, PageHeader, Panel, SplitBar, VERDICT, Warn } from "@/kit";
import { cn } from "@/lib/utils";
import { send } from "../bridge";
import { type RankBy, Ranking } from "./detect/breakdown";
import { type Call, DIM, DIMS, type Data, type Dim, type Filter, MODE, type Period, VERDICTS, actionTotals, coverage, demo, passes, rangeLabel, span, within } from "./detect/data";
import { type Ask, type Told, Watch } from "./detect/list";
import { Trend } from "./detect/trend";
import { Tuning } from "./detect/tuning";

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
  // The side panel: what is riskiest, or what the decisions suggest changing.
  const [side, setSide] = useState<"rank" | "tune">("rank");
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
  const totals = useMemo(() => actionTotals(d.flow ?? [], from, to, machine), [d, from, to, machine]);
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
  // The period as a report: its counts, the days, the gravest kinds, what the team decided
  // and changed, and every Call worth a look.
  const report = () => {
    const days = new Map<string, Record<string, number>>();
    for (const c of inPeriod) {
      if (c.at === null) continue;
      const day = new Date(c.at).toISOString().slice(0, 10), row = days.get(day) ?? { deny: 0, review: 0, allow: 0 };
      row[c.verdict]++;
      days.set(day, row);
    }
    const grave = inPeriod.filter((c) => c.kind === "exfiltration" || c.kind === "hijacked_goal");
    ask({
      do: "export", name: `withhuman-report-${new Date().toISOString().slice(0, 10)}.json`,
      text: JSON.stringify({
        exported_at: new Date().toISOString(), source: d.source,
        period: { from: Number.isFinite(from) ? new Date(from).toISOString() : null, to: new Date(Math.min(to, now)).toISOString() },
        counts: { deny: count(inPeriod, "deny"), review: count(inPeriod, "review"), allowed_hard_to_undo: count(inPeriod, "allow"), confirmed_attacks: attacks, marked_benign: new Set(inPeriod.filter((c) => marks[c.session]?.state === "benign").map((c) => c.session)).size },
        by_day: [...days.entries()].sort().map(([day, n]) => ({ day, ...n })),
        grave: { exfiltration: grave.filter((c) => c.kind === "exfiltration").length, hijacked_goal: grave.filter((c) => c.kind === "hijacked_goal").length, calls: grave },
        decisions: Object.entries(marks).map(([session, m]) => ({ session, ...m })).sort((a, b) => b.at - a.at),
        distrusted_sources: blocked,
        calls: inPeriod,
      }, null, 2),
    });
  };

  const chips: { key: string; label: string; value: string; drop: () => void }[] = [
    ...(period.range ? [{ key: "range", label: "기간", value: rangeLabel(period.range), drop: () => setPeriod((p) => ({ ...p, range: null })) }] : []),
    ...DIMS.filter((dim) => filter[dim] !== undefined).map((dim) => ({ key: dim, label: DIM[dim].label, value: dim === "session" ? d.calls.find((c) => c.session === filter.session)?.prompt ?? DIM.session.name(filter.session!) : DIM[dim].name(filter[dim]!), drop: () => setFilter((f) => ({ ...f, [dim]: undefined })) })),
  ];

  return (
    <div className="flex h-full min-w-0 flex-col">
      <PageHeader
        title="보안 현황"
        meta={<>
          <span className="inline-flex items-center gap-1.5"><i className={`size-1.5 rounded-full ${d.demo ? "bg-review" : "bg-allow"}`} /><span className="max-w-64 truncate">{d.source}</span></span>
          <Badge variant="secondary" className={`gap-1 border-0 font-semibold ${mode.cls}`}><i className="size-1.5 rounded-full bg-current" />{mode.label}</Badge>
          {d.failed && <Tooltip><TooltipTrigger asChild><span><Warn>불러오기 실패</Warn></span></TooltipTrigger><TooltipContent className="max-w-80 break-words">{d.failed}</TooltipContent></Tooltip>}
          {d.refreshed && <span className="tnum">갱신 {hm(d.refreshed)}</span>}
          {d.loading && <span className="inline-flex items-center gap-1.5"><LoaderCircle className="size-3.5 animate-spin" />불러오는 중</span>}
          {d.shared !== undefined && <Badge variant="secondary" className="border-0 font-normal text-muted-foreground">{d.shared ? "처리 기록 · 팀 공유" : "처리 기록 · 이 Mac"}</Badge>}
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
        <Button variant="outline" size="sm" onClick={report}><Download />보고서</Button>
        <Tooltip>
          <TooltipTrigger asChild><Button variant="ghost" size="icon-sm" aria-label="연결 설정" onClick={() => ask({ do: "settings" })}><Settings /></Button></TooltipTrigger>
          <TooltipContent>연결 설정</TooltipContent>
        </Tooltip>
      </PageHeader>

      <div className="scroll min-h-0 flex-1 overflow-y-auto px-6 pb-6">
        <div className="flex min-h-full flex-col gap-3">
          <Watch calls={shown} ask={ask} loading={blank} marks={marks} caseData={d.case} saved={d.saved} blocked={blocked} alerts={alerts} className="h-[clamp(420px,58vh,760px)] shrink-0" />
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
              title={
                <ToggleGroup type="single" size="sm" value={side} onValueChange={(v) => v && setSide(v as typeof side)} className="-ml-1.5">
                  <ToggleGroupItem value="rank" className="px-2 text-[13.5px] font-semibold text-muted-foreground data-[state=on]:text-foreground">위험 순위</ToggleGroupItem>
                  <ToggleGroupItem value="tune" className="px-2 text-[13.5px] font-semibold text-muted-foreground data-[state=on]:text-foreground">조정 후보</ToggleGroupItem>
                </ToggleGroup>
              }
              aside={side === "rank" && (
                <ToggleGroup type="single" size="sm" value={rankBy} onValueChange={(v) => v && setRankBy(v as RankBy)}>
                  {([["machine", "단말"], ["session", "세션"], ["tool", "도구"], ["action", "행동"]] as [RankBy, string][]).map(([v, l]) => <ToggleGroupItem key={v} value={v} className="px-2.5 text-[12.5px]">{l}</ToggleGroupItem>)}
                </ToggleGroup>
              )}>
              {side === "tune"
                ? <div className="scroll min-h-0 flex-1 overflow-y-auto px-2 py-2"><Tuning calls={inPeriod} marks={marks} blocked={blocked} shared={d.shared} ask={ask} onPick={(dim, key) => toggle(dim, key)} /></div>
                : <>
              {rankBy === "machine" && cover.active > 0 && (
                <div className="flex shrink-0 flex-col gap-2 border-b px-4 pb-3">
                  <SplitBar height={6} parts={[
                    { key: "blocking", label: "차단 모드", value: cover.blocking, color: "var(--allow)" },
                    { key: "watching", label: "막지 않는 모드", value: cover.watching, color: "var(--review)" },
                    { key: "none", label: "판정 없음", value: cover.active - cover.blocking - cover.watching, color: "var(--info)" },
                  ]} />
                  <div className="flex items-center gap-3">
                    <Legend items={[
                      { label: "차단 모드", color: "var(--allow)", value: cover.blocking },
                      { label: "막지 않는 모드", color: "var(--review)", value: cover.watching },
                      { label: "판정 없음", color: "var(--info)", value: cover.active - cover.blocking - cover.watching },
                    ]} />
                    {cover.undecided > 0 && <span className="ml-auto shrink-0"><Warn>판정 없는 호출 {cover.undecided}대</Warn></span>}
                  </div>
                </div>
              )}
              <div className="scroll min-h-0 flex-1 overflow-y-auto px-2 py-2">
                {blank
                  ? <div className="flex flex-col gap-2 px-2 py-1">{Array.from({ length: 6 }, (_, i) => <Skeleton key={i} className="h-9 rounded-lg" />)}</div>
                  : <Ranking calls={ranked} by={rankBy} picked={filter[rankBy]} onPick={(key) => toggle(rankBy, key)} totals={rankBy === "action" ? totals : undefined} />}
              </div>
                </>}
            </Panel>
          </div>
        </div>
      </div>
    </div>
  );
}
