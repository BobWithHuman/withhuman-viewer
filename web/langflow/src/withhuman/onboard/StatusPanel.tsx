// The first launch's side panel (the stepper modal's `sidePanel`), laid out as Langflow's
// knowledgeBaseUploadModal FilesPanel: an icon tile and a title with a count, then a scrolling
// list of rows (icon, name, a state on the right). Rows: the engine's parts, the judging, the
// agents picked and Python, or the company's engine; each with its state from `views.onboard`.

import ForwardedIconComponent from "@/components/common/genericIconComponent";
import ShadTooltip from "@/components/common/shadTooltipComponent";
import { Badge } from "@/components/ui/badge";
import { cn } from "@/utils/utils";
import { AgentIcon } from "../chrome/agent-icon";
import type { OnboardAgent, OnboardData, State } from "./data";

/** A state as one of Langflow's icons: a quiet circle, its spinner, a check, a mark (text on hover). */
export function StateIcon({ state, text }: { state: State; text?: string | null }) {
  if (state === "busy") return <ForwardedIconComponent name="Loader2" className="h-4 w-4 shrink-0 animate-spin text-muted-foreground" />;
  if (state === "ok") return <ForwardedIconComponent name="CircleCheck" className="h-4 w-4 shrink-0 text-accent-emerald-foreground" />;
  if (state === "err") {
    const mark = <ForwardedIconComponent name="CircleAlert" className="h-4 w-4 shrink-0 text-destructive" />;
    return text ? (
      <ShadTooltip content={text}>
        <span className="inline-flex">{mark}</span>
      </ShadTooltip>
    ) : (
      mark
    );
  }
  return <ForwardedIconComponent name="Circle" className="h-4 w-4 shrink-0 text-muted-foreground/40" />;
}

function Group({ label }: { label: string }) {
  return <div className="px-2 pb-1 pt-3 text-xs font-medium text-muted-foreground">{label}</div>;
}

function Row({ icon, name, mono, state, text, aside }: { icon: React.ReactNode; name: string; mono?: boolean; state?: State; text?: string | null; aside?: React.ReactNode }) {
  return (
    <div className="flex items-center justify-between gap-2 rounded-md px-2 py-1.5 hover:bg-muted">
      <div className="flex min-w-0 items-center gap-2">
        {icon}
        <span className={cn("truncate text-sm", mono && "font-mono text-xs")}>{name}</span>
      </div>
      <div className="flex shrink-0 items-center gap-1.5">
        {aside}
        {state && <StateIcon state={state} text={text} />}
      </div>
    </div>
  );
}

const icon = (name: string) => <ForwardedIconComponent name={name} className="h-4 w-4 shrink-0 text-muted-foreground" />;

/** The engine's state as a whole, from its parts and the run. */
export function engineState(d: OnboardData, started: boolean): State {
  const up = d.parts.filter((p) => p.up).length;
  if (d.run.state === "err") return "err";
  if (d.parts.length > 0 && up === d.parts.length && d.run.state === "ok") return "ok";
  return started || d.run.state === "busy" ? "busy" : "idle";
}

export function StatusPanel({
  d,
  server,
  judge,
  agents,
  needPython,
  started,
}: {
  d: OnboardData;
  /** The company's engine was picked. */
  server: boolean;
  judge: "rules" | "jev";
  /** The agents whose hooks go in (picked, or already on their way). */
  agents: OnboardAgent[];
  needPython: boolean;
  started: boolean;
}) {
  const up = d.parts.filter((p) => p.up).length;
  const part = (isUp: boolean): State => (isUp ? "ok" : d.run.state === "err" ? "err" : started || d.run.state === "busy" ? "busy" : "idle");
  const count = server ? null : `${up}/${d.parts.length}`;

  return (
    <div className="flex h-full flex-col" data-testid="onboard-status">
      <div className="flex items-center gap-2 p-3 pb-1 text-base font-semibold">
        <div className="flex h-8 w-8 items-center justify-center rounded-md bg-muted">
          <ForwardedIconComponent name="Activity" className="h-4 w-4" />
        </div>
        상태
        {count && d.parts.length > 0 && <span className="text-xs font-normal tabular-nums text-muted-foreground">({count})</span>}
      </div>
      <div className="flex-1 overflow-y-auto px-3 pb-3">
        {server ? (
          <>
            <Group label="회사 엔진" />
            <Row
              icon={icon("Database")}
              name="PostgreSQL"
              state={d.server.state}
              text={d.server.text}
              aside={
                d.server.state === "ok" && d.server.sessions !== null ? (
                  <Badge variant="successStatic" size="tag" className="tabular-nums">
                    세션 {d.server.sessions}
                  </Badge>
                ) : null
              }
            />
          </>
        ) : (
          <>
            <Group label="엔진" />
            {d.parts.map((p) => (
              <Row key={p.name} icon={icon("Box")} name={p.name} state={part(p.up)} text={d.run.state === "err" ? d.run.text : null} />
            ))}
            {d.run.state === "err" && d.run.text && (
              <div className="px-2 py-1">
                <Badge variant="errorStatic" size="tag" className="max-w-full truncate font-mono">
                  {d.run.text}
                </Badge>
              </div>
            )}
            <Group label="판정" />
            {judge === "rules" ? (
              <Row icon={icon("ListChecks")} name="규칙" state="ok" />
            ) : (
              <>
                <Row
                  icon={icon("Cpu")}
                  name="Jev"
                  state={d.jev.state}
                  text={d.jev.text}
                  aside={d.kept_key && d.jev.state === "idle" ? <ForwardedIconComponent name="KeyRound" className="h-3.5 w-3.5 text-muted-foreground" /> : null}
                />
                <Row icon={icon("Sparkles")} name={d.model} mono />
              </>
            )}
            <Group label="에이전트" />
            {agents.length === 0 ? (
              <Row icon={icon("Bot")} name="없음" />
            ) : (
              agents.map((a) => <Row key={a.kind} icon={<AgentIcon kind={a.kind} />} name={a.name} state={a.state} text={a.text} />)
            )}
            {needPython && <Row icon={icon("Terminal")} name="Python" aside={<Badge variant="secondaryStatic" size="tag">없음</Badge>} />}
          </>
        )}
      </div>
    </div>
  );
}
