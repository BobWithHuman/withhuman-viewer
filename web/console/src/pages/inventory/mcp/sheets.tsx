// What a picked tool and a picked server tell, in the sheet on the right. A tool: what it does,
// how the engine judged its calls, its 30 days, who called it and its latest calls. A server:
// where it runs, the machines it is set up on, and its tools.

import { Laptop } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { SheetDescription, SheetHeader, SheetTitle } from "@/components/ui/sheet";
import { Legend, NATURE, NatureIcon, ServiceLogo, SplitBar, VerdictBadge, ago, clock, num } from "@/kit";
import { cn } from "@/lib/utils";
import { Chip } from "../marks";
import type { View } from "../model";
import { Line, Section, When } from "../sheets";
import { DaysChart, Held } from "./parts";
import { COLUMN, DAYS, VERDICT_OF, column, isOn, machinesOf, split, type Server, type Tool } from "./types";

const NONE = <span className="px-2.5 text-[12.5px] text-muted-foreground">없음</span>;

/** A tool's or a server's calls by verdict as one bar, with the counts under it. */
function Verdicts({ of }: { of: Tool[] }) {
  const parts = split(of);
  if (!parts.some((p) => p.value > 0)) return NONE;
  return (
    <div className="flex flex-col gap-2 px-0.5">
      <SplitBar parts={parts} />
      <Legend items={parts} />
    </div>
  );
}

export function ToolSheet({ server, tool: t, view, onServer, onDevice }: {
  server: Server; tool: Tool; view: View; onServer: (name: string) => void; onDevice: (id: string) => void;
}) {
  const machine = (client: string) => view.machines.find((m) => isOn(client, m));
  // The calls by the machine they came from, the busiest first.
  const byClient = [...t.users.reduce((acc, u) => acc.set(u.client, (acc.get(u.client) ?? 0) + u.n), new Map<string, number>())]
    .map(([client, n]) => ({ client, n })).sort((a, b) => b.n - a.n);
  const c = column(t);
  return (
    <>
      <SheetHeader className="gap-2.5 border-b p-5 pr-12">
        <div className="flex items-center gap-2.5">
          <span className="grid size-9 shrink-0 place-items-center rounded-lg" style={{ background: `color-mix(in oklab, ${NATURE[t.nature].color} 16%, transparent)` }}>
            <NatureIcon n={t.nature} className="size-[18px]" />
          </span>
          <div className="flex min-w-0 flex-col">
            <SheetTitle className="truncate font-mono text-[15px] font-semibold">{t.name}</SheetTitle>
            <button onClick={() => onServer(server.name)} className="flex min-w-0 items-center gap-1.5 self-start rounded text-[12px] text-muted-foreground hover:text-foreground hover:underline">
              <ServiceLogo name={server.name} size={14} /><span className="truncate font-mono">{server.name}</span>
            </button>
          </div>
        </div>
        <SheetDescription className="text-[12.5px] leading-[1.55] text-foreground/80">{t.description}</SheetDescription>
        <div className="flex flex-wrap items-center gap-1.5">
          <Chip><NatureIcon n={t.nature} className="size-3" />{NATURE[t.nature].label}</Chip>
          {c === "none" ? <Chip className="text-muted-foreground">{COLUMN.none.label}</Chip> : <VerdictBadge v={c}>{COLUMN[c].label}</VerdictBadge>}
        </div>
      </SheetHeader>
      <div className="scroll flex min-h-0 flex-1 flex-col gap-5 overflow-y-auto p-5">
        <Section title="판정" count={t.calls} aside="30일">
          <Verdicts of={[t]} />
        </Section>

        <Section title="날짜별 호출" aside={<span className="tnum">마지막 {t.last ? `${clock(t.last)} · ${ago(t.last, view.now)}` : "–"}</span>}>
          <DaysChart className="h-[104px]" rows={Array.from({ length: DAYS }, (_, i) => ({ calls: t.days[i] ?? 0 }))} now={view.now}
            series={[{ key: "calls", label: "호출", color: c === "none" ? "var(--info)" : NATURE[t.nature].color }]} />
        </Section>

        <Section title="호출한 기기" count={byClient.length}>
          {!byClient.length && NONE}
          <div className="flex flex-col">
            {byClient.map((u) => {
              const m = machine(u.client);
              return (
                <Line key={u.client} onClick={m && (() => onDevice(m.id))}>
                  <Laptop className="size-4 shrink-0 text-muted-foreground" strokeWidth={1.75} />
                  <span className="w-[160px] shrink-0 truncate">{m?.name ?? u.client}</span>
                  <span className="h-1.5 min-w-0 flex-1 rounded-full bg-white/5"><i className="block h-full rounded-full bg-foreground/45" style={{ width: `${(u.n / Math.max(1, ...byClient.map((x) => x.n))) * 100}%` }} /></span>
                  <b className="tnum w-9 shrink-0 text-right font-medium">{num(u.n)}</b>
                </Line>
              );
            })}
          </div>
        </Section>

        <Section title="최근 호출" count={t.recent.length}>
          {!t.recent.length && NONE}
          <div className="flex flex-col">
            {t.recent.map((u, i) => (
              <div key={`${u.at}-${i}`} className="flex min-w-0 flex-col gap-1 rounded-md px-2.5 py-2">
                <div className="flex min-w-0 items-center gap-2">
                  <code className="min-w-0 flex-1 truncate font-mono text-[12px]" title={u.target}>{u.target}</code>
                  {!u.ran && <Chip className="shrink-0 text-muted-foreground">실행 안 됨</Chip>}
                  <VerdictBadge v={VERDICT_OF[u.decision]} />
                </div>
                <div className="flex min-w-0 items-center gap-1.5 text-[12px] text-muted-foreground">
                  <Laptop className="size-3.5 shrink-0" strokeWidth={1.75} />
                  <span className="truncate">{machine(u.client)?.name ?? u.client}</span>
                  <span className="tnum ml-auto shrink-0">{clock(u.at)}</span>
                </div>
              </div>
            ))}
          </div>
        </Section>
      </div>
    </>
  );
}

