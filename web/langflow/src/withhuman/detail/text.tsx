// A step's text in Langflow's code look (ported from web/record's code.tsx and longtext.tsx): a
// shell command laid out as a script, JSON laid out, the part the engine flagged (`mark`) and
// masked values marked; a long text (a prompt with a document pasted in, a long tool result)
// folded with a map of its paragraphs by kind, opened whole in Langflow's dialog with numbered
// lines and a search.

import { useEffect, useMemo, useRef, useState } from "react";
import ForwardedIconComponent from "@/components/common/genericIconComponent";
import ShadTooltip from "@/components/common/shadTooltipComponent";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogTitle } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { cn } from "@/utils/utils";

const BOX = "m-0 rounded-md border border-border bg-muted px-3 py-2.5 font-mono text-xs leading-[1.6] whitespace-pre-wrap text-foreground [overflow-wrap:anywhere] [tab-size:4]";
const MARK = "rounded-[3px] bg-destructive/20 px-0.5 text-destructive";

// --- commands ------------------------------------------------------------------

type Line = { level: number; text: string; lead?: string; end?: string };

/** A command cut at the operators that chain it, outside quotes, `$( )` and `{ }`. */
function cut(cmd: string, ops: string[]): { text: string; end: string }[] {
  const out: { text: string; end: string }[] = [];
  let cur = "", quote = "", depth = 0;
  for (let i = 0; i < cmd.length; i++) {
    const c = cmd[i];
    if (c === "\\" && quote !== "'") { cur += c + (cmd[i + 1] ?? ""); i++; continue; }
    if (quote) { if (c === quote) quote = ""; cur += c; continue; }
    if (c === "'" || c === '"') { quote = c; cur += c; continue; }
    if (c === "(" || c === "{") depth++;
    if (c === ")" || c === "}") depth = Math.max(0, depth - 1);
    const hit = depth === 0 && ops.find((o) => cmd.startsWith(o, i) && !(o === "|" && (cmd[i + 1] === "|" || cmd[i - 1] === "|")));
    if (hit) { out.push({ text: cur.trim(), end: hit }); cur = ""; i += hit.length - 1; continue; }
    cur += c;
  }
  out.push({ text: cur.trim(), end: "" });
  return out;
}

/** A command as a script: a statement a line, loop and branch bodies indented, a long line cut at its pipes. */
function script(cmd: string): Line[] {
  const lines: Line[] = [];
  let level = 0;
  for (const { text, end } of cut(cmd, ["&&", "||", ";", "\n"])) {
    let t = text;
    const op = end === "&&" || end === "||" ? end : undefined;
    for (let guard = 0; t && guard < 8; guard++) {
      const kw = /^(do|then|else|elif|done|fi|esac)\b\s*/.exec(t);
      if (!kw) break;
      const k = kw[1];
      if (k === "do" || k === "then") {
        const prev = lines[lines.length - 1];
        if (prev) prev.text += `; ${k}`; else lines.push({ level, text: k });
        level++;
        t = t.slice(kw[0].length);
      } else if (k === "else" || k === "elif") {
        level = Math.max(0, level - 1);
        if (k === "else") { lines.push({ level, text: "else" }); level++; t = t.slice(kw[0].length); }
        else { lines.push({ level, text: t, end: op }); t = ""; }
      } else {
        level = Math.max(0, level - 1);
        lines.push({ level, text: t, end: op });
        t = "";
      }
    }
    if (t) lines.push({ level, text: t, end: op });
  }
  return lines.flatMap((l) => {
    if (l.text.length <= 72) return [l];
    const parts = cut(l.text, ["|"]);
    if (parts.length < 2) return [l];
    return parts.map((p, i) => ({ level: l.level + (i ? 1 : 0), text: p.text, lead: i ? "|" : undefined, end: i === parts.length - 1 ? l.end : undefined }));
  });
}

const isShell = (tool?: string) => !!tool && /^(Bash|bash|sh|zsh|shell|PowerShell|exec_command)$/i.test(tool);
/** Whether some words read as a script or a command's output. */
export const scripted = (text: string, tool?: string) => /^#!/.test(text.trim()) || isShell(tool);

