// The way back from a step to the request it came from, as the engine traced it: a Call to the
// tool result that told it (a cause) or else to its turn's prompt; a tool result to the Call it
// answered; a prompt to the step of the turn before that its writer built on (its parent), and
// on until a prompt with none.

import type { RecordData, Step } from "./data";

export type Tie = "ask" | "cause" | "result" | "parent";
/** One link on the way: `from` led to `to`. */
export type Hop = { from: string; to: string; tie: Tie; label: string; p?: number };

export const TIE: Record<Tie, { color: string; label: string }> = {
  ask: { color: "#548cd0", label: "요청" },
  cause: { color: "#f2a93b", label: "외부 지시" },
  result: { color: "#bcbcbc", label: "결과" },
  parent: { color: "#58c0a4", label: "이어 물음" },
};

/** How a prompt's parent is named on its link, by what it was. */
export const PARENT_LABEL: Record<string, string> = {
  result: "결과 보고 물음",
  message: "답변 보고 물음",
  request: "이전 요청 이어서",
};

/** The way back from `id`, the step itself first. */
export function wayBack(data: RecordData, id: string | null): { ids: string[]; hops: Hop[] } {
  const index = new Map(data.steps.map((s, i) => [s.id, i]));
  const step = new Map(data.steps.map((s) => [s.id, s]));
  // A step picked in a session no longer shown has no way here.
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
  if (cause) return { from: cause[0], to: s.id, tie: "cause", label: TIE.cause.label };
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
