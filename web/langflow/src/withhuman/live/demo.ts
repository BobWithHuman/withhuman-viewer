// Opened on its own with `?live` in the hash (`#/flow/<id>?live`): the demo session grows as the
// engine would record it - it starts with its first steps and gets one more on a timer, then a
// third turn of calls, then stops (`live` off). Each step is a new `render` of the same session,
// as the app sends while a session runs.

import { DEMO_PERSONAL, setDemo, standalone, type RecordData, type Step } from "../bridge";

const TICK = 1_800;
const START = 2;

const EXTRA: Step[] = [
  { id: "p3", kind: "prompt", name: "타임아웃 설정 고치기", type: "프롬프트", turn: 3, detail: [["턴", "3"]], text: { label: "입력 프롬프트", body: "배치 타임아웃 600초로 올리는 PR 만들어 줘." } },
  { id: "c5", kind: "call", name: "Grep", type: "도구", turn: 3, verdict: "allow", detail: [["판정", "허용"]], text: { label: "인자", body: "timeout = 300" } },
  { id: "c6", kind: "call", name: "Read", type: "도구", turn: 3, verdict: "allow", detail: [["판정", "허용"]], text: { label: "인자", body: "config/refund-batch.toml" } },
  { id: "c7", kind: "call", name: "Edit", type: "도구", turn: 3, verdict: "allow", detail: [["판정", "허용"]], text: { label: "인자", body: "config/refund-batch.toml" } },
  { id: "c8", kind: "call", name: "Bash · cargo test", type: "도구", turn: 3, verdict: "allow", detail: [["판정", "허용"]], text: { label: "인자", body: "cargo test -p refund" } },
  { id: "c9", kind: "call", name: "github · create_pull_request", type: "도구", svc: "github", turn: 3, verdict: "review",
    catch: { name: "외부 쓰기", sub: "intent::review", score: 58 }, detail: [["판정", "승인 요청"]], text: { label: "인자", body: '{ "title": "refund-batch: timeout 600s" }' } },
  { id: "ans3", kind: "answer", name: "PR 안내", type: "답변", turn: 3, detail: [["턴", "3 끝"]], text: { label: "답변", body: "PR을 만들었습니다." } },
];
const EXTRA_GRAPHS = [
  { n: 6, parent: 5, via: "request", steps: ["p3"] },
  { n: 7, parent: 6, via: "ask", steps: ["c5", "c6", "c7", "c8", "c9", "ans3"] },
];

function counts(steps: Step[], causes: [string, string][]) {
  const calls = steps.reduce((n, s) => n + (s.kind === "call" ? 1 : s.kind === "group" ? s.inside?.length ?? 1 : 0), 0);
  return {
    calls,
    denied: steps.filter((s) => s.verdict === "deny" || s.verdict === "rejected").length,
    review: steps.filter((s) => s.verdict === "review").length,
    outside: steps.filter((s) => causes.some(([, c]) => c === s.id)).length,
  };
}

function at(k: number, all: Step[], subgraphs: NonNullable<RecordData["subgraphs"]>): RecordData {
  const now = Date.now();
  const steps = all.slice(0, k).map((s, i) => ({ ...s, at: now - (k - i) * TICK }));
  const done = k >= all.length;
  const id = DEMO_PERSONAL.session.id;
  const shell = DEMO_PERSONAL.shell!;
  return {
    ...DEMO_PERSONAL,
    steps,
    subgraphs,
    shell: {
      ...shell,
      sessions: shell.sessions.map((s) => (s.id === id ? { ...s, ...counts(steps, DEMO_PERSONAL.causes), at: now, live: !done } : s)),
    },
  };
}

if (standalone && /[?&]live\b/.test(window.location.hash)) {
  const all = [...DEMO_PERSONAL.steps, ...EXTRA];
  const graphs = [...(DEMO_PERSONAL.subgraphs ?? []), ...EXTRA_GRAPHS];
  let k = START;
  setDemo((s) => ({ ...s, record: at(k, all, graphs) }));
  const timer = window.setInterval(() => {
    k += 1;
    setDemo((s) => ({ ...s, record: at(k, all, graphs) }));
    if (k >= all.length) window.clearInterval(timer);
  }, TICK);
  (window as unknown as { __whLive?: () => number }).__whLive = () => k;
}
