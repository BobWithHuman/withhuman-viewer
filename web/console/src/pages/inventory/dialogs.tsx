// What the page asks before it acts: a key to issue, a key to revoke or a person to stop (both
// confirmed in an alert dialog, with who does it and why, for the record), the key just issued,
// shown this once, and the admin API's connection. The page sends what is filled in to the app
// and shows the app's answer here.

import { Ban, Check, Copy, KeyRound, Loader2, OctagonX, PlugZap } from "lucide-react";
import { AlertDialog, Dialog } from "radix-ui";
import { useState, type ReactNode } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { Person } from "@/kit";
import { cn } from "@/lib/utils";
import { Chip } from "./marks";

export type Ask =
  | { kind: "issue"; email: string }
  | { kind: "revoke"; key_id: string; email: string }
  | { kind: "stop"; email: string; keys: number }
  | { kind: "issued"; email: string; key_id: string; key: string }
  | { kind: "admin" };

function Field({ label, children }: { label: string; children: ReactNode }) {
  return <label className="flex flex-col gap-1.5 text-[12px] text-muted-foreground">{label}{children}</label>;
}

export function Asks({ ask, actor, admin, demo, busy, error, onSend, onCopy, onClose }: {
  ask: Ask; actor: string; admin: { api: string; has_token: boolean }; demo: boolean; busy: boolean; error: string | null;
  onSend: (message: Record<string, unknown>) => void; onCopy: (text: string) => void; onClose: () => void;
}) {
  const [email, setEmail] = useState("email" in ask ? ask.email : "");
  const [who, setWho] = useState(actor);
  const [why, setWhy] = useState("");
  const [api, setApi] = useState(admin.api);
  const [token, setToken] = useState("");
  const [copied, setCopied] = useState(false);

  const destructive = ask.kind === "revoke" || ask.kind === "stop";
  // Both dialogs have the same parts; the alert one cannot be dismissed by pressing outside it.
  const D = (destructive ? AlertDialog : Dialog) as typeof Dialog;
  const title = { issue: "키 발급", revoke: "키 회수", stop: "사용자 멈추기", issued: "키 발급됨", admin: "엔진 관리 API" }[ask.kind];
  const Icon = { issue: KeyRound, revoke: Ban, stop: OctagonX, issued: Check, admin: PlugZap }[ask.kind];
  const ready = ask.kind === "admin" ? api.trim() !== ""
    : email.trim() !== "" && who.trim() !== "" && (!destructive || why.trim() !== "");
  const submit = () => {
    if (ask.kind === "issue") onSend({ type: "issue", email: email.trim(), actor: who.trim(), note: why.trim() });
    if (ask.kind === "revoke") onSend({ type: "revoke_key", key_id: ask.key_id, email: ask.email, actor: who.trim(), reason: why.trim() });
    if (ask.kind === "stop") onSend({ type: "stop", email: ask.email, actor: who.trim(), reason: why.trim() });
    if (ask.kind === "admin") onSend({ type: "admin", api: api.trim(), token });
  };

  return (
    <D.Root open onOpenChange={(open) => { if (!open && !busy) onClose(); }}>
      <D.Portal>
        <D.Overlay className="fixed inset-0 z-50 bg-black/50 data-[state=open]:animate-in data-[state=open]:fade-in-0" />
        <D.Content aria-describedby={undefined} onOpenAutoFocus={ask.kind === "issued" ? (e) => e.preventDefault() : undefined}
          className="fixed top-1/2 left-1/2 z-50 flex w-[420px] max-w-[calc(100vw-32px)] -translate-x-1/2 -translate-y-1/2 flex-col gap-4 rounded-xl border bg-popover p-5 shadow-2xl outline-none data-[state=open]:animate-in data-[state=open]:fade-in-0 data-[state=open]:zoom-in-95">
          <div className="flex items-center gap-2.5">
            <span className={cn("grid size-8 shrink-0 place-items-center rounded-lg", destructive ? "bg-deny/15 text-deny" : ask.kind === "issued" ? "bg-allow/15 text-allow" : "bg-white/8 text-foreground")}><Icon className="size-4" strokeWidth={1.9} /></span>
            <D.Title className="text-[15px] font-semibold">{title}</D.Title>
          </div>

          {"email" in ask && ask.kind !== "issue" && (
            <div className="flex min-w-0 items-center gap-2 rounded-lg bg-white/5 px-3 py-2.5 text-[13px]">
              <Person name={ask.email} size={22} />
              <span className="truncate">{ask.email}</span>
              {"key_id" in ask && <code className="ml-auto shrink-0 font-mono text-[12px] text-muted-foreground">{ask.key_id}</code>}
              {ask.kind === "stop" && <span className="tnum ml-auto shrink-0 text-[12px] text-muted-foreground">활성 키 {ask.keys}</span>}
            </div>
          )}

          {ask.kind === "issued" && (
            <div className="flex items-center gap-2 rounded-lg border bg-background px-3 py-2">
              <code className="min-w-0 flex-1 font-mono text-[12px] break-all select-all">{ask.key}</code>
              <Tooltip>
                <TooltipTrigger asChild>
                  <Button variant="ghost" size="icon-sm" aria-label="복사" onClick={() => { onCopy(ask.key); setCopied(true); }}>
                    {copied ? <Check className="text-allow" /> : <Copy />}
                  </Button>
                </TooltipTrigger>
                <TooltipContent>{copied ? "복사됨" : "복사"}</TooltipContent>
              </Tooltip>
            </div>
          )}

          <div className="flex flex-wrap gap-1.5">
            {ask.kind === "issue" && <Chip>발급 직후 1회 표시</Chip>}
            {ask.kind === "revoke" && <><Chip className="bg-deny/15 text-deny">이 키의 호출 차단</Chip><Chip>되돌릴 수 없음</Chip></>}
            {ask.kind === "stop" && <><Chip className="bg-deny/15 text-deny">모든 호출 차단</Chip><Chip>키 전부 회수</Chip><Chip>킬스위치 기록</Chip></>}
            {ask.kind === "issued" && <><Chip className="bg-catch/15 text-catch">1회 표시</Chip><Chip><code className="font-mono">configure.py --key</code></Chip></>}
            {ask.kind === "admin" && <><Chip>키체인 저장</Chip>{admin.has_token && <Chip className="bg-allow/15 text-allow">토큰 저장됨</Chip>}{demo && <Chip className="bg-catch/15 text-catch">데모 · 앱 안에서만</Chip>}</>}
          </div>

          {ask.kind === "issue" && (
            <Field label="사용자 이메일"><Input type="email" autoFocus={!ask.email} value={email} placeholder="name@example.com" onChange={(e) => setEmail(e.target.value)} /></Field>
          )}
          {ask.kind !== "issued" && ask.kind !== "admin" && (
            <>
              <Field label="실행자"><Input value={who} onChange={(e) => setWho(e.target.value)} /></Field>
              <Field label={destructive ? "사유 (필수)" : "메모"}>
                <Input autoFocus={destructive || !!email} value={why} placeholder={destructive ? "main 강제 푸시" : "신규 입사자"} onChange={(e) => setWhy(e.target.value)}
                  onKeyDown={(e) => { if (e.key === "Enter" && ready && !busy) submit(); }} />
              </Field>
            </>
          )}
          {ask.kind === "admin" && (
            <>
              <Field label="주소"><Input value={api} placeholder="http://127.0.0.1:8787" onChange={(e) => setApi(e.target.value)} /></Field>
              <Field label="관리 토큰"><Input type="password" value={token} placeholder={admin.has_token ? "저장된 토큰 유지" : "WITHHUMAN_ADMIN_TOKEN"} onChange={(e) => setToken(e.target.value)} /></Field>
            </>
          )}

          {error && <p className="rounded-lg bg-deny/10 px-3 py-2 text-[12.5px] text-deny">{error}</p>}

          <div className="flex justify-end gap-2">
            {ask.kind === "issued" ? <Button onClick={onClose}>닫기</Button> : (
              <>
                <Button variant="ghost" disabled={busy} onClick={onClose}>취소</Button>
                <Button variant={destructive ? "destructive" : "default"} disabled={!ready || busy} onClick={submit}>
                  {busy && <Loader2 className="animate-spin" />}{ask.kind === "admin" ? "저장" : title}
                </Button>
              </>
            )}
          </div>
        </D.Content>
      </D.Portal>
    </D.Root>
  );
}
