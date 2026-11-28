// Everything about one step, in Langflow's look (ported from web/record's parts.tsx DetailBody):
// for a Call the engine stopped, why (the question that decided it, the rule, the score against
// the review and deny thresholds as a gauge, what it was linked to); the step's text with the
// flagged part marked; its facts; what a tool result told it (the Call whose result it was, the
// paragraph in its own words, how sure); a fold's steps.

import ForwardedIconComponent from "@/components/common/genericIconComponent";
import ShadTooltip from "@/components/common/shadTooltipComponent";
import { Badge } from "@/components/ui/badge";
import { cn } from "@/utils/utils";
import type { Step } from "../types";
import { KIND_ICON, RULE, TONE, VERDICT, judgeLabel, measureOf, pct, toneOf } from "./model";
import { Code, FieldLabel, LongText, Output, isLong, scripted } from "./text";

/** A step's kind icon on its tone's tile. */
export function KindTile({ s, size = "sm" }: { s: Step; size?: "sm" | "md" | "lg" }) {
  const box = size === "lg" ? "h-8 w-8 rounded-lg" : size === "md" ? "h-6 w-6 rounded-md" : "h-5 w-5 rounded";
  const icon = size === "lg" ? "h-4 w-4" : size === "md" ? "h-3.5 w-3.5" : "h-3 w-3";
  return (
    <span className={cn("grid shrink-0 place-items-center", box, TONE[toneOf(s)].tile)}>
      <ForwardedIconComponent name={KIND_ICON[s.kind]} className={icon} strokeWidth={2.2} />
    </span>
  );
}

/** What the engine said of a step, as a chip: refused, held, or how sure an instruction told it. */
export function VerdictBadge({ s, quiet }: { s: Step; quiet?: boolean }) {
  const tone = TONE[toneOf(s)];
  if (s.verdict && s.verdict !== "allow")
    return <Badge variant="secondaryStatic" size="tag" className={cn("shrink-0 whitespace-nowrap", tone.chip)}>{VERDICT[s.verdict]}</Badge>;
  if (s.catch)
    return <Badge variant="secondaryStatic" size="tag" className={cn("shrink-0 whitespace-nowrap tabular-nums", tone.chip)}>지시 {s.catch.score}%</Badge>;
  if (s.verdict === "allow" && !quiet) return <span className="shrink-0 text-xs text-muted-foreground">허용</span>;
  return null;
}

/** Why the engine decided as it did: the deciding name, and the score on its scale (allowed below
 * the review line, asked up to the deny line, refused above). */
export function Judge({ j }: { j: NonNullable<Step["judge"]> }) {
  const at = j.score != null ? Math.min(1, Math.max(0, j.score)) : null;
  const label = judgeLabel(j);
  return (
    <div className="grid gap-2">
      <div className="flex flex-wrap items-center gap-1.5">
        {label && <Badge variant="errorStatic" size="tag">{label}</Badge>}
        {j.stage && <Badge variant="secondaryStatic" size="tag">{j.stage}</Badge>}
        {j.rule && (
          <ShadTooltip content={j.rule}>
            <Badge variant="secondaryStatic" size="tag" className="font-mono">{RULE[j.rule] ?? j.rule}</Badge>
          </ShadTooltip>
        )}
        {j.without != null && <Badge variant="secondaryStatic" size="tag" className="tabular-nums">지시 없이 {pct(j.without)}</Badge>}
      </div>
      {at != null && (
        <div className="grid gap-1">
          <div className="relative flex h-1.5 overflow-visible rounded-full">
            <i className="h-full rounded-l-full bg-accent-emerald-foreground/45" style={{ width: pct(j.review) }} />
            <i className="h-full bg-accent-amber-foreground/55" style={{ width: pct(j.deny - j.review) }} />
            <i className="h-full flex-1 rounded-r-full bg-destructive/55" />
            <i className="absolute top-1/2 h-2.5 w-2.5 -translate-x-1/2 -translate-y-1/2 rounded-full bg-foreground ring-2 ring-background" style={{ left: pct(at) }} />
          </div>
          <div className="flex justify-between text-[11px] tabular-nums text-muted-foreground">
            <span>{measureOf(j)} <b className="font-semibold text-foreground">{pct(at)}</b></span>
            <span>승인 {pct(j.review)} · 차단 {pct(j.deny)}</span>
          </div>
        </div>
      )}
      {j.link && (
        <div className="flex items-center gap-1.5 text-xs text-muted-foreground">
          <ForwardedIconComponent name="Link2" className="h-3.5 w-3.5" />
          <span className="min-w-0 truncate text-foreground">{j.link}</span>
        </div>
      )}
    </div>
  );
}

