import * as TT from "@radix-ui/react-tooltip";
import { ReactFlowProvider } from "@xyflow/react";
import { TriangleAlert } from "lucide-react";
import { StrictMode, useSyncExternalStore } from "react";
import { createRoot } from "react-dom/client";
import { DEMO, type RecordData } from "./data";
import { GraphView } from "./GraphView";
import "./index.css";

// The page the app shows in 기록: it waits for the app to hand it a session
// (`WHRecord.render(data)`). Opened on its own, it shows the demo.

declare global {
  interface Window {
    WHRecord: { render: (d: RecordData) => void };
  }
}

let current: RecordData = DEMO;
const subs = new Set<() => void>();
window.WHRecord = { render: (d) => { current = d; subs.forEach((f) => f()); } };
const useRecord = () => useSyncExternalStore((f) => { subs.add(f); return () => subs.delete(f); }, () => current);

function App() {
  const data = useRecord();
  const caught = data.steps.filter((s) => s.catch).length;
  const stopped = data.steps.filter((s) => s.verdict === "deny" || s.verdict === "rejected" || s.verdict === "review").length;
  return (
    <TT.Provider>
      <div className="flex h-full min-h-0 flex-col overflow-hidden">
        <header className="shrink-0 px-5 pt-4 pb-3">
          <h1 className="truncate text-[22px] leading-tight font-semibold tracking-[-0.01em]">"{data.session.title}"</h1>
          <div className="mt-1.5 flex items-center gap-2 text-[13px] text-mut">
            <span>{data.session.device}</span><span className="text-dim">|</span><span className="font-mono text-[12px]">세션 {data.session.id}</span>
            {caught > 0 && <><span className="text-dim">|</span><span className="inline-flex items-center gap-1 rounded-md bg-catch/15 px-1.5 text-[12px] font-semibold text-catch"><TriangleAlert className="size-3" />원인 {caught}</span></>}
            {stopped > 0 && <span className="inline-flex items-center gap-1 rounded-md bg-block/15 px-1.5 text-[12px] font-semibold text-block">막음·확인 {stopped}</span>}
          </div>
        </header>
        <div className="mx-4 mb-4 flex min-h-0 flex-1 overflow-hidden rounded-xl bg-bg ring-1 ring-white/[.05]">
          <ReactFlowProvider><GraphView data={data} /></ReactFlowProvider>
        </div>
      </div>
    </TT.Provider>
  );
}

createRoot(document.getElementById("root")!).render(<StrictMode><App /></StrictMode>);
