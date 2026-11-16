// What the decisions suggest changing in the policy: how often a session the team looked at was
// nothing wrong, the kinds of held Calls people nearly always let run (candidates to loosen),
// the tools stopped most, and the sources the team distrusts. A row pressed narrows the page.

import { Ban, X } from "lucide-react";
import type { ReactNode } from "react";
import { Empty, pct } from "@/kit";
import { cn } from "@/lib/utils";
import { type Blocked, type Call, DIM, type Mark, toolName, tuning } from "./data";
import type { Ask } from "./list";

function Part({ title, aside, children }: { title: string; aside?: ReactNode; children: ReactNode }) {
  return (
    <section className="grid gap-1.5">
      <h3 className="flex items-center gap-2 px-2 text-[12px] font-medium text-muted-foreground">{title}<span className="tnum ml-auto font-normal">{aside}</span></h3>
      {children}
    </section>
  );
}
/** A row with a bar under it, as long as its share. */
function Bar({ label, value, share, color, mono, onClick }: { label: string; value: ReactNode; share: number; color: string; mono?: boolean; onClick?: () => void }) {
  return (
    <button onClick={onClick} disabled={!onClick} className="grid gap-1.5 rounded-lg px-2 py-1.5 text-left transition-colors enabled:hover:bg-accent/70">
      <span className="flex items-baseline gap-2"><span className={cn("min-w-0 flex-1 truncate text-[13px]", mono && "font-mono text-[12px]")}>{label}</span><span className="tnum shrink-0 text-[12.5px] font-medium">{value}</span></span>
      <span className="h-1.5 overflow-hidden rounded-full bg-white/[0.05]"><i className="block h-full rounded-full" style={{ width: `${Math.max(3, share * 100)}%`, background: color }} /></span>
    </button>
  );
}

export function Tuning({ calls, marks, blocked, shared, ask, onPick }: {
  calls: Call[]; marks: Record<string, Mark>; blocked: Blocked[]; ask: Ask; onPick: (dim: "kind" | "tool", key: string) => void;
  /** Whether the engine keeps the sources: it then holds what they cause; else they are only noted here. */
  shared?: boolean;
}) {
  const t = tuning(calls, marks);
  const most = t.tools[0]?.[1] ?? 1;
  return (
    <div className="flex flex-col gap-4">
      <Part title="오탐 비율" aside={<>확인 {t.marked}</>}>
        <div className="flex items-center gap-3 px-2">
          <span className="tnum text-[22px] leading-none font-semibold">{t.marked ? `${pct(t.benign, t.marked)}%` : "–"}</span>
          <span className="flex h-2 min-w-0 flex-1 gap-px overflow-hidden rounded-full bg-white/[0.05]">
            {t.attacks > 0 && <i className="h-full bg-deny" style={{ flex: t.attacks }} />}
            {t.benign > 0 && <i className="h-full bg-allow" style={{ flex: t.benign }} />}
          </span>
          <span className="tnum shrink-0 text-[12px] text-muted-foreground">공격 {t.attacks} · 정상 {t.benign}</span>
        </div>
      </Part>
      <Part title="완화 후보" aside={t.loosen.length || undefined}>
        {!t.loosen.length && <Empty />}
        {t.loosen.map((r) => <Bar key={r.kind} label={DIM.kind.name(r.kind)} value={<>승인 {pct(r.ran, r.n)}% <span className="font-normal text-muted-foreground">· {r.n}건</span></>} share={r.share} color="var(--allow)" onClick={() => onPick("kind", r.kind)} />)}
      </Part>
      <Part title="자주 막히는 도구">
        {!t.tools.length && <Empty />}
        {t.tools.map(([tool, n]) => <Bar key={tool} label={toolName(tool)} value={n.toLocaleString()} share={n / most} color="var(--deny)" mono onClick={() => onPick("tool", tool)} />)}
      </Part>
      <Part title="차단한 출처" aside={<>{shared ? "엔진 적용" : "기록만"}{blocked.length ? ` · ${blocked.length}` : ""}</>}>
        {!blocked.length && <Empty />}
        {blocked.map((b) => (
          <div key={b.source} className="flex min-w-0 items-center gap-2 rounded-lg px-2 py-1.5 text-[12.5px]">
            <Ban className="size-3.5 shrink-0 text-deny" strokeWidth={2} />
            <button onClick={() => ask({ do: "open", session: b.session, seq: b.seq })} className="min-w-0 flex-1 truncate text-left font-mono text-[12px] hover:underline" title={b.source}>{b.source}</button>
            <span className="shrink-0 text-[11.5px] text-muted-foreground">{b.by}</span>
            <button aria-label="차단 풀기" onClick={() => ask({ do: "source", source: b.source, on: false, session: b.session, seq: b.seq })} className="grid size-5 shrink-0 place-items-center rounded-full text-muted-foreground hover:bg-white/10 hover:text-foreground"><X className="size-3" /></button>
          </div>
        ))}
      </Part>
    </div>
  );
}
