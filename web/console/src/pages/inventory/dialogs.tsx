// What the page asks before it acts: the admin API's connection. The page sends what is filled
// in to the app and shows the app's answer here.

import { Loader2, PlugZap } from "lucide-react";
import { Dialog } from "radix-ui";
import { useState, type ReactNode } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Chip } from "./marks";

export type Ask = { kind: "admin" };

function Field({ label, children }: { label: string; children: ReactNode }) {
  return <label className="flex flex-col gap-1.5 text-[12px] text-muted-foreground">{label}{children}</label>;
}

export function Asks({ admin, demo, busy, error, onSend, onClose }: {
  admin: { api: string; has_token: boolean }; demo: boolean; busy: boolean; error: string | null;
  onSend: (message: Record<string, unknown>) => void; onClose: () => void;
}) {
  const [api, setApi] = useState(admin.api);
  const [token, setToken] = useState("");
  const ready = api.trim() !== "";
  const submit = () => onSend({ type: "admin", api: api.trim(), token });

  return (
    <Dialog.Root open onOpenChange={(open) => { if (!open && !busy) onClose(); }}>
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 z-50 bg-black/50 data-[state=open]:animate-in data-[state=open]:fade-in-0" />
        <Dialog.Content aria-describedby={undefined}
          className="fixed top-1/2 left-1/2 z-50 flex w-[420px] max-w-[calc(100vw-32px)] -translate-x-1/2 -translate-y-1/2 flex-col gap-4 rounded-xl border bg-popover p-5 shadow-2xl outline-none data-[state=open]:animate-in data-[state=open]:fade-in-0 data-[state=open]:zoom-in-95">
          <div className="flex items-center gap-2.5">
            <span className="grid size-8 shrink-0 place-items-center rounded-lg bg-white/8 text-foreground"><PlugZap className="size-4" /></span>
            <Dialog.Title className="text-[15px] font-semibold">엔진 관리 API</Dialog.Title>
          </div>

          <div className="flex flex-wrap gap-1.5">
            <Chip>키체인 저장</Chip>
            {admin.has_token && <Chip className="bg-allow/15 text-allow">토큰 저장됨</Chip>}
            {demo && <Chip className="bg-catch/15 text-catch">데모 · 앱 안에서만</Chip>}
          </div>

          <Field label="주소"><Input value={api} placeholder="http://127.0.0.1:8787" onChange={(e) => setApi(e.target.value)} /></Field>
          <Field label="관리 토큰">
            <Input type="password" value={token} placeholder={admin.has_token ? "저장된 토큰 유지" : "WITHHUMAN_ADMIN_TOKEN"} onChange={(e) => setToken(e.target.value)}
              onKeyDown={(e) => { if (e.key === "Enter" && ready && !busy) submit(); }} />
          </Field>

          {error && <p className="rounded-lg bg-deny/10 px-3 py-2 text-[12.5px] text-deny">{error}</p>}

          <div className="flex justify-end gap-2">
            <Button variant="ghost" disabled={busy} onClick={onClose}>취소</Button>
            <Button disabled={!ready || busy} onClick={submit}>{busy && <Loader2 className="animate-spin" />}저장</Button>
          </div>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
