// What the app (src/ui/onboard.rs, `OnboardPage`) hands the first launch as `views.onboard`, and
// what it is asked: `send({ page: "onboard", do: ... })` (the app routes `page` to the
// onboarding's listener). Same data and messages as web/console's pages/onboard.tsx.

import { send as sendUp } from "../bridge";

export type State = "idle" | "busy" | "ok" | "err";
export type Status = { state: State; text: string | null };

export type OnboardAgent = {
  kind: string;
  name: string;
  found: boolean;
  per_folder: boolean;
  per_profile: boolean;
  /** The folder picked for an agent set up per project folder. */
  folder: string | null;
} & Status;

export type OnboardData = {
  /** `developer`: a developer's machine, set up on the company's engine first. */
  role?: "personal" | "developer";
  /** Python 3.8 or later, which the hooks run with; none when there is none. */
  python: string | null;
  /** A Jev key is already kept in the keychain. */
  kept_key: boolean;
  jev: Status & { models: string[] };
  model: string;
  parts: { name: string; up: boolean }[];
  agents: OnboardAgent[];
  run: Status;
  server: Status & { sessions: number | null };
  /** Everything is up: the app can open. */
  done: boolean;
};

/** Where the app's own engine listens. */
export const ENGINE = "127.0.0.1:47380";

/** Asks the app's onboarding listener. */
export const send = (m: Record<string, unknown>) => sendUp({ page: "onboard", ...m });

/** Where an agent's hook goes: every folder (Claude Code), one folder, or a profile. */
export const scope = (a: OnboardAgent) => (a.kind === "claude" ? "모든 폴더" : a.per_folder ? "폴더별" : "프로필");

/** The store form of the company's engine, as the old page had it. */
export const STORE = {
  url: "postgres://127.0.0.1:15432/withhuman",
  user: "withhuman_viewer",
  database: "withhuman",
  password: "",
  engine: "",
  key: "",
};
