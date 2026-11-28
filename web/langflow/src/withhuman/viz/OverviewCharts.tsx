// The sessions overview's charts, above the list: calls per day stacked by what the engine
// decided (allowed / 확인 / 차단), the calls by folder, and the share of calls a tool result told
// (외부 지시). Drawn from `shell.sessions` (each session's counts, on the day of its last
// activity). Hover shows the numbers (Langflow's tooltip); a day or a folder pressed filters the
// list, pressed again lets it go; the ring toggles the 외부 지시 tab.

import { type ReactNode, useMemo } from "react";
import ShadTooltip from "@/components/common/shadTooltipComponent";
import { cn } from "@/utils/utils";
import type { ShellSession } from "../bridge";
import { C } from "./colors";

/** A local day as `YYYY-MM-DD`. */
export const dayKey = (ms: number) => {
  const d = new Date(ms);
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
};
/** `M/D` of a day key. */
export const dayLabel = (k: string) => `${Number(k.slice(5, 7))}/${Number(k.slice(8))}`;
/** The folder's last part, as the list shows it. */
export const folderOf = (s: ShellSession) => s.folder?.split("/").filter(Boolean).pop() ?? "—";

const MAX_DAYS = 31;
const TOP_FOLDERS = 5;

type Split = { allow: number; review: number; deny: number; calls: number; sessions: number };
const zero = (): Split => ({ allow: 0, review: 0, deny: 0, calls: 0, sessions: 0 });
function add(t: Split, s: ShellSession) {
  const deny = Math.min(s.denied, s.calls);
  const review = Math.min(s.review, s.calls - deny);
  t.deny += deny;
  t.review += review;
  t.allow += s.calls - deny - review;
  t.calls += s.calls;
  t.sessions += 1;
}

const SEGMENTS = [
  { key: "deny", label: "차단", color: C.deny },
  { key: "review", label: "확인", color: C.review },
  { key: "allow", label: "허용", color: C.allow },
] as const;

function Tip({ title, t }: { title: string; t: Split }) {
  return (
    <span className="flex min-w-[7rem] flex-col gap-1 tabular-nums">
      <span className="flex justify-between gap-3 font-semibold">
        <span>{title}</span>
        <span>세션 {t.sessions}</span>
      </span>
      {SEGMENTS.map((g) => (
        <span key={g.key} className="flex items-center justify-between gap-3">
          <span className="flex items-center gap-1.5">
            <i className="h-2 w-2 rounded-[2px]" style={{ background: g.color }} />
            {g.label}
          </span>
          <span>{t[g.key]}</span>
        </span>
      ))}
      <span className="flex justify-between gap-3 border-t border-current/20 pt-1">
        <span>호출</span>
        <span>{t.calls}</span>
      </span>
    </span>
  );
}

function Legend() {
  return (
    <span className="flex items-center gap-2.5 text-xxs text-muted-foreground">
      {SEGMENTS.map((g) => (
        <span key={g.key} className="flex items-center gap-1">
          <i className="h-2 w-2 rounded-[2px]" style={{ background: g.color }} />
          {g.label}
        </span>
      ))}
    </span>
  );
}

const Panel = ({ label, right, children, className }: { label: string; right?: ReactNode; children: ReactNode; className?: string }) => (
  <section className={cn("flex min-w-0 flex-col gap-2 rounded-lg border bg-background px-3 py-2.5", className)} aria-label={label}>
    <div className="flex items-center justify-between gap-2 text-xs font-medium text-muted-foreground">
      <span>{label}</span>
      {right}
    </div>
    {children}
  </section>
);

