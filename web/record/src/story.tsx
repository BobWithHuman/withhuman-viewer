// 경로's side: the Calls behind the step picked, each with why it ran: the tool result that told it
// (the Call that brought it back, the words, how sure) or the request of its turn. The Calls a
// turn's answer was written from come in too; the prompts and answers between are lines.

import { ChevronRight } from "lucide-react";
import { type ReactNode, useLayoutEffect, useRef, useState } from "react";
import { doing } from "./doing";
import type { RecordData, Step } from "./data";
import type { Hop } from "./path";
import { Output, scripted } from "./code";
import { DetailBody, Judge, KIND_ICON, VerdictChip, clock, cx, judgeLabel, pct, tone } from "./parts";


/** What a step is called here, in full: a tool loaded before use by its server and name
 * (`select:mcp__internal_workspace__dispatch_workspace_task`), else its name. */
const toolsLoaded = (s: Step) => {
  const body = s.text?.body?.trim();
  const tool = s.detail.find(([k]) => k === "도구")?.[1] ?? s.type;
  if (tool !== "ToolSearch" || !body?.startsWith("select:")) return null;
  return body.slice(7).split(",").map((n) => n.trim()).filter(Boolean).map((n) => {
    const m = /^mcp__(.+?)__(.+)$/.exec(n);
    return m ? `${m[2]} (${m[1]})` : n;
  });
};
export const nameOf = (s: Step) => {
  const loaded = toolsLoaded(s);
  return loaded ? `${loaded.join(", ")} 불러오기` : plain(s.name);
};
/** What a Call did, in a few words: a shell command by its doings, else its name. */
const doingOf = (s: Step) => {
  const tool = s.detail.find(([k]) => k === "도구")?.[1] ?? s.type;
  if (/^(Bash|PowerShell|sh|zsh|shell)$/i.test(tool) && s.text?.body) return doing(s.text.body) ?? nameOf(s);
  return nameOf(s);
};
/** A prompt's intent in full, as the engine stated it. */
const intentOf = (s: Step) => s.detail.find(([k]) => k === "의도")?.[1] ?? (s.quote ? null : s.name);

/** Where in an answer the next step was written from: the sentence sharing the most words with
 * it, and what follows. */
const tokens = (t: string) => new Set((t.toLowerCase().match(/[\p{L}\p{N}_]{2,}/gu) ?? []).map((w) => w.replace(/(으로|에서|에게|와|과|을|를|은|는|이|가|의|에|로)$/, "")));
function fromWhere(answer: string, next: string | null): string {
  const flat = plain(answer);
  if (!next) return flat;
  const want = tokens(next);
  const parts = flat.split(/(?<=[.!?。]|\))\s+|\s(?=\d+\s)/);
  let best = -1, at = 0, pos = 0;
  for (const part of parts) {
    const score = [...tokens(part)].filter((w) => want.has(w)).length;
    if (score > best) { best = score; at = pos; }
    pos += part.length + 1;
  }
  return best > 0 && at > 0 ? "…" + flat.slice(at) : flat;
}

