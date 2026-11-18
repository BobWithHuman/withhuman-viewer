// The developer's 훅 page, in 기록's frame and look: the WithHuman setups found on this machine,
// each with the engine it reports to, and a new setup. The app hands it what it found
// (`WHHooks.render(data)`) and does what is pressed (`__whHooksSend` in the app's window).

import { Check, Folder, FolderTree, Lock, Loader2, MonitorCog, Trash2, TriangleAlert } from "lucide-react";
import { useEffect, useState } from "react";
import { cx } from "./parts";

export type Scope = "device" | "all" | "folder";
export type Install = { at: Scope; place: string; file: string; engine: string | null; removable: boolean };
export type HooksData = {
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

const SCOPE: Record<Scope, { label: string; sub: string; tone: string; icon: typeof Folder }> = {
  device: { label: "기기 전체", sub: "관리자 · 다른 훅 막음", tone: "bg-teal/15 text-teal", icon: MonitorCog },
  all: { label: "모든 폴더", sub: "~/.claude", tone: "bg-act/15 text-act", icon: FolderTree },
  folder: { label: "이 폴더만", sub: "폴더 고르기", tone: "bg-white/8 text-fg/85", icon: Folder },
};

function Card({ title, count, children }: { title: string; count?: number; children: React.ReactNode }) {
  return (
    <section className="rounded-xl bg-panel p-4 ring-1 ring-white/[.06]">
      <h2 className="mb-3 flex items-center gap-2 text-[13px] font-semibold">
        {title}
        {count !== undefined && <span className="rounded-md bg-white/8 px-1.5 text-[11.5px] font-semibold text-mut tabular-nums">{count}</span>}
      </h2>
      {children}
    </section>
  );
}

function Row({ i, busy }: { i: Install; busy: string | null }) {
  const s = SCOPE[i.at];
  const working = busy === i.file;
  return (
    <div className="grid grid-cols-[88px_minmax(0,1fr)_minmax(0,200px)_72px] items-center gap-3 border-t border-white/[.06] py-2.5 first:border-t-0">
      <span className={cx("justify-self-start rounded-md px-1.5 py-0.5 text-[11.5px] font-semibold", s.tone)}>{s.label}</span>
      <span className="truncate font-mono text-[12px] text-fg/90" title={i.place}>{i.place}</span>
      <span className="truncate font-mono text-[12px] text-mut" title={i.engine ?? undefined}>{i.engine ?? "엔진 모름"}</span>
      <span className="justify-self-end">
        {i.removable ? (
          <button disabled={!!busy} onClick={() => send({ do: "remove", file: i.file })}
            className="inline-flex h-7 cursor-pointer items-center gap-1.5 rounded-md px-2 text-[12px] text-mut transition-colors hover:bg-block/15 hover:text-block disabled:cursor-default disabled:opacity-40">
            {working ? <Loader2 className="size-3.5 animate-spin" /> : <Trash2 className="size-3.5" />}제거
          </button>
        ) : (
          <span className="inline-flex items-center gap-1 text-[12px] text-dim"><Lock className="size-3.5" />관리자</span>
        )}
      </span>
    </div>
  );
}

function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return <label className="flex min-w-0 flex-1 flex-col gap-1.5 text-[12px] text-mut">{label}{children}</label>;
}
const input = "h-8 rounded-md bg-sunk px-2.5 font-mono text-[12.5px] text-fg ring-1 ring-white/[.08] outline-none placeholder:text-dim focus:ring-pick disabled:opacity-40";

export function HooksView({ data }: { data: HooksData }) {
  const [scope, setScope] = useState<Scope>("device");
  const [server, setServer] = useState(data.engine);
  const [key, setKey] = useState("");
  useEffect(() => setServer((s) => s || data.engine), [data.engine]);
  // A folder picked means a setup of that folder.
  useEffect(() => { if (data.folder) setScope("folder"); }, [data.folder]);
  const busy = data.busy;
  const ready = !busy && server.trim() !== "" && (scope !== "folder" || !!data.folder);
  return (
    // Its own compositing layer: WebKit leaves a scrolling box in a frame unpainted without one.
    <div className="scroll h-full overflow-y-auto [transform:translateZ(0)]">
      <div className="mx-auto flex max-w-[920px] flex-col gap-4 px-6 py-5">
        <header className="flex items-baseline gap-3">
          <h1 className="text-[22px] leading-tight font-semibold tracking-[-0.01em]">훅</h1>
          <span className="inline-flex items-baseline gap-1.5 text-[12.5px] text-mut">기기<b className="font-mono text-[12px] font-medium text-fg/85">{data.device ?? "없음"}</b></span>
        </header>

        <Card title="설치됨" count={data.installs.length}>
          {data.installs.length === 0
            ? <div className="flex items-center gap-2 py-1 text-[12.5px] text-hold"><TriangleAlert className="size-4" />없음</div>
            : data.installs.map((i) => <Row key={i.file} i={i} busy={busy} />)}
        </Card>

        <Card title="새로 설치">
          <div className="grid grid-cols-3 gap-2">
            {(Object.keys(SCOPE) as Scope[]).map((k) => {
              const s = SCOPE[k];
              const on = scope === k;
              const sub = k === "folder" && data.folder ? data.folder : s.sub;
              return (
                <button key={k} disabled={!!busy}
                  onClick={() => (k === "folder" ? send({ do: "pick" }) : setScope(k))}
                  className={cx("flex min-w-0 cursor-pointer flex-col items-start gap-1 rounded-lg px-3 py-2.5 text-left ring-1 transition-colors disabled:cursor-default",
                    on ? "bg-pick/20 ring-pick" : "bg-sunk ring-white/[.06] hover:bg-white/[.04]")}>
                  <span className="flex items-center gap-1.5 text-[13px] font-semibold"><s.icon className="size-4 text-mut" />{s.label}</span>
                  <span className="w-full truncate text-[11.5px] text-mut" title={sub}>{sub}</span>
                </button>
              );
            })}
          </div>
          <div className="mt-3 flex gap-2">
            <Field label="엔진 주소"><input className={input} value={server} placeholder="https://engine.example" onChange={(e) => setServer(e.target.value)} /></Field>
            <Field label="키 (선택)">
              <input className={input} type="password" value={key} placeholder={scope === "device" ? "사용자마다" : "whk_…"} disabled={scope === "device"} onChange={(e) => setKey(e.target.value)} />
            </Field>
          </div>
          <div className="mt-3 flex items-center gap-3">
            <button disabled={!ready} onClick={() => send({ do: "install", scope, server: server.trim(), key: key.trim() })}
              className="inline-flex h-8 cursor-pointer items-center gap-1.5 rounded-md bg-pick px-3 text-[12.5px] font-semibold text-white transition-opacity hover:opacity-90 disabled:cursor-default disabled:opacity-40">
              {busy === "install" && <Loader2 className="size-3.5 animate-spin" />}훅 설치
            </button>
            {data.note && (
              <span className={cx("inline-flex min-w-0 items-center gap-1.5 text-[12.5px]", data.note.ok ? "text-teal" : "text-block")}>
                {data.note.ok ? <Check className="size-4 shrink-0" /> : <TriangleAlert className="size-4 shrink-0" />}
                <span className="truncate" title={data.note.text}>{data.note.text}</span>
              </span>
            )}
          </div>
        </Card>
      </div>
    </div>
  );
}
