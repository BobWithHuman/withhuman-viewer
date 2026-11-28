// Follows the settings page the app opens from outside the page (`shell.page`: the first launch
// with no hooks opens 훅, its menu's 설정… opens 엔진), and leaves it when the app closes it. The
// session the app opens is followed by src/withhuman/flow/sync.ts. Mounted once in the header,
// inside the router.

import { useEffect, useRef } from "react";
import { useLocation } from "react-router-dom";
import { useCustomNavigate } from "@/customization/hooks/use-custom-navigate";
import { useWithHuman } from "../bridge";
import { flowPath, PAGES } from "./app";

export function AppSync() {
  const shell = useWithHuman().record?.shell;
  const navigate = useCustomNavigate();
  const { pathname } = useLocation();
  const page = shell?.page;
  const current = shell?.current;
  const seenPage = useRef<string | undefined>(undefined);

  useEffect(() => {
    const prev = seenPage.current;
    if (page === prev) return;
    seenPage.current = page;
    const to = page === "hooks" ? PAGES.hooks : page === "settings" ? PAGES.engine : null;
    if (to && !pathname.startsWith(to)) navigate(to);
    // Back to 기록 only when the app closes the settings page it had open and that page is still
    // the one shown (not on its first word, nor while moving from 훅 to 엔진 here).
    else if (page === "record" && current && ((prev === "hooks" && pathname.startsWith(PAGES.hooks)) || (prev === "settings" && pathname.startsWith(PAGES.engine))))
      navigate(flowPath(current));
  }, [page]); // eslint-disable-line react-hooks/exhaustive-deps

  return null;
}

export default AppSync;
