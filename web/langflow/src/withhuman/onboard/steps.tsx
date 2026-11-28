// The first launch's steps, as the content of Langflow's stepper modal, in the patterns of its
// knowledgeBaseUploadModal: labelled fields (`Label` over `Input` / `Select`, StepConfiguration),
// a choice as Langflow's radio option rows (`RadioGroup` in a bordered row, as the deploy dialog's
// provider phase), and the last step as StepReview's summary (`SummaryItem` rows under a header).

import type { ReactNode } from "react";
import ForwardedIconComponent from "@/components/common/genericIconComponent";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { RadioGroup, RadioGroupItem } from "@/components/ui/radio-group";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Separator } from "@/components/ui/separator";
import { SummaryItem } from "@/modals/knowledgeBaseUploadModal/components/SummaryItem";
import { cn } from "@/utils/utils";
import { AgentIcon } from "../chrome/agent-icon";
import { type OnboardAgent, type OnboardData, ENGINE, STORE, scope, send } from "./data";
import { StateIcon } from "./StatusPanel";

export type Where = "here" | "server";
export type Judge = "rules" | "jev";

/** One of Langflow's radio option rows: the item, an icon tile, the name over a muted line, a badge. */
function Option({ value, icon, name, sub, badge, testid }: { value: string; icon: ReactNode; name: string; sub?: ReactNode; badge?: ReactNode; testid?: string }) {
  const id = `onboard-${value}`;
  return (
    <div className="flex items-center gap-3 rounded-lg border bg-background p-3" data-testid={testid}>
      <RadioGroupItem value={value} id={id} />
      <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-md bg-muted">{icon}</div>
      <Label htmlFor={id} className="flex min-w-0 flex-1 cursor-pointer flex-col gap-0.5">
        <span className="text-sm font-medium">{name}</span>
        {sub && <span className="truncate text-xs text-muted-foreground">{sub}</span>}
      </Label>
      {badge}
    </div>
  );
}

const tile = (name: string) => <ForwardedIconComponent name={name} className="h-4 w-4" />;

/** A section's header inside a step, as StepReview's (icon and a medium label). */
function Head({ icon, children, aside }: { icon: string; children: ReactNode; aside?: ReactNode }) {
  return (
    <div className="flex items-center justify-between gap-2">
      <div className="flex items-center gap-2">
        <ForwardedIconComponent name={icon} className="h-4 w-4 text-muted-foreground" />
        <span className="text-sm font-medium">{children}</span>
      </div>
      {aside}
    </div>
  );
}

/** 1. Where the engine runs: this Mac, or the company's. */
export function StepWhere({ where, onWhere }: { where: Where; onWhere: (w: Where) => void }) {
  return (
    <RadioGroup value={where} onValueChange={(v) => onWhere(v as Where)}>
      <Option
        value="here"
        icon={tile("Laptop")}
        name="이 기기"
        sub={<span className="font-mono">{ENGINE}</span>}
        badge={<Badge variant="secondaryStatic" size="tag">권장</Badge>}
        testid="onboard-where-here"
      />
      <Option value="server" icon={tile("Network")} name="회사 엔진" sub="PostgreSQL" testid="onboard-where-server" />
    </RadioGroup>
  );
}

