import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { TooltipProvider } from "@/components/ui/tooltip";
import { useConsole } from "./bridge";
import Detect from "./pages/detect";
import Inventory from "./pages/inventory";
import Secrets from "./pages/secrets";
import "./index.css";

// One page per screen of the security team's app; the app says which, and hands it its data.
const PAGES = { detect: Detect, secrets: Secrets, inventory: Inventory };

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
