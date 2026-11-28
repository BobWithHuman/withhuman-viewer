// The personal app's first launch as a full-window page over 기록 (shown while `shell.page` is
// "onboard"): a step rail on the left, the step on the right, back / next below. Ported from
// web/console's pages/onboard.tsx with the same data (`views.onboard`, from src/ui/onboard.rs) and
// the same messages (`send({ page: "onboard", do: ... })`), drawn in Langflow's components:
//   시작     - a session drawn on Langflow's canvas (WelcomeCanvas), the app's name, 시작;
//   엔진     - this Mac (its engine's parts as dots) or the company's engine;
//   판정     - local rules, or Jev with its key checked and its model picked;
//   에이전트 - every agent with its logo: found or not, its folder, all / picked / later;
//   준비     - the engine and the hooks coming up (ReadyCanvas), then 체험 or 기록;
//   회사 엔진 - (the company's engine) the store's address and account, tested, saved.
// Finishing marks the first session's tour as wanted (tour/store.ts).

import { useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import ForwardedIconComponent from "@/components/common/genericIconComponent";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Input } from "@/components/ui/input";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs-button";
import { cn } from "@/utils/utils";
import { AgentIcon } from "../chrome/agent-icon";
import { dragProps } from "../chrome/app";
import { WithHumanLogo } from "../chrome/logo";
import { wantTour } from "../tour/store";
import { type OnboardAgent, type OnboardData, type State, ENGINE, STORE, scope, send } from "./data";
import { demoStart } from "./demo";
import { Chip, Choice, StateIcon, Title } from "./parts";
import { ReadyCanvas } from "./ReadyCanvas";
import { WelcomeCanvas } from "./Welcome";

type Where = "here" | "server";
type Judge = "rules" | "jev";
type Hooks = "all" | "pick" | "later";
type Step = "welcome" | "where" | "judge" | "agents" | "ready" | "store";

const FLOW: Record<Where, Step[]> = { here: ["welcome", "where", "judge", "agents", "ready"], server: ["welcome", "where", "store"] };
const RAIL: Record<Step, { label: string; icon: string }> = {
  welcome: { label: "시작", icon: "Sparkles" },
  where: { label: "엔진", icon: "Server" },
  judge: { label: "판정", icon: "Scale" },
  agents: { label: "에이전트", icon: "Bot" },
  ready: { label: "준비", icon: "Rocket" },
  store: { label: "회사 엔진", icon: "Network" },
};

/** Before the app's first data: nothing found yet. */
const EMPTY: OnboardData = {
  python: null, kept_key: false, jev: { state: "idle", text: null, models: [] }, model: "jev-1.14.0",
  parts: [], agents: [], run: { state: "idle", text: null }, server: { state: "idle", text: null, sessions: null }, done: false,
};

const isStep = (s: string | undefined): s is Step => !!s && s in RAIL;

