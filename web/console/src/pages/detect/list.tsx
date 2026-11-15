// What to look at now, as one list seen three ways: the latest Calls worth a look, the prompts
// whose turns were stopped most, and the Calls a person let run after the engine asked. A row
// opens its detail at the right, from where the Call opens in 기록 or in 감사's approvals.

import { type ColumnDef } from "@tanstack/react-table";
import { ArrowUpRight, Search } from "lucide-react";
import { useMemo, useState, type ReactNode } from "react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Sheet, SheetContent, SheetDescription, SheetFooter, SheetHeader, SheetTitle } from "@/components/ui/sheet";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { DataTable, Panel, VerdictBadge, Warn, ago, clock } from "@/kit";
import { cn } from "@/lib/utils";
import { type Call, DIM, MODE, SEV, SEVS, type Sev } from "./data";

/** What the page asks the app to do. */
export type Ask = (message: { do: "open" | "approvals" | "settings"; session?: string; seq?: number }) => void;

/** A prompt whose turns had Calls stopped: its stops, the newest first. */
type Stopped = { prompt: string; stops: Call[]; machines: string[] };
type View = "calls" | "prompts" | "approved";
const NO_PROMPT = "프롬프트 없음";
/** How many rows a view lists; the newest or the most come first. */
const ROWS = 300;

export function SevBadge({ s }: { s: Sev }) {
  return <Badge variant="secondary" className={cn("border-0 font-semibold", SEV[s].cls)}>{SEV[s].label}</Badge>;
}
const short = (tool: string | null) => (tool ?? "").replace(/^mcp__/, "").replace(/__/g, " · ");
const Mono = ({ children }: { children: ReactNode }) => <span className="font-mono text-[12px]">{children}</span>;
const What = ({ c }: { c: Call }) => (
  <span className="flex min-w-0 items-center gap-2">
    {c.tool && <span className="max-w-32 shrink-0 truncate rounded-md bg-secondary px-1.5 py-0.5 text-[11px] text-muted-foreground">{short(c.tool)}</span>}
    <code className="min-w-0 truncate font-mono text-[12px]">{c.target ?? c.tool}</code>
  </span>
);
const When = ({ at }: { at: number | null }) => <span className="tnum text-[12px] text-muted-foreground">{clock(at)}</span>;