/** What told a Call: the Call whose result it was (opening it), then the paragraph in its own words. */
function Told({ c, find, go }: { c: NonNullable<Step["catch"]>; find: (id: string) => Step | undefined; go: (id: string) => void }) {
  const from = c.by != null ? find(`s${c.by}`) : undefined;
  const words = c.para?.trim() || c.sub;
  return (
    <section>
      <FieldLabel label="지시한 곳">
        <span className={cn("flex items-center gap-1 text-xs font-semibold tabular-nums", TONE.catch.text)}>
          <ForwardedIconComponent name="TriangleAlert" className="h-3.5 w-3.5" />
          {c.score}%
        </span>
      </FieldLabel>
      <div className="overflow-hidden rounded-md border border-border">
        {(from || c.by != null) && (
          <button
            type="button"
            disabled={!from}
            onClick={() => from && go(from.id)}
            className="flex w-full min-w-0 items-center gap-2 border-b border-border px-3 py-2 text-left enabled:hover:bg-muted"
          >
            {from && <KindTile s={from} />}
            <span className="min-w-0 flex-1 truncate text-sm">{from?.name ?? `호출 #${c.by}`}</span>
            <span className="shrink-0 text-xs text-muted-foreground">{c.name}</span>
            {from && <ForwardedIconComponent name="ArrowUpRight" className="h-3.5 w-3.5 shrink-0 text-muted-foreground" />}
          </button>
        )}
        <div className="flex flex-wrap items-center gap-1.5 px-3 py-2">
          <Badge variant="secondaryStatic" size="tag" className={TONE.catch.chip}>{c.name}</Badge>
          {c.sub && c.sub !== words && <span className="font-mono text-[11px] text-muted-foreground">{c.sub}</span>}
        </div>
        {words && (
          <div className="px-2 pb-2">
            {isLong(words) ? <LongText label={c.name} text={words} /> : <Output text={words} shell={scripted(words, from?.type)} />}
          </div>
        )}
      </div>
    </section>
  );
}

/** A step's facts as label · value rows. */
function Facts({ s }: { s: Step }) {
  if (!s.detail.length && !s.catch) return null;
  return (
    <dl className="grid grid-cols-[72px_minmax(0,1fr)] gap-x-3 gap-y-1.5 text-xs">
      {s.catch && (
        <>
          <dt className="text-muted-foreground">지시 확률</dt>
          <dd className={cn("m-0 flex items-center gap-1 font-semibold tabular-nums", TONE.catch.text)}>
            <ForwardedIconComponent name="TriangleAlert" className="h-3.5 w-3.5" />
            {s.catch.score}%
          </dd>
        </>
      )}
      {s.detail.map(([k, v], i) => (
        <div key={i} className="contents">
          <dt className="text-muted-foreground">{k}</dt>
          <dd className="m-0 min-w-0 text-foreground [overflow-wrap:break-word]">
            {k === "판정" && s.verdict ? (
              <VerdictBadge s={{ ...s, catch: undefined }} />
            ) : k === "규칙" ? (
              <span className="flex flex-wrap items-center gap-1.5">
                <code className="rounded bg-muted px-1 font-mono text-[11px]">{v}</code>
                {RULE[v] && <span className="text-muted-foreground">{RULE[v]}</span>}
              </span>
            ) : (
              v
            )}
          </dd>
        </div>
      ))}
    </dl>
  );
}

export function DetailBody({ s, find, go }: { s: Step; find: (id: string) => Step | undefined; go: (id: string) => void }) {
  const stopped = s.verdict === "deny" || s.verdict === "review" || s.verdict === "rejected";
  const tool = s.detail.find(([k]) => k === "도구")?.[1] ?? s.type;
  return (
    <div className="space-y-4">
      {stopped && s.judge && (
        <section>
          <FieldLabel label={s.verdict === "deny" ? "차단 이유" : "확인 이유"} />
          <div className="rounded-md border border-border px-3 py-2.5"><Judge j={s.judge} /></div>
        </section>
      )}
      {s.text &&
        (isLong(s.text.body) ? (
          <LongText label={s.text.label} text={s.text.body} mark={s.text.mark} />
        ) : (
          <section>
            <FieldLabel label={s.text.label} />
            <Code text={s.text.body} mark={s.text.mark} tool={tool} />
          </section>
        ))}
      <Facts s={s} />
      {s.catch && <Told c={s.catch} find={find} go={go} />}
      {s.inside && (
        <div className="flex flex-wrap gap-1.5">
          {s.inside.map((t) => <Badge key={t} variant="secondaryStatic" size="tag" className="font-mono">{t}</Badge>)}
        </div>
      )}
    </div>
  );
}
