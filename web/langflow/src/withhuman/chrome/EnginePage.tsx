// Settings › 엔진, in Langflow's settings parts: the app's own engine as its parts answer, its logs;
// how Calls are judged (this machine's rules, or Jev with its key and model), applied by starting
// the engine again; the try; and taking the app off this machine. Same messages as web/record's
// EngineView: `send({ page: "engine", do: ... })`, handled by the app (src/ui/settings.rs). When the
// engine is not the app's own, the app's own settings sheet holds it: one row asks for it.

import { useEffect, useState } from "react";
import ForwardedIconComponent from "@/components/common/genericIconComponent";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import SettingsPageHeader from "@/pages/SettingsPage/components/SettingsPageHeader";
import { type EngineData, send as sendUp, useWithHuman } from "../bridge";
import { Dot, Loading, Row, Section, Spin, useAppPage } from "./settings-parts";

const send = (m: Record<string, unknown>) => sendUp({ page: "engine", ...m });

function Own({ d }: { d: EngineData }) {
  const [jev, setJev] = useState(d.jev);
  const [key, setKey] = useState("");
  const [model, setModel] = useState(d.model);
  const [armed, setArmed] = useState(false);
  useEffect(() => setJev(d.jev), [d.jev]);
  useEffect(() => setModel(d.model), [d.model]);
  const busy = d.status.state === "busy";
  const up = d.parts.filter((p) => p.up).length;
  const all = up === d.parts.length;
  const changed = jev !== d.jev || key.trim() !== "" || model !== d.model;
  const ready = !busy && changed && (!jev || d.has_key || key.trim() !== "");
  const models = d.models.includes(model) ? d.models : [...d.models, model];
  return (
    <div className="flex w-full flex-col gap-6 pb-8">
      <Section
        title={
          <>
            이 기기 엔진
            <Badge variant={all ? "successStatic" : "errorStatic"} size="tag" className="tabular-nums">
              {all ? "실행 중" : `${up}/${d.parts.length}`}
            </Badge>
          </>
        }
        action={
          <Button variant="outline" size="md" onClick={() => send({ do: "logs" })}>
            <ForwardedIconComponent name="ScrollText" className="h-4 w-4" />
            로그
          </Button>
        }
      >
        <Row
          icon={<ForwardedIconComponent name="Server" className="h-4 w-4 shrink-0 text-muted-foreground" />}
          label={<span className="font-mono">{d.url.replace(/^https?:\/\//, "")}</span>}
        />
        {d.parts.map((p) => (
          <Row
            key={p.name}
            icon={<Dot tone={p.up ? "ok" : "err"} className="mx-[5px]" />}
            label={p.name}
            control={
              p.up ? (
                <span className="text-xs text-muted-foreground">실행 중</span>
              ) : busy ? (
                <Spin />
              ) : (
                <span className="text-xs text-destructive">멈춤</span>
              )
            }
          />
        ))}
      </Section>

      <Section
        title="판정"
        action={
          <>
            {d.status.text && (
              <span className={d.status.state === "err" ? "max-w-[18rem] truncate text-xs text-destructive" : "max-w-[18rem] truncate text-xs text-muted-foreground"} title={d.status.text}>
                {d.status.text}
              </span>
            )}
            <Button size="md" disabled={!ready} loading={busy} onClick={() => send({ do: "apply", jev, key: key.trim(), model })}>
              적용 · 다시 시작
            </Button>
          </>
        }
      >
        <Row
          label="Jev"
          sub={jev ? "호출 → api.typesafe.ai · 원인 추적" : "이 기기 규칙"}
          control={<Switch checked={jev} onCheckedChange={setJev} disabled={busy} aria-label="Jev" />}
        />
        {jev && (
          <>
            <Row
              label="API 키"
              chip={d.has_key ? <Badge variant="successStatic" size="tag">저장됨</Badge> : undefined}
              control={
                <Input type="password" value={key} onChange={(e) => setKey(e.target.value)} placeholder={d.has_key ? "저장된 키 유지" : "jev_…"} disabled={busy} className="w-[260px]" />
              }
            />
            <Row
              label="모델"
              control={
                <Select value={model} onValueChange={setModel} disabled={busy}>
                  <SelectTrigger className="w-[260px]" aria-label="모델">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    {models.map((m) => (
                      <SelectItem key={m} value={m}>
                        {m}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              }
            />
          </>
        )}
      </Section>

      <Section title="체험">
        <Row
          icon={<ForwardedIconComponent name="FlaskConical" className="h-4 w-4 shrink-0 text-muted-foreground" />}
          label="체험 세션"
          sub="외부 문서 → 숨은 지시 → curl | sh"
          control={
            <Button variant="outline" size="md" disabled={busy} onClick={() => send({ do: "try" })}>
              실행
            </Button>
          }
        />
      </Section>

      <Section title="위험" danger>
        <Row
          icon={<ForwardedIconComponent name="Trash2" className="h-4 w-4 shrink-0 text-destructive" />}
          label="모두 제거"
          sub="훅 · 엔진 · 기록 · 키체인 · 설정"
          control={
            <Button variant="destructive" size="md" disabled={busy} onClick={() => (armed ? send({ do: "remove" }) : setArmed(true))}>
              {armed ? "제거 확인" : "제거"}
            </Button>
          }
        />
      </Section>
    </div>
  );
}

/** The engine is not the app's own: its settings are the app's sheet. */
function Elsewhere({ label, up }: { label: string; up: boolean }) {
  return (
    <div className="flex w-full flex-col gap-6 pb-8">
      <Section title="엔진">
        <Row
          icon={<Dot tone={up ? "ok" : "err"} className="mx-[5px]" />}
          label={label}
          control={
            <Button variant="outline" size="md" onClick={() => sendUp({ do: "settings" })}>
              앱 설정
            </Button>
          }
        />
      </Section>
    </div>
  );
}

export default function EnginePage() {
  const { record, views } = useWithHuman();
  const shell = record?.shell;
  const own = !!shell?.own;
  return (
    <div className="flex w-full flex-col gap-6">
      <SettingsPageHeader title="엔진" description="" />
      {!shell ? <Loading /> : own ? <OwnPage d={views.engine ?? null} /> : <Elsewhere label={shell.engine.label} up={shell.engine.up} />}
    </div>
  );
}

/** Only the app's own engine's page tells the app it is open (its data comes while it is). */
function OwnPage({ d }: { d: EngineData | null }) {
  useAppPage("settings");
  return d ? <Own d={d} /> : <Loading />;
}
