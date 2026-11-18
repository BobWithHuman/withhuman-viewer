// The developer's 훅 page, in 기록's frame and look: this machine as the engine knows it, the
// WithHuman setups found on it, and a new setup. The app hands it what it found
// (`WHHooks.render(data)`) and does what is pressed (`__whHooksSend` in the app's window).

import * as DM from "@radix-ui/react-dropdown-menu";
import * as TT from "@radix-ui/react-tooltip";
import {
  Ban, Check, CircleAlert, Copy, Folder, FolderOpen, FolderTree, KeyRound, Laptop, Loader2, MoreHorizontal,
  MonitorCog, RefreshCw, RotateCcw, ShieldCheck, ShieldOff, Trash2, Users,
} from "lucide-react";
import { useEffect, useState, type ReactNode } from "react";
import { cx } from "./parts";

export type Scope = "device" | "all" | "folder";
export type Install = {
  at: Scope; place: string; file: string; engine: string | null; installed_at: number | null; removable: boolean;
};
export type HooksData = {
  machine: { name: string; os: string };
  /** The engine checked (`/healthz`): the hooks' own, or one typed to try. */
  health: { server: string; ok: boolean; ms: number | null; error: string | null } | null;
  /** What the engine keeps of this machine; none when it does not know it. */
  record: {
    registered_at: number | null; last_seen: number | null; serial: boolean; edr: string | null;
    stopped: { at: number; by: string | null; reason: string | null } | null;
  } | null;
  /** This machine's client ID, from a setup; none before one. */
  device: string | null;
  /** The engine address the last setup used. */
  engine: string;
  installs: Install[];
  /** The folder picked for a setup of one folder. */
  folder: string | null;
  /** What is being done: `install`, or the file of the setup being taken out. */
  busy: string | null;
  note: { ok: boolean; text: string } | null;
};

const send = (m: Record<string, unknown>) =>
  (window.parent as unknown as { __whHooksSend?: (m: unknown) => void }).__whHooksSend?.(m);

const SCOPE: Record<Scope, { label: string; icon: typeof Folder; tone: string; traits: [typeof Folder, string][] }> = {
  device: {
    label: "기기 전체", icon: MonitorCog, tone: "text-teal",
    traits: [[Users, "모든 사용자"], [FolderTree, "모든 폴더"], [Ban, "다른 훅 차단"], [KeyRound, "관리자 암호"]],
  },
  all: { label: "모든 폴더", icon: FolderTree, tone: "text-act", traits: [[Laptop, "이 사용자"], [FolderTree, "모든 폴더"]] },
  folder: { label: "폴더", icon: Folder, tone: "text-mut", traits: [[Laptop, "이 사용자"], [Folder, "한 폴더"]] },
};

