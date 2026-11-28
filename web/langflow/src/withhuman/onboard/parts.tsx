// The onboarding's small parts, in Langflow's components and tokens: a step's state as an icon
// (dot, spinner, check, mark), a picked/unpicked choice card, a chip, a step's title.

import type { ReactNode } from "react";
import ForwardedIconComponent from "@/components/common/genericIconComponent";
import ShadTooltip from "@/components/common/shadTooltipComponent";
import { cn } from "@/utils/utils";
import type { State } from "./data";

/** A step's state: a quiet dot, a spinner, a check, or a mark whose tooltip holds the app's text. */
export function StateIcon({ state, text, className }: { state: State; text?: string | null; className?: string }) {
  if (state === "busy")
    return <ForwardedIconComponent name="Loader2" className={cn("h-4 w-4 shrink-0 animate-spin text-muted-foreground", className)} />;
  if (state === "ok")
    return (
      <span className={cn("flex h-4 w-4 shrink-0 items-center justify-center rounded-full bg-accent-emerald-foreground", className)}>
        <ForwardedIconComponent name="Check" className="h-3 w-3 text-background" strokeWidth={3} />
      </span>
    );
  if (state === "err") {
    const mark = <ForwardedIconComponent name="CircleAlert" className={cn("h-4 w-4 shrink-0 text-destructive", className)} />;
    return text ? <ShadTooltip content={text}><span className="inline-flex">{mark}</span></ShadTooltip> : mark;
  }
  return (
    <span className={cn("flex h-4 w-4 shrink-0 items-center justify-center", className)}>
      <i className="h-1.5 w-1.5 rounded-full bg-muted-foreground/50" />
    </span>
  );
}

/** A short chip beside a name. */
export function Chip({ children, tone = "muted", mono, className, title }: { children: ReactNode; tone?: "muted" | "ok" | "err" | "warn" | "blue"; mono?: boolean; className?: string; title?: string }) {
  return (
    <span
      title={title}
      className={cn(
        "inline-flex h-[18px] max-w-full shrink-0 items-center gap-1 truncate rounded px-1.5 text-[11px] font-medium leading-[14px]",
        tone === "muted" && "bg-muted text-muted-foreground",
        tone === "ok" && "bg-accent-emerald text-accent-emerald-foreground",
        tone === "err" && "bg-error-background text-error-foreground",
        tone === "warn" && "bg-accent-amber text-accent-amber-foreground",
        tone === "blue" && "border border-accent-blue-foreground/40 text-accent-blue-foreground",
        mono && "font-mono",
        className,
      )}
    >
      {children}
    </span>
  );
}

/** A choice among two or three, as a card that is picked (Langflow's selected node outline). */
export function Choice({
  icon,
  title,
  chip,
  sub,
  picked,
  onPick,
  children,
  testid,
}: {
  icon: string;
  title: string;
  chip?: ReactNode;
  sub?: ReactNode;
  picked: boolean;
  onPick: () => void;
  children?: ReactNode;
  testid?: string;
}) {
  return (
    <button
      type="button"
      onClick={onPick}
      data-testid={testid}
      aria-pressed={picked}
      className={cn(
        "group flex w-full flex-col gap-3 rounded-xl border bg-background p-4 text-left shadow-sm transition-all hover:shadow-md",
        picked ? "border-ring ring-1 ring-ring" : "border-border",
      )}
    >
      <div className="flex w-full items-center gap-3">
        <span className={cn("flex h-9 w-9 shrink-0 items-center justify-center rounded-lg", picked ? "bg-primary text-primary-foreground" : "bg-muted text-foreground")}>
          <ForwardedIconComponent name={icon} className="h-[18px] w-[18px]" />
        </span>
        <div className="flex min-w-0 flex-1 flex-col gap-1">
          <div className="flex items-center gap-2 text-sm font-semibold">
            {title}
            {chip}
          </div>
          {sub && <div className="min-w-0 truncate text-xs text-muted-foreground">{sub}</div>}
        </div>
        <span
          className={cn(
            "flex h-[18px] w-[18px] shrink-0 items-center justify-center rounded-full border-2",
            picked ? "border-primary bg-primary" : "border-muted-foreground/40",
          )}
        >
          {picked && <i className="h-1.5 w-1.5 rounded-full bg-primary-foreground" />}
        </span>
      </div>
      {children}
    </button>
  );
}

/** A step's title: a short noun or two. */
export function Title({ children, aside }: { children: ReactNode; aside?: ReactNode }) {
  return (
    <div className="flex w-full items-center justify-between gap-3">
      <h2 className="font-chivo text-2xl font-semibold tracking-tight">{children}</h2>
      {aside}
    </div>
  );
}
