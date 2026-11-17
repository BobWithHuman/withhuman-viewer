// The kill switch: a machine's every tool call stopped, asked first in a dialog that names the
// machine, what stopping does, and why (kept with the stop); a stopped machine is let go again
// at once. The panel and the machine's sheet both use it.

import { Laptop, Loader2, OctagonX } from "lucide-react";
import { AlertDialog } from "radix-ui";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { useState } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { ago } from "@/kit";
import { cn } from "@/lib/utils";
import type { Machine } from "./model";

/** The switch as a button: 킬스위치, or 멈춤 풀기 on a stopped machine. */
export function KillSwitch({ m, now, busy, onStop, size = "sm" }: {
  m: Machine; now: number; busy?: boolean; onStop: (stopped: boolean, reason: string) => void; size?: "sm" | "xs";
}) {
  const [open, setOpen] = useState(false);
  const [why, setWhy] = useState("");
  if (m.stopped) {
    return (
      <Tooltip>
        <TooltipTrigger asChild>
          <Button size={size} className="shrink-0 gap-1.5 bg-deny text-white hover:bg-deny/85" onClick={() => onStop(false, "")}><OctagonX />멈춤 풀기</Button>
        </TooltipTrigger>
        <TooltipContent className="max-w-[280px]">
          <div className="flex flex-col gap-0.5 py-0.5">
            <span className="tnum">멈춤 · {ago(m.stopped.at, now)}{m.stopped.by ? ` · ${m.stopped.by}` : ""}{m.stopped.denied ? ` · 막힌 호출 ${m.stopped.denied}` : ""}</span>
            {m.stopped.reason && <span className="opacity-80">{m.stopped.reason}</span>}
          </div>
        </TooltipContent>
      </Tooltip>
    );
  }
  const stop = () => { onStop(true, why.trim()); setOpen(false); setWhy(""); };
  return (
    <AlertDialog.Root open={open} onOpenChange={(o) => { setOpen(o); if (!o) setWhy(""); }}>
      <Tooltip>
        <TooltipTrigger asChild>
          <AlertDialog.Trigger asChild>
            <Button size={size} variant="outline" disabled={!!m.edrOnly}
              className="shrink-0 gap-1.5 border-deny/50 text-deny hover:bg-deny/15 hover:text-deny"><OctagonX />킬스위치</Button>
          </AlertDialog.Trigger>
        </TooltipTrigger>
        <TooltipContent>{m.edrOnly ? "엔진에 없는 기기" : "이 기기의 모든 도구 호출 차단"}</TooltipContent>
      </Tooltip>
      <AlertDialog.Portal>
        <AlertDialog.Overlay className="fixed inset-0 z-50 bg-black/55 data-[state=open]:animate-in data-[state=open]:fade-in-0" />
        <AlertDialog.Content aria-describedby={undefined}
          className="fixed top-1/2 left-1/2 z-50 flex w-[400px] max-w-[calc(100vw-32px)] -translate-x-1/2 -translate-y-1/2 flex-col gap-4 rounded-xl border bg-popover p-5 shadow-2xl outline-none data-[state=open]:animate-in data-[state=open]:fade-in-0 data-[state=open]:zoom-in-95">
          <div className="flex items-center gap-2.5">
            <span className="grid size-9 shrink-0 place-items-center rounded-lg bg-deny/15 text-deny"><OctagonX className="size-[18px]" /></span>
            <div className="flex min-w-0 flex-col">
              <AlertDialog.Title className="text-[15px] font-semibold">킬스위치</AlertDialog.Title>
              <span className="truncate font-mono text-[12.5px] text-muted-foreground">{m.name}</span>
            </div>
          </div>
          <label className="flex flex-col gap-1.5 text-[12px] text-muted-foreground">사유
            <Input autoFocus value={why} placeholder="유출 의심 · 조사 중" onChange={(e) => setWhy(e.target.value)}
              onKeyDown={(e) => { if (e.key === "Enter" && why.trim()) stop(); }} />
          </label>
          <div className="flex justify-end gap-2">
            <AlertDialog.Cancel asChild><Button variant="ghost">취소</Button></AlertDialog.Cancel>
            <Button disabled={!why.trim() || busy} onClick={stop} className={cn("gap-1.5 bg-deny text-white hover:bg-deny/85")}>
              {busy ? <Loader2 className="animate-spin" /> : <OctagonX />}멈추기
            </Button>
          </div>
        </AlertDialog.Content>
      </AlertDialog.Portal>
    </AlertDialog.Root>
  );
}

/** The machines the kill switch stopped, managed in one place: a button that counts them and opens
 * them with when, by whom and why each was stopped and the calls refused since, each let go from
 * here or picked to look at. */
export function StoppedMachines({ machines, now, onPick, onRelease }: {
  machines: Machine[]; now: number; onPick: (id: string) => void; onRelease: (id: string) => void;
}) {
  const stopped = machines.filter((m) => m.stopped).sort((a, b) => (b.stopped!.at ?? 0) - (a.stopped!.at ?? 0));
  return (
    <Popover>
      <PopoverTrigger asChild>
        <Button variant="outline" size="sm" className={cn("gap-1.5", stopped.length && "border-deny/50 text-deny hover:bg-deny/15 hover:text-deny")}>
          <OctagonX />멈춘 기기<span className="tnum">{stopped.length}</span>
        </Button>
      </PopoverTrigger>
      <PopoverContent align="end" className="w-[420px] gap-0 p-0">
        <div className="flex items-center gap-1.5 border-b px-3 py-2.5 text-[12.5px] font-semibold"><OctagonX className="size-3.5 text-deny" />멈춘 기기<span className="tnum font-normal text-muted-foreground">{stopped.length}</span></div>
        <div className="scroll max-h-[360px] overflow-y-auto p-1">
          {!stopped.length && <div className="px-3 py-4 text-center text-[12.5px] text-muted-foreground">없음</div>}
          {stopped.map((m) => (
            <div key={m.id} className="flex items-start gap-2.5 rounded-md px-2 py-2 hover:bg-accent/50">
              <button onClick={() => onPick(m.id)} className="flex min-w-0 flex-1 flex-col gap-0.5 text-left">
                <span className="flex items-center gap-1.5 text-[13px] font-medium"><Laptop className="size-3.5 text-muted-foreground" strokeWidth={1.8} /><span className="truncate hover:underline">{m.name}</span></span>
                <span className="text-[12.5px] break-words">{m.stopped!.reason || <span className="text-muted-foreground">사유 없음</span>}</span>
                <span className="tnum text-[11px] text-muted-foreground">{ago(m.stopped!.at, now)}{m.stopped!.by ? ` · ${m.stopped!.by}` : ""}{m.stopped!.denied ? ` · 막힌 호출 ${m.stopped!.denied}` : ""}</span>
              </button>
              <Button size="xs" variant="outline" className="shrink-0" onClick={() => onRelease(m.id)}>풀기</Button>
            </div>
          ))}
        </div>
      </PopoverContent>
    </Popover>
  );
}
