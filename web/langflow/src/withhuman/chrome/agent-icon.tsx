// An agent's logo, from Langflow's own icon set (src/icons): Claude, OpenAI for Codex, Cursor,
// GitHub for Copilot, Google's Gemini mark for Gemini and Antigravity; any other agent a bot.

import ForwardedIconComponent from "@/components/common/genericIconComponent";
import { cn } from "@/utils/utils";

const ICON: Record<string, string> = {
  claude: "Claude",
  codex: "OpenAI",
  cursor: "Cursor",
  copilot: "Github",
  gemini: "GoogleGenerativeAI",
  antigravity: "GoogleGenerativeAI",
};

export function AgentIcon({ kind, className }: { kind: string; className?: string }) {
  // Langflow's brand icons differ in their own width; a fixed box keeps the rows' text aligned.
  return (
    <span className={cn("flex h-4 w-4 shrink-0 items-center justify-center overflow-hidden", className)}>
      <ForwardedIconComponent name={ICON[kind] ?? "Bot"} className="h-full w-full" />
    </span>
  );
}
