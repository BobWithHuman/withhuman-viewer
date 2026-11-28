// Small pieces the 훅 and 엔진 settings pages share, built from Langflow's settings parts: its page
// header, cards, rows separated the way its lists are, a spinner, and the page's open/close told to
// the app (the hooks' and the engine's data only comes while the app knows the page is open).

import { type ReactNode, useEffect } from "react";
import ForwardedIconComponent from "@/components/common/genericIconComponent";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { cn } from "@/utils/utils";
import { send } from "../bridge";

/** Tells the app this page is open, and that 기록 is again when it closes. */
export function useAppPage(page: "hooks" | "settings") {
  useEffect(() => {
    send({ do: "page", page });
    return () => send({ do: "page", page: "record" });
  }, [page]);
}

export const Spin = ({ className }: { className?: string }) => (
  <ForwardedIconComponent name="Loader2" className={cn("h-4 w-4 animate-spin text-muted-foreground", className)} />
);

export const Dot = ({ tone, className }: { tone: "ok" | "err" | "idle"; className?: string }) => (
  <i
    className={cn(
      "inline-block h-1.5 w-1.5 shrink-0 rounded-full",
      tone === "ok" ? "bg-accent-emerald-foreground" : tone === "err" ? "bg-destructive" : "bg-muted-foreground/50",
      className,
    )}
  />
);

/** A card of the settings page: its title, what stands at the title's right, and its rows. */
export function Section({ title, action, danger, children }: { title: ReactNode; action?: ReactNode; danger?: boolean; children: ReactNode }) {
  return (
    <Card className={cn(danger && "border-destructive/40")}>
      <CardHeader className="flex-row items-center justify-between gap-4 space-y-0">
        <CardTitle className={cn("flex items-center gap-2", danger && "text-destructive")}>{title}</CardTitle>
        {action && <div className="flex shrink-0 items-center gap-2">{action}</div>}
      </CardHeader>
      <CardContent className="flex flex-col">{children}</CardContent>
    </Card>
  );
}

/** One row of a card: a name (with an icon and a chip), what is under it, and its control. */
export function Row({
  label,
  icon,
  chip,
  sub,
  control,
  dim,
}: {
  label: ReactNode;
  icon?: ReactNode;
  chip?: ReactNode;
  sub?: ReactNode;
  control?: ReactNode;
  dim?: boolean;
}) {
  return (
    <div className={cn("flex min-h-[52px] items-center gap-3 border-t border-border py-2.5 first:border-t-0 first:pt-0", dim && "opacity-50")}>
      {icon}
      <div className="flex min-w-0 flex-1 flex-col gap-0.5">
        <div className="flex min-w-0 items-center gap-2 text-sm font-medium">
          <span className="truncate">{label}</span>
          {chip}
        </div>
        {sub && <div className="min-w-0 truncate text-xs text-muted-foreground">{sub}</div>}
      </div>
      {control && <div className="flex shrink-0 items-center gap-2">{control}</div>}
    </div>
  );
}

/** What the app said of the last thing pressed. */
export function Note({ ok, text }: { ok: boolean; text: string }) {
  return (
    <span className={cn("inline-flex min-w-0 items-center gap-1.5 text-xs", ok ? "text-accent-emerald-foreground" : "text-destructive")}>
      <ForwardedIconComponent name={ok ? "Check" : "CircleAlert"} className="h-3.5 w-3.5 shrink-0" />
      <span className="truncate" title={text}>
        {text}
      </span>
    </span>
  );
}

export const Loading = () => (
  <div className="flex h-40 w-full items-center justify-center">
    <Spin />
  </div>
);
