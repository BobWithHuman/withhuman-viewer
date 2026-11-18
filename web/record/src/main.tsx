import * as TT from "@radix-ui/react-tooltip";
import { ReactFlowProvider } from "@xyflow/react";
import { TriangleAlert } from "lucide-react";
import { StrictMode, useEffect, useState, useSyncExternalStore } from "react";
import { createRoot } from "react-dom/client";
import { DEMO, type RecordData } from "./data";
import { GraphView } from "./GraphView";
import { HooksView, type HooksData } from "./hooks";
import { ONLY, cx, flagged, type Only } from "./parts";
import "./index.css";

// The page the app shows in 기록: it waits for the app to hand it a session
// (`WHRecord.render(data)`). Opened on its own, it shows the demo. The developer's 훅 page is
// this page too, handed what the app found instead (`WHHooks.render(data)`).

declare global {
  interface Window {
    WHRecord: { render: (d: RecordData) => void };
    WHHooks: { render: (d: HooksData) => void };
  }
}

let current: RecordData = DEMO;
const subs = new Set<() => void>();
window.WHRecord = { render: (d) => { current = d; subs.forEach((f) => f()); } };
const useRecord = () => useSyncExternalStore((f) => { subs.add(f); return () => subs.delete(f); }, () => current);
let hooks: HooksData | null = null;
window.WHHooks = { render: (d) => { hooks = d; subs.forEach((f) => f()); } };
const useHooks = () => useSyncExternalStore((f) => { subs.add(f); return () => subs.delete(f); }, () => hooks);

function Root() {
  const h = useHooks();
  return h ? <HooksView data={h} /> : <App />;
}

function App() {
  const data = useRecord();
  // A chip picked holds the graph and the timeline to its steps; picked again, lets go.
  const [only, setOnly] = useState<Only | null>(null);
  useEffect(() => setOnly(null), [data]);
  const chip = (k: Only, cls: string) => {
    const n = data.steps.filter(ONLY[k].is).length;
    return n > 0 && (
      <button key={k} onClick={() => setOnly(only === k ? null : k)}
        className={cx("inline-flex cursor-pointer items-center gap-1 rounded-md px-1.5 text-[12px] font-semibold transition-colors", cls, only === k ? "ring-1 ring-current" : "opacity-90 hover:opacity-100")}>
        {k === "outside" && <TriangleAlert className="size-3" />}{ONLY[k].label} {n}
      </button>
    );
  };
  return (
    <TT.Provider>
      <div className="flex h-full min-h-0 flex-col overflow-hidden">
        <header className="shrink-0 px-5 pt-4 pb-3">
          <div className="flex min-w-0 items-center gap-2">
            <h1 className="truncate text-[22px] leading-tight font-semibold tracking-[-0.01em]">{data.session.quote ? `"${data.session.title}"` : data.session.title}</h1>
            {!!data.session.others?.length && (
              <span title={data.session.others.join("\n")} className="shrink-0 cursor-default rounded-md bg-white/8 px-1.5 py-0.5 text-[12px] font-semibold text-mut tabular-nums">+{data.session.others.length}</span>
            )}
          </div>
          <div className="mt-1.5 flex items-center gap-2 text-[13px] text-mut">
            <span>{data.session.device}</span><span className="text-dim">|</span><span className="font-mono text-[12px]">세션 {data.session.id}</span>
            {data.steps.some(flagged) && <span className="text-dim">|</span>}
            {chip("outside", "bg-catch/15 text-catch")}
            {chip("deny", "bg-block/15 text-block")}
            {chip("ask", "bg-hold/15 text-hold")}
          </div>
        </header>
        <div className="mx-4 mb-4 flex min-h-0 flex-1 overflow-hidden rounded-xl bg-bg ring-1 ring-white/[.05]">
          <ReactFlowProvider><GraphView data={data} only={only} setOnly={setOnly} /></ReactFlowProvider>
        </div>
      </div>
    </TT.Provider>
  );
}

createRoot(document.getElementById("root")!).render(<StrictMode><Root /></StrictMode>);