/** Calls per day, stacked from the baseline: 차단 at the top of each bar so it reads first. */
function Days({ sessions, all, day, pickDay }: { sessions: ShellSession[]; all: ShellSession[]; day: string | null; pickDay: (d: string | null) => void }) {
  const days = useMemo(() => {
    const today = new Date();
    today.setHours(12, 0, 0, 0);
    // The range is the whole list's, so a folder picked does not change the days shown.
    const first = all.reduce((m, s) => Math.min(m, s.at), Date.now());
    const span = Math.min(MAX_DAYS, Math.max(7, Math.round((today.getTime() - first) / 86_400_000) + 1));
    const keys = Array.from({ length: span }, (_, i) => dayKey(today.getTime() - (span - 1 - i) * 86_400_000));
    const by = new Map(keys.map((k) => [k, zero()]));
    for (const s of sessions) {
      const t = by.get(dayKey(s.at));
      if (t) add(t, s);
    }
    return keys.map((k) => ({ k, t: by.get(k)! }));
  }, [sessions, all]);
  const max = Math.max(1, ...days.map((d) => d.t.calls));
  const every = days.length > 16 ? 7 : days.length > 10 ? 3 : 1;
  return (
    <Panel label="일별 호출" right={<Legend />} className="flex-[2.2]">
      <div className="relative flex h-[92px] items-end gap-[3px]" role="list" data-testid="chart-days">
        <span className="pointer-events-none absolute left-0 top-0 text-xxs tabular-nums text-muted-foreground">{max}</span>
        <i className="pointer-events-none absolute inset-x-0 top-[7px] border-t border-dashed border-border" aria-hidden="true" />
        {days.map(({ k, t }) => {
          const on = day === k;
          const dim = day !== null && !on;
          return (
            <ShadTooltip key={k} content={<Tip title={dayLabel(k)} t={t} />} side="top" delayDuration={0} styleClasses="z-50">
              <button
                type="button"
                role="listitem"
                aria-label={`${dayLabel(k)} 호출 ${t.calls}`}
                aria-pressed={on}
                disabled={t.calls === 0}
                data-testid={`chart-day-${k}`}
                onClick={() => pickDay(on ? null : k)}
                className={cn(
                  "group relative flex h-full min-w-0 flex-1 flex-col justify-end rounded-[4px] pt-3 transition-opacity",
                  t.calls > 0 && "cursor-pointer hover:bg-muted",
                  dim && "opacity-35",
                )}
              >
                <span className="flex w-full flex-col-reverse gap-[2px] px-[18%]" style={{ height: `${(t.calls / max) * 100}%` }}>
                  {[...SEGMENTS].reverse().map((g) =>
                    t[g.key] > 0 ? (
                      <i
                        key={g.key}
                        className="w-full first:rounded-b-[2px] last:rounded-t-[4px]"
                        style={{ flexGrow: t[g.key], flexBasis: 0, minHeight: 2, background: g.color }}
                      />
                    ) : null,
                  )}
                </span>
                {t.calls === 0 && <i className="mx-[18%] h-[2px] rounded-full" style={{ background: C.track }} />}
                {on && <i className="absolute inset-0 rounded-[4px] ring-1 ring-foreground/60" aria-hidden="true" />}
              </button>
            </ShadTooltip>
          );
        })}
      </div>
      <div className="flex gap-[3px] text-xxs tabular-nums text-muted-foreground" aria-hidden="true">
        {days.map(({ k }, i) => (
          <span key={k} className={cn("min-w-0 flex-1 text-center", day === k && "font-semibold text-foreground")}>
            {i === days.length - 1 ? "오늘" : (days.length - 1 - i) % every === 0 ? dayLabel(k) : ""}
          </span>
        ))}
      </div>
    </Panel>
  );
}

/** Calls by folder: one stacked bar per folder, the biggest first. */
function Folders({ sessions, folder, pickFolder }: { sessions: ShellSession[]; folder: string | null; pickFolder: (f: string | null) => void }) {
  const rows = useMemo(() => {
    const by = new Map<string, Split>();
    for (const s of sessions) {
      const f = folderOf(s);
      const t = by.get(f) ?? zero();
      add(t, s);
      by.set(f, t);
    }
    const all = [...by.entries()].sort((a, b) => b[1].calls - a[1].calls);
    const top = all.slice(0, TOP_FOLDERS);
    // The picked folder stays in sight even when it is not among the biggest.
    if (folder && !top.some(([f]) => f === folder)) {
      const it = all.find(([f]) => f === folder);
      if (it) top[top.length - 1] = it;
    }
    return { top, more: all.length - top.length };
  }, [sessions, folder]);
  const max = Math.max(1, ...rows.top.map(([, t]) => t.calls));
  return (
    <Panel label="폴더" right={rows.more > 0 ? <span className="text-xxs tabular-nums">+{rows.more}</span> : undefined} className="flex-[1.5]">
      <div className="flex flex-col gap-[3px]" role="list" data-testid="chart-folders">
        {rows.top.map(([f, t]) => {
          const on = folder === f;
          const dim = folder !== null && !on;
          return (
            <ShadTooltip key={f} content={<Tip title={f} t={t} />} side="top" delayDuration={0} styleClasses="z-50">
              <button
                type="button"
                role="listitem"
                aria-pressed={on}
                aria-label={`${f} 호출 ${t.calls}`}
                data-testid={`chart-folder-${f}`}
                onClick={() => pickFolder(on ? null : f)}
                className={cn(
                  "grid h-[17px] grid-cols-[minmax(0,7.5rem)_1fr_2rem] items-center gap-2 rounded-[4px] px-1 text-xxs transition-opacity hover:bg-muted",
                  dim && "opacity-35",
                  on && "bg-muted",
                )}
              >
                <span className={cn("truncate text-left", on ? "font-semibold text-foreground" : "text-muted-foreground")}>{f}</span>
                <span className="flex h-[9px] gap-[2px]" style={{ width: `${(t.calls / max) * 100}%` }}>
                  {SEGMENTS.map((g) =>
                    t[g.key] > 0 ? (
                      <i key={g.key} className="h-full first:rounded-l-[2px] last:rounded-r-[4px]" style={{ flexGrow: t[g.key], flexBasis: 0, minWidth: 2, background: g.color }} />
                    ) : null,
                  )}
                </span>
                <span className="text-right tabular-nums text-muted-foreground">{t.calls}</span>
              </button>
            </ShadTooltip>
          );
        })}
      </div>
    </Panel>
  );
}