export function Onboard({ data }: { data: OnboardData | null | undefined }) {
  const d = data ?? EMPTY;
  const [where, setWhere] = useState<Where>(d.role === "developer" ? "server" : "here");
  const [step, setStep] = useState<Step>(isStep(demoStart.step) ? demoStart.step : "welcome");
  const [judge, setJudge] = useState<Judge>(demoStart.judge === "jev" || d.kept_key ? "jev" : "rules");
  const [key, setKey] = useState("");
  const [hooks, setHooks] = useState<Hooks>((demoStart.hooks as Hooks) ?? "all");
  const [picked, setPicked] = useState<Set<string>>(new Set());
  const [started, setStarted] = useState((d.run.state !== "idle" && d.run.state !== "err") || d.done);
  const [store, setStore] = useState(STORE);

  // The app's first data: the developer's machine starts on the company's engine, a kept key on
  // Jev, and the agents found (not per folder) are picked.
  const seeded = useRef(false);
  useEffect(() => {
    if (!data || seeded.current) return;
    seeded.current = true;
    if (data.role === "developer") setWhere("server");
    if (data.kept_key) setJudge("jev");
    setPicked(new Set(data.agents.filter((a) => a.found && !a.per_folder).map((a) => a.kind)));
  }, [data]);
  // A folder just picked for an agent picks that agent.
  const folders = useRef(new Map<string, string | null>());
  useEffect(() => {
    for (const a of d.agents) {
      const was = folders.current.get(a.kind);
      if (a.folder && a.folder !== was && was !== undefined) setPicked((p) => new Set(p).add(a.kind));
      folders.current.set(a.kind, a.folder);
    }
  }, [d.agents]);
  useEffect(() => {
    if (d.run.state === "err") setStarted(false);
  }, [d.run.state]);

  const busy = d.run.state === "busy" || d.agents.some((a) => a.state === "busy") || d.jev.state === "busy";
  const usable = (a: OnboardAgent) => a.found && (!a.per_folder || !!a.folder);
  const chosen = hooks === "later" ? [] : d.agents.filter((a) => usable(a) && (hooks === "all" || picked.has(a.kind)));
  const jevReady = judge === "rules" || d.jev.state === "ok" || (d.kept_key && key.trim() === "");
  const flow = FLOW[where];
  const at = flow.indexOf(step);
  const go = (n: number) => setStep(flow[Math.max(0, Math.min(flow.length - 1, at + n))]);
  const start = () => {
    setStarted(true);
    send({ do: "start", jev: judge === "jev", key: judge === "jev" ? key.trim() : "", model: d.model, agents: chosen.map((a) => a.kind) });
  };
  const open = (tryIt: boolean) => {
    wantTour();
    send({ do: "open", try: tryIt });
  };
  const locked = step === "ready" && (started || d.done);

  let page: ReactNode;
  let next: ReactNode = (
    <Button onClick={() => go(1)} data-testid="onboard-next">
      다음
      <ForwardedIconComponent name="ChevronRight" className="h-4 w-4" />
    </Button>
  );

  if (step === "welcome") {
    page = (
      <div className="flex flex-col items-center gap-8 pt-4">
        <WelcomeCanvas />
        <div className="flex items-center gap-3">
          <WithHumanLogo className="h-10 w-10" />
          <span className="font-chivo text-3xl font-semibold tracking-tight">WithHuman</span>
        </div>
        <Button size="lg" className="min-w-[160px]" onClick={() => go(1)} data-testid="onboard-start">
          시작
          <ForwardedIconComponent name="ArrowRight" className="h-4 w-4" />
        </Button>
      </div>
    );
    next = null;
  } else if (step === "where") {
    page = (
      <>
        <Title>엔진</Title>
        <Choice
          icon="Laptop"
          title="이 기기"
          chip={<Chip tone="blue">권장</Chip>}
          sub={<span className="font-mono">{ENGINE} · PostgreSQL 16</span>}
          picked={where === "here"}
          onPick={() => setWhere("here")}
          testid="onboard-where-here"
        >
          {d.parts.length > 0 && (
            <div className="flex flex-wrap items-center gap-x-4 gap-y-1.5 border-t pt-3 pl-12">
              {d.parts.map((p) => (
                <span key={p.name} className="flex items-center gap-1.5 text-xs text-muted-foreground">
                  <i className={cn("h-2 w-2 rounded-full transition-colors duration-300", p.up ? "bg-accent-emerald-foreground" : "bg-muted-foreground/30")} />
                  {p.name}
                </span>
              ))}
            </div>
          )}
        </Choice>
        <Choice
          icon="Network"
          title="회사 엔진"
          chip={<Chip>PostgreSQL</Chip>}
          sub="읽기 전용 계정"
          picked={where === "server"}
          onPick={() => setWhere("server")}
          testid="onboard-where-server"
        />
      </>
    );
    next = (
      <Button onClick={() => setStep(where === "here" ? "judge" : "store")} data-testid="onboard-next">
        다음
        <ForwardedIconComponent name="ChevronRight" className="h-4 w-4" />
      </Button>
    );
  } else if (step === "judge") {
    page = (
      <>
        <Title>판정</Title>
        <Choice icon="ListChecks" title="규칙" chip={<Chip>키 없음</Chip>} sub="로컬" picked={judge === "rules"} onPick={() => setJudge("rules")} testid="onboard-judge-rules" />
        <Choice
          icon="Cpu"
          title="Jev"
          chip={<Chip tone="blue">원인 추적</Chip>}
          sub={<span className="font-mono">api.typesafe.ai</span>}
          picked={judge === "jev"}
          onPick={() => setJudge("jev")}
          testid="onboard-judge-jev"
        />
        {judge === "jev" && (
          <div className="flex w-full flex-col gap-3 rounded-xl border bg-background p-4 shadow-sm" data-testid="onboard-jev">
            <div className="grid grid-cols-[56px_minmax(0,1fr)_auto_20px] items-center gap-x-3 gap-y-3">
              <span className="text-right text-xs font-medium text-muted-foreground">키</span>
              <Input
                type="password"
                value={key}
                onChange={(e) => setKey(e.target.value)}
                placeholder={d.kept_key ? "저장된 키" : "jev_…"}
                disabled={busy}
                className="font-mono"
                data-testid="onboard-jev-key"
                onKeyDown={(e) => {
                  if (e.key === "Enter" && key.trim() && !busy) send({ do: "check_jev", key: key.trim() });
                }}
              />
              <Button variant="outline" size="sm" disabled={busy || !key.trim()} onClick={() => send({ do: "check_jev", key: key.trim() })} data-testid="onboard-jev-check">
                확인
              </Button>
              <StateIcon state={d.jev.state} text={d.jev.text} />
              <span className="text-right text-xs font-medium text-muted-foreground">모델</span>
              <Select value={d.model} disabled={d.jev.models.length === 0} onValueChange={(m) => send({ do: "model", model: m })}>
                <SelectTrigger className="h-9 font-mono text-sm" data-testid="onboard-jev-model">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent className="z-[70]">
                  {(d.jev.models.length ? d.jev.models : [d.model]).map((m) => (
                    <SelectItem key={m} value={m} className="font-mono">
                      {m}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
              <span className="col-span-2 flex items-center gap-1.5">
                {d.jev.state === "ok" && <Chip tone="ok">{d.jev.text ?? "확인됨"}</Chip>}
                {d.jev.state === "err" && (
                  <Chip tone="err" mono title={d.jev.text ?? undefined} className="max-w-[180px]">
                    {d.jev.text ?? "실패"}
                  </Chip>
                )}
                {d.kept_key && d.jev.state === "idle" && (
                  <Chip>
                    <ForwardedIconComponent name="KeyRound" className="h-3 w-3" />
                    키 저장됨
                  </Chip>
                )}
              </span>
            </div>
          </div>
        )}
      </>
    );
    next = (
      <Button disabled={!jevReady || busy} onClick={() => go(1)} data-testid="onboard-next">
        다음
        <ForwardedIconComponent name="ChevronRight" className="h-4 w-4" />
      </Button>
    );
  } else if (step === "agents") {
    const rows = [...d.agents].sort((a, b) => Number(b.found) - Number(a.found));
    page = (
      <>
        <Title
          aside={
            <Tabs value={hooks} onValueChange={(v) => setHooks(v as Hooks)}>
              <TabsList>
                <TabsTrigger value="all" data-testid="onboard-hooks-all">전부</TabsTrigger>
                <TabsTrigger value="pick" data-testid="onboard-hooks-pick">따로</TabsTrigger>
                <TabsTrigger value="later" data-testid="onboard-hooks-later">나중에</TabsTrigger>
              </TabsList>
            </Tabs>
          }
        >
          에이전트
        </Title>
        {!d.python && hooks !== "later" && (
          <div className="flex w-full items-center gap-3 rounded-xl border border-accent-amber-foreground/50 bg-background px-4 py-3" data-testid="onboard-python">
            <ForwardedIconComponent name="Terminal" className="h-4 w-4 text-accent-amber-foreground" />
            <span className="text-sm font-medium">Python</span>
            <Chip tone="warn">없음</Chip>
            <Button variant="outline" size="sm" className="ml-auto h-8" onClick={() => send({ do: "python" })} data-testid="onboard-python-get">
              <ForwardedIconComponent name="Download" className="h-4 w-4" />
              설치
            </Button>
          </div>
        )}
        <div className={cn("grid w-full grid-cols-2 gap-3", hooks === "later" && "opacity-40")}>
          {rows.map((a) => {
            const on = hooks !== "later" && (hooks === "all" ? usable(a) : picked.has(a.kind) && usable(a));
            const canPick = hooks === "pick" && usable(a);
            const toggle = () =>
              setPicked((p) => {
                const n = new Set(p);
                if (n.has(a.kind)) n.delete(a.kind);
                else n.add(a.kind);
                return n;
              });
            return (
              <div
                key={a.kind}
                onClick={canPick ? toggle : undefined}
                data-testid={`onboard-agent-${a.kind}`}
                className={cn(
                  "flex min-h-[72px] items-center gap-3 rounded-xl border bg-background px-3 py-3 shadow-sm transition-all",
                  on ? "border-ring ring-1 ring-ring" : "border-border",
                  !a.found && "opacity-50 shadow-none",
                  canPick && "cursor-pointer hover:shadow-md",
                )}
              >
                <span className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg bg-muted">
                  <AgentIcon kind={a.kind} className="h-5 w-5" />
                </span>
                <div className="flex min-w-0 flex-1 flex-col gap-1">
                  <div className="flex min-w-0 items-center gap-1.5">
                    <span className="truncate text-sm font-semibold">{a.name}</span>
                    {!a.found ? <Chip>없음</Chip> : a.per_folder && !a.folder ? <Chip tone="warn">폴더</Chip> : <Chip>{scope(a)}</Chip>}
                  </div>
                  {a.found && a.per_folder && (
                    <button
                      type="button"
                      disabled={busy}
                      onClick={(e) => {
                        e.stopPropagation();
                        send({ do: "pick", agent: a.kind });
                      }}
                      className="flex min-w-0 items-center gap-1 text-left text-xs text-muted-foreground hover:text-foreground"
                      data-testid={`onboard-folder-${a.kind}`}
                    >
                      <ForwardedIconComponent name="FolderOpen" className="h-3.5 w-3.5 shrink-0" />
                      <span className={cn("truncate", a.folder && "font-mono")}>{a.folder ?? "폴더 선택"}</span>
                    </button>
                  )}
                </div>
                {a.state !== "idle" ? (
                  <StateIcon state={a.state} text={a.text} />
                ) : hooks === "pick" && a.found ? (
                  <Checkbox checked={on} disabled={!usable(a)} onClick={(e) => e.stopPropagation()} onCheckedChange={toggle} />
                ) : on ? (
                  <StateIcon state="ok" />
                ) : null}
              </div>
            );
          })}
        </div>
      </>
    );
  } else if (step === "ready") {
    const shown = d.agents.filter((a) => chosen.some((c) => c.kind === a.kind) || a.state !== "idle");
    const up = d.parts.filter((p) => p.up).length;
    const engine: State = d.run.state === "err" ? "err" : d.parts.length > 0 && up === d.parts.length && d.run.state === "ok" ? "ok" : started ? "busy" : "idle";
    const needPython = !d.python && chosen.length > 0;
    page = (
      <>
        <Title
          aside={
            needPython ? (
              <Button variant="outline" size="sm" onClick={() => send({ do: "python" })} data-testid="onboard-python-get">
                <ForwardedIconComponent name="Download" className="h-4 w-4" />
                Python
              </Button>
            ) : shown.length > 0 && started ? (
              <Chip tone={shown.some((a) => a.state === "err") ? "err" : shown.every((a) => a.state === "ok") ? "ok" : "muted"} className="tabular-nums">
                <ForwardedIconComponent name="Webhook" className="h-3 w-3" />훅 {shown.filter((a) => a.state === "ok").length}/{shown.length}
              </Chip>
            ) : null
          }
        >
          {d.done ? "준비 완료" : "준비"}
        </Title>
        <ReadyCanvas d={d} agents={shown} engine={engine} judge={judge} started={started} />
      </>
    );
    next = d.done ? (
      <div className="flex items-center gap-2">
        <Button variant="outline" onClick={() => open(false)} data-testid="onboard-open">
          <ForwardedIconComponent name="Workflow" className="h-4 w-4" />
          기록
        </Button>
        <Button onClick={() => open(true)} data-testid="onboard-try">
          <ForwardedIconComponent name="Play" className="h-4 w-4" />
          체험
        </Button>
      </div>
    ) : (
      <Button disabled={started || busy || needPython} onClick={start} data-testid="onboard-run">
        {started ? <ForwardedIconComponent name="Loader2" className="h-4 w-4 animate-spin" /> : <ForwardedIconComponent name="Play" className="h-4 w-4" />}
        시작
      </Button>
    );
  } else {
    const set = (k: keyof typeof store) => (e: React.ChangeEvent<HTMLInputElement>) => setStore({ ...store, [k]: e.target.value });
    const sbusy = d.server.state === "busy";
    const row = (label: string, el: ReactNode) => (
      <>
        <span className="text-right text-xs font-medium text-muted-foreground">{label}</span>
        {el}
      </>
    );
    page = (
      <>
        <Title>회사 엔진</Title>
        <div className="flex w-full flex-col gap-3 rounded-xl border bg-background p-4 shadow-sm">
          <div className="flex items-center gap-2 text-sm font-semibold">
            <ForwardedIconComponent name="Database" className="h-4 w-4 text-muted-foreground" />
            PostgreSQL
          </div>
          <div className="grid grid-cols-[96px_minmax(0,1fr)] items-center gap-x-3 gap-y-2.5">
            {row("주소", <Input className="font-mono" value={store.url} onChange={set("url")} data-testid="onboard-store-url" />)}
            {row("사용자", <Input value={store.user} onChange={set("user")} />)}
            {row("데이터베이스", <Input value={store.database} onChange={set("database")} />)}
            {row("비밀번호", <Input type="password" value={store.password} onChange={set("password")} />)}
          </div>
        </div>
        <div className="flex w-full flex-col gap-3 rounded-xl border bg-background p-4 shadow-sm">
          <div className="flex items-center gap-2 text-sm font-semibold">
            <ForwardedIconComponent name="Webhook" className="h-4 w-4 text-muted-foreground" />훅
          </div>
          <div className="grid grid-cols-[96px_minmax(0,1fr)] items-center gap-x-3 gap-y-2.5">
            {row("엔진 주소", <Input className="font-mono" value={store.engine} onChange={set("engine")} placeholder="https://engine.example" />)}
            {row("WithHuman 키", <Input type="password" className="font-mono" value={store.key} onChange={set("key")} placeholder="whk_…" />)}
          </div>
        </div>
        <div className="flex w-full items-center gap-2">
          <Button variant="outline" size="sm" disabled={sbusy || !store.url} onClick={() => send({ do: "server_test", ...store })} data-testid="onboard-store-test">
            <ForwardedIconComponent name="PlugZap" className="h-4 w-4" />
            연결 확인
          </Button>
          {d.server.state !== "idle" && <StateIcon state={d.server.state} text={d.server.text} />}
          {d.server.state === "ok" && d.server.sessions !== null && (
            <Chip tone="ok" className="tabular-nums">
              세션 {d.server.sessions}
            </Chip>
          )}
          {d.server.state === "err" && (
            <Chip tone="err" mono title={d.server.text ?? undefined} className="max-w-[260px]">
              {d.server.text ?? "실패"}
            </Chip>
          )}
        </div>
      </>
    );
    next = (
      <Button
        disabled={sbusy || d.server.state !== "ok"}
        onClick={() => {
          wantTour();
          send({ do: "server_save", ...store });
        }}
        data-testid="onboard-save"
      >
        {sbusy && d.server.sessions !== null ? <ForwardedIconComponent name="Loader2" className="h-4 w-4 animate-spin" /> : null}
        열기
      </Button>
    );
  }

  const canBack = at > 0 && !locked;
  const back = () => (step === "store" ? setStep("where") : go(-1));

  return (
    <div className="fixed inset-0 z-[60] flex flex-col bg-background text-foreground" data-testid="onboard">
      <header className="flex h-12 shrink-0 items-center gap-2 border-b pl-[88px] pr-4" {...dragProps}>
        <WithHumanLogo className="h-5 w-5" />
        <span className="text-sm font-semibold">WithHuman</span>
        <span className="ml-auto flex items-center gap-1 text-xs tabular-nums text-muted-foreground">
          {at + 1} / {flow.length}
        </span>
      </header>
      <div className="flex min-h-0 flex-1">
        <nav className="flex w-[220px] shrink-0 flex-col gap-1 border-r bg-background p-3" aria-label="단계" data-testid="onboard-rail">
          {flow.map((s, i) => {
            const done = i < at || (s === "ready" && d.done);
            const here = s === step;
            const reachable = i < at && !locked;
            return (
              <button
                key={s}
                type="button"
                disabled={!reachable}
                onClick={() => setStep(s)}
                className={cn(
                  "flex h-9 items-center gap-2.5 rounded-md px-2 text-sm transition-colors",
                  here ? "bg-muted font-semibold text-foreground" : done ? "text-foreground" : "text-muted-foreground",
                  reachable && "hover:bg-muted",
                )}
                data-testid={`onboard-rail-${s}`}
              >
                <span
                  className={cn(
                    "flex h-5 w-5 shrink-0 items-center justify-center rounded-full border text-[11px] tabular-nums",
                    done ? "border-accent-emerald-foreground bg-accent-emerald-foreground text-background" : here ? "border-foreground" : "border-muted-foreground/40",
                  )}
                >
                  {done ? <ForwardedIconComponent name="Check" className="h-3 w-3" strokeWidth={3} /> : i + 1}
                </span>
                <ForwardedIconComponent name={RAIL[s].icon} className="h-4 w-4 shrink-0 opacity-70" />
                {RAIL[s].label}
              </button>
            );
          })}
        </nav>
        <main className="custom-scroll min-h-0 flex-1 overflow-y-auto">
          <div className={cn("mx-auto flex flex-col items-start gap-4 px-8 pb-10 pt-10", step === "welcome" || step === "ready" ? "max-w-[760px]" : "max-w-[620px]")}>
            {page}
          </div>
        </main>
      </div>
      {next !== null && (
        <footer className="flex h-16 shrink-0 items-center gap-3 border-t px-6">
          <div className="w-[196px]">
            {canBack && (
              <Button variant="ghost" onClick={back} data-testid="onboard-back">
                <ForwardedIconComponent name="ChevronLeft" className="h-4 w-4" />
                이전
              </Button>
            )}
          </div>
          <div className="flex flex-1 items-center justify-center gap-1.5">
            {flow.map((s, i) => (
              <i key={s} className={cn("h-1.5 rounded-full transition-all", i === at ? "w-5 bg-primary" : i < at ? "w-1.5 bg-primary/50" : "w-1.5 bg-muted-foreground/25")} />
            ))}
          </div>
          <div className="flex min-w-[196px] justify-end">{next}</div>
        </footer>
      )}
    </div>
  );
}

/** The onboarding while the app is on its first launch. */
export function FirstLaunch({ page, data, hasRecord }: { page: string | undefined; data: OnboardData | null | undefined; hasRecord: boolean }) {
  const show = page === "onboard" || (!hasRecord && !!data);
  const key = useMemo(() => (show ? "on" : "off"), [show]);
  if (!show) return null;
  return <Onboard key={key} data={data} />;
}
