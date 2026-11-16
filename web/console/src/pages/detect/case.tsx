// One Call worth a look, opened to judge: was it an attack, or nothing wrong. On top, side by
// side, what the person asked and the words the agent read that told it to act. Down the left,
// in order: what it read, what it was about to do, and why the engine decided as it did. On the right,
// where else: the session's other Calls, the other sessions that read the same thing, and the
// machine's latest. The team marks it an attack or fine, saves its evidence as a file, or opens
// it in 기록.

import { ArrowUpRight, Ban, Download, FileText, Globe, Hand, LoaderCircle, MessageSquareText, OctagonX, ShieldAlert, SquareTerminal } from "lucide-react";
import { useState, type ReactNode } from "react";
import { Button } from "@/components/ui/button";
import { SheetDescription, SheetHeader, SheetTitle } from "@/components/ui/sheet";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { VerdictBadge, Warn, ago, clock } from "@/kit";
import { cn } from "@/lib/utils";
import { ACTION, type Blocked, type Call, type Case, type CaseGroup, DIM, MARK, MARKS, MODE, type Mark, SEV, hostOf, toolName } from "./data";
import type { Ask } from "./list";

const Mono = ({ children, className }: { children: ReactNode; className?: string }) => <code className={cn("font-mono text-[12px]", className)}>{children}</code>;

/** A step of why it happened: its number and name on a line running down, and what it holds. */
function Step({ n, icon: Icon, title, tone, aside, last, children }: { n: number; icon: typeof FileText; title: string; tone?: string; aside?: ReactNode; last?: boolean; children: ReactNode }) {
  return (
    <section className="relative grid grid-cols-[28px_minmax(0,1fr)] gap-x-3">
      {!last && <span className="absolute top-8 bottom-[-18px] left-[13.5px] w-px bg-border" />}
      <span className="z-10 grid size-7 place-items-center rounded-full border bg-card" style={tone ? { borderColor: tone, color: tone } : undefined}><Icon className="size-3.5" strokeWidth={2} /></span>
      <div className="min-w-0 pb-[18px]">
        <h3 className="flex h-7 items-center gap-2 text-[12.5px] font-medium"><span className="tnum text-muted-foreground">{n}</span>{title}<span className="ml-auto flex items-center gap-2 text-[12px] font-normal text-muted-foreground">{aside}</span></h3>
        <div className="mt-1.5 grid gap-2">{children}</div>
      </div>
    </section>
  );
}
const Box = ({ children, tone, className }: { children: ReactNode; tone?: string; className?: string }) => (
  <div className={cn("rounded-lg border bg-background px-3 py-2.5 text-[13px] leading-relaxed break-words whitespace-pre-wrap", className)} style={tone ? { borderColor: `color-mix(in oklab, ${tone} 55%, transparent)`, background: `color-mix(in oklab, ${tone} 9%, var(--background))` } : undefined}>{children}</div>
);
function Side({ title, count, children }: { title: string; count?: number; children: ReactNode }) {
  return (
    <section className="grid gap-1.5">
      <h3 className="flex items-center gap-2 text-[12px] font-medium text-muted-foreground">{title}{count !== undefined && <span className="tnum rounded-full bg-secondary px-1.5 text-[11px]">{count}</span>}</h3>
      {children}
    </section>
  );
}
const Rows = ({ children }: { children: ReactNode }) => <div className="flex flex-col overflow-hidden rounded-lg border">{children}</div>;
const rowCls = "flex min-w-0 items-center gap-2 border-b px-2.5 py-2 text-left text-[12.5px] transition-colors last:border-b-0 hover:bg-accent/70";

