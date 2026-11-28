// What the record panel says of a step, without React: what the engine decided (VERDICT, ONLY),
// how a step is coloured in Langflow's tokens (tone), the engine's rule names (RULE, WORST), and
// the way back from a step to the request it came from (wayBack, ported from
// web/record/src/path.ts), each hop named and, where the engine gave one, with its probability.

import type { RecordData, Step, Verdict } from "../types";

export const clock = (at: Step["at"]) => {
  if (at === undefined) return "";
  if (typeof at === "string") return at;
  const d = new Date(at);
  const z = (n: number) => String(n).padStart(2, "0");
  return `${z(d.getHours())}:${z(d.getMinutes())}:${z(d.getSeconds())}`;
};

export const pct = (p: number) => `${Math.round(p * 100)}%`;

/** Langflow icon (Lucide) per kind of step. */
export const KIND_ICON: Record<Step["kind"], string> = {
  device: "Laptop", agent: "Bot", prompt: "MessageSquare", model: "CircleDot", call: "Wrench",
  result: "FileText", answer: "MessageSquareText", group: "Layers",
};

export const VERDICT: Record<Verdict, string> = { allow: "허용", review: "승인 요청", rejected: "승인 거절", deny: "차단" };

/** The panel's filter chips: the steps a tool result told, the refused, those the person was asked about. */
export type Only = "outside" | "deny" | "ask";
export const ONLY: Record<Only, { label: string; is: (s: Step) => boolean }> = {
  outside: { label: "외부 지시", is: (s) => !!s.catch },
  deny: { label: "차단", is: (s) => s.verdict === "deny" },
  ask: { label: "승인 요청", is: (s) => s.verdict === "review" || s.verdict === "rejected" },
};

export const flagged = (s: Step) => !!s.catch || (!!s.verdict && s.verdict !== "allow");
export const refused = (s: Step) => s.verdict === "deny" || s.verdict === "rejected";

export type Tone = "deny" | "hold" | "catch" | "quiet";
/** A step's colour: refused red, held amber, caught yellow, anything else plain. */
export const toneOf = (s: Step): Tone =>
  s.verdict === "deny" ? "deny" : s.verdict === "review" || s.verdict === "rejected" ? "hold" : s.catch ? "catch" : "quiet";

/** The tone as Langflow classes: the icon tile, the chip, the text. */
export const TONE: Record<Tone, { tile: string; chip: string; text: string; hsl: string }> = {
  deny: { tile: "bg-error-background text-error-foreground", chip: "bg-error-background text-error-foreground", text: "text-destructive", hsl: "var(--destructive)" },
  hold: { tile: "bg-accent-amber text-accent-amber-foreground", chip: "bg-accent-amber text-accent-amber-foreground", text: "text-accent-amber-foreground", hsl: "var(--accent-amber-foreground)" },
  catch: { tile: "bg-datatype-yellow/20 text-datatype-yellow", chip: "bg-datatype-yellow/20 text-datatype-yellow", text: "text-datatype-yellow", hsl: "var(--datatype-yellow)" },
  quiet: { tile: "bg-muted text-muted-foreground", chip: "bg-muted text-muted-foreground", text: "text-muted-foreground", hsl: "var(--muted-foreground)" },
};

/** What the engine said of a step, in a chip's words. */
export const saidOf = (s: Step) =>
  s.verdict && s.verdict !== "allow" ? VERDICT[s.verdict] : s.catch ? `지시 ${s.catch.score}%` : null;

/** An engine rule, by what it decided on. */
export const RULE: Record<string, string> = {
  "intent::mismatch": "의도 밖", "intent::review": "의도 밖일 수 있음", "intent::match": "의도 안", "intent::hijack": "목표 탈취",
  "intent::unknown": "의도 없음", "trace::unresolved": "원인 못 찾음", "trace::no_subgraph": "추적 대상 없음", "cause::none": "원인 없음",
  "safeguard::change": "에이전트 설정 변경", "causal::instruction": "외부 지시",
};
/** The worst of what a Call breaks. */
export const WORST: Record<string, string> = {
  explicit: "사용자 조건 어김", unrelated_change: "요청과 무관한 변경", exfiltration: "데이터 외부 전송", unconfirmed_irreversible: "되돌릴 수 없는 작업",
  security_change: "보안 설정 변경", out_of_scope_access: "요청 범위 밖 접근", hijacked_goal: "끌려온 새 목표",
};
type Judge = NonNullable<Step["judge"]>;
/** What decided a Call, in one name: what the second question asked, else the worst breach, else the rule. */
export const judgeLabel = (j: Judge) =>
  (j.stage !== "1차 질문" && j.asked) || (j.worst && (WORST[j.worst] ?? j.worst)) || (j.rule && (RULE[j.rule] ?? j.rule)) || null;
