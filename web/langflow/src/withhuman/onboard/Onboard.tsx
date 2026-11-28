// The personal app's first launch, in Langflow's own multi-step modal (modals/stepperModal) as its
// knowledgeBaseUploadModal uses it: the header (icon tile, title, progress), the step in the
// bordered content box, the side panel (StatusPanel: the parts, the judging, the agents, live),
// the footer (StepperModalFooter: back / next / the primary). Shown alone in the app's small
// first-launch window: always open, no close button, a plain backdrop instead of the dimming,
// Escape and outside presses ignored. Same data (`views.onboard`, src/ui/onboard.rs) and messages
// (`send({ page: "onboard", do })`) as web/console's pages/onboard.tsx. Steps:
//   엔진      - this Mac or the company's engine;
//   판정      - local rules, or Jev with its key checked and its model picked;
//   에이전트  - every agent with its logo, found or not, a folder for some, Python missing;
//   준비      - the summary, 시작; then 체험 / 기록;
//   회사 엔진 - (the company's engine, after 엔진) the store's form, tested, 열기.
// Finishing marks the first session's tour as wanted (tour/store.ts).

import { useEffect, useMemo, useRef, useState } from "react";
import { MODAL_HEIGHT_DEFAULT } from "@/modals/knowledgeBaseUploadModal/constants";
import { StepperModal, StepperModalFooter, useStepperState } from "@/modals/stepperModal";
import { dragProps } from "../chrome/app";
import { wantTour } from "../tour/store";
import { type OnboardAgent, type OnboardData, STORE, send } from "./data";
import { demoStart } from "./demo";
import { StatusPanel } from "./StatusPanel";
import { type Judge, type Where, StepAgents, StepJudge, StepReady, StepStore, StepWhere } from "./steps";

/**
 * The modal's size: Langflow's dialog width (`max-w-lg`, 512 px) and the knowledge-base modal's
 * default height (580 px), with the stepper's side panel (300 px + its 1 px rule) beside it.
 * The app's first-launch window = 813 x 582 logical px (the panel overhangs 1 px above and below).
 */
const WIDTH = "w-[512px]";
const HEIGHT = String(MODAL_HEIGHT_DEFAULT);

type Step = "where" | "judge" | "agents" | "ready" | "store";
const HEAD: Record<Step, { title: string; icon: string }> = {
  where: { title: "엔진", icon: "Server" },
  judge: { title: "판정", icon: "Scale" },
  agents: { title: "에이전트", icon: "Bot" },
  ready: { title: "준비", icon: "Rocket" },
  store: { title: "회사 엔진", icon: "Network" },
};
const FLOW: Record<Where, Step[]> = { here: ["where", "judge", "agents", "ready"], server: ["where", "store"] };

/** Before the app's first data: nothing found yet. */
const EMPTY: OnboardData = {
  python: null, kept_key: false, jev: { state: "idle", text: null, models: [] }, model: "jev-1.14.0",
  parts: [], agents: [], run: { state: "idle", text: null }, server: { state: "idle", text: null, sessions: null }, done: false,
};

const usable = (a: OnboardAgent) => a.found && (!a.per_folder || !!a.folder);

