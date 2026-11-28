// The first session's tour: wanted once the onboarding finishes (kept in localStorage, so it
// survives the reload the app may do while the try session comes), shown when a session's canvas
// has its nodes, and remembered as done once finished or skipped. The engine menu's 둘러보기
// starts it again. `#...tour` in the URL (opened on its own) asks for it too, for screenshots.

import { create } from "zustand";

const KEY = "withhuman.tour";

const read = () => {
  try {
    return window.localStorage.getItem(KEY);
  } catch {
    return null;
  }
};
const write = (v: string) => {
  try {
    window.localStorage.setItem(KEY, v);
  } catch {
    // No storage: the tour is simply asked again next time.
  }
};

type Tour = {
  /** Wanted: shown as soon as a session's canvas is ready. */
  waiting: boolean;
  /** On screen, at mark `step`. */
  active: boolean;
  step: number;
  begin: () => void;
  go: (step: number) => void;
  /** Finished or skipped: not shown again on its own. */
  end: () => void;
};

export const useTour = create<Tour>((set) => ({
  waiting: read() === "want" || /[?&]tour\b/.test(window.location.hash),
  active: false,
  step: 0,
  begin: () => set({ waiting: false, active: true, step: 0 }),
  go: (step) => set({ step }),
  end: () => {
    write("done");
    set({ waiting: false, active: false, step: 0 });
  },
}));

/** The onboarding finished: the tour comes with the first session. */
export function wantTour() {
  if (read() === "done") return;
  write("want");
  useTour.setState({ waiting: true, active: false, step: 0 });
}

/** The engine menu's 둘러보기: again, as soon as a session's canvas is ready. */
export function startTour() {
  useTour.setState({ waiting: true, active: false, step: 0 });
}
