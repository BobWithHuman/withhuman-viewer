// The personal app's own parts, as OpenClaw's macOS app draws them (openclaw/openclaw, MIT:
// OnboardingView+Layout.swift, OnboardingView+Pages.swift, SelectableRow.swift): 16pt cards with
// a soft shadow, selectable rows tinted with the accent and ending in a check or an arrow, step
// rows that go dotted → spinner → green check, link buttons, rounded fields and the prominent
// default button. Shared by the first launch, the 훅 page and the engine's settings. The colours
// are the Mac's own, light or dark; the typeface is the app's, Pretendard.

import { Bot, Check, CircleAlert, CircleArrowRight, CircleDashed, Loader2, Terminal } from "lucide-react";
import type { CSSProperties, ReactNode } from "react";
import { cn } from "@/lib/utils";
import { ServiceLogo } from "@/kit";

export type State = "idle" | "busy" | "ok" | "err";
export type Status = { state: State; text: string | null };

const LIGHT = `--bg:#ffffff; --card:#ffffff; --fg:#1d1d1f; --fg2:rgba(0,0,0,.5); --fg3:rgba(0,0,0,.26); --accent:#007aff;
  --sel:rgba(0,122,255,.12); --selline:rgba(0,122,255,.45); --hover:rgba(120,120,128,.08); --line:rgba(0,0,0,.08);
  --field:#ffffff; --fieldline:rgba(0,0,0,.16); --shadow:0 3px 16px rgba(0,0,0,.06), 0 0 0 .5px rgba(0,0,0,.05);
  --glow:rgba(59,130,246,.34); --green:#28cd41; --orange:#ff9500; --red:#ff3b30; color-scheme: light;`;
const DARK = `--bg:#1e1e1e; --card:#2a2a2a; --fg:#f5f5f7; --fg2:rgba(255,255,255,.55);
  --fg3:rgba(255,255,255,.25); --accent:#0a84ff; --sel:rgba(10,132,255,.16); --selline:rgba(10,132,255,.5);
  --hover:rgba(255,255,255,.06); --line:rgba(255,255,255,.08); --field:#1c1c1c; --fieldline:rgba(255,255,255,.14);
  --shadow:0 3px 16px rgba(0,0,0,.25), 0 0 0 .5px rgba(255,255,255,.06); --glow:rgba(56,189,248,.35);
  --green:#32d74b; --orange:#ff9f0a; --red:#ff453a; color-scheme: dark;`;

/** The Mac's colours as `--` variables on `.mac`: following the appearance, or dark (`.mac.dark`,
 * inside the app, whose window is dark). The app's typeface throughout. */
export const MAC_STYLE = `
.mac { ${LIGHT} font-family: "Pretendard Variable", Pretendard, -apple-system, BlinkMacSystemFont, "Apple SD Gothic Neo", sans-serif;
  -webkit-font-smoothing: antialiased; letter-spacing: -0.003em; }
@media (prefers-color-scheme: dark) { .mac { ${DARK} } }
.mac.mac-dark { ${DARK} }
`;
export const v = (name: string) => `var(--${name})`;

/** The platform's own control (pop-up menu, push button), drawn by WebKit as macOS does. */
export const NATIVE: CSSProperties = { all: "revert", font: "inherit", fontSize: 13 };

/** `onboardingCard`: rounded 16, the control background, a soft shadow. */
export function MCard({ children, pad = 14, gap = 12, className }: { children: ReactNode; pad?: number; gap?: number; className?: string }) {
  return <div className={cn("flex w-full flex-col rounded-[16px]", className)} style={{ background: v("card"), boxShadow: v("shadow"), padding: pad, gap }}>{children}</div>;
}

/** A section's name over its card, as a grouped form labels one. */
export function Section({ title, aside, children }: { title: string; aside?: ReactNode; children: ReactNode }) {
  return (
    <section className="flex w-full flex-col gap-2">
      <div className="flex min-h-6 items-center justify-between px-1">
        <h2 className="text-[13px] font-semibold" style={{ color: v("fg") }}>{title}</h2>
        {aside}
      </div>
      {children}
    </section>
  );
}

/** `featureRow`: the accent's glyph, a headline and its line under it. */
export function Feature({ icon: Icon, title, sub }: { icon: typeof Bot; title: string; sub: string }) {
  return (
    <div className="flex items-start gap-3 py-1">
      <Icon className="mt-[1px] w-[26px] shrink-0" size={19} strokeWidth={2.3} style={{ color: v("accent") }} />
      <div className="flex min-w-0 flex-col gap-1">
        <span className="text-[13px] font-semibold" style={{ color: v("fg") }}>{title}</span>
        <span className="text-[11.5px]" style={{ color: v("fg2") }}>{sub}</span>
      </div>
    </div>
  );
}