function laidOut(text: string): string | null {
  const t = text.trim();
  if (!/^[[{]/.test(t)) return null;
  try { return JSON.stringify(JSON.parse(t), null, 2); } catch { return null; }
}

/** Words kept whole, a path breaking only after its slashes. */
function Words({ text, first }: { text: string; first?: boolean }) {
  let seen = false;
  return (
    <>
      {text.split(/(\s+)/).map((w, i) => {
        if (!w || /^\s+$/.test(w)) return w;
        const prog = first && !seen;
        seen = true;
        const cls = prog ? "font-semibold" : /^--?[\w-]/.test(w) ? "text-accent-blue-foreground" : /^['"]/.test(w) ? "text-accent-emerald-foreground" : undefined;
        return (
          <span key={i} className={cls}>
            {w.split(/(?<=[/\\])/).map((p, j) => <span key={j} className="inline-block max-w-full align-top [overflow-wrap:anywhere]">{p}</span>)}
          </span>
        );
      })}
    </>
  );
}

function MarkedLine({ text, mark, first }: { text: string; mark?: string; first?: boolean }) {
  if (!mark || !text.includes(mark)) return <Words text={text} first={first} />;
  const parts = text.split(mark);
  return (
    <>
      {parts.map((p, i) => (
        <span key={i}>
          <Words text={p} first={first && i === 0} />
          {i < parts.length - 1 && <mark className={MARK}><Words text={mark} /></mark>}
        </span>
      ))}
    </>
  );
}

/** A step's own text: a command after `$` laid out as a script, else as typed (JSON laid out). */
export function Code({ text, mark, tool }: { text: string; mark?: string; tool?: string }) {
  if (isShell(tool)) {
    const raw = text.includes("<<");
    const lines: Line[] = raw ? text.replace(/\s+$/, "").replace(/^\s*\n/, "").split("\n").map((l) => ({ level: 0, text: l })) : script(text);
    return (
      <pre className={BOX}>
        {lines.map((l, i) => (
          <div key={i} className="flex min-h-[1.6em]">
            <span className="w-[2ch] shrink-0 select-none text-muted-foreground">{i ? (raw ? ">" : "") : "$"}</span>
            <span className="min-w-0 flex-1" style={{ paddingLeft: `${l.level * 2 + 2}ch`, textIndent: "-2ch" }}>
              {l.lead && <span className="font-semibold text-muted-foreground">{l.lead} </span>}
              <MarkedLine text={l.text} mark={mark} first={!l.lead} />
              {l.end && <span className="font-semibold text-muted-foreground"> {l.end}</span>}
            </span>
          </div>
        ))}
      </pre>
    );
  }
  const body = laidOut(text) ?? text;
  return (
    <pre className={BOX}>
      {body.split("\n").map((l, i) => <div key={i} className="min-h-[1.6em]"><MarkedLine text={l} mark={mark} /></div>)}
    </pre>
  );
}

/** What a tool gave back, line by line; a command's output with each line's program set apart.
 * `lines` cuts it to its first lines, the rest behind a press. */
export function Output({ text, shell, lines, mark }: { text: string; shell: boolean; lines?: number; mark?: string }) {
  const [all, setAll] = useState(false);
  const rows = text.replace(/\s+$/, "").split("\n");
  const cutAt = lines != null && !all && rows.length > lines;
  const shown = cutAt ? rows.slice(0, lines) : rows;
  return (
    <div>
      <pre className={BOX}>
        {shown.map((l, i) => (
          <div key={i} className="min-h-[1.6em]">
            {shell && /^\s*#/.test(l) ? <span className="text-muted-foreground">{l}</span> : shell ? <Words text={l} first /> : <MarkedLine text={l} mark={mark} />}
          </div>
        ))}
      </pre>
      {lines != null && rows.length > lines && (
        <Button variant="ghost" size="xs" className="mt-1 h-6 px-1.5 text-xs text-muted-foreground" onClick={() => setAll((a) => !a)}>
          {all ? "접기" : `전체 ${rows.length}줄`}
        </Button>
      )}
    </div>
  );
}

// --- long texts ----------------------------------------------------------------

type ParaKind = "text" | "code" | "log" | "doc";
const PARA: Record<ParaKind, { label: string; color: string }> = {
  text: { label: "글", color: "hsl(var(--muted-foreground))" },
  doc: { label: "문서", color: "hsl(var(--accent-emerald-foreground))" },
  code: { label: "코드", color: "hsl(var(--accent-blue-foreground))" },
  log: { label: "로그", color: "hsl(var(--accent-purple-foreground))" },
};
type Para = { kind: ParaKind; text: string; line: number; flag: boolean };

/** A value the engine masked (`abcd…wxyz`), or one kept as stars. */
const MASKED = /\S{2,}…\S{2,}|\*{4,}/;
const LOG = /(^|\s)(\d{2}:\d{2}:\d{2}|\d{4}-\d{2}-\d{2}[T ]\d{2}:)|\b(ERROR|WARN(ING)?|INFO|DEBUG|FATAL|Traceback|Exception|panicked)\b|^\s+at\s+\S+\(.*:\d+\)|^\$ /m;
const DOC = /^(#{1,6}\s|>\s|[-*]\s|\d+\.\s|\|.*\|)/;
const CODE_LINE = /^(\s{2,}|\t)|[{};]\s*$|^\s*(fn|def|class|import|from|const|let|function|return|if|for|SELECT|INSERT|UPDATE)\b|=>|::/;

function kindOf(lines: string[]): ParaKind {
  const n = Math.max(1, lines.filter((l) => l.trim()).length);
  const share = (re: RegExp) => lines.filter((l) => l.trim() && re.test(l)).length / n;
  if (share(LOG) >= 0.5) return "log";
  if (share(CODE_LINE) >= 0.6) return "code";
  if (share(DOC) >= 0.4) return "doc";
  return "text";
}

/** The text in paragraphs: a fenced block whole, otherwise split at blank lines. */
function paragraphs(text: string, mark?: string): Para[] {
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
const escape = (s: string) => s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");

/** The paragraphs as one bar, each as wide as it is long, coloured by kind, dotted where flagged. */
function ParaMap({ paras, on, onPick, found }: { paras: Para[]; on?: number; onPick: (i: number) => void; found?: string }) {
  const q = found?.toLowerCase();
  return (
    <div className="flex h-3 w-full items-stretch gap-[2px]">
      {paras.map((p, i) => (
        <ShadTooltip key={i} content={`${PARA[p.kind].label} · ${count(p.text.length)}자 · ${p.text.replace(/\s+/g, " ").slice(0, 48)}`}>
          <button
            type="button"
            onClick={() => onPick(i)}
            style={{ flexGrow: Math.max(p.text.length, 20), background: PARA[p.kind].color }}
            className={cn("relative min-w-[4px] rounded-[2px] opacity-60 transition-opacity hover:opacity-100", on === i && "opacity-100 ring-2 ring-foreground/70")}
          >
            {p.flag && <i className="absolute -top-[3px] left-1/2 size-[7px] -translate-x-1/2 rounded-full bg-destructive ring-2 ring-background" />}
            {q && p.text.toLowerCase().includes(q) && <i className="absolute inset-x-0 -bottom-[5px] h-[3px] rounded-full bg-accent-amber-foreground" />}
          </button>
        </ShadTooltip>
      ))}
    </div>
  );
}

function Legend({ paras, total }: { paras: Para[]; total: number }) {
  const kinds = (Object.keys(PARA) as ParaKind[]).filter((k) => paras.some((p) => p.kind === k));
  const flaggedN = paras.filter((p) => p.flag).length;
  return (
    <div className="flex flex-wrap items-center gap-x-2.5 gap-y-1 text-[11px] tabular-nums text-muted-foreground">
      <span className="text-foreground">{count(total)}자</span>
      <span>문단 {paras.length}</span>
      {kinds.map((k) => (
        <span key={k} className="inline-flex items-center gap-1">
          <i className="size-1.5 rounded-full" style={{ background: PARA[k].color }} />
          {PARA[k].label} {paras.filter((p) => p.kind === k).length}
        </span>
      ))}
      {flaggedN > 0 && <span className="inline-flex items-center gap-1 text-destructive"><i className="size-1.5 rounded-full bg-destructive" />민감 {flaggedN}</span>}
    </div>
  );
}

/** A paragraph with the flagged part, masked values and what was searched for marked. */
function Marked({ text, mark, find }: { text: string; mark?: string; find?: string }) {
  const parts: { t: string; k?: "mark" | "find" | "masked" }[] = [];
  const re = new RegExp([mark && escape(mark), find && escape(find), MASKED.source].filter(Boolean).map((s) => `(${s})`).join("|"), find ? "gi" : "g");
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
      {parts.map((p, i) =>
        p.k === "mark" ? <mark key={i} className={MARK}>{p.t}</mark>
        : p.k === "find" ? <mark key={i} data-find className="rounded-[2px] bg-accent-amber text-accent-amber-foreground data-[on]:ring-2 data-[on]:ring-accent-amber-foreground">{p.t}</mark>
        : p.k === "masked" ? <span key={i} className={MARK}>{p.t}</span>
        : <span key={i}>{p.t}</span>,
      )}
    </>
  );
}

/** A label over a text: the step's own field name, and an action at the right. */
export function FieldLabel({ label, children }: { label: string; children?: React.ReactNode }) {
  return (
    <div className="mb-1.5 flex items-center gap-2">
      <span className="text-xs font-medium text-muted-foreground">{label}</span>
      <span className="ml-auto flex items-center gap-1">{children}</span>
    </div>
  );
}

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
      <FieldLabel label={label}>
        <ShadTooltip content="크게 보기">
          <Button variant="ghost" size="iconSm" className="h-6 w-6 text-muted-foreground" onClick={() => setBig(on ?? 0)} aria-label="크게 보기">
            <ForwardedIconComponent name="Maximize2" className="h-3.5 w-3.5" />
          </Button>
        </ShadTooltip>
      </FieldLabel>
      <div className="mb-2 space-y-1.5 rounded-md border border-border px-2.5 py-2">
        <ParaMap paras={paras} on={on} onPick={pick} />
        <Legend paras={paras} total={text.length} />
      </div>
      <div className="relative">
        <div className={cn("custom-scroll overflow-hidden rounded-md border border-border bg-muted font-mono text-xs leading-[1.55] text-foreground", open ? "max-h-[420px] overflow-y-auto" : "max-h-[150px]")}>
          {paras.map((p, i) => (
            <div
              key={i}
              ref={(el) => { refs.current[i] = el; }}
              onClick={() => setOn(i)}
              className={cn("relative whitespace-pre-wrap break-words border-l-2 px-3 py-1.5", on === i && "bg-background/60")}
              style={{ borderColor: on === i ? PARA[p.kind].color : "transparent" }}
            >
              <Marked text={p.text} mark={mark} />
            </div>
          ))}
        </div>
        {!open && <div className="pointer-events-none absolute inset-x-px bottom-px h-14 rounded-b-md bg-gradient-to-t from-muted to-transparent" />}
      </div>
      <Button variant="ghost" size="xs" className="mt-1 h-7 w-full gap-1 text-xs text-muted-foreground" onClick={() => setOpen(!open)}>
        <ForwardedIconComponent name={open ? "ChevronUp" : "ChevronDown"} className="h-3.5 w-3.5" />
        {open ? "접기" : "펼치기"}
      </Button>
      {big !== null && <Sheet label={label} text={text} mark={mark} paras={paras} from={big} onClose={() => setBig(null)} />}
    </div>
  );
}

/** The whole text in Langflow's dialog: the map, each line numbered, and a search that steps through what it finds. */
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
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="flex h-[min(860px,calc(100vh-48px))] w-[min(1080px,calc(100vw-48px))] max-w-none flex-col gap-0 p-0">
        <div className="flex items-center gap-3 border-b px-5 py-3 pr-12">
          <DialogTitle className="text-sm font-semibold">{label}</DialogTitle>
          <span className="text-xs tabular-nums text-muted-foreground">{count(text.length)}자 · {count(text.split("\n").length)}줄</span>
          <div className="relative ml-auto flex w-[300px] items-center">
            <ForwardedIconComponent name="Search" className="absolute left-2.5 h-3.5 w-3.5 text-muted-foreground" />
            <Input
              autoFocus
              value={find}
              onChange={(e) => { setFind(e.target.value); setHit(0); }}
              onKeyDown={(e) => { if (e.key === "Enter") step(e.shiftKey ? -1 : 1); }}
              placeholder="찾기"
              className="h-8 pl-8 pr-24 text-xs"
            />
            <span className="absolute right-1 flex items-center gap-0.5">
              {q && <span className="px-1 text-xs tabular-nums text-muted-foreground">{hits ? `${hit + 1}/${hits}` : "0"}</span>}
              <Button variant="ghost" size="iconSm" className="h-6 w-6" onClick={() => step(-1)} aria-label="이전"><ForwardedIconComponent name="ChevronUp" className="h-3.5 w-3.5" /></Button>
              <Button variant="ghost" size="iconSm" className="h-6 w-6" onClick={() => step(1)} aria-label="다음"><ForwardedIconComponent name="ChevronDown" className="h-3.5 w-3.5" /></Button>
            </span>
          </div>
        </div>
        <div className="space-y-2 border-b px-5 py-2.5">
          <ParaMap paras={paras} on={on} onPick={go} found={q} />
          <Legend paras={paras} total={text.length} />
        </div>
        <div ref={body} className="custom-scroll min-h-0 flex-1 overflow-auto py-2 font-mono text-xs leading-[1.6] text-foreground">
          {paras.map((p, i) => (
            <div
              key={i}
              ref={(el) => { refs.current[i] = el; }}
              onClick={() => setOn(i)}
              className={cn("grid grid-cols-[52px_1fr] border-l-2", on === i && "bg-muted")}
              style={{ borderColor: on === i ? PARA[p.kind].color : "transparent" }}
            >
              {p.text.split("\n").map((l, j) => (
                <div key={j} className="contents">
                  <span className="select-none pr-3 text-right tabular-nums text-muted-foreground">{p.line + j + 1}</span>
                  <span className="whitespace-pre-wrap break-words pr-5"><Marked text={l} mark={mark} find={q} /></span>
                </div>
              ))}
              <span className="col-span-2 h-2" />
            </div>
          ))}
        </div>
      </DialogContent>
    </Dialog>
  );
}
