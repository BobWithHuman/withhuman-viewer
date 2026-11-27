// The personal app's first launch, before the app opens: where the engine is (this machine, or the
// company's), how Calls are judged, which agents' hooks go in (every one found, picked one by
// one, or later), then the engine and the hooks coming up. Laid out as OpenClaw's macOS setup is
// (openclaw/openclaw, MIT: OnboardingView+Layout.swift): the 630-wide window, the 130pt icon and
// its glow, the large title, the parts of `./mac`, and the bar of back, dots and the prominent
// Next. It follows the Mac's light or dark appearance. The app (`src/ui/onboard.rs`) hands it how
// each step went and does what is pressed.

import { ChevronLeft, CircleAlert, Cpu, FolderOpen, Laptop, ListChecks, Loader2, Lock, Network, Sparkles, Trash2 } from "lucide-react";
import { useEffect, useState, type ReactNode } from "react";
import { send as sendTo } from "@/bridge";
import { AgentIcon, Choice, Feature, Field, Heading, Indicator, LinkButton, MAC_STYLE, MCard, NATIVE, Prominent, Segmented, StepRow, v, type State, type Status } from "./mac";
import appIcon from "../../../../icons/128x128@2x.png";

type Agent = {
  kind: string; name: string; found: boolean; per_folder: boolean; per_profile: boolean;
  /** The folder picked for an agent set up per project folder. */
  folder: string | null;
} & Status;
type Data = {
  /** `developer`: a developer's machine, set up on the company's engine first. */
  role?: "personal" | "developer";
  /** Python 3.8 or later, which the hooks run with; none when there is none. */
  python: string | null;
  kept_key: boolean;
  jev: Status & { models: string[] };
  model: string;
  parts: { name: string; up: boolean }[];
  agents: Agent[];
  run: Status;
  server: Status & { sessions: number | null };
  /** Everything is up: the app can open. */
  done: boolean;
};

const DEMO: Data = {
  python: "/opt/homebrew/bin/python3", kept_key: false,
  jev: { state: "idle", text: null, models: [] }, model: "jev-1.14.0",
  parts: ["PostgreSQL", "Decision", "Generator", "Collector"].map((name) => ({ name, up: false })),
  agents: [
    { kind: "claude", name: "Claude Code", found: true, per_folder: false, per_profile: false, folder: null, state: "idle", text: null },
    { kind: "codex", name: "Codex", found: true, per_folder: true, per_profile: false, folder: null, state: "idle", text: null },
    { kind: "cursor", name: "Cursor", found: true, per_folder: true, per_profile: false, folder: null, state: "idle", text: null },
    { kind: "hermes", name: "Hermes", found: false, per_folder: false, per_profile: true, folder: null, state: "idle", text: null },
  ],
  run: { state: "idle", text: null }, server: { state: "idle", text: null, sessions: null }, done: false,
};

const send = (m: Record<string, unknown>) => sendTo("onboard", m);

type Where = "here" | "server";
type Judge = "rules" | "jev";
type Hooks = "all" | "pick" | "later";
type Step = "welcome" | "where" | "judge" | "agents" | "ready" | "store";
const FLOW: Record<Where, Step[]> = { here: ["welcome", "where", "judge", "agents", "ready"], server: ["welcome", "where", "store"] };

const scope = (a: Agent) => (a.kind === "claude" ? "모든 폴더" : a.per_folder ? "폴더별" : "프로필");
const ENGINE = "127.0.0.1:47380";