const CALLS: ColumnDef<Call>[] = [
  { id: "sev", header: "심각도", size: 96, accessorFn: (c) => -SEVS.indexOf(c.sev), cell: ({ row }) => <SevBadge s={row.original.sev} /> },
  { id: "kind", header: "유형", size: 208, accessorFn: (c) => DIM.kind.name(DIM.kind.of(c)) },
  { id: "target", header: "대상", accessorFn: (c) => c.target ?? "", cell: ({ row }) => <What c={row.original} /> },
  { id: "source", header: "원인", size: 100, accessorFn: (c) => DIM.source.name(c.source), cell: ({ getValue }) => <span className="text-muted-foreground">{getValue<string>()}</span> },
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
const prompts = (most: number): ColumnDef<Stopped>[] => [
  { id: "prompt", header: "프롬프트", accessorKey: "prompt", cell: ({ row }) => <span className={cn(row.original.prompt === NO_PROMPT && "text-muted-foreground")}>{row.original.prompt}</span> },
  {
    id: "stops", header: "차단", size: 200, accessorFn: (p) => p.stops.length,
    cell: ({ row }) => (
      <span className="flex items-center gap-2">
        <span className="h-1.5 flex-1 overflow-hidden rounded-full bg-white/[0.05]"><i className="block h-full rounded-full bg-deny" style={{ width: `${(row.original.stops.length / most) * 100}%` }} /></span>
        <b className="tnum w-6 text-right font-semibold">{row.original.stops.length}</b>
      </span>
    ),
  },
  { id: "machine", header: "단말", size: 184, accessorFn: (p) => p.machines[0], cell: ({ row }) => <span className="flex items-center gap-1.5"><span className="min-w-0 truncate font-mono text-[12px]">{row.original.machines[0]}</span>{row.original.machines.length > 1 && <span className="tnum shrink-0 text-[11.5px] text-muted-foreground">+{row.original.machines.length - 1}</span>}</span> },
  { id: "at", header: "최근", size: 104, accessorFn: (p) => p.stops[0].at ?? 0, cell: ({ row }) => <When at={row.original.stops[0].at} /> },
];

export function Watch({ calls, ask, className }: { calls: Call[]; ask: Ask; className?: string }) {
  const [view, setView] = useState<View>("calls");
  const [query, setQuery] = useState("");
  const [call, setCall] = useState<Call | null>(null);
  const [prompt, setPrompt] = useState<Stopped | null>(null);

  const found = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return calls;
    return calls.filter((c) => [c.target, c.prompt, c.machine, c.tool, c.folder, c.reason, DIM.kind.name(DIM.kind.of(c))].some((t) => t?.toLowerCase().includes(q)));
  }, [calls, query]);
  const newest = useMemo(() => [...found].sort((a, b) => (b.at ?? 0) - (a.at ?? 0) || b.seq - a.seq), [found]);
  const stopped = useMemo(() => {
    const by = new Map<string, Stopped>();
    for (const c of newest) {
      if (c.verdict !== "deny") continue;
      const key = c.prompt ?? NO_PROMPT;
      let row = by.get(key);
      if (!row) by.set(key, (row = { prompt: key, stops: [], machines: [] }));
      row.stops.push(c);
      if (!row.machines.includes(c.machine)) row.machines.push(c.machine);
    }
    return [...by.values()].sort((a, b) => b.stops.length - a.stops.length || a.prompt.localeCompare(b.prompt));
  }, [newest]);
  // The hardest to undo first, then the newest.
  const approved = useMemo(() => newest.filter((c) => c.verdict === "review" && c.ran).sort((a, b) => Number(b.hard) - Number(a.hard)), [newest]);
  const promptColumns = useMemo(() => prompts(stopped[0]?.stops.length ?? 1), [stopped]);

  const tab = (v: View, label: string, n: number) => (
    <TabsTrigger value={v} className="gap-1.5 px-2.5 text-[13px]">{label}<span className="tnum text-[11.5px] text-muted-foreground">{n.toLocaleString()}</span></TabsTrigger>
  );
  return (
    <Panel
      pad={false} className={cn("min-h-0 overflow-hidden [&>[data-slot=card-content]]:flex [&>[data-slot=card-content]]:min-h-0 [&>[data-slot=card-content]]:flex-1 [&>[data-slot=card-content]]:flex-col", className)}
      title={<Tabs value={view} onValueChange={(v) => setView(v as View)}><TabsList>{tab("calls", "탐지", newest.length)}{tab("prompts", "차단을 일으킨 프롬프트", stopped.length)}{tab("approved", "사용자 승인", approved.length)}</TabsList></Tabs>}
      aside={<>
        {view === "approved" && <Button variant="ghost" size="sm" onClick={() => ask({ do: "approvals" })}>감사 승인 기록<ArrowUpRight /></Button>}
        <span className="relative">
          <Search className="pointer-events-none absolute top-1/2 left-2 size-3.5 -translate-y-1/2 text-muted-foreground" />
          <Input value={query} onChange={(e) => setQuery(e.target.value)} placeholder="검색" className="h-7 w-52 pl-7 text-[12.5px] md:text-[12.5px]" />
        </span>
      </>}
    >
      <div className="min-h-0 flex-1 border-t [&_[data-slot=table-container]]:scroll [&_[data-slot=table-container]]:h-full [&_[data-slot=table-container]]:overflow-y-auto [&_td:first-child]:pl-4 [&_td:last-child]:pr-4 [&_th:first-child]:pl-4 [&_th:last-child]:pr-4">
        {view === "calls" && <DataTable columns={CALLS} rows={newest.slice(0, ROWS)} onRow={setCall} picked={(c) => c === call} dense />}
        {view === "prompts" && <DataTable columns={promptColumns} rows={stopped.slice(0, ROWS)} onRow={setPrompt} picked={(p) => p === prompt} dense />}
        {view === "approved" && <DataTable columns={APPROVED} rows={approved.slice(0, ROWS)} onRow={setCall} picked={(c) => c === call} dense />}
      </div>

      <Sheet open={!!prompt && !call} onOpenChange={(open) => !open && setPrompt(null)}>
        <SheetContent className="gap-0 data-[side=right]:w-[460px] data-[side=right]:sm:max-w-[460px]">
          {prompt && <>
            <SheetHeader className="gap-1.5 border-b pr-12">
              <div className="flex items-center gap-1.5"><VerdictBadge v="deny">차단 {prompt.stops.length}</VerdictBadge></div>
              <SheetTitle className="text-[15px]">차단을 일으킨 프롬프트</SheetTitle>
              <SheetDescription className="tnum text-[12px]">{clock(prompt.stops[0].at)} · {ago(prompt.stops[0].at)}</SheetDescription>
            </SheetHeader>
            <div className="scroll flex min-h-0 flex-1 flex-col gap-4 overflow-auto p-4">
              <Field label="프롬프트"><Quote>{prompt.prompt}</Quote></Field>
              <Field label="차단된 호출">
                <div className="flex flex-col overflow-hidden rounded-lg border">
                  {prompt.stops.map((c) => (
                    <button key={`${c.session}-${c.seq}`} onClick={() => setCall(c)} className="flex items-center gap-2 border-b px-2.5 py-2 text-left transition-colors last:border-b-0 hover:bg-accent/70">
                      <SevBadge s={c.sev} />
                      <code className="min-w-0 flex-1 truncate font-mono text-[12px]">{c.target ?? c.tool}</code>
                      <When at={c.at} />
                    </button>
                  ))}
                </div>
              </Field>
            </div>
            <SheetFooter className="flex-row border-t">
              <Button onClick={() => ask({ do: "open", session: prompt.stops[0].session, seq: prompt.stops[0].seq })}>최근 차단 기록에서 열기<ArrowUpRight /></Button>
            </SheetFooter>
          </>}
        </SheetContent>
      </Sheet>

      <Sheet open={!!call} onOpenChange={(open) => !open && setCall(null)}>
        <SheetContent className="gap-0 data-[side=right]:w-[460px] data-[side=right]:sm:max-w-[460px]">
          {call && <>
            <SheetHeader className="gap-1.5 border-b pr-12">
              <div className="flex flex-wrap items-center gap-1.5">
                <SevBadge s={call.sev} />
                <VerdictBadge v={call.verdict}>{call.verdict === "review" ? (call.ran ? "확인 요청 · 승인" : "확인 요청 · 반려") : undefined}</VerdictBadge>
                {call.hard && <Warn>되돌리기 어려움</Warn>}
              </div>
              <SheetTitle className="text-[15px]">{DIM.kind.name(DIM.kind.of(call))}</SheetTitle>
              <SheetDescription className="tnum text-[12px]">{clock(call.at)} · {ago(call.at)}</SheetDescription>
            </SheetHeader>
            <div className="scroll flex min-h-0 flex-1 flex-col gap-4 overflow-auto p-4">
              <Field label="대상"><pre className="rounded-lg border bg-background p-3 font-mono text-[12px] leading-relaxed break-all whitespace-pre-wrap">{call.target ?? call.tool}</pre></Field>
              <dl className="grid grid-cols-[72px_minmax(0,1fr)] gap-x-3 gap-y-2 text-[12.5px]">
                <Row label="도구"><Mono>{call.tool ?? "–"}</Mono></Row>
                <Row label="단말"><Mono>{call.machine}</Mono></Row>
                <Row label="폴더"><Mono>{call.cwd ?? (call.folder || "–")}</Mono></Row>
                <Row label="원인">{DIM.source.name(call.source)}</Row>
                <Row label="규칙"><Mono>{call.rule ?? "–"}</Mono></Row>
                <Row label="엔진">{(MODE[call.mode ?? "none"] ?? MODE.none).label}</Row>
                <Row label="실행">{call.ran ? "실행됨" : "실행 안 됨"}</Row>
              </dl>
              {call.reason && <Field label="판정 이유"><Quote>{call.reason}</Quote></Field>}
              {call.prompt && <Field label="프롬프트"><Quote>{call.prompt}</Quote></Field>}
            </div>
            <SheetFooter className="flex-row border-t">
              <Button onClick={() => ask({ do: "open", session: call.session, seq: call.seq })}>기록에서 열기<ArrowUpRight /></Button>
              {call.verdict === "review" && <Button variant="outline" onClick={() => ask({ do: "approvals", session: call.session, seq: call.seq })}>감사 승인 기록<ArrowUpRight /></Button>}
            </SheetFooter>
          </>}
        </SheetContent>
      </Sheet>
    </Panel>
  );
}

const Field = ({ label, children }: { label: string; children: ReactNode }) => (
  <section className="flex flex-col gap-1.5"><h3 className="text-[12px] text-muted-foreground">{label}</h3>{children}</section>
);
const Row = ({ label, children }: { label: string; children: ReactNode }) => (
  <><dt className="text-muted-foreground">{label}</dt><dd className="min-w-0 break-all">{children}</dd></>
);
const Quote = ({ children }: { children: ReactNode }) => (
  <p className="rounded-lg bg-secondary/60 px-3 py-2.5 text-[13px] leading-relaxed break-words whitespace-pre-wrap">{children}</p>
);
