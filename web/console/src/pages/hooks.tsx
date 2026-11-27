// The personal app's 훅 page, in the parts of `./mac` (OpenClaw's macOS look, dark inside the
// app): this machine and the engine its hooks report to; every agent the setup knows, each set up
// or taken out on its own or every found one at once; the setups found on this machine; and a
// setup made by hand. The app (`src/ui/dev_setup.rs`, `HookPage`) hands it what it found and
// does what is pressed.

import { FolderOpen, FolderTree, Laptop, MonitorCog, RefreshCw, UserCog, Folder, Loader2, Check, CircleAlert } from "lucide-react";
import { useEffect, useState } from "react";
import { send as sendTo } from "@/bridge";
import { AgentIcon, Badge, Bordered, Choice, Field, FormGrid, FormLabel, Heading, LinkButton, MAC_STYLE, MCard, NATIVE, Prominent, Section, Segmented, v } from "./mac";

type Scope = "device" | "all" | "folder" | "profile";
type Install = {
  agent: string; agent_name: string;
  at: Scope; place: string; file: string; engine: string | null; installed_at: number | null; removable: boolean;
};
type Agent = { kind: string; name: string; found: boolean; per_folder: boolean; per_profile: boolean };
type Data = {
  machine: { name: string; os: string };
  /** The engine checked (`/healthz`): the hooks' own, or one typed to try. */
  health: { server: string; ok: boolean; ms: number | null; error: string | null } | null;
  /** What the engine keeps of this machine; none when it does not know it. */
  record: { registered_at: number | null; last_seen: number | null; serial: boolean; edr: string | null; stopped: { at: number; by: string | null; reason: string | null } | null } | null;
  device: string | null;
  /** The engine address the last setup used. */
  engine: string;
  /** The hooks report to the app's own engine; that engine sends Calls to Jev. */
  own: boolean;
  jev: boolean;
  installs: Install[];
  agents: Agent[];
  /** The folder picked for a setup of one folder. */
  folder: string | null;
  /** What is being done: `install`, `all`, `reset`, `agent:<kind>`, or the file being taken out. */
  busy: string | null;
  note: { ok: boolean; text: string; at: "agents" | "setup" } | null;
};

const DEMO: Data = {
  machine: { name: "MacBook Pro", os: "macOS 26.1" },
  health: { server: "http://127.0.0.1:47380", ok: true, ms: 3, error: null },
  record: { registered_at: Date.now() - 86_400_000, last_seen: Date.now() - 60_000, serial: true, edr: null, stopped: null },
  device: "default/3f2a9c1e-aaaa", engine: "http://127.0.0.1:47380", own: true, jev: false,
  installs: [
    { agent: "claude", agent_name: "Claude Code", at: "all", place: "~/.claude/settings.json", file: "/Users/me/.claude/settings.json", engine: "http://127.0.0.1:47380", installed_at: Date.now() - 3_600_000, removable: true },
    { agent: "cursor", agent_name: "Cursor", at: "folder", place: "/Users/me/work/shop", file: "/Users/me/work/shop/.cursor/hooks.json", engine: "http://127.0.0.1:47380", installed_at: Date.now() - 7_200_000, removable: true },
  ],
  agents: [
    { kind: "claude", name: "Claude Code", found: true, per_folder: false, per_profile: false },
    { kind: "codex", name: "Codex", found: true, per_folder: true, per_profile: false },
    { kind: "cursor", name: "Cursor", found: true, per_folder: true, per_profile: false },
    { kind: "copilot", name: "Copilot", found: false, per_folder: true, per_profile: false },
    { kind: "antigravity", name: "Antigravity", found: false, per_folder: true, per_profile: false },
    { kind: "hermes", name: "Hermes", found: true, per_folder: false, per_profile: true },
    { kind: "openclaw", name: "OpenClaw", found: false, per_folder: false, per_profile: true },
  ],
  folder: "/Users/me/work/shop", busy: null, note: null,
};

