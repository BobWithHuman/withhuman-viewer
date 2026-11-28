// A recorded session as a Langflow flow: each box of the session (the SubGraphs the engine cut it
// into, grouped as web/record's GraphView groups them) becomes one Langflow generic node, laid
// out left to right as web/record's `layoutTree` lays them (a column per depth, a row per box in
// the order its first step was made), and the ties between boxes become Langflow edges with
// Langflow's own encoded handles, so the canvas draws them as it draws its own.

import type { RecordData, Step } from "../types";

type Json = any;

/** Langflow's handle id: its keys sorted, `"` written `œ` (utils/reactflowUtils' scapedJSONStringfy,
 * copied so the mock does not load Langflow's stores before the page does). */
function stringify(v: unknown): string {
  if (v === undefined) return "null";
  if (v === null || typeof v !== "object") return JSON.stringify(v);
  if (Array.isArray(v)) return `[${v.map(stringify).join(",")}]`;
  const o = v as Record<string, unknown>;
  return `{${Object.keys(o).sort().map((k) => `"${k}":${stringify(o[k])}`).join(",")}}`;
}
const handleId = (v: object) => stringify(v).replace(/"/g, "œ");

// --- the boxes (ported from web/record/src/GraphView.tsx `boxes()`) ---------

export type Box = { n: number; ns: number[]; parent: number | null; via: string; p?: number | null; steps: string[] };

/** The SubGraphs as boxes: the agent's steps that hang the same way from one SubGraph (the tools
 * a prompt asked for, one by one, and the turn's answer) share a box; what a tool result told and
 * a prompt written from the turn before keep their own, where the line branches. */
export function boxes(data: RecordData): Box[] {
  const gs = [...(data.subgraphs ?? [])].sort((a, b) => a.n - b.n);
  const first = new Map(data.steps.map((s) => [s.id, s.kind]));
  const joins = (g: (typeof gs)[number]) =>
    g.parent != null && (g.via === "ask" || g.via === "answer") && first.get(g.steps[0]) !== "prompt";
  const boxOf = new Map<number, number>();
  const shared = new Map<number, number>();
  let last: number | undefined;
  for (const g of gs) {
    const into = joins(g) && shared.get(g.parent!) === last ? last : undefined;
    boxOf.set(g.n, into ?? g.n);
    if (joins(g) && into === undefined) shared.set(g.parent!, g.n);
    last = boxOf.get(g.n);
  }
  const out = new Map<number, Box>();
  for (const g of gs) {
    const b = boxOf.get(g.n)!;
    const box = out.get(b) ?? { n: b, ns: [], parent: null, via: "none", p: null, steps: [] };
    box.ns.push(g.n);
    box.steps.push(...g.steps);
    if (b === g.n && g.parent != null) {
      box.parent = boxOf.get(g.parent) ?? g.parent;
      box.via = g.via;
      box.p = g.p;
    }
    out.set(b, box);
  }
  const order = new Map(data.steps.map((s, i) => [s.id, i]));
  for (const box of out.values()) box.steps.sort((a, b) => (order.get(a) ?? 0) - (order.get(b) ?? 0));
  return [...out.values()];
}

/** Without SubGraphs (an older engine): each prompt its own box, what followed it one box
 * hanging from it. */
function turns(data: RecordData): Box[] {
  const out: Box[] = [];
  let prompt = null as Box | null;
  let tools = null as Box | null;
  for (const s of data.steps) {
    if (s.kind === "prompt") {
      const before: Box | null = prompt;
      prompt = { n: out.length, ns: [out.length], parent: before ? before.n : null, via: before ? "request" : "none", p: null, steps: [s.id] };
      out.push(prompt);
      tools = null;
      continue;
    }
    if (!tools) {
      tools = { n: out.length, ns: [out.length], parent: prompt?.n ?? null, via: prompt ? "ask" : "none", p: null, steps: [] };
      out.push(tools);
    }
    tools.steps.push(s.id);
  }
  // A prompt the engine traced to a step of the turn before hangs from that step's box.
  for (const b of out) {
    const tie = data.parents?.find((p) => p.prompt === b.steps[0]);
    const into = tie && out.find((o) => o !== b && o.steps.includes(tie.from));
    if (tie && into) Object.assign(b, { parent: into.n, via: tie.via, p: tie.p });
  }
  return out;
}

// --- what a step and a tie are called ----------------------------------------

const VERDICT: Record<string, string> = { review: "승인 요청", rejected: "승인 거절", deny: "차단" };
/** What the engine said of a step, as web/record's VerdictChip says it. */
export const said = (s: Step) =>
  s.verdict && s.verdict !== "allow" ? VERDICT[s.verdict] : s.catch ? `지시 ${s.catch.score}%` : null;
const firstLine = (t?: string) => t?.split("\n").map((l) => l.trim()).find(Boolean);
const valueOf = (s: Step) => (s.kind === "group" ? s.inside?.join(" · ") ?? s.type : firstLine(s.text?.body) ?? s.type);
const labelOf = (s: Step) => (s.kind === "answer" ? "답변" : s.kind === "result" ? `결과 · ${s.name}` : s.quote ? `"${s.name}"` : s.name);
/** The field's hover note: the step's facts, as label · value chips. */
const infoOf = (s: Step) =>
  [
    s.catch && `${s.catch.name} ${s.catch.score}%`,
    s.catch?.sub,
    ...s.detail.filter(([k]) => k !== "판정").map(([k, v]) => `${k} · ${v}`),
    typeof s.at === "string" ? s.at : s.at ? new Date(s.at).toLocaleTimeString("ko-KR", { hour12: false }) : null,
  ].filter(Boolean).join("\n");

const TIE: Record<string, string> = {
  ask: "요청에 따라", cause: "외부 지시", answer: "요청의 답변", result: "결과 보고 물음", message: "답변 보고 물음",
  request: "이전 요청 이어서", untraced: "추적 없음", loose: "출처 못 찾음",
};
const tieName = (b: Box) => `${TIE[b.via] ?? TIE.ask}${b.p != null ? ` ${Math.round(b.p * 100)}%` : ""}`;

// --- the components (also what `/all` lists) ---------------------------------

export const CATEGORY = "withhuman";
export const PROMPT = "WithhumanPrompt";
export const TOOLS = "WithhumanToolCalls";
export const ANSWER = "WithhumanAnswer";
/** Langflow's types: what the person asked travels as a Message, what a tool returned as JSON. */
const ASKED = "Message";
const RETURNED = "JSON";

function component(type: string, display_name: string, icon: string, description: string): Json {
  return {
    base_classes: [],
    beta: false,
    conditional_paths: [],
    custom_fields: {},
    description,
    display_name,
    documentation: "",
    edited: false,
    field_order: [],
    frozen: false,
    icon,
    legacy: false,
    lf_version: "1.13.4",
    metadata: { module: `withhuman.${type}` },
    output_types: [],
    outputs: [],
    pinned: false,
    template: { _type: "Component" },
    tool_mode: false,
  };
}

/** `/all`: only the three kinds of box a session is drawn with. */
export function componentTypes(): Json {
  return {
    [CATEGORY]: {
      [PROMPT]: component(PROMPT, "프롬프트", "MessageSquare", ""),
      [TOOLS]: component(TOOLS, "도구 호출", "Wrench", ""),
      [ANSWER]: component(ANSWER, "답변", "MessageSquareText", ""),
    },
  };
}

function output(name: string, display_name: string, type: string, grouped: boolean): Json {
  return {
    allows_loop: false,
    cache: true,
    display_name,
    group_outputs: grouped,
    method: "",
    name,
    selected: type,
    tool_mode: false,
    types: [type],
    value: "__UNDEFINED__",
  };
}

export const fieldKey = (id: string) => `step_${id.replace(/[^A-Za-z0-9]/g, "_")}`;

/** A step as a field: its name, what it ran, read-only (Langflow's copy field). */
function stepField(s: Step): Json {
  const verdict = said(s);
  return {
    advanced: false,
    copy_field: true,
    display_name: verdict ? `${labelOf(s)} · ${verdict}` : labelOf(s),
    dynamic: false,
    info: infoOf(s),
    input_types: [],
    list: false,
    load_from_db: false,
    multiline: true,
    name: fieldKey(s.id),
    placeholder: "",
    readonly: true,
    required: false,
    show: true,
    title_case: false,
    trace_as_input: false,
    type: "str",
    value: valueOf(s) ?? "",
  };
}

/** Where a tie lands: a handle row named for the tie, as Langflow names a connected input. */
function tieField(name: string, display_name: string, type: string): Json {
  return {
    advanced: false,
    display_name,
    dynamic: false,
    info: "",
    input_types: [type],
    list: false,
    name,
    placeholder: "",
    required: false,
    show: true,
    title_case: false,
    trace_as_input: false,
    type: "other",
    value: "",
  };
}

// --- the flow ---------------------------------------------------------------

/** A node's parts, in px, as Langflow's card measures them (React Flow measures the real ones; these
 * only space the rows): header, a tie row, a copy field, an output row, the padding of a node with
 * no output, the gap between rows, a column. */
const W = 320, HEADER = 61, TIE_ROW = 48, FIELD = 106, OUT = 32, PAD = 16, GAP = 40, COL = W + 120;
/** The description's height: text-xs in 288px, a Hangul glyph about 12px wide, others about 6.5. */
function descHeight(text: string) {
  const lines = text.split("\n\n").reduce((n, p) => {
    const w = [...p].reduce((a, c) => a + (/[\u1100-\u11ff\u3000-\u9fff\uac00-\ud7af]/.test(c) ? 12 : 6.5), 0);
    return n + Math.max(1, Math.ceil(w / 288));
  }, 0);
  return lines * 20 + (text.split("\n\n").length - 1) * 10 + 12;
}

/** The prompt as the node's description: its first two lines of prose, short (pasted documents,
 * logs and code after them stay in the record). */
function promptText(s: Step) {
  const body = s.text?.body ?? s.name;
  const lines = body
    .split("\n")
    .map((l) => l.trim().replace(/^(#{1,6}|>|[-*+]|\d+\.)\s+/, ""))
    .filter((l) => l && !l.startsWith("```"));
  let text = lines.slice(0, 2).join("\n\n");
  if (text.length > 200) text = `${text.slice(0, 199)}…`;
  else if (lines.length > 2) text += " …";
  return text;
}

export type Converted = { flow: Json; focusNode: string | null; nodeOf: Map<string, string> };

/** The session as a Langflow flow (`GET flows/<id>`). */
export function toFlow(data: RecordData, meta: Json): Converted {
  const step = new Map(data.steps.map((s) => [s.id, s]));
  const order = new Map(data.steps.map((s, i) => [s.id, i]));
  const all = (data.subgraphs?.length ? boxes(data) : turns(data))
    .map((b) => ({ ...b, steps: b.steps.filter((id) => step.has(id)) }))
    .filter((b) => b.steps.length)
    .sort((a, b) => (order.get(a.steps[0]) ?? 0) - (order.get(b.steps[0]) ?? 0));
  const byN = new Map(all.map((b) => [b.n, b]));
  const hangs = (b: Box) => b.parent != null && byN.has(b.parent) && b.parent !== b.n;
  const depth = new Map<number, number>();
  const depthOf = (n: number, seen: Set<number> = new Set()): number => {
    if (depth.has(n)) return depth.get(n)!;
    const b = byN.get(n)!;
    seen.add(n);
    const d = hangs(b) && !seen.has(b.parent!) ? depthOf(b.parent!, seen) + 1 : 0;
    depth.set(n, d);
    return d;
  };
  const kindOf = (b: Box) => {
    const first = step.get(b.steps[0])!;
    return first.kind === "prompt" ? PROMPT : b.steps.every((id) => step.get(id)!.kind === "answer") ? ANSWER : TOOLS;
  };
  const nodeId = (b: Box) => `${kindOf(b)}-S${b.n}`;

  /** The step of the parent box a box hangs from (web/record's `from`). */
  const from = (b: Box): string => {
    const parent = byN.get(b.parent!)!;
    const inParent = (id: string) => parent.steps.includes(id);
    let id: string | undefined;
    if (b.via === "cause") id = data.causes.find(([a, c]) => b.steps.includes(c) && inParent(a))?.[0];
    else if (b.via === "message" || b.via === "result" || b.via === "request")
      id = data.parents?.find((p) => b.steps.includes(p.prompt) && inParent(p.from))?.from;
    else id = parent.steps.find((s) => step.get(s)?.kind === "prompt");
    return id ?? parent.steps[0];
  };

  // Each box's outgoing ties, by the step they leave from.
  const ties = all.filter(hangs).map((b) => ({ child: b, parent: byN.get(b.parent!)!, src: from(b) }));
  const outsOf = new Map<number, string[]>();
  for (const t of ties) {
    const list = outsOf.get(t.parent.n) ?? [];
    if (!list.includes(t.src)) list.push(t.src);
    outsOf.set(t.parent.n, list);
  }

  const nodes: Json[] = [];
  const outputOf = new Map<string, { node: string; name: string; type: string; dataType: string }>();
  const tieOf = new Map<number, { node: string; field: string; type: string }>();
  const nodeOf = new Map<string, string>();
  let y = 0;
  for (const b of all) {
    const kind = kindOf(b);
    const id = nodeId(b);
    const first = step.get(b.steps[0])!;
    const prompt = kind === PROMPT ? first : null;
    const rows = b.steps.map((s) => step.get(s)!).filter((s) => s !== prompt);
    const calls = rows.filter((s) => s.kind === "call" || s.kind === "group").length;
    const display = kind === PROMPT ? "프롬프트" : kind === ANSWER ? "답변" : calls ? `도구 호출 ${calls}` : "도구";
    const icon = kind === PROMPT ? "MessageSquare" : kind === ANSWER ? "MessageSquareText" : "Wrench";
    b.steps.forEach((s) => nodeOf.set(s, id));

    const template: Json = { _type: "Component" };
    const field_order: string[] = [];
    // Where the tie from the parent box lands.
    const parentOut = hangs(b) ? from(b) : null;
    const tieType = parentOut && step.get(parentOut)?.kind === "result" ? RETURNED : ASKED;
    if (parentOut) {
      const name = "tie_in";
      template[name] = tieField(name, tieName(b), tieType);
      field_order.push(name);
      tieOf.set(b.n, { node: id, field: name, type: tieType });
    }
    for (const s of rows) {
      const f = stepField(s);
      template[f.name] = f;
      field_order.push(f.name);
    }

    // Outputs: what the box hands on. One per step a tie leaves from, else the box's own.
    const srcs = outsOf.get(b.n) ?? [];
    const outType = kind === TOOLS ? RETURNED : ASKED;
    const outName = kind === PROMPT ? "요청" : kind === TOOLS ? "결과" : "답변";
    const outputs: Json[] = [];
    if (srcs.length > 1) {
      for (const src of srcs) {
        const s = step.get(src)!;
        const name = `out_${fieldKey(src)}`;
        outputs.push(output(name, `${outName} · ${s === prompt ? "프롬프트" : labelOf(s)}`, outType, true));
        outputOf.set(src, { node: id, name, type: outType, dataType: kind });
      }
    } else if (kind !== ANSWER || srcs.length) {
      const name = kind === PROMPT ? "request" : kind === TOOLS ? "result" : "answer";
      outputs.push(output(name, outName, outType, false));
      for (const src of srcs) outputOf.set(src, { node: id, name, type: outType, dataType: kind });
    }

    const height =
      HEADER + (prompt ? descHeight(promptText(prompt)) : 0) + (parentOut ? TIE_ROW : 0) + rows.length * FIELD +
      (outputs.length ? outputs.length * OUT : PAD);
    const x = depthOf(b.n) * COL;
    nodes.push({
      id,
      type: "genericNode",
      position: { x, y },
      data: {
        id,
        type: kind,
        showNode: true,
        selected_output: outputs.length === 1 ? outputs[0].name : undefined,
        node: {
          ...component(kind, display, icon, prompt ? promptText(prompt) : ""),
          field_order,
          template,
          outputs,
          base_classes: [outType],
        },
      },
      selected: false,
      dragging: false,
    });
    y += height + GAP;
  }

  const edges: Json[] = ties.map(({ child, src }) => {
    const out = outputOf.get(src)!;
    const tie = tieOf.get(child.n)!;
    const sourceHandle = { dataType: out.dataType, id: out.node, name: out.name, output_types: [out.type] };
    const targetHandle = { fieldName: tie.field, id: tie.node, inputTypes: [tie.type], type: "other" };
    const sh = handleId(sourceHandle);
    const th = handleId(targetHandle);
    return {
      id: `reactflow__edge-${out.node}${sh}-${tie.node}${th}`,
      source: out.node,
      sourceHandle: sh,
      target: tie.node,
      targetHandle: th,
      data: { sourceHandle, targetHandle },
      animated: false,
      className: "",
      selected: false,
    };
  });

  // Open on the step asked for, else the last Call the engine refused.
  const refused = [...data.steps].reverse().find((s) => s.verdict === "deny" || s.verdict === "rejected");
  const focusStep = (data.focus && step.has(data.focus) ? data.focus : null) ?? refused?.id ?? null;

  return {
    flow: {
      ...meta,
      data: { nodes, edges, viewport: { x: 0, y: 0, zoom: 1 } },
      locked: true,
      is_component: false,
    },
    focusNode: focusStep ? nodeOf.get(focusStep) ?? null : null,
    nodeOf,
  };
}
