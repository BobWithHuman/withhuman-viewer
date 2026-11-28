// Keeps Langflow in step with what the app renders:
// - the sessions list changed: Langflow's flow and project queries are refetched;
// - more of the open session (same id, new steps): the canvas is set again in place, the
//   viewport kept;
// - another session the page did not ask for (the app opened it): the page goes to its canvas;
// - a session's canvas opened: the record panel opens on `focus` (or the last refused Call) with
//   its way lit, framed once Langflow has fitted the canvas.
// Imported after Langflow's entry (main.tsx), so its stores and query client exist.

import { cloneDeep } from "lodash";
import { queryClient } from "@/contexts";
import useFlowStore from "@/stores/flowStore";
import useFlowsManagerStore from "@/stores/flowsManagerStore";
import { processFlows } from "@/utils/reactflowUtils";
import { current, subscribe } from "../bridge";
import { useRecordPanel } from "../detail/store";
import { router } from "../router";
import { openFlow, sessions, settle, wasAsked } from "./sessions";

const signature = (v: unknown) => JSON.stringify(v);

let listSig = signature(sessions());
let openId = current().record?.session.id ?? null;
let graphSig: string | null = null;

const graphOf = (flow: { data: unknown } | undefined) => signature(flow?.data ?? null);

function refetchLists() {
  for (const key of ["useGetRefreshFlowsQuery", "useGetFolders", "useGetFolder", "useGetFlowsMCP"])
    void queryClient.invalidateQueries({ queryKey: [key] });
}

/** The open session's newer graph onto the canvas, as Langflow's own load does, without a fit. */
function applyInPlace() {
  const conv = openFlow();
  if (!conv) return;
  const sig = graphOf(conv.flow);
  if (sig === graphSig) return;
  graphSig = sig;
  const flow = cloneDeep(conv.flow);
  processFlows([flow]);
  useFlowsManagerStore.getState().setCurrentFlow(flow);
}

subscribe(() => {
  const rec = current().record;
  if (!rec) return;
  const list = signature(sessions());
  if (list !== listSig) {
    listSig = list;
    refetchLists();
  }
  const id = rec.session.id;
  if (id !== openId) {
    openId = id;
    if (wasAsked(id)) {
      // The page asked for it: Langflow's own load is waiting on it (mock GET flows/<id>).
      settle(id);
      return;
    }
    // The app opened it: show its canvas unless a settings page is open. No session at all (the
    // app's empty frame, id "") stays on the overview, which draws the empty state.
    if (!id) return;
    const r = router();
    const path = r?.state.location.pathname ?? "";
    if (r && !path.startsWith("/settings") && path !== `/flow/${id}`) void r.navigate(`/flow/${id}`);
    return;
  }
  if (id && useFlowsManagerStore.getState().currentFlowId === id) applyInPlace();
});

// A session's canvas opened: remember its graph, open the record panel on its focus (else its
// latest refused Call) with the way back shown, and frame that way once Langflow has fitted.
let lastOpened = "";
useFlowsManagerStore.subscribe((state) => {
  const id = state.currentFlowId;
  if (id === lastOpened) return;
  lastOpened = id;
  const rec = current().record;
  if (!id || rec?.session.id !== id) return;
  const conv = openFlow();
  graphSig = conv ? graphOf(conv.flow) : null;
  const panel = useRecordPanel.getState();
  panel.start(rec);
  const picked = useRecordPanel.getState().picked;
  if (!picked) return;
  // After Langflow's own request (made right after it sets the flow), so this one is the last.
  window.setTimeout(() => useFlowStore.getState().requestFitView(() => useRecordPanel.getState().bring([picked])), 0);
});
