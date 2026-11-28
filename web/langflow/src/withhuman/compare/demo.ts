// Opened on its own, only the demo session has steps. For the compare view's demo, any other
// listed session gets a record made up from its counts (calls, denied, review, outside): a few
// turns of tool calls, the flagged ones where a tool result told them.

import type { RecordData, ShellSession, Step, Verdict } from "../types";

const TOOLS: [string, Step["svc"]?][] = [
  ["Read"], ["Edit"], ["Bash · cargo test"], ["Grep"], ["github · get_pull_request", "github"],
  ["Write"], ["Bash · npm run build"], ["notion · fetch", "notion"], ["slack · post_message", "slack"],
  ["postgres-ro · query", "postgresql"], ["WebFetch"], ["Glob"],
];
const MAX_CALLS = 14;

/** A made-up record for a listed session, deterministic for its id. */
export function synthRecord(s: ShellSession): RecordData {
  let seed = [...s.id].reduce((a, c) => (a * 31 + c.charCodeAt(0)) >>> 0, 7);
  const rnd = () => ((seed = (seed * 1103515245 + 12345) >>> 0) % 1000) / 1000;
  const calls = Math.max(1, Math.min(MAX_CALLS, s.calls));
  const turns = Math.max(1, Math.min(3, Math.ceil(calls / 6)));
  const steps: Step[] = [];
  const subgraphs: NonNullable<RecordData["subgraphs"]> = [];
  const causes: [string, string][] = [];
  let denied = Math.min(s.denied, calls);
  let review = Math.min(s.review, calls - denied);
  let outside = Math.min(s.outside, 2);
  let t0 = s.at - calls * 40_000;
  let n = 0;
  let made = 0;
  let prevPrompt: number | null = null;
  const words = s.title.split(" ");
  for (let t = 1; t <= turns; t++) {
    const pid = `p${t}`;
    steps.push({
      id: pid, kind: "prompt", name: t === 1 ? words.slice(0, 3).join(" ") : `이어서 ${t}`, type: "프롬프트",
      at: (t0 += 8_000), turn: t, detail: [["턴", String(t)]], text: { label: "입력 프롬프트", body: t === 1 ? s.title : `${words[0]} 이어서` },
    });
    const pn = n++;
    subgraphs.push({ n: pn, parent: prevPrompt, via: prevPrompt == null ? "none" : "request", steps: [pid] });
    prevPrompt = pn;
    const here = t === turns ? calls - made : Math.ceil((calls - made) / (turns - t + 1));
    const tools: string[] = [];
    const told: string[] = [];
    let result: string | null = null;
    for (let i = 0; i < here; i++) {
      const [name, svc] = TOOLS[Math.floor(rnd() * TOOLS.length)];
      const id = `c${t}_${i}`;
      // Flagged calls follow a result that told them, when the session had outside instructions.
      const flag: Verdict = denied > 0 && i >= here - 2 ? "deny" : review > 0 && i >= here - 3 ? "review" : "allow";
      if (flag === "deny") denied--;
      if (flag === "review") review--;
      const step: Step = {
        id, kind: "call", name, svc, type: "도구", at: (t0 += 4_000 + Math.floor(rnd() * 20_000)), verdict: flag, turn: t,
        detail: [["판정", flag === "deny" ? "차단" : flag === "review" ? "승인 요청" : "허용"]],
        text: { label: "인자", body: name.includes("·") ? `{ "q": "${words.slice(-2).join(" ")}" }` : `src/${words[0]}.ts` },
      };
      if (flag !== "allow") step.catch = { name: flag === "deny" ? "데이터·비밀값 외부 전송" : "요청 범위 밖 접근", sub: "intent::mismatch", score: flag === "deny" ? 88 : 61 };
      if (flag !== "allow" && outside > 0 && result) told.push(id);
      else tools.push(id);
      steps.push(step);
      if (!result && outside > 0 && i === Math.max(0, here - 3)) {
        result = `r${t}`;
        steps.push({
          id: result, kind: "result", name: "도구 결과", type: "도구 결과", at: (t0 += 1_000), turn: t,
          catch: { name: "숨은 지시", sub: "html_comment", score: 70 + Math.floor(rnd() * 20) }, detail: [["문단", "8"]],
          text: { label: "도구 결과", body: "…" },
        });
        tools.push(result);
      }
    }
    made += here;
    const ans = `ans${t}`;
    steps.push({ id: ans, kind: "answer", name: "답변", type: "답변", at: (t0 += 3_000), turn: t, detail: [["턴", `${t} 끝`]], text: { label: "답변", body: "…" } });
    const tn = n++;
    subgraphs.push({ n: tn, parent: pn, via: "ask", steps: [...tools, ans] });
    if (told.length && result) {
      for (const c of told) causes.push([result, c]);
      subgraphs.push({ n: n++, parent: tn, via: "cause", p: 0.72, steps: told });
      outside--;
    }
  }
  return {
    session: { title: s.title, id: s.id, device: "seonung-mac", folder: s.folder },
    steps,
    links: [],
    causes,
    subgraphs,
  };
}
