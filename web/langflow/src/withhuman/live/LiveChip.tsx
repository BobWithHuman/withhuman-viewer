// The header's 실시간 chip while the open session is still running: the live dot, and, once the
// person has moved the canvas, a crosshair - pressing the chip follows the newest step again.

import ForwardedIconComponent from "@/components/common/genericIconComponent";
import ShadTooltip from "@/components/common/shadTooltipComponent";
import { cn } from "@/utils/utils";
import { follow, useLive } from "./follow";

export function LiveChip() {
  const { live, following } = useLive();
  if (!live) return null;
  return (
    <ShadTooltip content={following ? "따라가는 중" : "따라가기"} side="bottom">
      <button
        type="button"
        onClick={follow}
        data-testid="live-chip"
        aria-pressed={following}
        className={cn(
          "mr-2 flex h-6 shrink-0 items-center gap-1.5 rounded-full border px-2 text-xs font-medium transition-colors",
          following
            ? "border-transparent bg-accent-emerald text-accent-emerald-foreground"
            : "border-border bg-background text-muted-foreground hover:text-foreground",
        )}
      >
        <i className="h-1.5 w-1.5 animate-pulse rounded-full bg-accent-emerald-foreground" aria-hidden="true" />
        실시간
        {!following && <ForwardedIconComponent name="LocateFixed" className="h-3.5 w-3.5" aria-hidden="true" />}
      </button>
    </ShadTooltip>
  );
}
