// The chrome's ties to the app (Rust): the look it paints behind the frame, a press on bare chrome
// moving the window, and the settings pages it is told are open. Everything asked goes through the
// bridge's `send`.

import type React from "react";
import { useDarkStore } from "@/stores/darkStore";
import { send } from "../bridge";

/** Where a session's canvas lives: the flow route, flow id = session id. */
export const flowPath = (id: string) => `/flow/${id}`;

/** The settings pages and the `page` the app is told for each. */
export const PAGES = { hooks: "/settings/hooks", engine: "/settings/engine" } as const;

/** A press on bare header space moves the window, a double press zooms it, as a title bar does. */
const INTERACTIVE = "button,input,textarea,select,a,[role=button],[role=menuitem],[role=combobox],[data-radix-popper-content-wrapper]";
export const dragProps = {
  onMouseDown: (e: React.MouseEvent) => {
    if (e.button === 0 && !(e.target as HTMLElement).closest(INTERACTIVE)) send({ do: "drag" });
  },
  onDoubleClick: (e: React.MouseEvent) => {
    if (!(e.target as HTMLElement).closest(INTERACTIVE)) send({ do: "zoom" });
  },
};

// The app paints the area behind the frame in the page's look: tell it the look now and on every
// change (Langflow's theme switch writes the dark store; "system" resolves to dark or light there).
let painted: string | null = null;
const paint = (dark: boolean) => {
  const theme = dark ? "dark" : "light";
  if (theme === painted) return;
  painted = theme;
  send({ do: "theme", theme });
};
paint(useDarkStore.getState().dark);
useDarkStore.subscribe((s) => paint(s.dark));