/** 2. Judging: local rules, or Jev with its key checked and its model picked. */
export function StepJudge({ d, judge, onJudge, keyText, onKey, busy }: { d: OnboardData; judge: Judge; onJudge: (j: Judge) => void; keyText: string; onKey: (k: string) => void; busy: boolean }) {
  const check = () => {
    if (keyText.trim() && !busy) send({ do: "check_jev", key: keyText.trim() });
  };
  return (
    <div className="flex flex-col">
      <RadioGroup value={judge} onValueChange={(v) => onJudge(v as Judge)}>
        <Option value="rules" icon={tile("ListChecks")} name="규칙" sub="로컬" badge={<Badge variant="secondaryStatic" size="tag">키 없음</Badge>} testid="onboard-judge-rules" />
        <Option value="jev" icon={tile("Cpu")} name="Jev" sub={<span className="font-mono">api.typesafe.ai</span>} badge={<Badge variant="secondaryStatic" size="tag">원인 추적</Badge>} testid="onboard-judge-jev" />
      </RadioGroup>
      {judge === "jev" && (
        <div data-testid="onboard-jev">
          <Separator className="my-4" />
          <div className="flex flex-col gap-2">
            <Label htmlFor="onboard-jev-key" className="text-sm font-medium">
              키
            </Label>
            <div className="flex items-center gap-2">
              <Input
                id="onboard-jev-key"
                type="password"
                value={keyText}
                onChange={(e) => onKey(e.target.value)}
                onKeyDown={(e) => e.key === "Enter" && check()}
                placeholder={d.kept_key ? "저장된 키" : "jev_…"}
                disabled={busy}
                className="font-mono"
                data-testid="onboard-jev-key"
              />
              <Button variant="outline" disabled={busy || !keyText.trim()} onClick={check} data-testid="onboard-jev-check">
                {d.jev.state === "busy" && <ForwardedIconComponent name="Loader2" className="h-4 w-4 animate-spin" />}
                확인
              </Button>
            </div>
            <div className="flex min-h-[18px] items-center gap-1.5">
              {d.jev.state === "ok" && (
                <Badge variant="successStatic" size="tag">
                  {d.jev.text ?? "확인됨"}
                </Badge>
              )}
              {d.jev.state === "err" && (
                <Badge variant="errorStatic" size="tag" className="max-w-full truncate font-mono">
                  {d.jev.text ?? "실패"}
                </Badge>
              )}
              {d.kept_key && d.jev.state === "idle" && (
                <Badge variant="secondaryStatic" size="tag" className="gap-1">
                  <ForwardedIconComponent name="KeyRound" className="h-3 w-3" />키 저장됨
                </Badge>
              )}
            </div>
          </div>
          <div className="flex flex-col gap-2 pt-2">
            <Label className="text-sm font-medium">모델</Label>
            <Select value={d.model} disabled={d.jev.models.length === 0} onValueChange={(m) => send({ do: "model", model: m })}>
              <SelectTrigger className="font-mono" data-testid="onboard-jev-model">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {(d.jev.models.length ? d.jev.models : [d.model]).map((m) => (
                  <SelectItem key={m} value={m} className="font-mono">
                    {m}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>
        </div>
      )}
    </div>
  );
}

/** Python is missing: the hooks need it. A row with 설치. */
function PythonRow() {
  return (
    <div className="flex items-center gap-3 rounded-lg border border-accent-amber-foreground/50 bg-background p-3" data-testid="onboard-python">
      <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-md bg-muted">{tile("Terminal")}</div>
      <span className="flex-1 text-sm font-medium">Python</span>
      <Badge variant="secondaryStatic" size="tag">
        없음
      </Badge>
      <Button variant="outline" size="sm" onClick={() => send({ do: "python" })} data-testid="onboard-python-get">
        <ForwardedIconComponent name="Download" className="h-4 w-4" />
        설치
      </Button>
    </div>
  );
}

/** 3. The agents, each a checkbox row with its logo; found or not; a folder picked for some. */
export function StepAgents({
  d,
  picked,
  onToggle,
  needPython,
  busy,
}: {
  d: OnboardData;
  picked: Set<string>;
  onToggle: (kind: string) => void;
  needPython: boolean;
  busy: boolean;
}) {
  const rows = [...d.agents].sort((a, b) => Number(b.found) - Number(a.found));
  const usable = (a: OnboardAgent) => a.found && (!a.per_folder || !!a.folder);
  return (
    <div className="flex flex-col gap-1.5">
      {needPython && <PythonRow />}
      {rows.map((a) => {
        const id = `onboard-agent-${a.kind}`;
        const on = usable(a) && picked.has(a.kind);
        return (
          <div key={a.kind} className={cn("flex items-center gap-3 rounded-lg border bg-background px-3 py-2", !a.found && "opacity-50")} data-testid={id}>
            <Checkbox id={id + "-box"} checked={on} disabled={!usable(a) || busy} onCheckedChange={() => onToggle(a.kind)} />
            <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-md bg-muted">
              <AgentIcon kind={a.kind} />
            </div>
            <Label htmlFor={id + "-box"} className={cn("flex min-w-0 flex-1 flex-col gap-0.5", usable(a) && "cursor-pointer")}>
              <span className="truncate text-sm font-medium">{a.name}</span>
              <span className={cn("truncate text-xs text-muted-foreground", a.folder && "font-mono")}>{!a.found ? "없음" : a.per_folder ? (a.folder ?? "폴더") : scope(a)}</span>
            </Label>
            {a.found && a.per_folder && (
              <Button variant="ghost" size="sm" disabled={busy} onClick={() => send({ do: "pick", agent: a.kind })} data-testid={`onboard-folder-${a.kind}`}>
                <ForwardedIconComponent name="FolderOpen" className="h-4 w-4" />
                {a.folder ? "변경" : "폴더"}
              </Button>
            )}
            {a.state !== "idle" && <StateIcon state={a.state} text={a.text} />}
          </div>
        );
      })}
    </div>
  );
}

/** 4. The summary before 시작 (StepReview's): engine, judging, agents, Python. */
export function StepReady({ d, judge, agents, needPython }: { d: OnboardData; judge: Judge; agents: OnboardAgent[]; needPython: boolean }) {
  return (
    <div className="flex flex-col gap-3" data-testid="onboard-ready">
      <Head icon="FileStack">요약</Head>
      <div>
        <SummaryItem icon="Server" label="엔진" value="이 기기" />
        <SummaryItem icon="Network" label="주소" value={ENGINE} />
        <SummaryItem icon="Scale" label="판정" value={judge === "jev" ? "Jev" : "규칙"} />
        {judge === "jev" && <SummaryItem icon="Cpu" label="모델" value={d.model} />}
        <div className="flex items-center justify-between py-1.5">
          <div className="flex items-center gap-2 text-sm text-muted-foreground">
            <ForwardedIconComponent name="Bot" className="h-4 w-4" />
            <span>에이전트</span>
          </div>
          <div className="flex items-center gap-1.5">
            {agents.map((a) => (
              <AgentIcon key={a.kind} kind={a.kind} className="h-3.5 w-3.5" />
            ))}
            <span className="text-sm font-medium tabular-nums">{agents.length || "없음"}</span>
          </div>
        </div>
        <SummaryItem icon="Terminal" label="Python" value={d.python ? d.python : "없음"} />
      </div>
      {needPython && <PythonRow />}
    </div>
  );
}

/** (The company's engine) its store and the hooks' engine; 연결 확인 is the footer's left button. */
export function StepStore({ d, store, onStore }: { d: OnboardData; store: typeof STORE; onStore: (s: typeof STORE) => void }) {
  const set = (k: keyof typeof STORE) => (e: React.ChangeEvent<HTMLInputElement>) => onStore({ ...store, [k]: e.target.value });
  const field = (k: keyof typeof STORE, label: string, opts: { mono?: boolean; password?: boolean; placeholder?: string } = {}) => (
    <div className="flex flex-col gap-2">
      <Label htmlFor={`onboard-store-${k}`} className="text-xs text-muted-foreground">
        {label}
      </Label>
      <Input
        id={`onboard-store-${k}`}
        type={opts.password ? "password" : "text"}
        className={cn(opts.mono && "font-mono")}
        value={store[k]}
        onChange={set(k)}
        placeholder={opts.placeholder}
        data-testid={`onboard-store-${k}`}
      />
    </div>
  );
  return (
    <div className="flex flex-col gap-3">
      <Head icon="Database">PostgreSQL</Head>
      {field("url", "주소", { mono: true })}
      <div className="grid grid-cols-2 gap-3">
        {field("user", "사용자")}
        {field("database", "데이터베이스")}
      </div>
      {field("password", "비밀번호", { password: true })}
      <Separator className="my-1" />
      <Head icon="Webhook">훅</Head>
      <div className="grid grid-cols-2 gap-3">
        {field("engine", "엔진 주소", { mono: true, placeholder: "https://engine.example" })}
        {field("key", "WithHuman 키", { mono: true, password: true, placeholder: "whk_…" })}
      </div>
    </div>
  );
}
