// How the page and the app talk. The app (Rust) shows this page in a frame and calls
// `WHConsole.render(page, data)` with the screen's name and its data as JSON; a page asks the app
// to do something with `send(page, message)`, which the app receives as JSON
// (`parent.__whConsoleSend`). Opened on its own (no app), a page shows its demo data and `send`
// only logs.

import { useSyncExternalStore } from "react";

export type PageName = "detect" | "secrets" | "inventory" | "onboard" | "hooks" | "engine";

type State = { page: PageName; data: unknown | null };
const hash = location.hash.slice(1) as PageName;
let state: State = { page: (["detect", "secrets", "inventory", "onboard", "hooks", "engine"] as PageName[]).includes(hash) ? hash : "inventory", data: null };
const subs = new Set<() => void>();

declare global {
  interface Window {
    WHConsole: { render: (page: PageName, data: unknown) => void };
    __whConsoleSend?: (page: string, message: unknown) => void;
  }
}

window.WHConsole = {
  render: (page, data) => {
    state = { page, data };
    subs.forEach((f) => f());
  },
};

export const useConsole = () => useSyncExternalStore((f) => { subs.add(f); return () => subs.delete(f); }, () => state);

/** Asks the app to act: issue a key, stop a person, open a session. */
export function send(page: PageName, message: Record<string, unknown>) {
  const up = (window.parent as Window).__whConsoleSend;
  if (up && window.parent !== window) up(page, message);
  else console.log("send", page, message);
}