/** The share of calls a tool result told, as a ring; pressed, the list shows those sessions. */
function Outside({ sessions, on, toggle }: { sessions: ShellSession[]; on: boolean; toggle: () => void }) {
  const calls = sessions.reduce((n, s) => n + s.calls, 0);
  const out = sessions.reduce((n, s) => n + Math.min(s.outside, s.calls), 0);
  const with_ = sessions.filter((s) => s.outside > 0).length;
  const share = calls ? out / calls : 0;
  const R = 26;
  const L = 2 * Math.PI * R;
  const tip = (
    <span className="flex min-w-[7rem] flex-col gap-1 tabular-nums">
      <span className="flex justify-between gap-3 font-semibold">
        <span>외부 지시</span>
        <span>{out}</span>
      </span>
      <span className="flex justify-between gap-3">
        <span>호출</span>
        <span>{calls}</span>
      </span>
      <span className="flex justify-between gap-3">
        <span>세션</span>
        <span>
          {with_} / {sessions.length}
        </span>
      </span>
    </span>
  );
  return (
    <Panel label="외부 지시" className="flex-[1]">
      <ShadTooltip content={tip} side="left" delayDuration={0} styleClasses="z-50">
        <button
          type="button"
          aria-pressed={on}
          aria-label={`외부 지시 ${Math.round(share * 100)}%`}
          data-testid="chart-outside"
          onClick={toggle}
          className={cn("flex items-center gap-3 rounded-md p-1 hover:bg-muted", on && "bg-muted ring-1 ring-border")}
        >
          <svg width="68" height="68" viewBox="0 0 68 68" aria-hidden="true" className="shrink-0">
            <circle cx="34" cy="34" r={R} fill="none" stroke={C.track} strokeWidth="8" />
            <circle
              cx="34"
              cy="34"
              r={R}
              fill="none"
              stroke={C.outside}
              strokeWidth="8"
              strokeLinecap="round"
              strokeDasharray={`${Math.max(share > 0 ? 2 : 0, share * L)} ${L}`}
              transform="rotate(-90 34 34)"
              style={{ transition: "stroke-dasharray .4s ease" }}
            />
            <text x="34" y="38" textAnchor="middle" className="fill-foreground text-[13px] font-semibold tabular-nums">
              {Math.round(share * 100)}%
            </text>
          </svg>
          <span className="flex flex-col items-start gap-0.5 text-xxs tabular-nums text-muted-foreground">
            <span className="text-base font-semibold text-foreground">{out}</span>
            <span>호출 {calls}</span>
            <span>세션 {with_}</span>
          </span>
        </button>
      </ShadTooltip>
    </Panel>
  );
}

export function OverviewCharts({
  sessions, day, folder, outside, pickDay, pickFolder, toggleOutside,
}: {
  sessions: ShellSession[];
  day: string | null;
  folder: string | null;
  outside: boolean;
  pickDay: (d: string | null) => void;
  pickFolder: (f: string | null) => void;
  toggleOutside: () => void;
}) {
  // Each chart counts the sessions the other filters leave, so its own pick stays visible.
  const forDays = useMemo(() => (folder ? sessions.filter((s) => folderOf(s) === folder) : sessions), [sessions, folder]);
  const forFolders = useMemo(() => (day ? sessions.filter((s) => dayKey(s.at) === day) : sessions), [sessions, day]);
  const forRing = useMemo(() => forFolders.filter((s) => !folder || folderOf(s) === folder), [forFolders, folder]);
  return (
    <div className="mb-4 flex flex-col gap-2 lg:flex-row" data-testid="overview-charts">
      <Days sessions={forDays} all={sessions} day={day} pickDay={pickDay} />
      <Folders sessions={forFolders} folder={folder} pickFolder={pickFolder} />
      <Outside sessions={forRing} on={outside} toggle={toggleOutside} />
    </div>
  );
}
