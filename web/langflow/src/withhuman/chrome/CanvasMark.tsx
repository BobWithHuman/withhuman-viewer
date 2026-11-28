// The canvas while a session is on its way or arrived with nothing in it: a small mark in the
// middle instead of a blank page. While the flow is not there yet (the app is asked for the
// session), Langflow's loading mark (WithHuman's icon, pulsing); a flow with no node pulses the
// same for a moment (the app may still be filling it), then settles on an empty mark.

import { useEffect, useState } from "react";
import ForwardedIconComponent from "@/components/common/genericIconComponent";
import LoadingComponent from "@/components/common/loadingComponent";
import useFlowStore from "@/stores/flowStore";

const SETTLE = 2500;

export function CanvasMark() {
  const hasFlow = useFlowStore((s) => !!s.currentFlow);
  const empty = useFlowStore((s) => s.nodes.length === 0);
  const flowId = useFlowStore((s) => s.currentFlow?.id);
  const [settled, setSettled] = useState(false);

  useEffect(() => {
    setSettled(false);
    if (!hasFlow || !empty) return;
    const t = window.setTimeout(() => setSettled(true), SETTLE);
    return () => window.clearTimeout(t);
  }, [hasFlow, empty, flowId]);

  if (hasFlow && !empty) return null;
  return (
    <div className="pointer-events-none absolute inset-0 z-10 flex items-center justify-center" data-testid="withhuman-canvas-mark">
      {hasFlow && settled ? (
        <div className="flex flex-col items-center gap-2 text-muted-foreground">
          <ForwardedIconComponent name="Workflow" className="h-6 w-6" aria-hidden="true" />
          <span className="text-xs">단계 없음</span>
        </div>
      ) : (
        <LoadingComponent remSize={8} />
      )}
    </div>
  );
}

export default CanvasMark;

/** The app's pulsing mark on its own page (where Langflow would show its sign-in). */
export function WithhumanWait() {
  return (
    <div className="flex h-screen w-full items-center justify-center bg-background">
      <LoadingComponent remSize={12} />
    </div>
  );
}
