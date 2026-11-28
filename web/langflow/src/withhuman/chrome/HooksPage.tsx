// Settings › 훅, in Langflow's settings parts: this machine and the engine its hooks report to; every
// agent the setup knows, set up or taken out on its own or every found one at once; the setups
// found here, with a reset; and a setup made by hand. Same messages as web/record's HooksView:
// `send({ page: "hooks", do: ... })`, handled by the app (src/ui/dev_setup.rs).

import { useEffect, useState } from "react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import SettingsPageHeader from "@/pages/SettingsPage/components/SettingsPageHeader";
import ForwardedIconComponent from "@/components/common/genericIconComponent";
import { type HooksData, type Scope, send as sendUp, useWithHuman } from "../bridge";
import { AgentIcon } from "./agent-icon";
import { Dot, Loading, Note, Row, Section, Spin, useAppPage } from "./settings-parts";

const send = (m: Record<string, unknown>) => sendUp({ page: "hooks", ...m });

const SCOPE: Record<Scope, { label: string; icon: string }> = {
  device: { label: "기기 전체", icon: "MonitorCog" },
  all: { label: "모든 폴더", icon: "FolderTree" },
  folder: { label: "폴더", icon: "Folder" },
  profile: { label: "프로필", icon: "UserCog" },
};
const host = (url: string | null) => (url ? url.replace(/^https?:\/\//, "").replace(/\/$/, "") : "–");
const ago = (at: number | null) => {
  if (!at) return "–";
  const s = Math.max(0, (Date.now() - at) / 1000);
  if (s < 60) return "방금";
  if (s < 3600) return `${Math.floor(s / 60)}분 전`;
  if (s < 86400) return `${Math.floor(s / 3600)}시간 전`;
  return `${Math.floor(s / 86400)}일 전`;
};
const baseName = (p: string) => p.split("/").filter(Boolean).pop() ?? p;

function Machine({ d }: { d: HooksData }) {
  const h = d.health;
  const state = d.record?.stopped ? "단말 차단" : d.installs.length ? "보호됨" : "훅 없음";
  const tone = d.record?.stopped ? "errorStatic" : d.installs.length ? "successStatic" : "secondaryStatic";
  return (
    <Section
      title="이 기기"
      action={
        <Button variant="outline" size="md" onClick={() => send({ do: "check", server: d.engine })}>
          다시 확인
        </Button>
      }
    >
      <Row
        icon={<ForwardedIconComponent name="Laptop" className="h-4 w-4 shrink-0 text-muted-foreground" />}
        label={d.machine.name}
        chip={
          <Badge variant={tone} size="tag">
            {state}
          </Badge>
        }
        sub={d.machine.os}
        control={d.own ? <span className="text-xs text-muted-foreground">{d.jev ? "Jev" : "로컬 규칙"}</span> : undefined}
      />
      <Row
        icon={<ForwardedIconComponent name="Server" className="h-4 w-4 shrink-0 text-muted-foreground" />}
        label={<span className="font-mono">{host(h?.server ?? d.engine)}</span>}
        chip={<Dot tone={!h ? "idle" : h.ok ? "ok" : "err"} />}
        sub={h ? (h.ok ? (h.ms != null ? `${h.ms}ms` : "응답") : (h.error ?? "응답 없음")) : undefined}
      />
    </Section>
  );
}

function Agents({ d }: { d: HooksData }) {
  const busy = d.busy;
  const count = (kind: string) => d.installs.filter((i) => i.agent === kind).length;
  const rows = [...d.agents].sort((a, b) => Number(b.found || b.kind === "claude") - Number(a.found || a.kind === "claude"));
  const needsFolder = d.agents.some((a) => a.found && a.per_folder);
  return (
    <Section
      title="에이전트"
      action={
        <>
          {d.note?.at === "agents" && <Note ok={d.note.ok} text={d.note.text} />}
          <Button size="md" disabled={!!busy} loading={busy === "all"} onClick={() => send({ do: "install_all" })}>
            전부 설치
          </Button>
        </>
      }
    >
      {needsFolder && (
        <Row
          icon={<ForwardedIconComponent name="Folder" className="h-4 w-4 shrink-0 text-muted-foreground" />}
          label="폴더"
          sub={<span className="font-mono">{d.folder ?? "선택 안 됨"}</span>}
          control={
            <Button variant="outline" size="md" disabled={!!busy} onClick={() => send({ do: "pick" })}>
              선택
            </Button>
          }
        />
      )}
      {rows.map((a) => {
        const n = count(a.kind);
        const here = a.found || a.kind === "claude";
        const scope = a.kind === "claude" ? "모든 폴더" : a.per_folder ? (d.folder ? `폴더 · ${baseName(d.folder)}` : "폴더별") : "프로필";
        return (
          <Row
            key={a.kind}
            dim={!here}
            icon={<AgentIcon kind={a.kind} />}
            label={a.name}
            chip={
              n > 0 ? (
                <Badge variant="successStatic" size="tag">
                  설치 {n}
                </Badge>
              ) : undefined
            }
            sub={here ? scope : "없음"}
            control={
              busy === `agent:${a.kind}` ? (
                <Spin />
              ) : n > 0 ? (
                <Button variant="ghost" size="md" className="text-destructive" disabled={!!busy} onClick={() => send({ do: "remove_agent", agent: a.kind })}>
                  제거
                </Button>
              ) : here ? (
                <Button variant="outline" size="md" disabled={!!busy || (a.per_folder && !d.folder)} onClick={() => send({ do: "install_agent", agent: a.kind })}>
                  설치
                </Button>
              ) : undefined
            }
          />
        );
      })}
    </Section>
  );
}

function Installs({ d }: { d: HooksData }) {
  const [armed, setArmed] = useState(false);
  useEffect(() => {
    if (!d.busy) setArmed(false);
  }, [d.busy, d.installs.length]);
  return (
    <Section
      title={
        <>
          설치
          <Badge variant="secondaryStatic" size="tag" className="tabular-nums">
            {d.installs.length}
          </Badge>
        </>
      }
      action={
        d.installs.length > 0 ? (
          d.busy === "reset" ? (
            <Spin />
          ) : (
            <Button variant="destructive" size="md" disabled={!!d.busy} onClick={() => (armed ? send({ do: "reset" }) : setArmed(true))}>
              {armed ? "초기화 확인" : "초기화"}
            </Button>
          )
        ) : undefined
      }
    >
      {d.installs.length === 0 && <Row label="설치 없음" dim />}
      {d.installs.map((i) => {
        const s = SCOPE[i.at];
        return (
          <Row
            key={i.file}
            icon={<AgentIcon kind={i.agent} />}
            label={i.agent_name}
            chip={
              <Badge variant="secondaryStatic" size="tag" className="gap-1">
                <ForwardedIconComponent name={s.icon} className="h-3 w-3" />
                {s.label}
              </Badge>
            }
            sub={
              <span className="font-mono" title={i.file}>
                {i.place} · {host(i.engine)} · {ago(i.installed_at)}
              </span>
            }
            control={
              d.busy === i.file ? (
                <Spin />
              ) : (
                <Button variant="ghost" size="md" className="text-destructive" disabled={!!d.busy} onClick={() => send({ do: "remove", file: i.file })}>
                  제거
                </Button>
              )
            }
          />
        );
      })}
    </Section>
  );
}

function Pick({ value, onChange, options, label }: { value: string; onChange: (v: string) => void; options: [string, string][]; label: string }) {
  return (
    <Select value={value} onValueChange={onChange}>
      <SelectTrigger className="w-[260px]" aria-label={label}>
        <SelectValue />
      </SelectTrigger>
      <SelectContent>
        {options.map(([v, l]) => (
          <SelectItem key={v} value={v}>
            {l}
          </SelectItem>
        ))}
      </SelectContent>
    </Select>
  );
}

/** A setup made by hand: any agent, any engine, any scope it takes. */
function NewSetup({ d }: { d: HooksData }) {
  const [kind, setKind] = useState("claude");
  const agent = d.agents.find((a) => a.kind === kind);
  const scopes: Scope[] = !agent || kind === "claude" ? ["device", "all", "folder"] : agent.per_folder ? ["folder"] : ["profile"];
  const [scope, setScope] = useState<Scope>("all");
  const [server, setServer] = useState(d.engine);
  const [key, setKey] = useState("");
  useEffect(() => {
    if (!scopes.includes(scope)) setScope(scopes[0]);
  }, [kind]); // eslint-disable-line react-hooks/exhaustive-deps
  useEffect(() => setServer((s) => s || d.engine), [d.engine]);
  const h = d.health && host(d.health.server) === host(server) ? d.health : null;
  const ready = !d.busy && server.trim() !== "" && scopes.includes(scope) && (scope !== "folder" || !!d.folder);
  return (
    <Section
      title="직접 설치"
      action={
        <>
          {d.note?.at === "setup" && <Note ok={d.note.ok} text={d.note.text} />}
          <Button size="md" disabled={!ready} loading={d.busy === "install"} onClick={() => send({ do: "install", agent: kind, scope, server: server.trim(), key: key.trim() })}>
            설치
          </Button>
        </>
      }
    >
      <Row
        label="에이전트"
        control={<Pick value={kind} onChange={setKind} label="에이전트" options={d.agents.filter((a) => a.found || a.kind === "claude").map((a) => [a.kind, a.name])} />}
      />
      <Row label="범위" control={<Pick value={scope} onChange={(v) => setScope(v as Scope)} label="범위" options={scopes.map((s) => [s, SCOPE[s].label])} />} />
      {scope === "folder" && (
        <Row
          label="폴더"
          sub={<span className="font-mono">{d.folder ?? "선택 안 됨"}</span>}
          control={
            <Button variant="outline" size="md" disabled={!!d.busy} onClick={() => send({ do: "pick" })}>
              선택
            </Button>
          }
        />
      )}
      <Row
        label="엔진 주소"
        chip={h ? <Badge variant={h.ok ? "successStatic" : "errorStatic"} size="tag">{h.ok ? `${h.ms}ms` : "응답 없음"}</Badge> : undefined}
        control={
          <Input
            value={server}
            onChange={(e) => setServer(e.target.value)}
            placeholder="https://engine.example"
            className="w-[260px] font-mono"
            onBlur={() => server.trim() && send({ do: "check", server: server.trim() })}
          />
        }
      />
      {scope !== "device" && (
        <Row
          label="WithHuman 키"
          control={<Input type="password" value={key} onChange={(e) => setKey(e.target.value)} placeholder="whk_… (선택)" className="w-[260px]" />}
        />
      )}
    </Section>
  );
}

export default function HooksPage() {
  useAppPage("hooks");
  const d = useWithHuman().views.hooks ?? null;
  return (
    <div className="flex w-full flex-col gap-6">
      <SettingsPageHeader title="훅" description="" />
      {d ? (
        <div className="flex w-full flex-col gap-6 pb-8">
          <Machine d={d} />
          <Agents d={d} />
          <Installs d={d} />
          <NewSetup d={d} />
        </div>
      ) : (
        <Loading />
      )}
    </div>
  );
}
