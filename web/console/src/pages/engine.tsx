// The personal app's own engine, in the parts of `./mac` (OpenClaw's macOS look, dark inside the
// app), shown in the settings sheet: its parts as they answer; how Calls are judged (the rules on
// this machine, or Jev with its key and model), applied by starting the engine again; the try;
// and taking the app off this machine. The app (`src/ui/settings.rs`, `OwnSheet`) hands it the
// engine's state and does what is pressed.

import { Cpu, FlaskConical, ListChecks, Loader2, ScrollText, Trash2 } from "lucide-react";
import { useEffect, useState } from "react";
import { send as sendTo } from "@/bridge";
import appIcon from "../../../../icons/128x128@2x.png";
import { Badge, Bordered, Choice, Field, FormGrid, FormLabel, LinkButton, MAC_STYLE, MCard, NATIVE, Prominent, Section, StateMark, v, type Status } from "./mac";

type Data = {
  parts: { name: string; up: boolean }[];
  /** Where the hooks send their events. */
  url: string;
  has_key: boolean;
  /** Calls go to Jev now. */
  jev: boolean;
  models: string[];
  model: string;
  /** What the last thing pressed ended with. */
  status: Status;
};

const DEMO: Data = {
  parts: ["PostgreSQL", "Decision", "Generator", "Collector"].map((name) => ({ name, up: true })),
  url: "http://127.0.0.1:47380", has_key: true, jev: false, models: ["jev-1.14.0", "jev-latest"], model: "jev-1.14.0",
  status: { state: "idle", text: null },
};

const send = (m: Record<string, unknown>) => sendTo("engine", m);

export default function Engine({ data }: { data: unknown }) {
  const d = (data as Data | null) ?? DEMO;
  const [jev, setJev] = useState(d.jev);
  const [key, setKey] = useState("");
  const [model, setModel] = useState(d.model);
  const [armed, setArmed] = useState(false);
  useEffect(() => setJev(d.jev), [d.jev]);
  useEffect(() => setModel(d.model), [d.model]);
  const busy = d.status.state === "busy";
  const up = d.parts.filter((p) => p.up).length;
  const changed = jev !== d.jev || key.trim() !== "" || model !== d.model;
  const ready = !busy && changed && (!jev || d.has_key || key.trim() !== "");

  return (
    <div className="mac mac-dark flex h-screen flex-col" style={{ background: v("bg"), color: v("fg") }}>
      <style>{MAC_STYLE}</style>
      <div className="scroll min-h-0 flex-1 overflow-y-auto">
        <div className="flex flex-col gap-5 px-6 pt-5 pb-4">
          <div className="flex items-center gap-3">
            <img src={appIcon} alt="" className="size-12" />
            <div className="flex min-w-0 flex-1 flex-col">
              <h1 className="text-[20px] font-bold tracking-[-0.02em]">이 기기 엔진</h1>
              <span className="font-mono text-[11.5px]" style={{ color: v("fg2") }}>{d.url.replace(/^https?:\/\//, "")}</span>
            </div>
            <Badge tone={up === d.parts.length ? "green" : "orange"}>{up === d.parts.length ? "실행 중" : `${up}/${d.parts.length}`}</Badge>
          </div>

          <Section title="엔진" aside={<LinkButton onClick={() => send({ do: "logs" })}><ScrollText className="size-3.5" />로그</LinkButton>}>
            <MCard pad={14} gap={10}>
              <div className="grid grid-cols-2 gap-x-4 gap-y-2.5">
                {d.parts.map((p) => (
                  <span key={p.name} className="flex items-center gap-2.5 text-[13px] font-medium">
                    <StateMark state={p.up ? "ok" : busy ? "busy" : "err"} />{p.name}
                  </span>
                ))}
              </div>
            </MCard>
          </Section>

          <Section title="판정">
            <MCard pad={8} gap={4}>
              <Choice icon={<ListChecks className="size-[19px]" strokeWidth={2.2} />} title="규칙" badge="로컬" sub="이 기기 안에서만" selected={!jev} onClick={() => setJev(false)} />
              <Choice icon={<Cpu className="size-[19px]" strokeWidth={2.2} />} title="Jev" sub="호출 내용 → api.typesafe.ai · 원인 추적" selected={jev} onClick={() => setJev(true)} />
              {jev && (
                <div className="px-[10px] pt-2 pb-1">
                  <FormGrid label={64}>
                    <FormLabel>API 키</FormLabel>
                    <Field secure value={key} onChange={setKey} placeholder={d.has_key ? "저장된 키 유지" : "jev_…"} disabled={busy} />
                    <span />
                    <FormLabel>모델</FormLabel>
                    <select style={{ ...NATIVE, justifySelf: "start" }} value={model} onChange={(e) => setModel(e.target.value)}>
                      {(d.models.includes(model) ? d.models : [...d.models, model]).map((m) => <option key={m} value={m}>{m}</option>)}
                    </select>
                    <span />
                  </FormGrid>
                </div>
              )}
            </MCard>
          </Section>

          <Section title="기타">
            <MCard pad={8} gap={2}>
              <Choice indicator={false} icon={<FlaskConical className="size-[19px]" strokeWidth={2.2} />} title="체험 세션" sub="외부 문서 → 숨은 지시 → curl | sh"
                trailing={<Bordered disabled={busy} onClick={() => send({ do: "try" })}>실행</Bordered>} />
              <Choice indicator={false} icon={<Trash2 className="size-[19px]" strokeWidth={2.2} style={{ color: v("red") }} />} title="모두 제거" sub="훅 · 엔진 · 기록 · 키체인 · 설정"
                trailing={<LinkButton tone="red" disabled={busy} onClick={() => (armed ? send({ do: "remove" }) : setArmed(true))}>{armed ? "제거 확인" : "제거…"}</LinkButton>} />
            </MCard>
          </Section>
        </div>
      </div>
      <footer className="flex shrink-0 items-center gap-2 border-t px-6 py-3" style={{ borderColor: v("line") }}>
        <span className="mr-auto inline-flex min-w-0 items-center gap-1.5 text-[12px]" style={{ color: d.status.state === "err" ? v("orange") : v("fg2") }}>
          {d.status.state !== "idle" && <StateMark state={d.status.state} />}<span className="truncate">{d.status.text}</span>
        </span>
        <Bordered onClick={() => send({ do: "close" })}>닫기</Bordered>
        <Prominent disabled={!ready} onClick={() => send({ do: "apply", jev, key: key.trim(), model })}>{busy && <Loader2 className="size-3.5 animate-spin" />}적용 · 다시 시작</Prominent>
      </footer>
    </div>
  );
}
