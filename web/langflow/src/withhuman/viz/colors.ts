// The charts' colours, from Langflow's own tokens (it ships no chart palette): what the engine
// decided is a status, so allowed / held / refused take Langflow's emerald, status yellow and
// destructive, and what a tool result told takes its indigo. Allowed is kept quiet so the flagged
// parts stand out. Written as CSS colours (Langflow's Tailwind colours take no opacity modifier).

export const C = {
  allow: "hsl(var(--accent-emerald-foreground) / 0.55)",
  review: "var(--status-yellow)",
  deny: "hsl(var(--destructive))",
  outside: "hsl(var(--accent-indigo-foreground))",
  quiet: "hsl(var(--muted-foreground) / 0.45)",
  track: "hsl(var(--muted))",
} as const;
