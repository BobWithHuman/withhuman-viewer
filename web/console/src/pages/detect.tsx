// 보안 현황, the security team's main page, in three parts. The flow: every Call from the kind
// of action it is, through what made the agent take it, to how the engine decided it, with the
// Calls stopped or held day by day under it (dragging across the days makes them the period).
// Where: the machines by the Calls worth a look on each, and which of them the engine blocks
// on. And the list of what to look at first. Pressing a band, a figure, a machine or a row
// narrows the rest; what is applied shows as chips, each removable. The counting is done here,
// over the Calls the app hands over once.

import { Settings, X } from "lucide-react";
import { useMemo, useState } from "react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { Legend, PageHeader, Panel, SplitBar, VERDICT, Warn } from "@/kit";
import { cn } from "@/lib/utils";
import { send } from "../bridge";
import { Breakdown } from "./detect/breakdown";
import { type Call, DIM, DIMS, type Data, type Dim, type Filter, MODE, type Period, VERDICTS, coverage, demo, passes, rangeLabel, span, within } from "./detect/data";
import { ActionFlow, flowRows } from "./detect/flow";
import { type Ask, Watch } from "./detect/list";
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
  // Whether the flow shows every Call or only those stopped or held.
  const [flowOf, setFlowOf] = useState<"all" | "held">("all");
  const ask: Ask = (message) => send("detect", message);

  const toggle = (dim: Dim, key: string) => setFilter((f) => ({ ...f, [dim]: f[dim] === key ? undefined : key }));
  const { from, to, before } = span(period, now);
  const inPeriod = useMemo(() => d.calls.filter((c) => within(c, from, to)), [d, from, to]);
  const shown = useMemo(() => inPeriod.filter((c) => passes(c, filter)), [inPeriod, filter]);
  // Each figure and the machines count with their own choice left out, so it stays a choice.
  const byVerdict = useMemo(() => inPeriod.filter((c) => passes(c, filter, "verdict")), [inPeriod, filter]);
  const byMachine = useMemo(() => inPeriod.filter((c) => passes(c, filter, "machine")), [inPeriod, filter]);
  const earlier = useMemo(() => (before === null ? null : d.calls.filter((c) => within(c, before, from) && passes(c, filter, "verdict"))), [d, before, from, filter]);
  const count = (list: Call[] | null, v: string) => (list ? list.filter((c) => c.verdict === v).length : null);
  const machine = filter.machine === undefined ? null : d.machines.indexOf(filter.machine);
  const cover = useMemo(() => coverage(d.cover, from, to, machine), [d, from, to, machine]);
  // The chart's stretch: all time starts at the first Call worth a look.
  const first = useMemo(() => d.calls.reduce((a, c) => (c.at !== null && c.at < a ? c.at : a), now), [d, now]);
  const rows = useMemo(() => flowRows(d.flow ?? [], from, to, machine, flowOf === "held" ? ["deny", "review"] : undefined), [d, from, to, machine, flowOf]);
  const mode = MODE[cover.mode] ?? MODE.none;
  const blank = d.loading && !d.calls.length;

  const chips: { key: string; label: string; value: string; drop: () => void }[] = [
    ...(period.range ? [{ key: "range", label: "기간", value: rangeLabel(period.range), drop: () => setPeriod((p) => ({ ...p, range: null })) }] : []),
    ...DIMS.filter((dim) => filter[dim] !== undefined).map((dim) => ({ key: dim, label: DIM[dim].label, value: DIM[dim].name(filter[dim]!), drop: () => setFilter((f) => ({ ...f, [dim]: undefined })) })),
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
          <div className="grid h-[clamp(470px,56vh,640px)] shrink-0 grid-cols-[minmax(0,2.4fr)_minmax(0,1fr)] gap-3">
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
                </div>
              }
              aside={
                <ToggleGroup type="single" size="sm" value={flowOf} onValueChange={(v) => v && setFlowOf(v as typeof flowOf)}>
                  <ToggleGroupItem value="all" className="px-2.5 text-[12.5px]">전체 호출</ToggleGroupItem>
                  <ToggleGroupItem value="held" className="px-2.5 text-[12.5px]">차단·확인</ToggleGroupItem>
                </ToggleGroup>
              }
            >
              <div className="min-h-0 flex-1"><ActionFlow rows={rows} filter={filter} onFilter={setFilter} /></div>
              <div className="mt-2 h-[92px] shrink-0 border-t pt-2">
                <Trend calls={shown} from={Number.isFinite(from) ? from : first} to={Number.isFinite(to) ? to : now + 1} onRange={(range) => setPeriod((p) => ({ ...p, range }))} />
              </div>
            </Panel>

            <Panel className={`h-full ${FILL}`} pad={false} title="단말" count={cover.active}
              aside={cover.undecided > 0 && <Warn>판정 없는 호출 {cover.undecided}대</Warn>}>
              {cover.active > 0 && (
                <div className="flex shrink-0 flex-col gap-2 border-b px-4 pb-3">
                  <SplitBar height={6} parts={[
                    { key: "blocking", label: "차단 모드", value: cover.blocking, color: "var(--allow)" },
                    { key: "watching", label: "막지 않는 모드", value: cover.watching, color: "var(--review)" },
                    { key: "none", label: "판정 없음", value: cover.active - cover.blocking - cover.watching, color: "var(--info)" },
                  ]} />
                  <Legend items={[
                    { label: "차단 모드", color: "var(--allow)", value: cover.blocking },
                    { label: "막지 않는 모드", color: "var(--review)", value: cover.watching },
                    { label: "판정 없음", color: "var(--info)", value: cover.active - cover.blocking - cover.watching },
                  ]} />
                </div>
              )}
              <div className="scroll min-h-0 flex-1 overflow-y-auto px-2 py-2">
                <Breakdown calls={byMachine} dim="machine" picked={filter.machine} onPick={(key) => toggle("machine", key)} />
              </div>
            </Panel>
          </div>

          <Watch calls={shown} ask={ask} className="min-h-[420px] flex-1 basis-[420px]" />
        </div>
      </div>
    </div>
  );
}