/** A text as one line of reading: markdown's marks and pictographs off, spaces folded. */
const plain = (text: string) => text
  .replace(/```[a-z]*|`/g, " ").replace(/^\s*#{1,6}\s*/gm, "").replace(/\*\*|__|~~/g, "").replace(/^\s*[-*>]\s+/gm, "")
  .replace(/[\p{Extended_Pictographic}\u{FE0F}\u{20E3}]/gu, "").replace(/\s+/g, " ").trim();

/** A text as it reads, its lines kept: markdown's marks and pictographs off, blank runs folded. */
const readable = (text: string) => text
  .replace(/```[a-z]*|`/g, "").replace(/^\s*#{1,6}\s*/gm, "").replace(/\*\*|__|~~/g, "").replace(/^\s*[-*>]\s+/gm, "· ")
  .replace(/[\p{Extended_Pictographic}\u{FE0F}\u{20E3}]/gu, "").replace(/[ \t]+/g, " ").replace(/\n\s*\n+/g, "\n").trim();

/** One labelled line of a card: what it is, and the text, cut at so many lines. */
function Field({ label, children, text, lines, mono }: { label: string; children?: ReactNode; text?: string; lines?: number; mono?: boolean }) {
  return (
    <>
      <dt className="pt-px text-[11.5px] text-mut">{label}</dt>
      <dd className={cx("m-0 min-w-0 text-[12.5px] leading-[1.55] text-fg/85 [overflow-wrap:anywhere]", mono && "font-mono text-[12px]")}
        style={lines ? { display: "-webkit-box", WebkitBoxOrient: "vertical", WebkitLineClamp: lines, overflow: "hidden" } : undefined}>{children ?? text}</dd>
    </>
  );
}

/** Why a Call ran: a tool result told it (the Call that brought it back, the words, how sure),
 * or the request of its turn. */
type Why = { by: "result"; from: Step; quote?: string; p?: number } | { by: "request"; prompt: Step } | { by: "none" };
type Item =
  | { kind: "tool"; s: Step; n: number; why: Why; end: boolean }
  | { kind: "quiet"; steps: Step[] }
  | { kind: "prompt"; s: Step; next?: Step }
  | { kind: "answer"; s: Step; next?: Step };

/** The Calls behind the step picked, in order: those on the way, those whose results told them,
 * and the Calls of each turn whose answer the way passes (what the answer was written from);
 * the prompts and answers of the way between, as the links they are. */
function items(data: RecordData, way: { ids: string[]; hops: Hop[] }): Item[] {
  const index = new Map(data.steps.map((s, i) => [s.id, i]));
  const byId = new Map(data.steps.map((s) => [s.id, s]));
  const isCall = (s?: Step) => !!s && (s.kind === "call" || s.kind === "group");
  const turnOf = (id: string) => { for (let i = index.get(id) ?? 0; i >= 0; i--) if (data.steps[i].kind === "prompt") return data.steps[i].id; return null; };
  // The Call a Call's cause came back from (a result is the Call's that brought it).
  const causeOf = (id: string): Step | undefined => {
    const from = data.causes.find(([, b]) => b === id)?.[0];
    const s = from ? byId.get(from) : undefined;
    if (s?.kind === "result") return byId.get(data.links.find(([, b]) => b === s.id)?.[0] ?? "");
    return s;
  };
  const on = new Set(way.ids);
  const keep = new Set<string>();
  const follow = (id: string) => { for (let c = causeOf(id), g = 0; c && !keep.has(c.id) && g < 50; c = causeOf(c.id), g++) keep.add(c.id); };
  for (const id of way.ids) if (isCall(byId.get(id))) { keep.add(id); follow(id); }
  // A turn whose answer is on the way: its Calls before the answer, which the answer saw.
  for (const id of way.ids) {
    if (byId.get(id)?.kind !== "answer") continue;
    const turn = turnOf(id), at = index.get(id) ?? 0;
    for (const s of data.steps) if (isCall(s) && turnOf(s.id) === turn && (index.get(s.id) ?? 0) < at) { keep.add(s.id); follow(s.id); }
  }
  // A card for each Call that matters: on the way, caught or refused, or whose result told one
  // that does; the rest of a turn, a folded line.
  const loud = (s: Step) => on.has(s.id) || (s.verdict && s.verdict !== "allow") || !!s.catch;
  const card = new Set<string>();
  for (const id of keep) {
    const s = byId.get(id)!;
    if (!loud(s)) continue;
    card.add(id);
    for (let c = causeOf(id), g = 0; c && g < 50; c = causeOf(c.id), g++) card.add(c.id);
  }
  const shown = data.steps.filter((s) => keep.has(s.id) || (on.has(s.id) && (s.kind === "prompt" || s.kind === "answer")));
  let n = 0;
  const out: Item[] = [];
  shown.forEach((s, i) => {
    const next = shown[i + 1];
    if (s.kind === "prompt") return void out.push({ kind: "prompt", s, next });
    if (s.kind === "answer") return void out.push({ kind: "answer", s, next });
    if (!card.has(s.id)) {
      const last = out[out.length - 1];
      if (last?.kind === "quiet") last.steps.push(s); else out.push({ kind: "quiet", steps: [s] });
      return;
    }
    const from = causeOf(s.id);
    const turn = turnOf(s.id);
    const why: Why = from ? { by: "result", from, quote: s.catch?.sub, p: s.catch ? s.catch.score / 100 : undefined }
      : turn && byId.get(turn) ? { by: "request", prompt: byId.get(turn)! } : { by: "none" };
    out.push({ kind: "tool", s, n: ++n, why, end: s.id === way.ids[0] });
  });
  return out;
}

/** Calls of a turn that matter to none of the rest: a folded line, opening to them. */
function QuietRow({ steps }: { steps: Step[] }) {
  const [open, setOpen] = useState(false);
  return (
    <div className="relative pb-3 pl-10">
      <span className="absolute top-1 left-[9px] z-10 size-2.5 rounded-full bg-chip ring-4 ring-panel" />
      <button onClick={() => setOpen((o) => !o)} className="flex items-center gap-1.5 text-[11.5px] text-mut hover:text-fg">
        <span className="flex gap-0.5">{steps.slice(0, 6).map((s) => <i key={s.id} className={cx("size-2 rounded-[2px] opacity-70", tone(s))} />)}</span>
        도구 {steps.length}
        <ChevronRight className={cx("size-3.5 transition-transform", open && "rotate-90")} />
      </button>
      {open && (
        <div className="mt-1 grid gap-1">
          {steps.map((s) => (
            <div key={s.id} className="flex min-w-0 items-center gap-1.5 text-[12px] text-fg/70">
              <span className={cx("grid size-4 shrink-0 place-items-center rounded [&_svg]:size-[10px] [&_svg]:stroke-[2.4]", tone(s))}>{KIND_ICON[s.kind]}</span>
              <span className="min-w-0 flex-1 truncate">{doingOf(s)}</span>
              <span className="shrink-0 text-[11px] text-dim tabular-nums">{clock(s.at)}</span>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

/** A Call of the way, a card: what it ran and why. Open, all of it. */
function ToolCard({ it, num, picked, onPick, cardRef }: {
  it: Extract<Item, { kind: "tool" }>; num: Map<string, number>; picked: boolean; onPick: () => void; cardRef: (el: HTMLDivElement | null) => void;
}) {
  const { s, n, why, end } = it;
  const [open, setOpen] = useState(false);
  const loaded = toolsLoaded(s);
  const body = s.text?.body;
  const flagged = (s.verdict && s.verdict !== "allow") || !!s.catch;
  const told = s.catch?.para?.trim() || why.by === "result" && why.quote;
  // A Call the engine stopped is told in red, as it is marked; one it let through, in yellow.
  const stopped = s.verdict === "deny";
  return (
    <div ref={cardRef} className="relative pb-3 pl-10">
      <span className={cx("absolute top-0 left-0 z-10 grid size-7 place-items-center rounded-full text-[12px] font-bold tabular-nums ring-4 ring-panel", flagged ? tone(s) : "bg-chip text-fg")}>{n}</span>
      <div className={cx("rounded-lg bg-bg px-3 py-2.5 transition-shadow", picked ? "ring-2 ring-pick" : end ? "ring-1 ring-white/25" : "ring-1 ring-white/8")}>
        <div onClick={onPick} className="flex w-full min-w-0 cursor-pointer items-start gap-1.5">
          <span className={cx("mt-px grid size-[18px] shrink-0 place-items-center rounded [&_svg]:size-[11px] [&_svg]:stroke-[2.4]", tone(s))}>{KIND_ICON[s.kind]}</span>
          <span className="min-w-0 flex-1 text-[13px] leading-snug font-semibold text-fg [overflow-wrap:break-word] [word-break:keep-all]">{doingOf(s)}</span>
          {flagged && <VerdictChip s={s} />}
          <span className="mt-px shrink-0 text-[11px] text-dim tabular-nums">{clock(s.at)}</span>
        </div>
        <dl className="m-0 mt-2 grid grid-cols-[30px_minmax(0,1fr)] gap-x-2.5 gap-y-1.5">
          {why.by === "result" && (
            <Field label="이유">
              <span className="flex flex-wrap items-center gap-1.5">
                <span className={cx("font-semibold [word-break:keep-all]", stopped ? "text-block" : "text-catch")}>{num.has(why.from.id) ? `${num.get(why.from.id)}번 ` : ""}{doingOf(why.from)} 결과</span>
                {why.p != null && <span className={cx("rounded px-1.5 text-[11px] tabular-nums", stopped ? "bg-block/12 text-block/90" : "bg-catch/12 text-catch/90")} title="엔진이 이 호출의 원인 후보(요청·결과·없음) 중 가장 유력하다고 본 것">원인 1순위 · {pct(why.p)}</span>}
              </span>
              {told && <span className="mt-1.5 block"><Output text={told} shell={scripted(told, why.from.type)} lines={6} /></span>}
            </Field>
          )}
          {why.by === "request" && <Field label="이유" text="요청" />}
          {s.judge && (end || (s.verdict && s.verdict !== "allow")) && <Field label="판정"><Judge j={s.judge} /></Field>}
        </dl>
        {/* What it really ran, on demand. */}
        {(body || s.detail.length > 0) && (
          <button onClick={() => setOpen((o) => !o)} className="mt-2 flex items-center gap-1 text-[11.5px] text-mut hover:text-fg">
            <ChevronRight className={cx("size-3.5 transition-transform", open && "rotate-90")} />실제 값
          </button>
        )}
        {open && (
          <div className="mt-2">
            <DetailBody s={{ ...s, catch: undefined, text: loaded ? { label: "불러온 도구", body: loaded.join("\n") } : s.text, detail: s.detail.filter(([k]) => k !== "판정" && k !== "출처" && !(loaded && k === "도구")) }} />
          </div>
        )}
      </div>
    </div>
  );
}

/** A prompt or an answer between the Calls: a line of what was asked or said. */
function LinkRow({ it, picked, onPick, refFn }: { it: Extract<Item, { kind: "prompt" | "answer" }>; picked: boolean; onPick: () => void; refFn: (el: HTMLDivElement | null) => void }) {
  const s = it.s;
  const next = it.next;
  const nextText = next ? [intentOf(next), next.text?.body].filter(Boolean).join(" ") : null;
  const body = s.text?.body ?? s.name;
  const text = s.kind === "answer" ? fromWhere(body, nextText) : plain(body);
  // Cut at three lines; pressed, all of it, its lines kept.
  const [full, setFull] = useState(false);
  const [cut, setCut] = useState(false);
  const box = useRef<HTMLDivElement>(null);
  useLayoutEffect(() => { const el = box.current; if (el && !full) setCut(el.scrollHeight > el.clientHeight + 1); }, [text, full]);
  const quoted = (x: string) => (s.kind === "prompt" ? `"${x}"` : x);
  return (
    <div ref={refFn} className="relative pb-3 pl-10">
      <span className={cx("absolute top-1.5 left-[5px] z-10 grid size-[18px] place-items-center rounded ring-4 ring-panel [&_svg]:size-[11px] [&_svg]:stroke-[2.4]", tone(s))}>{KIND_ICON[s.kind]}</span>
      <div className={cx("-mx-2 rounded-lg px-2 py-1 transition-colors", picked ? "bg-pick/15 ring-2 ring-pick" : "hover:bg-white/[.03]")}>
      <div onClick={onPick} className="flex cursor-pointer items-baseline gap-1.5 text-[11.5px]">
        <span className="font-semibold text-mut">{s.kind === "prompt" ? "요청" : "답변"}</span>
        <span className="text-dim tabular-nums">{clock(s.at)}</span>
      </div>
      <div ref={box} onClick={() => (cut || full) && setFull((f) => !f)}
        className={cx("mt-0.5 text-[12.5px] leading-[1.5] text-fg/80 [overflow-wrap:anywhere]", full ? "whitespace-pre-line" : "line-clamp-3", (cut || full) && "cursor-pointer")}>
        {full ? quoted(readable(body)) : quoted(text)}
      </div>
      {(cut || full) && (
        <button onClick={() => setFull((f) => !f)} className="mt-1 flex items-center gap-1 text-[11.5px] text-mut hover:text-fg">
          <ChevronRight className={cx("size-3.5 transition-transform", full && "rotate-90")} />{full ? "접기" : "전체"}
        </button>
      )}
      </div>
    </div>
  );
}

/** The step the way ends at, kept at the top of the side: what is being explained. */
export function StoryHead({ s, turns, steps, onClick }: { s: Step; turns: number; steps: number; onClick: () => void }) {
  return (
    <button onClick={onClick} className="mx-3 mb-3 flex items-center gap-2.5 rounded-lg bg-bg px-3 py-2.5 text-left ring-1 ring-white/10 hover:ring-white/20">
      <span className={cx("grid size-8 shrink-0 place-items-center rounded-lg [&_svg]:size-4 [&_svg]:stroke-[2.4]", tone(s))}>{KIND_ICON[s.kind]}</span>
      <span className="min-w-0 flex-1">
        <span className="line-clamp-2 text-[13.5px] leading-snug font-semibold text-fg [overflow-wrap:break-word] [word-break:keep-all]">{doingOf(s)}</span>
        <span className="flex items-center gap-1.5 text-[11.5px] text-mut tabular-nums">{s.type}<span className="text-dim">|</span>{clock(s.at)}<span className="text-dim">|</span>{steps}단계 · 턴 {turns}</span>
        {s.judge && judgeLabel(s.judge) && (
          <span className="mt-1 flex">
            <span className="rounded bg-block/15 px-1.5 py-px text-[11px] text-block">{judgeLabel(s.judge)}</span>
          </span>
        )}
      </span>
      <VerdictChip s={s} />
    </button>
  );
}

/** The Calls behind the step picked, numbered, each with why it ran; the prompts and answers
 * between them as lines. */
export function Story({ data, way, picked, onPick, refs }: {
  data: RecordData; way: { ids: string[]; hops: Hop[] }; picked: string | null; onPick: (id: string) => void;
  refs: { current: Record<string, HTMLDivElement | null> };
}) {
  const list = items(data, way);
  const num = new Map(list.flatMap((it) => (it.kind === "tool" ? [[it.s.id, it.n] as const] : [])));
  return (
    <div className="relative">
      <i className="absolute top-3 bottom-6 left-[13px] w-[2px] bg-white/10" />
      {list.map((it) => it.kind === "quiet" ? <QuietRow key={`q${it.steps[0].id}`} steps={it.steps} /> : it.kind === "tool"
        ? <ToolCard key={it.s.id} it={it} num={num} picked={picked === it.s.id} onPick={() => onPick(it.s.id)} cardRef={(el) => (refs.current[it.s.id] = el)} />
        : <LinkRow key={it.s.id} it={it} picked={picked === it.s.id} onPick={() => onPick(it.s.id)} refFn={(el) => (refs.current[it.s.id] = el)} />)}
    </div>
  );
}
