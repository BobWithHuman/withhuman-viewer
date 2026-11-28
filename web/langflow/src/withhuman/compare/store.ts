// The sessions picked for a compare: up to two ids, shared by the overview's rows and the session
// sidebar's row menu. A third pick drops the oldest.

import { create } from "zustand";

type Picks = { ids: string[]; toggle: (id: string) => void; clear: () => void };

export const useComparePicks = create<Picks>((set) => ({
  ids: [],
  toggle: (id) =>
    set(({ ids }) => ({ ids: ids.includes(id) ? ids.filter((x) => x !== id) : [...ids, id].slice(-2) })),
  clear: () => set({ ids: [] }),
}));

/** Where the compare view lives: the overview's route with both ids in its query. */
export const comparePath = (a: string, b: string) => `/all?compare=${encodeURIComponent(a)},${encodeURIComponent(b)}`;