const send = (m: Record<string, unknown>) => sendTo("hooks", m);
const SCOPE: Record<Scope, { label: string; icon: typeof Folder }> = {
  device: { label: "기기 전체", icon: MonitorCog }, all: { label: "모든 폴더", icon: FolderTree },
  folder: { label: "폴더", icon: Folder }, profile: { label: "프로필", icon: UserCog },
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

function Note({ note }: { note: Data["note"] }) {
  if (!note) return null;
  return (
    <span className="inline-flex min-w-0 items-center gap-1.5 text-[12px]" style={{ color: note.ok ? v("green") : v("orange") }}>
      {note.ok ? <Check className="size-3.5 shrink-0" strokeWidth={2.6} /> : <CircleAlert className="size-3.5 shrink-0" />}
      <span className="truncate" title={note.text}>{note.text}</span>
    </span>
  );
}

/** This machine, and the engine its hooks report to. */
function Machine({ d }: { d: Data }) {
  const h = d.health;
  const r = d.record;
  return (
    <div className="flex w-full items-end justify-between gap-4">
      <div className="flex min-w-0 items-center gap-3">
        <span className="grid size-11 shrink-0 place-items-center rounded-[11px]" style={{ background: v("card"), boxShadow: v("shadow"), color: v("fg2") }}><Laptop className="size-[22px]" strokeWidth={1.9} /></span>
        <Heading align="left" title={<span className="text-[22px]">{d.machine.name}</span>} />
      </div>
      <div className="flex shrink-0 items-center gap-1.5">
        {r?.stopped ? <Badge tone="red">단말 차단</Badge> : d.installs.length ? <Badge tone="green">보호됨</Badge> : <Badge tone="orange">훅 없음</Badge>}
        {d.own && <Badge tone={d.jev ? "orange" : "green"}>{d.jev ? "Jev ↗" : "로컬 규칙"}</Badge>}
        <span className="inline-flex items-center gap-1.5 rounded-full px-2 py-[2px] font-mono text-[11px]" style={{ background: v("hover"), color: v("fg2") }}>
          <i className="size-1.5 rounded-full" style={{ background: !h ? v("fg3") : h.ok ? v("green") : v("red") }} />
          {host(h?.server ?? d.engine)}{h?.ok && h.ms != null && <span style={{ color: v("fg3") }}>{h.ms}ms</span>}
        </span>
        <button onClick={() => send({ do: "check", server: d.engine })} className="grid size-6 place-items-center rounded-md" style={{ color: v("fg2") }} title="다시 확인"><RefreshCw className="size-3.5" /></button>
      </div>
    </div>
  );
}

function Agents({ d }: { d: Data }) {
  const busy = d.busy;
  const count = (kind: string) => d.installs.filter((i) => i.agent === kind).length;
  const rows = [...d.agents].sort((a, b) => Number(b.found || b.kind === "claude") - Number(a.found || a.kind === "claude"));
  const needsFolder = d.agents.some((a) => a.found && a.per_folder);
  return (
    <Section title="에이전트" aside={
      <span className="flex items-center gap-3">
        {needsFolder && <LinkButton disabled={!!busy} onClick={() => send({ do: "pick" })}><FolderOpen className="size-3.5" />{d.folder ? baseName(d.folder) : "폴더 선택"}</LinkButton>}
        <Prominent disabled={!!busy} onClick={() => send({ do: "install_all" })}>{busy === "all" && <Loader2 className="size-3.5 animate-spin" />}전부 설치</Prominent>
      </span>
    }>
      <MCard pad={8} gap={2}>
        {rows.map((a) => {
          const n = count(a.kind);
          const here = a.found || a.kind === "claude";
          const working = busy === `agent:${a.kind}`;
          const scope = a.kind === "claude" ? "모든 폴더" : a.per_folder ? (d.folder ? `폴더 · ${baseName(d.folder)}` : "폴더별") : "프로필";
          return (
            <Choice key={a.kind} dim={!here} indicator={false} icon={<AgentIcon kind={a.kind} />} title={a.name}
              badge={n > 0 ? `설치 ${n}` : undefined} sub={here ? scope : "없음"}
              trailing={working ? <Loader2 className="size-4 animate-spin" style={{ color: v("fg2") }} />
                : n > 0 ? <LinkButton tone="red" disabled={!!busy} onClick={() => send({ do: "remove_agent", agent: a.kind })}>제거</LinkButton>
                : here ? <Bordered disabled={!!busy || (a.per_folder && !d.folder)} onClick={() => send({ do: "install_agent", agent: a.kind })}>설치</Bordered>
                : null} />
          );
        })}
      </MCard>
      {d.note?.at === "agents" && <div className="px-1"><Note note={d.note} /></div>}
    </Section>
  );
}

function Installs({ d }: { d: Data }) {
  const [armed, setArmed] = useState(false);
  useEffect(() => { if (!d.busy) setArmed(false); }, [d.busy, d.installs.length]);
  return (
    <Section title={`설치 ${d.installs.length}`} aside={d.installs.length > 0 && (
      d.busy === "reset" ? <Loader2 className="size-4 animate-spin" style={{ color: v("fg2") }} />
        : <LinkButton tone="red" disabled={!!d.busy} onClick={() => (armed ? send({ do: "reset" }) : setArmed(true))}>{armed ? "전부 지우기 확인" : "초기화"}</LinkButton>
    )}>
      <MCard pad={8} gap={2}>
        {d.installs.length === 0 ? (
          <Choice indicator={false} icon={<CircleAlert className="size-[18px]" style={{ color: v("orange") }} />} title="설치 없음" />
        ) : d.installs.map((i) => {
          const s = SCOPE[i.at];
          return (
            <Choice key={i.file} indicator={false} icon={<AgentIcon kind={i.agent} />}
              title={<span className="inline-flex items-center gap-1.5">{i.agent_name}<span className="inline-flex items-center gap-1 text-[11.5px] font-normal" style={{ color: v("fg2") }}><s.icon className="size-3" />{s.label}</span></span>}
              sub={<span title={i.file}>{i.place}</span>} mono
              trailing={
                <span className="flex shrink-0 items-center gap-3">
                  <span className="text-right font-mono text-[11px]" style={{ color: v("fg2") }}>{host(i.engine)}</span>
                  <span className="w-12 text-right text-[11.5px]" style={{ color: v("fg3") }}>{ago(i.installed_at)}</span>
                  {d.busy === i.file ? <Loader2 className="size-4 animate-spin" style={{ color: v("fg2") }} />
                    : <LinkButton tone="red" disabled={!!d.busy} onClick={() => send({ do: "remove", file: i.file })}>제거</LinkButton>}
                </span>
              } />
          );
        })}
      </MCard>
    </Section>
  );
}

/** A setup made by hand: any agent, any engine, any scope it takes. */
function NewSetup({ d }: { d: Data }) {
  const [kind, setKind] = useState("claude");
  const agent = d.agents.find((a) => a.kind === kind);
  const scopes: Scope[] = !agent || kind === "claude" ? ["device", "all", "folder"] : agent.per_folder ? ["folder"] : ["profile"];
  const [scope, setScope] = useState<Scope>("all");
  const [server, setServer] = useState(d.engine);
  const [key, setKey] = useState("");
  useEffect(() => { if (!scopes.includes(scope)) setScope(scopes[0]); }, [kind]);
  useEffect(() => setServer((s) => s || d.engine), [d.engine]);
  const h = d.health && host(d.health.server) === host(server) ? d.health : null;
  const ready = !d.busy && server.trim() !== "" && scopes.includes(scope) && (scope !== "folder" || !!d.folder);
  return (
    <Section title="직접 설치">
      <MCard pad={16} gap={14}>
        <FormGrid>
          <FormLabel>에이전트</FormLabel>
          <select style={{ ...NATIVE, justifySelf: "start" }} value={kind} onChange={(e) => setKind(e.target.value)}>
            {d.agents.filter((a) => a.found || a.kind === "claude").map((a) => <option key={a.kind} value={a.kind}>{a.name}</option>)}
          </select>
          <span />
          <FormLabel>범위</FormLabel>
          <div className="justify-self-start">
            {scopes.length > 1
              ? <Segmented value={scope} onChange={setScope} options={scopes.map((s) => [s, SCOPE[s].label] as [Scope, string])} />
              : <span className="text-[13px]" style={{ color: v("fg") }}>{SCOPE[scopes[0]].label}</span>}
          </div>
          <span />
          {scope === "folder" && (<>
            <FormLabel>폴더</FormLabel>
            <span className="truncate font-mono text-[12px]" style={{ color: d.folder ? v("fg") : v("fg3") }}>{d.folder ?? "선택 안 됨"}</span>
            <LinkButton disabled={!!d.busy} onClick={() => send({ do: "pick" })}>선택…</LinkButton>
          </>)}
          <FormLabel>엔진 주소</FormLabel>
          <Field mono value={server} onChange={setServer} placeholder="https://engine.example" onBlur={() => server.trim() && send({ do: "check", server: server.trim() })} />
          <span className="inline-flex w-[64px] items-center gap-1 text-[11.5px]" style={{ color: !h ? v("fg3") : h.ok ? v("green") : v("orange") }}>
            {!h ? "–" : h.ok ? <><Check className="size-3.5" strokeWidth={2.6} />{h.ms}ms</> : <CircleAlert className="size-3.5" />}
          </span>
          <FormLabel>WithHuman 키</FormLabel>
          {scope === "device" ? <span className="text-[13px]" style={{ color: v("fg3") }}>사용자별</span>
            : <Field secure value={key} onChange={setKey} placeholder="whk_… (선택)" />}
          <span />
        </FormGrid>
        <div className="flex items-center justify-end gap-3">
          <span className="mr-auto min-w-0">{d.note?.at === "setup" && <Note note={d.note} />}</span>
          <Prominent disabled={!ready} onClick={() => send({ do: "install", agent: kind, scope, server: server.trim(), key: key.trim() })}>
            {d.busy === "install" && <Loader2 className="size-3.5 animate-spin" />}설치
          </Prominent>
        </div>
      </MCard>
    </Section>
  );
}

export default function Hooks({ data }: { data: unknown }) {
  const d = (data as Data | null) ?? DEMO;
  return (
    <div className="mac mac-dark scroll h-screen overflow-y-auto" style={{ background: v("bg"), color: v("fg") }}>
      <style>{MAC_STYLE}</style>
      <div className="mx-auto flex max-w-[720px] flex-col gap-6 px-7 py-6">
        <Machine d={d} />
        <Agents d={d} />
        <Installs d={d} />
        <NewSetup d={d} />
      </div>
    </div>
  );
}