export function CaseView({ call, group, data, marks, blocked, ask, onCall }: {
  call: Call; group: CaseGroup; data: Case | null | undefined; marks: Record<string, Mark>; blocked: Blocked[]; ask: Ask; onCall: (c: Call) => void;
}) {
  // The mark shown is this Call's session's; setting one marks every session of the case.
  const mark = marks[call.session];
  // The kill switch asks twice: the first press arms it.
  const [armed, setArmed] = useState(false);
  // The case the app read is this Call's once its session and number match.
  const read = data && data.session === call.session && data.seq === call.seq && !data.loading ? data : null;
  const told = read?.told ?? null;
  const others = group.calls.filter((c) => c !== call && c.session === call.session);
  // The other sessions the same cause reached, each with its Calls worth a look.
  const reached = group.sessions.filter((s) => s !== call.session).map((s) => group.calls.filter((c) => c.session === s));
  const user = read?.user ?? call.user ?? null;
  const host = hostOf(call.target);
  // What the agent read is a source the team can distrust from here on.
  const source = told?.target || null;
  const distrusted = !!source && blocked.some((b) => b.source === source);
  const evidence = () => ask({
    do: "export", name: `withhuman-case-${call.session.slice(0, 8)}-${call.seq}.json`,
    text: JSON.stringify({ exported_at: new Date().toISOString(), mark: mark ?? null, user, source_distrusted: distrusted, call, session_calls: group.calls, why: read }, null, 2),
  });
  return (
    <>
      <SheetHeader className="gap-2 border-b pr-12">
        <div className="flex flex-wrap items-center gap-1.5">
          <span className={cn("rounded-md px-1.5 text-[12px] leading-[22px] font-semibold", SEV[call.sev].cls)}>{SEV[call.sev].label}</span>
          <VerdictBadge v={call.verdict}>{call.verdict === "review" ? (call.ran ? "사용자 확인 · 승인" : "사용자 확인 · 반려") : undefined}</VerdictBadge>
          {call.hard && <Warn>되돌리기 어려움</Warn>}
          <span className="tnum ml-1 text-[12px] text-muted-foreground">{clock(call.at)} · {ago(call.at)}</span>
        </div>
        <div className="flex items-center gap-3">
          <SheetTitle className="min-w-0 flex-1 truncate text-[16px]">{DIM.kind.name(DIM.kind.of(call))}</SheetTitle>
          <ToggleGroup type="single" variant="outline" size="sm" spacing={0} value={mark?.state ?? ""} onValueChange={(v) => ask({ do: "triage", sessions: group.sessions, state: v || null })}>
            {MARKS.map((m) => <ToggleGroupItem key={m} value={m} className={cn("px-2.5 text-[12.5px]", m === "attack" && "data-[state=on]:bg-deny/20 data-[state=on]:text-deny", m === "benign" && "data-[state=on]:bg-allow/20 data-[state=on]:text-allow", m === "reviewing" && "data-[state=on]:bg-review/20 data-[state=on]:text-review")}>{MARK[m].label}</ToggleGroupItem>)}
          </ToggleGroup>
          {user && (
            <Button variant="outline" size="sm" className={cn(armed && "border-deny bg-deny/15 text-deny hover:bg-deny/25 hover:text-deny")} onBlur={() => setArmed(false)}
              onClick={() => { if (armed) { ask({ do: "stop", email: user, session: call.session, seq: call.seq }); setArmed(false); } else setArmed(true); }}>
              <OctagonX />{armed ? "한 번 더" : "멈춤"}
            </Button>
          )}
          <Button variant="outline" size="sm" onClick={evidence}><Download />증거</Button>
          <Button size="sm" onClick={() => ask({ do: "open", session: call.session, seq: call.seq })}>기록<ArrowUpRight /></Button>
        </div>
        <SheetDescription className="sr-only">{DIM.kind.name(DIM.kind.of(call))}</SheetDescription>
      </SheetHeader>

      <div className="grid shrink-0 grid-cols-2 gap-3 border-b p-5">
        <section className="grid content-start gap-1.5">
          <h3 className="flex items-center gap-1.5 text-[12px] font-medium text-muted-foreground"><MessageSquareText className="size-3.5" />사용자 요청</h3>
          <Box className="max-h-[132px] overflow-y-auto">{read?.request?.text ?? call.prompt ?? <span className="text-muted-foreground">프롬프트 없음</span>}</Box>
        </section>
        <section className="grid content-start gap-1.5">
          <h3 className="flex items-center gap-1.5 text-[12px] font-medium text-catch"><FileText className="size-3.5" />유도 문구{told && <span className="tnum ml-auto">{Math.round(told.probability * 100)}%</span>}</h3>
          <Box tone={told?.quote ? "var(--catch)" : undefined} className="max-h-[132px] overflow-y-auto">
            {!read ? <span className="text-muted-foreground">불러오는 중</span>
              : told?.quote ? told.quote
                : <span className="text-muted-foreground">{call.source === "request" ? "없음 · 사용자 요청에서 바로" : told ? "글귀 없음" : "원인 찾지 못함"}</span>}
          </Box>
        </section>
      </div>

      <div className="grid min-h-0 flex-1 grid-cols-[minmax(0,1.5fr)_minmax(280px,1fr)]">
        <div className="scroll min-h-0 overflow-y-auto p-5">
          {read?.request?.intent && <div className="mb-4 flex items-center gap-2 text-[12px] text-muted-foreground"><span className="rounded bg-secondary px-1.5 leading-[20px]">의도</span><span className="min-w-0 truncate">{read.request.intent}</span></div>}
          <Step n={1} icon={FileText} title="읽은 것" tone={told ? "var(--catch)" : undefined}
            aside={!read ? <LoaderCircle className="size-3.5 animate-spin" /> : told ? <span className="tnum font-medium text-catch">{Math.round(told.probability * 100)}%</span> : undefined}>
            {!read && <Box className="text-muted-foreground">불러오는 중</Box>}
            {read?.error && <Box className="text-deny">{read.error}</Box>}
            {read && !read.error && !told && <Box className="text-muted-foreground">{call.source === "request" ? "사용자 요청에서 바로" : read.traced === false ? "원인 찾지 못함" : DIM.source.name(call.source)}</Box>}
            {told && (
              <>
                <button onClick={() => ask({ do: "open", session: call.session, seq: told.seq })} className="flex min-w-0 items-center gap-2 rounded-lg border bg-background px-3 py-2 text-left transition-colors hover:bg-accent/70">
                  {told.tool && <span className="max-w-40 shrink-0 truncate rounded-md bg-secondary px-1.5 py-0.5 text-[11px] text-muted-foreground">{toolName(told.tool)}</span>}
                  <Mono className="min-w-0 flex-1 truncate">{told.target || "대상 없음"}</Mono>
                  <ArrowUpRight className="size-3.5 shrink-0 text-muted-foreground" />
                </button>
                {told.paragraph != null && <span className="tnum text-[12px] text-muted-foreground">문단 {told.paragraph + 1}{told.error ? " · 도구 오류" : ""}</span>}
                {source && (
                  <Button variant="outline" size="sm" className={cn("justify-self-start", distrusted && "border-deny/60 bg-deny/12 text-deny hover:bg-deny/20 hover:text-deny")}
                    onClick={() => ask({ do: "source", source, on: !distrusted, session: call.session, seq: call.seq })}>
                    <Ban />{distrusted ? "출처 차단됨" : "출처 차단"}
                  </Button>
                )}
              </>
            )}
          </Step>

          <Step n={2} icon={SquareTerminal} title="하려던 행동" tone={SEV[call.sev].color} aside={<>{ACTION[call.action]?.label}<span>{call.ran ? "실행됨" : "실행 안 됨"}</span></>}>
            <Box tone={SEV[call.sev].color} className="font-mono text-[12px]">
              {call.tool && <span className="mr-2 rounded-md bg-secondary px-1.5 py-0.5 font-sans text-[11px] text-muted-foreground">{toolName(call.tool)}</span>}
              {call.target ?? call.tool}
            </Box>
            {host && <div className="flex items-center gap-2 text-[12.5px]"><span className="text-muted-foreground">전송 대상</span><Globe className="size-3.5 text-muted-foreground" /><Mono>{host}</Mono></div>}
            {call.leak && (
              <div className="flex flex-wrap items-center gap-1.5 text-[12.5px]">
                <span className="rounded bg-deny/15 px-1.5 leading-[20px] font-semibold text-deny">유출된 값</span>
                {call.leak.kinds.map((k) => <span key={k} className="rounded bg-secondary px-1.5 leading-[20px]">{k}</span>)}
                <span className="text-muted-foreground">→</span>
                {call.leak.places.map((p) => <Mono key={p}>{p.replace(" (MCP)", "")}</Mono>)}
              </div>
            )}
          </Step>

          <Step n={3} icon={ShieldAlert} title="엔진 판정" last aside={call.rule ? <Mono>{call.rule}</Mono> : undefined}>
            <Box>{call.reason ?? <span className="text-muted-foreground">이유 없음</span>}</Box>
          </Step>
        </div>

        <div className="scroll flex min-h-0 flex-col gap-5 overflow-y-auto border-l p-5">
          <dl className="grid grid-cols-[64px_minmax(0,1fr)] gap-x-3 gap-y-2 text-[12.5px]">
            <dt className="text-muted-foreground">사용자</dt><dd className="min-w-0 truncate">{user ?? <span className="text-muted-foreground">모름</span>}</dd>
            <dt className="text-muted-foreground">단말</dt><dd className="min-w-0 truncate"><Mono>{call.machine}</Mono></dd>
            <dt className="text-muted-foreground">폴더</dt><dd className="min-w-0 truncate"><Mono>{call.cwd ?? (call.folder || "–")}</Mono></dd>
            <dt className="text-muted-foreground">엔진</dt><dd>{(MODE[call.mode ?? "none"] ?? MODE.none).label}</dd>
            {mark && <><dt className="text-muted-foreground">표시</dt><dd className="flex items-center gap-2"><span className={cn("rounded px-1.5 text-[11.5px] leading-[20px] font-semibold", MARK[mark.state].cls)}>{MARK[mark.state].label}</span><span className="tnum truncate text-[12px] text-muted-foreground">{mark.by} · {ago(mark.at)}</span></dd></>}
          </dl>

          {others.length > 0 && (
            <Side title="이 세션의 다른 탐지" count={others.length}>
              <Rows>{others.map((c) => (
                <button key={c.seq} onClick={() => onCall(c)} className={rowCls}>
                  <span className={cn("shrink-0 rounded px-1.5 text-[11px] leading-[19px] font-semibold", SEV[c.sev].cls)}>{SEV[c.sev].label}</span>
                  <Mono className="min-w-0 flex-1 truncate">{c.target ?? c.tool}</Mono>
                </button>
              ))}</Rows>
            </Side>
          )}

          {reached.length > 0 && (
            <Side title="같은 원인이 닿은 다른 세션" count={reached.length}>
              <Rows>{reached.slice(0, 12).map((list) => (
                <button key={list[0].session} onClick={() => onCall(list[0])} className={rowCls}>
                  <span className="grid min-w-0 flex-1 gap-0.5">
                    <span className="truncate">{list[0].prompt ?? "프롬프트 없음"}</span>
                    <span className="flex items-center gap-2 text-[11.5px] text-muted-foreground"><Mono className="truncate text-[11px]">{list[0].machine}</Mono><span className="tnum shrink-0">{ago(list[0].at)}</span></span>
                  </span>
                  {marks[list[0].session] && <span className={cn("shrink-0 rounded px-1.5 text-[11px] leading-[19px] font-semibold", MARK[marks[list[0].session].state].cls)}>{MARK[marks[list[0].session].state].label}</span>}
                  <span className="tnum inline-flex shrink-0 items-center gap-1 rounded bg-deny/15 px-1.5 text-[11.5px] leading-[20px] font-medium text-deny"><Hand className="size-3" />{list.length}</span>
                </button>
              ))}</Rows>
            </Side>
          )}

          <Side title="같은 것을 읽은 다른 세션" count={read?.elsewhere_total ?? (read ? 0 : undefined)}>
            {!read && <div className="grid h-12 place-items-center rounded-lg border text-muted-foreground"><LoaderCircle className="size-4 animate-spin" /></div>}
            {read && !read.elsewhere?.length && <div className="grid h-12 place-items-center rounded-lg border text-[12.5px] text-muted-foreground">없음</div>}
            {!!read?.elsewhere?.length && (
              <Rows>{read.elsewhere.map((e) => (
                <button key={e.session} onClick={() => ask({ do: "open", session: e.session, seq: e.seq })} className={rowCls}>
                  <span className="grid min-w-0 flex-1 gap-0.5">
                    <span className="truncate">{e.prompt ?? "프롬프트 없음"}</span>
                    <span className="flex items-center gap-2 text-[11.5px] text-muted-foreground"><Mono className="truncate text-[11px]">{e.machine}</Mono><span className="tnum shrink-0">{ago(e.at)}</span></span>
                  </span>
                  {e.flagged > 0 ? <span className="tnum inline-flex shrink-0 items-center gap-1 rounded bg-deny/15 px-1.5 text-[11.5px] leading-[20px] font-medium text-deny"><Hand className="size-3" />{e.flagged}</span> : <span className="shrink-0 text-[11.5px] text-muted-foreground">탐지 없음</span>}
                </button>
              ))}</Rows>
            )}
          </Side>

          {!!read?.user_recent?.length && (
            <Side title="이 사용자의 다른 탐지" count={read.user_recent.length}>
              <Rows>{read.user_recent.slice(0, 8).map((r) => (
                <button key={`${r.session}-${r.seq}`} onClick={() => ask({ do: "open", session: r.session, seq: r.seq })} className={rowCls}>
                  <VerdictBadge v={r.verdict as Call["verdict"]} />
                  <Mono className="min-w-0 flex-1 truncate">{r.target ?? toolName(r.tool)}</Mono>
                  <span className="tnum shrink-0 text-[11.5px] text-muted-foreground">{ago(r.at)}</span>
                </button>
              ))}</Rows>
            </Side>
          )}

          {!!read?.machine_recent?.length && (
            <Side title="이 단말의 최근 탐지" count={read.machine_recent.length}>
              <Rows>{read.machine_recent.slice(0, 8).map((r) => (
                <button key={`${r.session}-${r.seq}`} onClick={() => ask({ do: "open", session: r.session, seq: r.seq })} className={rowCls}>
                  <VerdictBadge v={r.verdict as Call["verdict"]} />
                  <Mono className="min-w-0 flex-1 truncate">{r.target ?? toolName(r.tool)}</Mono>
                  <span className="tnum shrink-0 text-[11.5px] text-muted-foreground">{ago(r.at)}</span>
                </button>
              ))}</Rows>
            </Side>
          )}
        </div>
      </div>
    </>
  );
}
