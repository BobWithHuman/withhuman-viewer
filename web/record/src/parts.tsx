import * as TT from "@radix-ui/react-tooltip";
import { ArrowUpRight, Bot, CircleDot, FileText, Laptop, Layers, MessageSquareText, SquareTerminal, TriangleAlert, Wrench } from "lucide-react";
import { createContext, useContext, type ReactNode } from "react";
import { siClaude, siGithub, siNotion, siPostgresql } from "simple-icons";
import type { Kind, Step, Svc, Verdict } from "./data";
import { Code, Output, scripted } from "./code";
import { isLong, LongText } from "./longtext";

export const clock = (at: Step["at"]) => {
  if (at === undefined) return "";
  if (typeof at === "string") return at;
  const d = new Date(at), z = (n: number) => String(n).padStart(2, "0");
  return `${z(d.getHours())}:${z(d.getMinutes())}:${z(d.getSeconds())}`;
};

export const cx = (...c: (string | false | null | undefined)[]) => c.filter(Boolean).join(" ");

export function Tip({ label, children, side = "bottom" }: { label: string; children: ReactNode; side?: "top" | "bottom" | "right" | "left" }) {
  return (
    <TT.Root delayDuration={250}>
      <TT.Trigger asChild>{children}</TT.Trigger>
      <TT.Portal>
        <TT.Content side={side} sideOffset={6} className="z-50 rounded-md border border-white/10 bg-[#111] px-2 py-1 text-[11.5px] text-white shadow-xl">{label}</TT.Content>
      </TT.Portal>
    </TT.Root>
  );
}

export function SvcLogo({ s, size = 14 }: { s: Svc; size?: number }) {
  if (s === "slack") {
    return (
      <svg width={size} height={size} viewBox="0 0 24 24" aria-hidden>
        <rect x="9.5" y="2" width="5" height="9" rx="2.5" fill="#36C5F0" /><rect x="13" y="9.5" width="9" height="5" rx="2.5" fill="#2EB67D" />
        <rect x="9.5" y="13" width="5" height="9" rx="2.5" fill="#ECB22E" /><rect x="2" y="9.5" width="9" height="5" rx="2.5" fill="#E01E5A" />
      </svg>
    );
  }
  const icon = { notion: siNotion, postgresql: siPostgresql, github: siGithub, claude: siClaude }[s];
  return <svg width={size} height={size} viewBox="0 0 24 24" aria-hidden><path d={icon.path} fill={s === "notion" || s === "github" ? "#111" : `#${icon.hex}`} /></svg>;
}

/** A logo on a white disc, so dark marks read on the dark page. */
export function LogoDisc({ s, size = 20 }: { s: Svc; size?: number }) {
  return <span className="grid shrink-0 place-items-center rounded-full bg-white" style={{ width: size, height: size }}><SvcLogo s={s} size={Math.round(size * 0.6)} /></span>;
}

export const KIND_ICON: Record<Kind, ReactNode> = {
  device: <Laptop />, agent: <Bot />, prompt: <SquareTerminal />, model: <CircleDot />, call: <Wrench />, result: <FileText />,
  answer: <MessageSquareText />, group: <Layers />,
};
export const VERDICT: Record<Verdict, [string, string]> = {
  allow: ["허용", "bg-teal/15 text-teal"], review: ["승인 요청", "bg-hold/15 text-hold"], rejected: ["승인 거절", "bg-hold/15 text-hold"], deny: ["차단", "bg-block/15 text-block"],
};
/** What the header's chips pick out: the steps a tool response told, the refused, and those the person was asked about. */
export type Only = "outside" | "deny" | "ask";
export const ONLY: Record<Only, { label: string; is: (s: Step) => boolean }> = {
  outside: { label: "외부 지시", is: (s) => !!s.catch },
  deny: { label: "차단", is: (s) => s.verdict === "deny" },
  ask: { label: "승인 요청", is: (s) => s.verdict === "review" || s.verdict === "rejected" },
};
export const flagged = (s: Step) => !!s.catch || (!!s.verdict && s.verdict !== "allow");

/** A step's colour: refused red, held amber, caught yellow; anything else plain grey, so what
 * was caught is what stands out. */
export function tone(s: Step) {
  if (s.verdict === "deny") return "bg-block text-[#1d1d1d]";
  if (s.verdict === "rejected" || s.verdict === "review") return "bg-hold text-[#1d1d1d]";
  if (s.catch) return "bg-catch text-[#1d1d1d]";
  return "bg-[#4a4a4a] text-[#e6e6e6]";
}
export function toneHex(s: Step) {
  if (s.verdict === "deny") return "#ef6b5e";
  if (s.verdict === "rejected" || s.verdict === "review") return "#f2a93b";
  if (s.catch) return "#f8d448";
  return "#7a7a7a";
}