const host = (url: string | null) => (url ? url.replace(/^https?:\/\//, "") : "–");
const ago = (at: number | null) => {
  if (!at) return "–";
  const s = Math.max(0, (Date.now() - at) / 1000);
  if (s < 60) return "방금";
  if (s < 3600) return `${Math.floor(s / 60)}분 전`;
  if (s < 86400) return `${Math.floor(s / 3600)}시간 전`;
  return `${Math.floor(s / 86400)}일 전`;
};
const shortId = (id: string) => {
  const [team, rest] = id.includes("/") ? id.split("/", 2) : ["", id];
  const cut = rest.split("-")[0];
  return team ? `${team}/${cut}` : cut;
};

function Tip({ label, children }: { label: string; children: ReactNode }) {
  return (
    <TT.Root delayDuration={250}>
      <TT.Trigger asChild>{children}</TT.Trigger>
      <TT.Portal>
        <TT.Content side="bottom" sideOffset={6} className="z-50 max-w-[420px] rounded-md bg-[#111] px-2 py-1 font-mono text-[11.5px] break-all text-fg shadow-lg ring-1 ring-white/10">{label}</TT.Content>
      </TT.Portal>
    </TT.Root>
  );
}

function Panel({ title, aside, children, pad = true }: { title: string; aside?: ReactNode; children: ReactNode; pad?: boolean }) {
  return (
    <section className="overflow-hidden rounded-xl bg-panel ring-1 ring-white/[.06]">
      <header className="flex h-11 items-center justify-between border-b border-white/[.06] px-4">
        <h2 className="text-[13px] font-semibold">{title}</h2>
        {aside}
      </header>
      <div className={pad ? "p-4" : ""}>{children}</div>
    </section>
  );
}

/** A fact about the machine: its name above, its value below, tinted when it calls for a look. */
function Fact({ label, value, tone }: { label: string; value: ReactNode; tone?: "ok" | "warn" | "bad" }) {
  return (
    <div className="flex min-w-0 flex-col gap-1 px-4 py-3">
      <span className="text-[11.5px] text-dim">{label}</span>
      <span className={cx("truncate text-[13px] font-medium", tone === "ok" && "text-teal", tone === "warn" && "text-hold", tone === "bad" && "text-block")}>{value}</span>
    </div>
  );
}

function Machine({ data }: { data: HooksData }) {
  const [copied, setCopied] = useState(false);
  const r = data.record;
  const covered = data.installs.find((i) => i.at === "device") ?? data.installs.find((i) => i.at === "all") ?? data.installs[0];
  const copy = () => {
    if (!data.device) return;
    navigator.clipboard?.writeText(data.device).then(() => { setCopied(true); setTimeout(() => setCopied(false), 1200); });
  };
  return (
    <section className="overflow-hidden rounded-xl bg-panel ring-1 ring-white/[.06]">
      <div className="flex items-center gap-4 px-4 py-4">
        <span className="grid size-11 shrink-0 place-items-center rounded-lg bg-white/[.06]"><Laptop className="size-5 text-fg/80" /></span>
        <div className="min-w-0 flex-1">
          <div className="truncate text-[17px] font-semibold tracking-[-0.01em]">{data.machine.name}</div>
          <div className="mt-0.5 flex min-w-0 items-center gap-2 text-[12.5px] text-mut">
            <span>{data.machine.os}</span>
            {data.device && (
              <>
                <span className="text-dim">·</span>
                <Tip label={data.device}>
                  <button onClick={copy} className="inline-flex min-w-0 cursor-pointer items-center gap-1 rounded px-1 font-mono text-[12px] text-mut hover:bg-white/[.06] hover:text-fg">
                    {shortId(data.device)}{copied ? <Check className="size-3 text-teal" /> : <Copy className="size-3" />}
                  </button>
                </Tip>
              </>
            )}
          </div>
        </div>
        {r?.stopped ? (
          <span className="inline-flex items-center gap-1.5 rounded-md bg-block/15 px-2 py-1 text-[12px] font-semibold text-block"><Ban className="size-3.5" />단말 차단됨</span>
        ) : covered ? (
          <span className="inline-flex items-center gap-1.5 rounded-md bg-teal/15 px-2 py-1 text-[12px] font-semibold text-teal"><ShieldCheck className="size-3.5" />보호됨 · {SCOPE[covered.at].label}</span>
        ) : (
          <span className="inline-flex items-center gap-1.5 rounded-md bg-hold/15 px-2 py-1 text-[12px] font-semibold text-hold"><ShieldOff className="size-3.5" />미보호</span>
        )}
      </div>
      <div className="grid grid-cols-4 divide-x divide-white/[.06] border-t border-white/[.06]">
        <Fact label="엔진 등록" value={r?.registered_at ? ago(r.registered_at) : "안 됨"} tone={r?.registered_at ? undefined : "warn"} />
        <Fact label="마지막 신호" value={ago(r?.last_seen ?? null)} />
        <Fact label="Wazuh" value={r?.edr ? (r.edr === "active" ? "연결됨" : r.edr) : "짝 없음"} tone={r?.edr === "active" ? "ok" : "warn"} />
        <Fact label="단말 차단" value={r?.stopped ? `${ago(r.stopped.at)}${r.stopped.by ? ` · ${r.stopped.by}` : ""}` : "없음"} tone={r?.stopped ? "bad" : undefined} />
      </div>
    </section>
  );
}

function Engine({ data }: { data: HooksData }) {
  const h = data.health;
  return (
    <div className="flex items-center gap-2">
      <span className="inline-flex h-7 items-center gap-2 rounded-md bg-white/[.05] px-2.5 text-[12px]">
        <i className={cx("size-1.5 rounded-full", !h ? "bg-dim" : h.ok ? "bg-teal" : "bg-block")} />
        <span className="font-mono text-fg/85">{host(h?.server ?? data.engine)}</span>
        <span className="text-dim">{!h ? "확인 중" : h.ok ? `${h.ms}ms` : h.error}</span>
      </span>
      <Tip label="다시 확인">
        <button onClick={() => send({ do: "check", server: data.engine })} className="grid size-7 cursor-pointer place-items-center rounded-md text-mut hover:bg-white/[.06] hover:text-fg">
          <RefreshCw className="size-3.5" />
        </button>
      </Tip>
    </div>
  );
}

/** Taking every setup out at once, whatever made it, pressed twice: the menu, then the item. */
function ResetAll({ data }: { data: HooksData }) {
  const busy = data.busy;
  if (busy === "reset") return <span className="flex items-center gap-1.5 text-[12px] text-mut"><Loader2 className="size-3.5 animate-spin" />지우는 중</span>;
  return (
    <DM.Root>
      <DM.Trigger asChild disabled={!!busy}>
        <button className="flex h-7 cursor-pointer items-center gap-1.5 rounded-md px-2 text-[12px] text-mut hover:bg-white/[.06] hover:text-fg disabled:opacity-40">
          <RotateCcw className="size-3.5" />초기화
        </button>
      </DM.Trigger>
      <DM.Portal>
        <DM.Content align="end" sideOffset={4} className="z-50 min-w-[180px] rounded-lg bg-[#1b1b1b] p-1 text-[12.5px] shadow-xl ring-1 ring-white/10">
          <DM.Item onSelect={() => send({ do: "reset" })}
            className="flex cursor-pointer items-center gap-2 rounded-md px-2 py-1.5 text-block outline-none data-[highlighted]:bg-block/15">
            <Trash2 className="size-3.5" />모두 지우기
            <span className="ml-auto flex items-center gap-1.5 text-dim tabular-nums">
              {data.installs.length}{data.installs.some((i) => i.at === "device") && <KeyRound className="size-3" />}
            </span>
          </DM.Item>
        </DM.Content>
      </DM.Portal>
    </DM.Root>
  );
}

function Installs({ data }: { data: HooksData }) {
  const busy = data.busy;
  return (
    <Panel title="설치" pad={false} aside={
      <span className="flex items-center gap-2">
        {data.installs.length > 0 && <ResetAll data={data} />}
        <span className="rounded-md bg-white/8 px-1.5 text-[11.5px] font-semibold text-mut tabular-nums">{data.installs.length}</span>
      </span>
    }>
      {data.installs.length === 0 ? (
        <div className="flex items-center gap-2 px-4 py-5 text-[12.5px] text-hold"><CircleAlert className="size-4" />설치 없음</div>
      ) : (
        <table className="w-full table-fixed text-[12.5px]">
          <thead className="text-left text-[11.5px] text-dim">
            <tr className="border-b border-white/[.06]">
              <th className="w-[124px] px-4 py-2 font-normal">범위</th>
              <th className="px-2 py-2 font-normal">위치</th>
              <th className="w-[190px] px-2 py-2 font-normal">엔진</th>
              <th className="w-[92px] px-2 py-2 font-normal">설치</th>
              <th className="w-[52px] px-2 py-2" />
            </tr>
          </thead>
          <tbody>
            {data.installs.map((i) => {
              const s = SCOPE[i.at];
              const working = busy === i.file;
              return (
                <tr key={i.file} className="border-b border-white/[.04] last:border-b-0 hover:bg-white/[.02]">
                  <td className="px-4 py-2.5"><span className="inline-flex items-center gap-1.5 font-medium"><s.icon className={cx("size-4", s.tone)} />{s.label}</span></td>
                  <td className="px-2 py-2.5"><Tip label={i.file}><span className="block truncate font-mono text-[12px] text-fg/85">{i.place}</span></Tip></td>
                  <td className="truncate px-2 py-2.5 font-mono text-[12px] text-mut">{host(i.engine)}</td>
                  <td className="px-2 py-2.5 text-mut tabular-nums">{ago(i.installed_at)}</td>
                  <td className="px-2 py-2.5 text-right">
                    {working ? <Loader2 className="ml-auto size-4 animate-spin text-mut" /> : (
                      <DM.Root>
                        <DM.Trigger asChild disabled={!!busy}>
                          <button className="grid size-7 cursor-pointer place-items-center rounded-md text-mut hover:bg-white/[.06] hover:text-fg disabled:opacity-40"><MoreHorizontal className="size-4" /></button>
                        </DM.Trigger>
                        <DM.Portal>
                          <DM.Content align="end" sideOffset={4} className="z-50 min-w-[140px] rounded-lg bg-[#1b1b1b] p-1 text-[12.5px] shadow-xl ring-1 ring-white/10">
                            <DM.Item onSelect={() => send({ do: "remove", file: i.file })}
                              className="flex cursor-pointer items-center gap-2 rounded-md px-2 py-1.5 text-block outline-none data-[highlighted]:bg-block/15">
                              <Trash2 className="size-3.5" />제거{i.at === "device" && <KeyRound className="ml-auto size-3 text-dim" />}
                            </DM.Item>
                          </DM.Content>
                        </DM.Portal>
                      </DM.Root>
                    )}
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      )}
    </Panel>
  );
}

const input = "h-9 w-full min-w-0 rounded-md bg-sunk px-3 font-mono text-[12.5px] text-fg ring-1 ring-white/[.08] outline-none placeholder:text-dim focus:ring-pick disabled:opacity-40";

function NewSetup({ data }: { data: HooksData }) {
  const [scope, setScope] = useState<Scope>("device");
  const [server, setServer] = useState(data.engine);
  const [key, setKey] = useState("");
  useEffect(() => setServer((s) => s || data.engine), [data.engine]);
  useEffect(() => { if (data.folder) setScope("folder"); }, [data.folder]);
  const busy = data.busy;
  const ready = !busy && server.trim() !== "" && (scope !== "folder" || !!data.folder);
  const h = data.health && data.health.server.replace(/\/$/, "") === server.trim().replace(/\/$/, "") ? data.health : null;
  return (
    <Panel title="새 설치">
      <div className="inline-flex rounded-lg bg-sunk p-0.5 ring-1 ring-white/[.06]">
        {(Object.keys(SCOPE) as Scope[]).map((k) => {
          const s = SCOPE[k];
          return (
            <button key={k} disabled={!!busy} onClick={() => setScope(k)}
              className={cx("inline-flex h-8 cursor-pointer items-center gap-1.5 rounded-md px-3 text-[12.5px] font-medium transition-colors disabled:cursor-default",
                scope === k ? "bg-white/[.09] text-fg shadow-sm" : "text-mut hover:text-fg")}>
              <s.icon className={cx("size-3.5", scope === k && s.tone)} />{s.label}
            </button>
          );
        })}
      </div>
      <div className="mt-3 flex flex-wrap gap-1.5">
        {SCOPE[scope].traits.map(([Icon, label]) => (
          <span key={label} className="inline-flex items-center gap-1.5 rounded-md bg-white/[.05] px-2 py-1 text-[11.5px] text-mut"><Icon className="size-3.5" />{label}</span>
        ))}
      </div>

      <div className="mt-4 grid grid-cols-[120px_minmax(0,1fr)] items-center gap-x-4 gap-y-3 text-[12.5px]">
        {scope === "folder" && (
          <>
            <span className="text-mut">폴더</span>
            <div className="flex gap-2">
              <div className={cx(input, "flex items-center truncate", !data.folder && "text-dim")}>{data.folder ?? "선택 안 됨"}</div>
              <button disabled={!!busy} onClick={() => send({ do: "pick" })}
                className="inline-flex h-9 shrink-0 cursor-pointer items-center gap-1.5 rounded-md bg-white/[.07] px-3 font-medium hover:bg-white/[.11] disabled:opacity-40">
                <FolderOpen className="size-3.5" />선택
              </button>
            </div>
          </>
        )}
        <span className="text-mut">엔진 주소</span>
        <div className="flex items-center gap-2">
          <input className={input} value={server} placeholder="https://engine.example" onChange={(e) => setServer(e.target.value)}
            onBlur={() => server.trim() && send({ do: "check", server: server.trim() })} />
          <span className={cx("inline-flex w-[92px] shrink-0 items-center gap-1 text-[12px]", !h ? "text-dim" : h.ok ? "text-teal" : "text-block")}>
            {!h ? "–" : h.ok ? <><Check className="size-3.5" />{h.ms}ms</> : <><CircleAlert className="size-3.5" />{h.error}</>}
          </span>
        </div>
        <span className="text-mut">WithHuman 키</span>
        {scope === "device"
          ? <span className="inline-flex h-9 items-center gap-1.5 text-dim"><Users className="size-3.5" />사용자별</span>
          : <input className={cx(input, "max-w-[420px]")} type="password" value={key} placeholder="whk_… (선택)" onChange={(e) => setKey(e.target.value)} />}
      </div>

      <div className="mt-5 flex items-center justify-end gap-3 border-t border-white/[.06] pt-4">
        {data.note && (
          <span className={cx("mr-auto inline-flex min-w-0 items-center gap-1.5 text-[12.5px]", data.note.ok ? "text-teal" : "text-block")}>
            {data.note.ok ? <Check className="size-4 shrink-0" /> : <CircleAlert className="size-4 shrink-0" />}
            <span className="truncate" title={data.note.text}>{data.note.text}</span>
          </span>
        )}
        <button disabled={!ready} onClick={() => send({ do: "install", scope, server: server.trim(), key: key.trim() })}
          className="inline-flex h-9 cursor-pointer items-center gap-1.5 rounded-md bg-pick px-4 text-[12.5px] font-semibold text-white transition-opacity hover:opacity-90 disabled:cursor-default disabled:opacity-40">
          {busy === "install" ? <Loader2 className="size-3.5 animate-spin" /> : scope === "device" && <KeyRound className="size-3.5" />}설치
        </button>
      </div>
    </Panel>
  );
}

export function HooksView({ data }: { data: HooksData }) {
  return (
    <TT.Provider>
      {/* Its own compositing layer: WebKit leaves a scrolling box in a frame unpainted without one. */}
      <div className="scroll h-full overflow-y-auto [transform:translateZ(0)]">
        <div className="mx-auto flex max-w-[980px] flex-col gap-4 px-6 py-5">
          <header className="flex items-center justify-between">
            <h1 className="text-[22px] leading-tight font-semibold tracking-[-0.01em]">훅</h1>
            <Engine data={data} />
          </header>
          <Machine data={data} />
          <Installs data={data} />
          <NewSetup data={data} />
        </div>
      </div>
    </TT.Provider>
  );
}
