import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { TooltipProvider } from "@/components/ui/tooltip";
import { useConsole } from "./bridge";
import Detect from "./pages/detect";
import Inventory from "./pages/inventory";
import Onboard from "./pages/onboard";
import Hooks from "./pages/hooks";
import Engine from "./pages/engine";
import Secrets from "./pages/secrets";
import "./index.css";

// One page per screen of the security team's app, and the personal app's first launch; the app
// says which, and hands it its data.
const PAGES = { detect: Detect, secrets: Secrets, inventory: Inventory, onboard: Onboard, hooks: Hooks, engine: Engine };

function App() {
  const { page, data } = useConsole();
  const Page = PAGES[page];
  return (
    <TooltipProvider delayDuration={200}>
      <Page data={data} />
    </TooltipProvider>
  );
}

createRoot(document.getElementById("root")!).render(<StrictMode><App /></StrictMode>);