export function VerdictChip({ s }: { s: Step }) {
  if (s.verdict && s.verdict !== "allow") return <span className={cx("inline-flex shrink-0 items-center gap-1 rounded-md px-2 py-0.5 text-[12px] font-semibold whitespace-nowrap", VERDICT[s.verdict][1])}><i className="size-1.5 rounded-full bg-current" />{VERDICT[s.verdict][0]}</span>;
  if (s.catch) return <span className="inline-flex shrink-0 items-center gap-1 rounded-md bg-catch/15 px-2 py-0.5 text-[12px] font-semibold whitespace-nowrap text-catch"><TriangleAlert className="size-3" />외부 지시 {s.catch.score}%</span>;
  if (s.verdict === "allow") return <span className="shrink-0 rounded-md bg-white/6 px-2 py-0.5 text-[12px] whitespace-nowrap text-mut">허용</span>;
  return null;
}

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
export const pct = (p: number) => `${Math.round(p * 100)}%`;

/** Why the engine decided as it did: the question that decided it (the second, with what it
 * asked, when it was asked), the rule and the worst breach, and the score on its scale: allowed
 * below the review line, asked up to the deny line, refused above. */
/** What decided a Call, in one name: what the second question asked, else the worst breach,
 * else the rule. */
export const judgeLabel = (j: NonNullable<Step["judge"]>) =>
  (j.stage !== "1차 질문" && j.asked) || (j.worst && (WORST[j.worst] ?? j.worst)) || (j.rule && (RULE[j.rule] ?? j.rule)) || null;

export function Judge({ j }: { j: NonNullable<Step["judge"]> }) {
  const at = j.score != null ? Math.min(1, Math.max(0, j.score)) : null;
  // The first question's scale is how far the Call departs; a rule's on a path, how risky it is;
  // the second question's, how likely.
  const measure = j.stage === "1차 질문" || !j.stage ? "이탈" : j.rule?.startsWith("causal::") ? "위험" : "확률";
  const label = judgeLabel(j);
  return (
    <span className="grid gap-1.5">
      {label && <span className="justify-self-start rounded bg-block/15 px-1.5 py-px text-[11.5px] font-semibold text-block" title={[j.stage, j.rule].filter(Boolean).join(" · ")}>{label}</span>}
      {at != null && (
        <span className="grid gap-1">
          <span className="relative flex h-1.5 overflow-hidden rounded-full">
            <i className="h-full bg-teal/55" style={{ width: pct(j.review) }} />
            <i className="h-full bg-hold/55" style={{ width: pct(j.deny - j.review) }} />
            <i className="h-full flex-1 bg-block/55" />
            <i className="absolute top-1/2 size-2.5 -translate-x-1/2 -translate-y-1/2 rounded-full bg-fg ring-2 ring-bg" style={{ left: pct(at) }} />
          </span>
          <span className="flex justify-between text-[10.5px] text-dim tabular-nums">
            <span>{measure} <b className="font-semibold text-fg/85">{pct(at)}</b></span>
            <span>차단 {pct(j.deny)}</span>
          </span>
        </span>
      )}
    </span>
  );
}

/** What the card can reach of the rest of the record: a step by its ID, and opening one. */
export const Nav = createContext<{ step: (id: string) => Step | undefined; go: (id: string) => void } | null>(null);

/** Everything about one step: its text, the engine's numbers, why it was decided so, and what
 * told it: the result's words in full, after the Call that brought them back. */