/** `SelectionStateIndicator`: a filled check when picked, an arrow in a circle otherwise. */
export function Indicator({ selected }: { selected: boolean }) {
  return selected
    ? <span className="grid size-[17px] shrink-0 place-items-center rounded-full" style={{ background: v("accent") }}><Check className="size-[11px] text-white" strokeWidth={3.2} /></span>
    : <CircleArrowRight className="size-[17px] shrink-0" strokeWidth={1.7} style={{ color: v("fg2") }} />;
}

/** `connectionChoiceButton` in `openClawSelectableRowChrome`. `indicator: false` leaves the
 * check and arrow out, for a row whose end holds its own control. */
export function Choice({ icon, title, badge, sub, mono, selected = false, onClick, trailing, dim, indicator = true }: {
  icon: ReactNode; title: ReactNode; badge?: string; sub?: ReactNode; mono?: boolean; selected?: boolean; onClick?: () => void;
  trailing?: ReactNode; dim?: boolean; indicator?: boolean;
}) {
  return (
    <div onClick={onClick}
      className={cn("flex w-full items-center gap-3 rounded-[10px] px-[10px] py-2 transition-colors", dim && "opacity-45")}
      style={{ background: selected ? v("sel") : undefined, boxShadow: selected ? `inset 0 0 0 1px ${v("selline")}` : undefined }}
      onMouseEnter={(e) => { if (!selected && onClick) e.currentTarget.style.background = v("hover"); }}
      onMouseLeave={(e) => { if (!selected) e.currentTarget.style.background = ""; }}>
      <span className="grid w-[26px] shrink-0 place-items-center" style={{ color: selected ? v("accent") : v("fg2") }}>{icon}</span>
      <div className="flex min-w-0 flex-1 flex-col gap-[2px]">
        <div className="flex items-center gap-1.5">
          <span className="truncate text-[13px] font-semibold" style={{ color: v("fg") }}>{title}</span>
          {badge && <Badge>{badge}</Badge>}
        </div>
        {sub && <span className={cn("truncate text-[11.5px]", mono && "font-mono text-[11px]")} style={{ color: v("fg2") }}>{sub}</span>}
      </div>
      {trailing}
      {indicator && <Indicator selected={selected} />}
    </div>
  );
}

export function Badge({ children, tone = "accent" }: { children: ReactNode; tone?: "accent" | "green" | "orange" | "red" | "plain" }) {
  const color = tone === "plain" ? v("fg2") : v(tone);
  return (
    <span className="inline-flex shrink-0 items-center gap-1 rounded-full px-1.5 py-[1px] text-[10.5px] font-semibold"
      style={{ color, background: tone === "plain" ? v("hover") : `color-mix(in srgb, ${color} 16%, transparent)` }}>
      {children}
    </span>
  );
}

/** How a step stands: dotted while waiting, a spinner while it runs, a green check or an orange mark. */
export function StateMark({ state }: { state: State }) {
  if (state === "busy") return <Loader2 className="size-[17px] animate-spin" style={{ color: v("fg2") }} />;
  if (state === "ok") return <span className="grid size-[17px] place-items-center rounded-full" style={{ background: v("green") }}><Check className="size-[11px] text-white" strokeWidth={3.2} /></span>;
  if (state === "err") return <CircleAlert className="size-[18px]" style={{ color: v("orange") }} />;
  return <CircleDashed className="size-[17px]" style={{ color: v("fg3") }} />;
}

/** `installStepRow`. */
export function StepRow({ state, title, detail, mono }: { state: State; title: ReactNode; detail: ReactNode; mono?: boolean }) {
  return (
    <div className="flex items-start gap-3">
      <span className="grid h-[22px] w-[26px] shrink-0 place-items-center"><StateMark state={state} /></span>
      <div className="flex min-w-0 flex-col gap-[2px]">
        <span className="text-[13px] font-semibold" style={{ color: state === "idle" ? v("fg2") : v("fg") }}>{title}</span>
        <span className={cn("truncate text-[11.5px]", mono && "font-mono text-[11px]")} style={{ color: state === "err" ? v("orange") : v("fg2") }}>{detail}</span>
      </div>
    </div>
  );
}

