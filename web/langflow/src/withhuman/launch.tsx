// The first launch over Langflow's page, in a React root of its own (so it shows whatever route
// Langflow is on, even before Langflow has drawn): the onboarding while `shell.page` is "onboard",
// and the first session's tour. Inside Langflow's <body id="body">, so its theme classes and
// tokens apply; its own TooltipProvider for Langflow's tooltips.

import { createRoot } from "react-dom/client";
import { TooltipProvider } from "@/components/ui/tooltip";
import { useWithHuman } from "./bridge";
import { FirstLaunch } from "./onboard/Onboard";
import { Tour } from "./tour/Tour";

function Launch() {
  const { record, views } = useWithHuman();
  return (
    <TooltipProvider skipDelayDuration={0}>
      <FirstLaunch page={record?.shell?.page} data={views.onboard} hasRecord={!!record} />
      <Tour />
    </TooltipProvider>
  );
}

const host = document.createElement("div");
host.id = "withhuman-launch";
document.body.appendChild(host);
createRoot(host).render(<Launch />);