export function ServerSheet({ server: s, view, onTool, onDevice }: {
  server: Server; view: View; onTool: (server: string, tool: string) => void; onDevice: (id: string) => void;
}) {
  const calls = s.tools.reduce((a, t) => a + t.calls, 0);
  const installs = [...s.installs].sort((a, b) => b.at - a.at);
  const tools = [...s.tools].sort((a, b) => b.calls - a.calls || a.name.localeCompare(b.name));
  return (
    <>
      <SheetHeader className="gap-2.5 border-b p-5 pr-12">
        <div className="flex items-center gap-2.5">
          <ServiceLogo name={s.name} size={36} />
          <div className="flex min-w-0 flex-col">
            <SheetTitle className="truncate font-mono text-[15px] font-semibold">{s.name}</SheetTitle>
            <SheetDescription className="text-[12px]">MCP 서버</SheetDescription>
          </div>
        </div>
        <div className="flex flex-wrap items-center gap-1.5">
          <Chip>{s.transport === "local" ? "로컬" : "원격"}</Chip>
          <Chip><Laptop className="size-3" />설치 단말 {machinesOf(s)}</Chip>
          <Chip className={cn(!calls && "text-muted-foreground")}>호출 {num(calls)}</Chip>
        </div>
      </SheetHeader>
      <div className="scroll flex min-h-0 flex-1 flex-col gap-5 overflow-y-auto p-5">
        <div className="flex flex-col gap-1 rounded-lg bg-white/[0.04] p-3.5">
          <span className="text-[11.5px] text-muted-foreground">{s.transport === "local" ? "실행 명령" : "서버 주소"}</span>
          <code className="font-mono text-[12px] leading-[1.5] break-all">{s.endpoint}</code>
        </div>

        <Section title="판정" count={calls} aside="30일">
          <Verdicts of={s.tools} />
        </Section>

        <Section title="설치 단말" count={installs.length}>
          {!installs.length && NONE}
          <div className="flex flex-col">
            {installs.map((i) => {
              const m = view.machines.find((x) => isOn(i.client, x));
              return (
                <Line key={`${i.user} ${i.client}`} onClick={m && (() => onDevice(m.id))}>
                  <Laptop className="size-4 shrink-0 text-muted-foreground" strokeWidth={1.75} />
                  <span className="min-w-0 flex-1 truncate">{m?.name ?? i.client}</span>
                  <When>{clock(i.at)}</When>
                </Line>
              );
            })}
          </div>
        </Section>

        <Section title="도구" count={tools.length}>
          <div className="flex flex-col">
            {tools.map((t) => (
              <Line key={t.name} onClick={() => onTool(s.name, t.name)}>
                <NatureIcon n={t.nature} />
                <span className={cn("min-w-0 truncate font-mono text-[12px]", !t.calls && "text-muted-foreground")}>{t.name}</span>
                <Held t={t} />
                {t.calls > 0 ? <b className="tnum ml-auto shrink-0 font-medium">{num(t.calls)}</b> : <Badge variant="outline" className="ml-auto shrink-0 border-dashed font-normal text-muted-foreground">{COLUMN.none.label}</Badge>}
              </Line>
            ))}
          </div>
        </Section>
      </div>
    </>
  );
}
