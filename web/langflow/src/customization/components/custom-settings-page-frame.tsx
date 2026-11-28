import type { ReactNode } from "react";
import type { To } from "react-router-dom";
import PageLayout from "@/components/common/pageLayout";
import SideBarButtonsComponent from "@/components/core/sidebarComponent";
import { SidebarProvider } from "@/components/ui/sidebar";
import type { SettingsSidebarItem } from "@/pages/SettingsPage/use-settings-sidebar-items";
import { useWithHuman } from "@/withhuman/bridge";

export interface CustomSettingsPageFrameProps {
  children: ReactNode;
  description: string;
  items: SettingsSidebarItem[];
  title: string;
}

export default function CustomSettingsPageFrame({
  children,
  description,
  items,
  title,
}: CustomSettingsPageFrameProps) {
  // withhuman: titled 설정 with no description line; back goes to the open
  // session's canvas (the app may have opened settings with no history).
  const current = useWithHuman().record?.shell?.current;
  void title;
  void description;
  return (
    <PageLayout
      backTo={(current ? `/flow/${current}` : -1) as To}
      title="설정"
      description=""
    >
      <SidebarProvider width="15rem" defaultOpen={false} className="min-h-0">
        <SideBarButtonsComponent items={items} />
        {/* Only the content scrolls, so the Settings header and the menu stay in view; a sticky element inside a page sticks to this box. */}
        {/* The box reaches over the page's right and bottom padding, so its scrollbar keeps the page's spacing from the cards. */}
        <main
          data-testid="settings-content-scroll"
          className="-mb-6 -mr-6 flex min-h-0 min-w-0 flex-1 overflow-y-auto overflow-x-clip pb-6 pr-6"
        >
          <div className="flex min-w-0 flex-1 flex-col overflow-x-clip pt-1">
            {children}
          </div>
        </main>
      </SidebarProvider>
    </PageLayout>
  );
}