/** An agent's own logo, or a glyph for one without. */
export function AgentIcon({ kind, size = 20 }: { kind: string; size?: number }) {
  if (["claude", "cursor", "copilot", "antigravity"].includes(kind)) return <ServiceLogo name={kind} size={size} />;
  if (kind === "codex") return <Terminal style={{ width: size - 2, height: size - 2 }} strokeWidth={2.2} />;
  return <Bot style={{ width: size - 2, height: size - 2 }} strokeWidth={2.2} />;
}

/** `.largeTitle.weight(.semibold)` over `.title3` secondary. */
export function Heading({ title, sub, align = "center" }: { title: ReactNode; sub?: string; align?: "center" | "left" }) {
  return (
    <div className={cn("flex flex-col gap-1.5", align === "center" ? "items-center text-center" : "items-start")}>
      <h1 className="text-[26px] leading-[1.15] font-bold tracking-[-0.02em]" style={{ color: v("fg") }}>{title}</h1>
      {sub && <p className="text-[15px]" style={{ color: v("fg2") }}>{sub}</p>}
    </div>
  );
}

/** `.buttonStyle(.link)`. */
export function LinkButton({ children, onClick, disabled, tone = "accent" }: { children: ReactNode; onClick: () => void; disabled?: boolean; tone?: "accent" | "red" }) {
  return <button onClick={onClick} disabled={disabled} className="inline-flex items-center gap-1 text-[13px] disabled:opacity-40" style={{ color: v(tone) }}>{children}</button>;
}

/** A rounded text field, as AppKit's. */
export function Field({ value, onChange, placeholder, secure, mono, disabled, onBlur }: {
  value: string; onChange: (s: string) => void; placeholder?: string; secure?: boolean; mono?: boolean; disabled?: boolean; onBlur?: () => void;
}) {
  return (
    <input type={secure ? "password" : "text"} value={value} placeholder={placeholder} disabled={disabled} onChange={(e) => onChange(e.target.value)} onBlur={onBlur}
      className={cn("h-[26px] w-full min-w-0 rounded-[6px] px-2 text-[13px] outline-none focus:shadow-[0_0_0_3px_rgba(0,122,255,.35)] disabled:opacity-50", mono && "font-mono text-[12px]")}
      style={{ background: v("field"), color: v("fg"), border: `1px solid ${v("fieldline")}` }} />
  );
}

/** `.borderedProminent`: the window's default button. */
export function Prominent({ children, disabled, onClick, tone = "accent" }: { children: ReactNode; disabled?: boolean; onClick: () => void; tone?: "accent" | "red" }) {
  return (
    <button disabled={disabled} onClick={onClick}
      className="inline-flex h-[28px] min-w-[96px] items-center justify-center gap-1.5 rounded-[7px] px-4 text-[13px] font-semibold text-white shadow-[0_1px_1.5px_rgba(0,0,0,.18),inset_0_.5px_0_rgba(255,255,255,.25)] active:brightness-110 disabled:opacity-40"
      style={{ background: v(tone) }}>
      {children}
    </button>
  );
}

/** `.bordered`: a plain push button beside the prominent one. */
export function Bordered({ children, disabled, onClick }: { children: ReactNode; disabled?: boolean; onClick: () => void }) {
  return (
    <button disabled={disabled} onClick={onClick}
      className="inline-flex h-[28px] items-center justify-center gap-1.5 rounded-[7px] px-3.5 text-[13px] font-medium disabled:opacity-40"
      style={{ background: v("hover"), color: v("fg"), boxShadow: `inset 0 0 0 .5px ${v("line")}` }}>
      {children}
    </button>
  );
}

/** A segmented control, as AppKit's. */
export function Segmented<T extends string>({ value, options, onChange }: { value: T; options: [T, string][]; onChange: (v: T) => void }) {
  return (
    <div className="inline-flex rounded-[7px] p-[2px]" style={{ background: v("hover") }}>
      {options.map(([k, label]) => (
        <button key={k} onClick={() => onChange(k)} className="h-[24px] min-w-[72px] rounded-[5px] px-3 text-[12.5px] font-medium"
          style={value === k ? { background: v("card"), color: v("fg"), boxShadow: "0 1px 2px rgba(0,0,0,.15)" } : { color: v("fg2") }}>{label}</button>
      ))}
    </div>
  );
}

/** A label beside its value, as a form's row. */
export function FormGrid({ children, label = 92 }: { children: ReactNode; label?: number }) {
  return <div className="grid items-center gap-x-3 gap-y-2.5" style={{ gridTemplateColumns: `${label}px minmax(0,1fr) auto` }}>{children}</div>;
}
export function FormLabel({ children }: { children: ReactNode }) {
  return <span className="text-right text-[13px]" style={{ color: v("fg2") }}>{children}</span>;
}
