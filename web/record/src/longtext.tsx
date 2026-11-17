// A long text (a prompt with a document pasted in, a long tool result) shown so the rest of the
// step stays in reach: a map of its paragraphs by kind, sized by length and dotted where a
// sensitive value or the part to show sits; the first lines, unfolded on demand; and the whole
// of it, with line numbers and a search, in a sheet over the page.

import * as Dialog from "@radix-ui/react-dialog";
import { ChevronDown, ChevronUp, Maximize2, Search, X } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { cx, Tip } from "./parts";

export type ParaKind = "text" | "code" | "log" | "doc";
export const PARA: Record<ParaKind, { label: string; color: string }> = {
  text: { label: "글", color: "#c9c9c9" },
  doc: { label: "문서", color: "#58c0a4" },
  code: { label: "코드", color: "#548cd0" },
  log: { label: "로그", color: "#c48ef0" },
};

export type Para = { kind: ParaKind; text: string; line: number; flag: boolean };

/** A value the engine masked (`abcd…wxyz`), or one kept as stars. */
const MASKED = /\S{2,}…\S{2,}|\*{4,}/;
const LOG = /(^|\s)(\d{2}:\d{2}:\d{2}|\d{4}-\d{2}-\d{2}[T ]\d{2}:)|\b(ERROR|WARN(ING)?|INFO|DEBUG|FATAL|Traceback|Exception|panicked)\b|^\s+at\s+\S+\(.*:\d+\)|^\$ /m;
const DOC = /^(#{1,6}\s|>\s|[-*]\s|\d+\.\s|\|.*\|)/;
const CODE_LINE = /^(\s{2,}|\t)|[{};]\s*$|^\s*(fn|def|class|import|from|const|let|function|return|if|for|SELECT|INSERT|UPDATE)\b|=>|::/;

/** What kind a block of lines is, by the share of its lines that look so. */
function kindOf(lines: string[]): ParaKind {
  const n = Math.max(1, lines.filter((l) => l.trim()).length);
  const share = (re: RegExp) => lines.filter((l) => l.trim() && re.test(l)).length / n;
  if (share(LOG) >= 0.5) return "log";
  if (share(CODE_LINE) >= 0.6) return "code";
  if (share(DOC) >= 0.4) return "doc";
  return "text";
}

/** The text in paragraphs: a fenced block whole, otherwise split at blank lines. */
export function paragraphs(text: string, mark?: string): Para[] {
  const lines = text.split("\n");
  const out: Para[] = [];
  let cur: string[] = [], start = 0, fence = false;
  const push = (kind?: ParaKind) => {
    if (cur.some((l) => l.trim())) {
      const body = cur.join("\n");
      out.push({ kind: kind ?? kindOf(cur), text: body, line: start, flag: MASKED.test(body) || (!!mark && body.includes(mark)) });
    }
    cur = [];
  };
  lines.forEach((l, i) => {
    if (/^\s*(```|~~~)/.test(l)) {
      if (fence) { cur.push(l); push("code"); fence = false; return; }
      push(); fence = true; start = i; cur.push(l); return;
    }
    if (!fence && !l.trim()) { push(); start = i + 1; return; }
    if (!cur.length) start = i;
    cur.push(l);
  });
  push(fence ? "code" : undefined);
  return out;
}

/** Whether a text is long enough to fold. */
export const isLong = (text: string) => text.length > 600 || text.split("\n").length > 12;

const count = (n: number) => n.toLocaleString("ko-KR");

/** The paragraphs as one bar: each part as wide as it is long, coloured by its kind, dotted
 * where it holds a sensitive value or the part to show. */
function ParaMap({ paras, on, onPick, found }: { paras: Para[]; on?: number; onPick: (i: number) => void; found?: string }) {
  const q = found?.toLowerCase();
  return (
    <div className="flex h-3 w-full items-stretch gap-[2px]">
      {paras.map((p, i) => (
        <Tip key={i} label={`${PARA[p.kind].label} · ${count(p.text.length)}자 · ${p.text.replace(/\s+/g, " ").slice(0, 48)}`}>
          <button onClick={() => onPick(i)} style={{ flexGrow: Math.max(p.text.length, 20), background: PARA[p.kind].color }}
            className={cx("relative min-w-[4px] rounded-[2px] opacity-60 transition-opacity hover:opacity-100", on === i && "opacity-100 ring-2 ring-white/70")}>
            {p.flag && <i className="absolute -top-[3px] left-1/2 size-[7px] -translate-x-1/2 rounded-full bg-block ring-2 ring-panel" />}
            {q && p.text.toLowerCase().includes(q) && <i className="absolute inset-x-0 -bottom-[5px] h-[3px] rounded-full bg-catch" />}
          </button>
        </Tip>
      ))}
    </div>
  );
}

/** How much of each kind, as dots and counts. */
function Legend({ paras, total }: { paras: Para[]; total: number }) {
  const kinds = (Object.keys(PARA) as ParaKind[]).filter((k) => paras.some((p) => p.kind === k));
  const flagged = paras.filter((p) => p.flag).length;
  return (
    <div className="flex flex-wrap items-center gap-x-2.5 gap-y-1 text-[11px] text-mut tabular-nums">
      <span className="text-fg/85">{count(total)}자</span>
      <span>문단 {paras.length}</span>
      {kinds.map((k) => (
        <span key={k} className="inline-flex items-center gap-1"><i className="size-1.5 rounded-full" style={{ background: PARA[k].color }} />{PARA[k].label} {paras.filter((p) => p.kind === k).length}</span>
      ))}
      {flagged > 0 && <span className="inline-flex items-center gap-1 text-[#ff9d92]"><i className="size-1.5 rounded-full bg-block" />민감 {flagged}</span>}
    </div>
  );
}

/** A paragraph's text with the part to show and what was searched for marked. */
function Marked({ text, mark, find }: { text: string; mark?: string; find?: string }) {
  const parts: { t: string; k?: "mark" | "find" | "masked" }[] = [];
  const re = new RegExp(
    [mark && escape(mark), find && escape(find), MASKED.source].filter(Boolean).map((s) => `(${s})`).join("|"),
    find ? "gi" : "g",
  );
  let at = 0;
  for (const m of text.matchAll(re)) {
    if (!m[0]) continue;
    parts.push({ t: text.slice(at, m.index) });
    const k = mark && m[0] === mark ? "mark" : find && m[0].toLowerCase() === find.toLowerCase() ? "find" : "masked";
    parts.push({ t: m[0], k });
    at = m.index! + m[0].length;
  }
  parts.push({ t: text.slice(at) });
  return (
    <>
      {parts.map((p, i) => p.k === "mark" ? <mark key={i} className="rounded-[3px] bg-block/25 px-0.5 text-[#ffb4ab]">{p.t}</mark>
        : p.k === "find" ? <mark key={i} data-find className="rounded-[2px] bg-catch/80 text-[#1d1d1d]">{p.t}</mark>
        : p.k === "masked" ? <span key={i} className="rounded-[3px] bg-block/20 px-0.5 text-[#ff9d92]">{p.t}</span>
        : <span key={i}>{p.t}</span>)}
    </>
  );
}
const escape = (s: string) => s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");

/** A long text folded to its first lines, with its map; unfolded in place or opened whole. */
export function LongText({ label, text, mark }: { label: string; text: string; mark?: string }) {
  const paras = useMemo(() => paragraphs(text, mark), [text, mark]);
  const [open, setOpen] = useState(false);
  const [big, setBig] = useState<number | null>(null);
  const [on, setOn] = useState<number>();
  const refs = useRef<(HTMLDivElement | null)[]>([]);
  const pick = (i: number) => {
    setOn(i);
    setOpen(true);
    requestAnimationFrame(() => refs.current[i]?.scrollIntoView({ block: "nearest", behavior: "smooth" }));
  };
  return (
    <div>
      <div className="mb-1.5 flex items-center gap-2">
        <span className="text-[11.5px] text-mut">{label}</span>
        <span className="ml-auto" />
        <Tip label="크게 보기"><button onClick={() => setBig(on ?? 0)} className="grid size-6 place-items-center rounded-md text-mut hover:bg-white/8 hover:text-fg"><Maximize2 className="size-3.5" /></button></Tip>
      </div>
      <div className="mb-2 space-y-1.5 rounded-md bg-sunk px-2.5 py-2">
        <ParaMap paras={paras} on={on} onPick={pick} />
        <Legend paras={paras} total={text.length} />
      </div>
      <div className="relative">
        <div className={cx("scroll overflow-hidden rounded-md bg-box font-mono text-[11.5px] leading-[1.55] text-[#e8e8e8]", open ? "max-h-[420px] overflow-y-auto" : "max-h-[150px]")}>
          {paras.map((p, i) => (
            <div key={i} ref={(el) => { refs.current[i] = el; }} onClick={() => setOn(i)}
              className={cx("relative border-l-2 px-3 py-1.5 whitespace-pre-wrap break-words", on === i ? "bg-white/[.04]" : "")}
              style={{ borderColor: on === i ? PARA[p.kind].color : "transparent" }}>
              <Marked text={p.text} mark={mark} />
            </div>
          ))}
        </div>
        {!open && <div className="pointer-events-none absolute inset-x-0 bottom-0 h-14 rounded-b-md bg-gradient-to-t from-box to-transparent" />}
      </div>
      <button onClick={() => setOpen(!open)} className="mt-1 flex w-full items-center justify-center gap-1 rounded-md py-1 text-[11.5px] text-mut hover:bg-white/6 hover:text-fg">
        {open ? <><ChevronUp className="size-3.5" />접기</> : <><ChevronDown className="size-3.5" />펼치기</>}
      </button>
      {big !== null && <Sheet label={label} text={text} mark={mark} paras={paras} from={big} onClose={() => setBig(null)} />}
    </div>
  );
}

/** The whole text over the page: the map beside it, each line numbered, and a search that marks
 * and counts what it finds and steps through it. */
function Sheet({ label, text, mark, paras, from, onClose }: { label: string; text: string; mark?: string; paras: Para[]; from: number; onClose: () => void }) {
  const [find, setFind] = useState("");
  const [hit, setHit] = useState(0);
  const [on, setOn] = useState(from);
  const body = useRef<HTMLDivElement>(null);
  const refs = useRef<(HTMLDivElement | null)[]>([]);
  const q = find.trim();
  const hits = useMemo(() => (q ? text.toLowerCase().split(q.toLowerCase()).length - 1 : 0), [text, q]);
  useEffect(() => { requestAnimationFrame(() => refs.current[from]?.scrollIntoView({ block: "start" })); }, [from]);
  useEffect(() => {
    const all = body.current?.querySelectorAll("[data-find]");
    all?.forEach((el, i) => el.toggleAttribute("data-on", i === hit));
    all?.[hit]?.scrollIntoView({ block: "center" });
  }, [hit, q]);
  const step = (by: number) => hits && setHit((h) => (h + by + hits) % hits);
  const go = (i: number) => { setOn(i); refs.current[i]?.scrollIntoView({ block: "start", behavior: "smooth" }); };
  return (
    <Dialog.Root open onOpenChange={(o) => !o && onClose()}>
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 z-50 bg-black/55" />
        <Dialog.Content aria-describedby={undefined}
          className="fixed top-1/2 left-1/2 z-50 flex h-[min(860px,calc(100vh-48px))] w-[min(1080px,calc(100vw-48px))] -translate-x-1/2 -translate-y-1/2 flex-col rounded-xl bg-panel shadow-[0_24px_80px_rgba(0,0,0,.6)] outline-none">
          <div className="flex items-center gap-3 border-b border-white/8 px-5 py-3">
            <Dialog.Title className="text-[14px] font-semibold">{label}</Dialog.Title>
            <span className="text-[12px] text-mut tabular-nums">{count(text.length)}자 · {count(text.split("\n").length)}줄</span>
            <div className="ml-auto flex h-8 w-[300px] items-center gap-1.5 rounded-md bg-sunk px-2 ring-1 ring-white/8 focus-within:ring-act">
              <Search className="size-3.5 shrink-0 text-mut" />
              <input autoFocus value={find} onChange={(e) => { setFind(e.target.value); setHit(0); }} placeholder="찾기"
                onKeyDown={(e) => { if (e.key === "Enter") step(e.shiftKey ? -1 : 1); }}
                className="min-w-0 flex-1 bg-transparent text-[12.5px] outline-none placeholder:text-dim" />
              {q && <span className="shrink-0 text-[11.5px] text-mut tabular-nums">{hits ? `${hit + 1}/${hits}` : "0"}</span>}
              <button onClick={() => step(-1)} className="grid size-5 place-items-center rounded text-mut hover:text-fg"><ChevronUp className="size-3.5" /></button>
              <button onClick={() => step(1)} className="grid size-5 place-items-center rounded text-mut hover:text-fg"><ChevronDown className="size-3.5" /></button>
            </div>
            <Dialog.Close className="grid size-7 place-items-center rounded-md text-mut hover:bg-white/8 hover:text-fg"><X className="size-4" /></Dialog.Close>
          </div>
          <div className="space-y-2 border-b border-white/8 px-5 py-2.5">
            <ParaMap paras={paras} on={on} onPick={go} found={q} />
            <Legend paras={paras} total={text.length} />
          </div>
          <div ref={body} className="scroll min-h-0 flex-1 overflow-auto py-2 font-mono text-[12px] leading-[1.6] text-[#e8e8e8]">
            {paras.map((p, i) => (
              <div key={i} ref={(el) => { refs.current[i] = el; }} onClick={() => setOn(i)}
                className={cx("grid grid-cols-[52px_1fr] border-l-2", on === i && "bg-white/[.03]")}
                style={{ borderColor: on === i ? PARA[p.kind].color : "transparent" }}>
                {p.text.split("\n").map((l, j) => (
                  <div key={j} className="contents">
                    <span className="pr-3 text-right text-dim select-none tabular-nums">{p.line + j + 1}</span>
                    <span className="pr-5 whitespace-pre-wrap break-words"><Marked text={l} mark={mark} find={q} /></span>
                  </div>
                ))}
                <span className="col-span-2 h-2" />
              </div>
            ))}
          </div>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
