// Router used in place of `createBrowserRouter` (swapped in routes.tsx at
// build time by vite.single.config.mts). An iframe `srcdoc` page has no real
// path, so routing lives in memory. The start path comes from the URL hash
// (`#/flow/<id>`, `#/all`, ...) or is the open session's canvas (the list
// until the app has rendered one; flow/sync.ts opens it then); the current
// path is mirrored back to the hash so the host can read or restore it.

import { createMemoryRouter } from "react-router-dom";
import { current } from "./bridge";

type Routes = Parameters<typeof createMemoryRouter>[0];
type Options = Parameters<typeof createMemoryRouter>[1];
export type Router = ReturnType<typeof createMemoryRouter>;

let made: Router | null = null;
/** The page's router, once Langflow has made it. */
export const router = () => made;

export function createWithhumanRouter(routes: Routes, options?: Options) {
  const fromHash = window.location.hash.replace(/^#/, "");
  const open = current().record?.session.id;
  // `#/onboard...` (the onboarding's demo, opened on its own) is no route: the list under it.
  const start = fromHash.startsWith("/") && fromHash !== "/" && !fromHash.startsWith("/onboard")
    ? fromHash
    : open ? `/flow/${open}` : "/all";
  const router = createMemoryRouter(routes, {
    ...options,
    basename: undefined,
    initialEntries: [start],
  });
  const mirror = (path: string) => {
    try {
      const here = window.location.href.split("#")[0];
      window.history.replaceState(window.history.state, "", `${here}#${path}`);
    } catch {
      // Hosts that forbid history changes simply do not get the mirror.
    }
  };
  mirror(start);
  router.subscribe(({ location }) => mirror(location.pathname + location.search));
  made = router;
  return router;
}