export function Onboard({ data }: { data: OnboardData | null | undefined }) {
  const d = data ?? EMPTY;
  const [where, setWhere] = useState<Where>(d.role === "developer" ? "server" : "here");
  const [judge, setJudge] = useState<Judge>(demoStart.judge === "jev" || d.kept_key ? "jev" : "rules");
  const [key, setKey] = useState("");
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
  const chosen = d.agents.filter((a) => usable(a) && picked.has(a.kind));
  const going = d.agents.filter((a) => chosen.includes(a) || a.state !== "idle");
  const jevReady = judge === "rules" || d.jev.state === "ok" || (d.kept_key && key.trim() === "");
  const needPython = !d.python && chosen.length > 0;

  const flow = FLOW[where];
  const steps = useMemo(
    () => flow.map((id) => ({ id, canNext: id === "judge" ? jevReady && !busy : true })),
    [flow, jevReady, busy],
  );
  const first = demoStart.step && flow.includes(demoStart.step as Step) ? flow.indexOf(demoStart.step as Step) + 1 : 1;
  const stepper = useStepperState({ steps, initialStep: first });
  const step = stepper.currentStepId as Step;

  const start = () => {
    setStarted(true);
    send({ do: "start", jev: judge === "jev", key: judge === "jev" ? key.trim() : "", model: d.model, agents: chosen.map((a) => a.kind) });
  };
  const open = (tryIt: boolean) => {
    wantTour();
    send({ do: "open", try: tryIt });
  };
  const toggle = (kind: string) =>
    setPicked((p) => {
      const n = new Set(p);
      if (n.has(kind)) n.delete(kind);
      else n.add(kind);
      return n;
    });

  // The window has no title bar: a press on bare header space moves it, as the app's other pages.
  useEffect(() => {
    const down = (e: MouseEvent) => {
      const t = e.target as HTMLElement;
      const header = t.closest('[role="dialog"]')?.firstElementChild;
      if (header?.contains(t)) dragProps.onMouseDown(e as unknown as React.MouseEvent);
    };
    document.addEventListener("mousedown", down);
    return () => document.removeEventListener("mousedown", down);
  }, []);

  let content: React.ReactNode;
  if (step === "where") content = <StepWhere where={where} onWhere={setWhere} />;
  else if (step === "judge") content = <StepJudge d={d} judge={judge} onJudge={setJudge} keyText={key} onKey={setKey} busy={busy} />;
  else if (step === "agents") content = <StepAgents d={d} picked={picked} onToggle={toggle} needPython={needPython} busy={busy || started} />;
  else if (step === "ready") content = <StepReady d={d} judge={judge} agents={going} needPython={needPython} />;
  else content = <StepStore d={d} store={store} onStore={setStore} />;

  // A hook that failed once everything stopped: 시작 can be pressed again.
  const failed = !busy && d.agents.some((a) => a.state === "err");
  const locked = step === "ready" && ((started && !failed) || d.done);
  const common = {
    currentStep: stepper.currentStep,
    totalSteps: stepper.totalSteps,
    onBack: locked ? undefined : stepper.back,
    onNext: stepper.next,
    nextDisabled: !stepper.canGoNext,
    nextLabel: "다음",
    backLabel: "이전",
  };
  let footer: React.ReactNode;
  if (step === "ready" && d.done)
    footer = <StepperModalFooter {...common} submitLabel="체험" onSubmit={() => open(true)} helpLabel="기록" onHelp={() => open(false)} submitTestId="onboard-try" />;
  else if (step === "ready")
    footer = <StepperModalFooter {...common} submitLabel="시작" onSubmit={start} submitDisabled={busy || needPython} isSubmitting={started && !failed} submitTestId="onboard-run" />;
  else if (step === "store")
    footer = (
      <StepperModalFooter
        {...common}
        submitLabel="열기"
        submitDisabled={d.server.state !== "ok"}
        isSubmitting={d.server.state === "busy" && d.server.sessions !== null}
        onSubmit={() => {
          wantTour();
          send({ do: "server_save", ...store });
        }}
        submitTestId="onboard-save"
        helpLabel="연결 확인"
        onHelp={() => {
          if (d.server.state !== "busy" && store.url) send({ do: "server_test", ...store });
        }}
      />
    );
  else footer = <StepperModalFooter {...common} />;

  const head = step === "ready" && d.done ? { title: "준비 완료", icon: "CircleCheck" } : HEAD[step];

  return (
    <StepperModal
      open
      onOpenChange={() => {}}
      hideCloseButton
      overlayClassName="bg-background backdrop-blur-none"
      // No entrance animation: in the app's WebKit frame it can stay on its first frame (opacity 0).
      className="max-h-none bg-background !animate-none !opacity-100"
      contentClassName="bg-muted"
      currentStep={stepper.currentStep}
      totalSteps={stepper.totalSteps}
      title={head.title}
      icon={head.icon}
      width={WIDTH}
      height={HEIGHT}
      sidePanel={<StatusPanel d={d} server={where === "server"} judge={judge} agents={going} needPython={needPython} started={started} />}
      sidePanelOpen
      footer={footer}
    >
      <div data-testid="onboard" data-step={step}>
        {content}
      </div>
    </StepperModal>
  );
}

/** The onboarding while the app is on its first launch. */
export function FirstLaunch({ page, data, hasRecord }: { page: string | undefined; data: OnboardData | null | undefined; hasRecord: boolean }) {
  const show = page === "onboard" || (!hasRecord && !!data);
  if (!show) return null;
  return <Onboard data={data} />;
}