export default function Onboard({ data }: { data: unknown }) {
  const d = (data as Data | null) ?? DEMO;
  const [where, setWhere] = useState<Where>(d.role === "developer" ? "server" : "here");
  const [step, setStep] = useState<Step>("welcome");
  const [judge, setJudge] = useState<Judge>(d.kept_key ? "jev" : "rules");
  const [key, setKey] = useState("");
  const [hooks, setHooks] = useState<Hooks>("all");
  const [picked, setPicked] = useState<Set<string>>(() => new Set(d.agents.filter((a) => a.found && !a.per_folder).map((a) => a.kind)));
  const [started, setStarted] = useState(false);
  const [store, setStore] = useState({ url: "postgres://127.0.0.1:15432/withhuman", user: "withhuman_viewer", database: "withhuman", password: "", engine: "", key: "" });
  useEffect(() => { if (d.run.state === "err") setStarted(false); }, [d.run.state]);

  const busy = d.run.state === "busy" || d.agents.some((a) => a.state === "busy") || d.jev.state === "busy";
  const usable = (a: Agent) => a.found && (!a.per_folder || !!a.folder);
  const chosen = hooks === "later" ? [] : d.agents.filter((a) => usable(a) && (hooks === "all" || picked.has(a.kind)));
  const jevReady = judge === "rules" || d.jev.state === "ok" || (d.kept_key && key.trim() === "");
  const flow = FLOW[where];
  const at = flow.indexOf(step);
  const go = (n: number) => setStep(flow[Math.max(0, Math.min(flow.length - 1, at + n))]);
  const start = () => {
    setStarted(true);
    send({ do: "start", jev: judge === "jev", key: judge === "jev" ? key.trim() : "", model: d.model, agents: chosen.map((a) => a.kind) });
  };
  const up = d.parts.filter((p) => p.up).length;

  let page: ReactNode;
  let next: ReactNode = <Prominent onClick={() => go(1)}>다음</Prominent>;

  if (step === "welcome") {
    page = (
      <>
        <Heading title="WithHuman" sub="코딩 에이전트 기록 · 판정" />
        <div className="w-full max-w-[520px]">
          <MCard gap={14} pad={16}>
            <Feature icon={Sparkles} title="기록" sub="요청 → 의도 → 호출 → 원인" />
            <Feature icon={Lock} title="이 기기 안에" sub="내장 엔진 · 로컬 저장 · Docker 없음" />
            <Feature icon={Trash2} title="언제든 제거" sub="훅 · 엔진 · 기록 한 번에" />
          </MCard>
        </div>
      </>
    );
  } else if (step === "where") {
    page = (
      <>
        <Heading title="엔진 위치" />
        <MCard>
          <Choice icon={<Laptop className="size-[19px]" strokeWidth={2.2} />} title="이 기기" badge="권장" sub={`${ENGINE} · PostgreSQL 16`} mono
            selected={where === "here"} onClick={() => setWhere("here")} />
          <Choice icon={<Network className="size-[19px]" strokeWidth={2.2} />} title="회사 엔진" sub="PostgreSQL · 읽기 전용 계정"
            selected={where === "server"} onClick={() => setWhere("server")} />
        </MCard>
      </>
    );
  } else if (step === "judge") {
    page = (
      <>
        <Heading title="판정" />
        <MCard>
          <Choice icon={<ListChecks className="size-[19px]" strokeWidth={2.2} />} title="규칙" badge="키 없음" sub="이 기기 안에서만"
            selected={judge === "rules"} onClick={() => setJudge("rules")} />
          <Choice icon={<Cpu className="size-[19px]" strokeWidth={2.2} />} title="Jev" sub="호출 내용 → api.typesafe.ai · 원인 추적" mono={false}
            selected={judge === "jev"} onClick={() => setJudge("jev")} />
        </MCard>
        {judge === "jev" && (
          <MCard gap={10}>
            <div className="grid grid-cols-[64px_minmax(0,1fr)_auto] items-center gap-x-3 gap-y-2.5 px-[10px]">
              <span className="text-right text-[13px]" style={{ color: v("fg2") }}>API 키</span>
              <Field secure value={key} onChange={setKey} placeholder={d.kept_key ? "저장된 키" : "jev_…"} disabled={busy} />
              <button style={NATIVE} disabled={busy || !key.trim()} onClick={() => send({ do: "check_jev", key: key.trim() })}>확인</button>
              <span className="text-right text-[13px]" style={{ color: v("fg2") }}>모델</span>
              <select style={{ ...NATIVE, justifySelf: "start" }} value={d.model} disabled={d.jev.models.length === 0} onChange={(e) => send({ do: "model", model: e.target.value })}>
                {(d.jev.models.length ? d.jev.models : [d.model]).map((m) => <option key={m} value={m}>{m}</option>)}
              </select>
              <span className="grid w-[17px] place-items-center">
                {d.jev.state === "busy" ? <Loader2 className="size-4 animate-spin" style={{ color: v("fg2") }} />
                  : d.jev.state === "ok" ? <Indicator selected /> : d.jev.state === "err" ? <CircleAlert className="size-4" style={{ color: v("orange") }} /> : null}
              </span>
            </div>
            {d.jev.state === "err" && <span className="px-[10px] text-[11px]" style={{ color: v("orange") }}>{d.jev.text}</span>}
          </MCard>
        )}
      </>
    );
    next = <Prominent disabled={!jevReady || busy} onClick={() => go(1)}>다음</Prominent>;
  } else if (step === "agents") {
    const rows = [...d.agents].sort((a, b) => Number(b.found) - Number(a.found));
    page = (
      <>
        <Heading title="에이전트" />
        <Segmented value={hooks} onChange={setHooks} options={[["all", "전부"], ["pick", "따로"], ["later", "나중에"]]} />
        {hooks === "later" ? (
          <MCard><StepRow state="idle" title="훅" detail="앱 · 훅 화면" /></MCard>
        ) : (
          <MCard gap={4} pad={10}>
            {rows.map((a) => {
              const on = hooks === "all" ? usable(a) : picked.has(a.kind) && usable(a);
              const sub = !a.found ? "없음" : a.per_folder ? (a.folder ?? "폴더별 · 폴더 선택") : scope(a);
              return (
                <Choice key={a.kind} dim={!a.found} icon={<AgentIcon kind={a.kind} />} title={a.name} sub={sub} mono={!!a.folder}
                  selected={on}
                  onClick={a.found && hooks === "pick" && usable(a) ? () => setPicked((p) => { const n = new Set(p); if (n.has(a.kind)) n.delete(a.kind); else n.add(a.kind); return n; }) : undefined}
                  trailing={a.per_folder && a.found ? (
                    <span onClick={(e) => e.stopPropagation()}><LinkButton disabled={busy} onClick={() => send({ do: "pick", agent: a.kind })}><FolderOpen className="size-3.5" />{a.folder ? "변경" : "폴더"}</LinkButton></span>
                  ) : undefined} />
              );
            })}
          </MCard>
        )}
      </>
    );
  } else if (step === "ready") {
    const shown = d.agents.filter((a) => chosen.some((c) => c.kind === a.kind) || a.state !== "idle");
    const engineState: State = d.run.state === "err" ? "err" : up === d.parts.length && d.run.state === "ok" ? "ok" : started ? "busy" : "idle";
    page = (
      <>
        <Heading title={d.done ? "준비 완료" : "시작 준비"} />
        <MCard gap={14} pad={16}>
          <StepRow state={engineState} title="엔진" mono detail={d.run.state === "err" ? d.run.text : `${ENGINE} · ${up}/${d.parts.length} · ${judge === "jev" ? "Jev" : "로컬 규칙"}`} />
          <StepRow state={d.python ? "ok" : "err"} title="Python" mono detail={d.python ?? "없음"} />
          {shown.map((a) => (
            <StepRow key={a.kind} state={a.state === "idle" && !started ? "idle" : a.state === "idle" ? "busy" : a.state} title={a.name}
              detail={a.state === "err" ? a.text : a.folder ?? scope(a)} mono={!!a.folder && a.state !== "err"} />
          ))}
        </MCard>
        {!d.python && chosen.length > 0 && <LinkButton onClick={() => send({ do: "python" })}>Python 설치…</LinkButton>}
        {d.done && <LinkButton onClick={() => send({ do: "open", try: true })}>체험 세션 열기</LinkButton>}
      </>
    );
    next = d.done
      ? <Prominent onClick={() => send({ do: "open", try: false })}>기록 열기</Prominent>
      : <Prominent disabled={started || busy || (!d.python && chosen.length > 0)} onClick={start}>{started && <Loader2 className="size-3.5 animate-spin" />}시작</Prominent>;
  } else {
    const set = (k: keyof typeof store) => (s: string) => setStore({ ...store, [k]: s });
    const sbusy = d.server.state === "busy";
    const row = (label: string, el: ReactNode) => (<><span className="text-right text-[13px]" style={{ color: v("fg2") }}>{label}</span>{el}</>);
    page = (
      <>
        <Heading title="회사 엔진" />
        <MCard gap={10} pad={16}>
          <div className="grid grid-cols-[92px_minmax(0,1fr)] items-center gap-x-3 gap-y-2.5">
            {row("주소", <Field mono value={store.url} onChange={set("url")} />)}
            {row("사용자", <Field value={store.user} onChange={set("user")} />)}
            {row("데이터베이스", <Field value={store.database} onChange={set("database")} />)}
            {row("비밀번호", <Field secure value={store.password} onChange={set("password")} />)}
          </div>
        </MCard>
        <MCard gap={10} pad={16}>
          <div className="grid grid-cols-[92px_minmax(0,1fr)] items-center gap-x-3 gap-y-2.5">
            {row("훅 엔진 주소", <Field value={store.engine} onChange={set("engine")} placeholder="https://engine.example" />)}
            {row("WithHuman 키", <Field secure value={store.key} onChange={set("key")} placeholder="whk_…" />)}
          </div>
        </MCard>
        <div className="flex items-center gap-2 text-[12px]">
          {d.server.state !== "idle" && <StepRow state={d.server.state} title="연결" detail={d.server.text} />}
          <button style={NATIVE} disabled={sbusy || !store.url} onClick={() => send({ do: "server_test", ...store })}>연결 확인</button>
        </div>
      </>
    );
    next = <Prominent disabled={sbusy || d.server.state !== "ok"} onClick={() => send({ do: "server_save", ...store })}>열기</Prominent>;
  }

  if (step === "where") next = <Prominent onClick={() => setStep(where === "here" ? "judge" : "store")}>다음</Prominent>;
  const canBack = at > 0 && !(step === "ready" && (started || d.done));

  return (
    <div className="mac flex h-screen flex-col select-none" style={{ background: v("bg"), color: v("fg") }}>
      <style>{MAC_STYLE}</style>
      {/* GlowingOpenClawIcon: 130pt, offset 10, in a 145pt band; its glow a tenth of its size. */}
      <div className="flex h-[145px] shrink-0 items-end justify-center">
        <img src={appIcon} alt="" className="size-[130px] translate-y-[10px]" style={{ filter: `drop-shadow(0 0 13px ${v("glow")}) drop-shadow(0 0 40px ${v("glow")})` }} />
      </div>
      <main className="scroll min-h-0 flex-1 overflow-y-auto px-7">
        <div className="mx-auto flex max-w-[574px] flex-col items-center gap-4 pt-7 pb-4">{page}</div>
      </main>
      <footer className="flex min-h-[60px] shrink-0 items-end gap-5 px-7 pb-[13px]">
        <div className="min-w-[80px]">
          {canBack && (
            <button onClick={() => (step === "store" ? setStep("where") : go(-1))} className="grid size-8 place-items-center opacity-80" style={{ color: v("fg2") }}>
              <ChevronLeft className="size-[18px]" strokeWidth={2.2} />
            </button>
          )}
        </div>
        <div className="flex flex-1 justify-center">
          {flow.map((s, i) => (
            <span key={s} className="grid size-7 place-items-center">
              <i className="size-2 rounded-full" style={{ background: i === at ? v("accent") : "rgba(142,142,147,.3)" }} />
            </span>
          ))}
        </div>
        <div className="flex min-w-[80px] justify-end pb-[2px]">{next}</div>
      </footer>
    </div>
  );
}
