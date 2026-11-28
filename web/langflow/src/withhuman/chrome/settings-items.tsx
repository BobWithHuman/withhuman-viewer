// The settings page's menu: 훅 and 엔진 only (Langflow's own sections have no backing here), with
// Langflow's icon style; and their routes, added under /settings through Langflow's
// CustomRoutesStore seam.

import { Route } from "react-router-dom";
import ForwardedIconComponent from "@/components/common/genericIconComponent";
import type { SettingsSidebarItem } from "@/pages/SettingsPage/use-settings-sidebar-items";
import EnginePage from "./EnginePage";
import HooksPage from "./HooksPage";

const icon = (name: string) => <ForwardedIconComponent name={name} className="w-4 flex-shrink-0 justify-start stroke-[1.5]" />;

export const SETTINGS_ITEMS: SettingsSidebarItem[] = [
  { title: "훅", href: "/settings/hooks", icon: icon("Webhook") },
  { title: "엔진", href: "/settings/engine", icon: icon("Server") },
];

export function settingsRoutes() {
  return [<Route key="hooks" path="hooks" element={<HooksPage />} />, <Route key="engine" path="engine" element={<EnginePage />} />];
}