export function DetailBody({ s }: { s: Step }) {
  const stopped = s.verdict === "deny" || s.verdict === "review" || s.verdict === "rejected";
  return (
    <div className="space-y-3">
      {stopped && s.judge && (
        <div>
          <div className="mb-1.5 text-[11.5px] text-mut">{s.verdict === "deny" ? "차단 이유" : "확인 이유"}</div>
          <div className="rounded-md bg-box px-3 py-2.5"><Judge j={s.judge} /></div>
        </div>
      )}
      {s.text && (isLong(s.text.body)
        ? <LongText label={s.text.label} text={s.text.body} mark={s.text.mark} />
        : <div><div className="mb-1.5 text-[11.5px] text-mut">{s.text.label}</div><Code text={s.text.body} mark={s.text.mark} tool={s.detail.find(([k]) => k === "도구")?.[1] ?? s.type} /></div>)}
      <dl className="grid grid-cols-[64px_minmax(0,1fr)] gap-x-3 gap-y-1 text-[11.5px]">
        {s.catch && <><dt className="text-mut">지시 확률</dt><dd className="m-0 flex items-center gap-1.5 text-fg"><TriangleAlert className="size-3.5 fill-catch text-[#282828]" />{s.catch.score}%</dd></>}
        {s.detail.map(([k, v], i) => (
          <div key={i} className="contents">
            <dt className="text-mut">{k}</dt>
            <dd className="m-0 min-w-0 text-fg/85 [overflow-wrap:break-word]">{k === "판정" && s.verdict ? <span className={cx("rounded-md px-1.5 py-px text-[11px] font-semibold", VERDICT[s.verdict][1])}>{v}</span> : k === "규칙" ? <code className="font-mono text-[11px]">{v}</code> : v}</dd>
          </div>
        ))}
      </dl>
      {s.catch && <Told c={s.catch} />}
      {s.inside && <div className="flex flex-wrap gap-1.5">{s.inside.map((t) => <span key={t} className="rounded-md bg-chip px-1.5 py-0.5 font-mono text-[11px]">{t}</span>)}</div>}
    </div>
  );
}

/** What told a Call: the Call whose result it was (opening it), then the result's words in full,
 * with how sure the engine is. */
function Told({ c }: { c: NonNullable<Step["catch"]> }) {
  const nav = useContext(Nav);
  const from = c.by != null && nav ? nav.step(`s${c.by}`) : undefined;
  const words = c.para?.trim() || c.sub;
  return (
    <div>
      <div className="mb-1.5 flex items-center text-[11.5px] text-mut">
        지시한 곳
        <span className="ml-auto flex items-center gap-1 font-semibold text-fg"><TriangleAlert className="size-3.5 fill-catch text-[#242424]" />{c.score}%</span>
      </div>
      <div className="overflow-hidden rounded-md bg-box">
        {(from || c.by != null) && (
          <button disabled={!from || !nav} onClick={() => from && nav?.go(from.id)}
            className="flex w-full min-w-0 items-center gap-2 border-b border-white/[.06] px-3 py-2 text-left enabled:cursor-pointer enabled:hover:bg-white/[.04]">
            {from && <span className={cx("grid size-5 shrink-0 place-items-center rounded [&_svg]:size-3 [&_svg]:stroke-[2.4]", tone(from))}>{KIND_ICON[from.kind]}</span>}
            <span className="min-w-0 flex-1 truncate text-[12px] text-fg">{from?.name ?? `호출 #${c.by}`}</span>
            <span className="shrink-0 text-[11px] text-dim">{from?.type ?? ""} · {c.name}</span>
            {from && <ArrowUpRight className="size-3.5 shrink-0 text-mut" />}
          </button>
        )}
        {words && (isLong(words)
          ? <div className="px-1 py-1"><LongText label={c.name} text={words} /></div>
          : <Output text={words} shell={scripted(words, from?.type)} />)}
      </div>
    </div>
  );
}

/** The right column of A and B: the picked step in full. */
export function DetailPanel({ s, onClose }: { s: Step | undefined; onClose: () => void }) {
  return (
    <aside className="my-3 mr-3 flex min-h-0 w-[460px] shrink-0 flex-col rounded-xl bg-panel shadow-[0_12px_40px_rgba(0,0,0,.45)]">
      {s ? (
        <>
          <div className="flex items-start gap-3 border-b border-white/8 px-5 py-4">
            <span className={cx("mt-0.5 grid size-8 shrink-0 place-items-center rounded-lg [&_svg]:size-4 [&_svg]:stroke-[2.4]", tone(s))}>{KIND_ICON[s.kind]}</span>
            <div className="min-w-0 flex-1">
              <div className="flex items-center gap-2"><span className="truncate text-[15px] font-semibold">{s.name}</span>{s.svc && <LogoDisc s={s.svc} size={18} />}</div>
              <div className="mt-0.5 flex items-center gap-2 text-[12px] text-mut">{s.type}<span className="text-dim">|</span>11월 14일 {s.at}<VerdictChip s={s} /></div>
            </div>
            <button onClick={onClose} className="grid size-7 place-items-center rounded-md text-mut hover:bg-white/8 hover:text-fg">✕</button>
          </div>
          <div className="scroll min-h-0 flex-1 overflow-auto px-5 py-4"><DetailBody s={s} /></div>
        </>
      ) : (
        <div className="grid flex-1 place-items-center text-[13px] text-dim">없음</div>
      )}
    </aside>
  );
}
