// 경로 drawn as a flow, at the size it reads: each step of the way a card (what kind, when, its
// name, what it said or ran, the part to show as the problem, the verdict), the earliest first,
// running on to the next line where the width ends; between two cards an arrow named by how the
// one led to the next; the cards of one turn in one box.

import { Fragment, type ReactNode } from "react";
import type { RecordData, Step } from "./data";
import { type Hop, TIE } from "./path";
import { KIND_ICON, LogoDisc, VerdictChip, clock, cx, tone } from "./parts";

const KIND: Partial<Record<Step["kind"], string>> = { prompt: "프롬프트", call: "도구 호출", result: "도구 결과", answer: "답변", group: "묶음" };

/** The part of a text worth a card: around the part to show, else its start. */
function excerpt(body: string, mark?: string): { before: string; mark?: string; after: string } {
  const flat = body.replace(/\s+/g, " ").trim();
  const m = mark?.replace(/\s+/g, " ").trim();
  const at = m ? flat.indexOf(m) : -1;
  if (m && at >= 0) {
    const from = Math.max(0, at - 40);
    return { before: (from > 0 ? "…" : "") + flat.slice(from, at), mark: m, after: flat.slice(at + m.length, at + m.length + 60) };
  }
  return { before: flat.slice(0, 200), after: "" };
}

function WayCard({ s, picked, onPick }: { s: Step; picked: boolean; onPick: () => void }) {
  const ex = s.text && excerpt(s.text.body, s.text.mark);
  const mono = s.kind === "call" || s.kind === "result";
  return (
    <button id={`way-${s.id}`} onClick={onPick}
      className={cx("flex w-[232px] shrink-0 cursor-pointer flex-col self-stretch rounded-xl bg-panel text-left shadow-[0_10px_28px_rgba(0,0,0,.35)] transition-shadow",
        picked ? "ring-2 ring-pick" : "ring-1 ring-white/10 hover:ring-white/25")}>
      <div className="flex items-center gap-2 px-3 pt-3">
        <span className={cx("grid size-[26px] shrink-0 place-items-center rounded-md [&_svg]:size-[15px] [&_svg]:stroke-[2.3]", tone(s))}>{KIND_ICON[s.kind]}</span>
        <span className="text-[12px] text-mut">{KIND[s.kind] ?? s.type}</span>
        {s.svc && <LogoDisc s={s.svc} size={16} />}
        <span className="ml-auto text-[11.5px] text-dim tabular-nums">{clock(s.at)}</span>
      </div>
      <div className="px-3 pt-2 text-[14px] leading-snug font-semibold text-fg [overflow-wrap:anywhere]">{s.quote ? `"${s.name}"` : s.name}</div>
      {ex && (
        <div className="mx-3 mt-2 rounded-md bg-box px-2.5 py-1.5">
          <div className={cx("line-clamp-4 text-[12px] leading-[1.5] text-fg/80 [overflow-wrap:anywhere]", mono && "font-mono text-[11.5px]")}>
            {ex.before}{ex.mark && <mark className="rounded-[3px] bg-block/25 px-0.5 text-[#ffb4ab]">{ex.mark}</mark>}{ex.after}
          </div>
        </div>
      )}
      <div className="mt-auto flex min-h-[38px] items-center gap-1.5 px-3 pt-2 pb-3">
        <VerdictChip s={s} />
        {s.fresh && <span className="rounded-md bg-act/15 px-1.5 py-0.5 text-[11px] font-semibold text-act">새 요청</span>}
      </div>
    </button>
  );
}

/** How one step led to the next: its name over an arrow, the steps it passes under. */
function Link({ h, p, skipped }: { h: Hop; p?: number; skipped: number }) {
  const color = TIE[h.tie].color;
  return (
    <div className="flex w-[118px] shrink-0 flex-col items-center justify-center gap-1.5 self-center px-1.5">
      <span className="rounded-md px-2 py-0.5 text-center text-[12px] leading-tight font-semibold ring-1"
        style={{ color, background: `color-mix(in oklab, ${color} 14%, #282828)`, ["--tw-ring-color" as string]: `color-mix(in oklab, ${color} 40%, transparent)` }}>
        {h.label}{p != null && <span className="ml-1 font-normal tabular-nums opacity-80">{Math.round(p * 100)}%</span>}
      </span>
      <svg width="100%" height="12" viewBox="0 0 100 12" preserveAspectRatio="none" className="overflow-visible" aria-hidden>
        <line x1="0" y1="6" x2="92" y2="6" stroke={color} strokeWidth="3" vectorEffect="non-scaling-stroke" />
        <path d="M90,0 L100,6 L90,12 Z" fill={color} />
      </svg>
      <span className={cx("rounded-full px-2 py-px text-[11px] whitespace-nowrap text-mut tabular-nums", skipped ? "bg-chip" : "invisible")}>건너뜀 {skipped}</span>
    </div>
  );
}

/** Each step's turn: the prompts before it, counted. */
function turns(data: RecordData) {
  const out = new Map<string, number>();
  let n = 0;
  for (const s of data.steps) {
    if (s.kind === "prompt") n++;
    out.set(s.id, s.turn ?? n);
  }
  return out;
}

/** 경로: the way alone, the earliest first. */
export function WayFlow({ data, way, picked, onPick }: { data: RecordData; way: { ids: string[]; hops: Hop[] }; picked: string | null; onPick: (id: string) => void }) {
  const order = [...way.ids].reverse();
  const index = new Map(data.steps.map((s, i) => [s.id, i]));
  const byId = new Map(data.steps.map((s) => [s.id, s]));
  const turnOf = turns(data);
  const link = (to: string) => {
    const h = way.hops.find((x) => x.to === to);
    if (!h) return null;
    const skipped = Math.max(0, Math.abs((index.get(h.to) ?? 0) - (index.get(h.from) ?? 0)) - 1);
    // An instruction a result held: how sure the engine is, from the step it made.
    const score = h.tie === "cause" ? byId.get(h.to)?.catch?.score : undefined;
    return <Link h={h} p={h.p ?? (score != null ? score / 100 : undefined)} skipped={skipped} />;
  };
  // The way in runs of one turn.
  const runs: string[][] = [];
  for (const id of order) {
    const last = runs[runs.length - 1];
    if (last && turnOf.get(last[0]) === turnOf.get(id)) last.push(id);
    else runs.push([id]);
  }
  return (
    <div className="scroll absolute inset-0 overflow-auto px-6 pt-16 pb-14">
      <div className="flex flex-wrap items-stretch gap-y-6">
        {runs.map((run, r) => (
          <Fragment key={run[0]}>
            {r > 0 && link(run[0])}
            <Turn label={turnOf.get(run[0]) ? `턴 ${turnOf.get(run[0])}` : null}>
              {run.map((id, i) => (
                <Fragment key={id}>
                  {i > 0 && link(id)}
                  <WayCard s={byId.get(id)!} picked={picked === id} onPick={() => onPick(id)} />
                </Fragment>
              ))}
            </Turn>
          </Fragment>
        ))}
      </div>
    </div>
  );
}

function Turn({ label, children }: { label: string | null; children: ReactNode }) {
  return (
    <section className="relative flex max-w-full flex-wrap items-stretch gap-y-4 rounded-2xl border border-white/[.06] bg-white/[.025] p-3 pt-8">
      {label && <span className="absolute top-2.5 left-3.5 text-[12px] font-semibold text-mut">{label}</span>}
      {children}
    </section>
  );
}
