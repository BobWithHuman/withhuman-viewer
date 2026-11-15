// The small marks the inventory's parts share: a machine's state, how its agent is let run, what
// its engine does, and which agent it is.

import { SquareTerminal } from "lucide-react";
import type { ReactNode } from "react";
import { Badge } from "@/components/ui/badge";
import { ServiceLogo } from "@/kit";
import { cn } from "@/lib/utils";
import { AGENT, ENGINE, PERM, STATE, type State } from "./model";

export const Dash = () => <span className="text-muted-foreground/60">–</span>;

export function Chip({ children, className }: { children: ReactNode; className?: string }) {
  return <Badge variant="secondary" className={cn("gap-1 border-0 bg-white/8 font-medium text-foreground/80", className)}>{children}</Badge>;
}

export function StateBadge({ state }: { state: State }) {
  return <Badge variant="secondary" className={cn("gap-1.5 border-0 font-medium", STATE[state].cls)}><i className={cn("size-1.5 rounded-full bg-current", state === "running" && "animate-pulse")} />{STATE[state].label}</Badge>;
}

/** How an agent was let run; running tools without asking is the one marked. */
export function PermBadge({ mode }: { mode: string | null }) {
  if (!mode) return <Dash />;
  const p = PERM[mode];
  if (mode === "bypassPermissions") return <Badge variant="secondary" className="border-0 bg-deny/15 font-semibold text-deny">{p.label}</Badge>;
  return <span className="inline-flex min-w-0 items-center gap-1.5 text-foreground/85"><i className="size-1.5 shrink-0 rounded-full" style={{ background: p?.color ?? "var(--info)" }} /><span className="truncate">{p?.label ?? mode}</span></span>;
}

export function EngineBadge({ mode }: { mode: string | null }) {
  const e = mode ? ENGINE[mode] : null;
  if (!e) return <Dash />;
  const Icon = e.icon;
  return <span className="inline-flex items-center gap-1.5" style={{ color: e.color }}><Icon className="size-3.5" strokeWidth={1.9} /><span className="text-foreground/85">{e.label}</span></span>;
}

/** The agent's logo: Claude's own, a terminal for Codex, a letter otherwise. */
export function AgentMark({ agent, size = 18, label }: { agent: string | null; size?: number; label?: boolean }) {
  if (!agent) return <Dash />;
  const mark = agent === "claude-code"
    ? <ServiceLogo name="claude" size={size} />
    : agent === "codex"
      ? <span className="grid shrink-0 place-items-center rounded-full bg-white text-[#111]" style={{ width: size, height: size }}><SquareTerminal style={{ width: size * 0.62, height: size * 0.62 }} strokeWidth={2} /></span>
      : <ServiceLogo name={agent} size={size} />;
  if (!label) return mark;
  return <span className="inline-flex min-w-0 items-center gap-2">{mark}<span className="truncate">{AGENT[agent]?.label ?? agent}</span></span>;
}