/** What the score measures: the first question's departure, a path rule's risk, the second's likelihood. */
export const measureOf = (j: Judge) => (j.stage === "1차 질문" || !j.stage ? "이탈" : j.rule?.startsWith("causal::") ? "위험" : "확률");

// --- the way back ------------------------------------------------------------

export type Tie = "ask" | "cause" | "result" | "parent";
/** One link on the way: `from` led to `to`. */
export type Hop = { from: string; to: string; tie: Tie; label: string; p?: number };
export type Way = { ids: string[]; hops: Hop[] };

export const TIE: Record<Tie, { label: string; hsl: string }> = {
  ask: { label: "요청에 따라", hsl: "var(--accent-blue-foreground)" },
  cause: { label: "외부 지시", hsl: "var(--destructive)" },
  result: { label: "결과 반환", hsl: "var(--muted-foreground)" },
  parent: { label: "이어 물음", hsl: "var(--accent-emerald-foreground)" },
};
/** How a prompt's parent is named on its link, by what it was. */
export const PARENT_LABEL: Record<string, string> = { result: "결과 보고 물음", message: "답변 보고 물음", request: "이전 요청 이어서" };

/** The way back from `id`, the step itself first. */
export function wayBack(data: RecordData, id: string | null): Way {
  const index = new Map(data.steps.map((s, i) => [s.id, i]));
  const step = new Map(data.steps.map((s) => [s.id, s]));
  if (!id || !step.has(id)) return { ids: [], hops: [] };
  const ids = [id];
  const hops: Hop[] = [];
  const seen = new Set(ids);
  let cur = id;
  for (let guard = 0; guard < data.steps.length; guard++) {
    const s = step.get(cur);
    if (!s) break;
    const back = before(data, s, index, step);
    if (!back || seen.has(back.from)) break;
    hops.push(back);
    ids.push(back.from);
    seen.add(back.from);
    cur = back.from;
  }
  return { ids, hops };
}

function before(data: RecordData, s: Step, index: Map<string, number>, step: Map<string, Step>): Hop | null {
  if (s.kind === "prompt") {
    const p = data.parents?.find((x) => x.prompt === s.id);
    return p && step.has(p.from) ? { from: p.from, to: s.id, tie: "parent", label: PARENT_LABEL[p.via] ?? TIE.parent.label, p: p.p } : null;
  }
  const cause = data.causes.find(([, b]) => b === s.id);
  if (cause) return { from: cause[0], to: s.id, tie: "cause", label: TIE.cause.label, p: causeP(data, cause[0], s.id, step) };
  if (s.kind === "result") {
    const link = data.links.find(([, b]) => b === s.id);
    if (link) return { from: link[0], to: s.id, tie: "result", label: TIE.result.label };
  }
  // Its turn's prompt: the last before it.
  const at = index.get(s.id) ?? 0;
  for (let i = at - 1; i >= 0; i--) {
    if (data.steps[i].kind === "prompt") return { from: data.steps[i].id, to: s.id, tie: "ask", label: TIE.ask.label };
  }
  return null;
}

/** How sure the engine is that a tool result told a Call: the SubGraph's own figure, else how
 * sure it was the result held an instruction. */
function causeP(data: RecordData, from: string, to: string, step: Map<string, Step>) {
  const g = data.subgraphs?.find((x) => x.via === "cause" && x.steps.includes(to));
  if (g?.p != null) return g.p;
  const c = step.get(from)?.catch;
  return c ? c.score / 100 : undefined;
}

/** The step to open a session on: the Call opened from another page, else the latest refused Call. */
export const opening = (data: RecordData) =>
  (data.focus && data.steps.some((s) => s.id === data.focus) ? data.focus : null) ??
  [...data.steps].reverse().find((s) => (s.kind === "call" || s.kind === "group") && refused(s))?.id ??
  null;
