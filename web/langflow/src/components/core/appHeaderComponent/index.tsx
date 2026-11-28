import { useEffect, useRef, useState } from "react";
import AlertDropdown from "@/alerts/alertDropDown";
import ForwardedIconComponent from "@/components/common/genericIconComponent";
import ModelProviderCount from "@/components/common/modelProviderCountComponent";
import ShadTooltip from "@/components/common/shadTooltipComponent";
import { Separator } from "@/components/ui/separator";
import CustomAccountMenu from "@/customization/components/custom-AccountMenu";
import CustomLangflowCounts from "@/customization/components/custom-langflow-counts";
import { CustomOrgSelector } from "@/customization/components/custom-org-selector";
import useTheme from "@/customization/hooks/use-custom-theme";
import useAlertStore from "@/stores/alertStore";
import { dragProps } from "@/withhuman/chrome/app";
import { AppSync } from "@/withhuman/chrome/AppSync";
import FlowMenu from "./components/FlowMenu";

export default function AppHeader(): JSX.Element {
  const notificationCenter = useAlertStore((state) => state.notificationCenter);
  const [activeState, setActiveState] = useState<"notifications" | null>(null);
  const notificationRef = useRef<HTMLButtonElement | null>(null);
  const notificationContentRef = useRef<HTMLDivElement | null>(null);
  useTheme();

  useEffect(() => {
    function handleClickOutside(event: MouseEvent) {
      const target = event.target as Node;
      const isNotificationButton = notificationRef.current?.contains(target);
      const isNotificationContent =
        notificationContentRef.current?.contains(target);

      if (!isNotificationButton && !isNotificationContent) {
        setActiveState(null);
      }
    }

    document.addEventListener("mousedown", handleClickOutside);
    return () => {
      document.removeEventListener("mousedown", handleClickOutside);
    };
  }, []);

  const getNotificationBadge = () => {
    const baseClasses = "absolute h-1 w-1 rounded-full bg-destructive";
    return notificationCenter
      ? `${baseClasses} right-[0.3rem] top-[5px]`
      : "hidden";
  };

  return (
    <header
      className={`z-10 flex h-[48px] w-full items-center justify-between border-b pr-5 pl-[80px] dark:bg-background`}
      data-testid="app-header"
      {...dragProps}
    >
      <AppSync />
      {/* Left Section */}
      <div
        className={`z-30 flex shrink-0 items-center gap-2`}
        data-testid="header_left_section_wrapper"
      >
        {/* withhuman: no logo button; its height kept, which the centred title lines up with. */}
        <span aria-hidden className="h-8" />
        <CustomOrgSelector />
      </div>

      {/* Middle Section */}
      <div className="absolute left-1/2 -translate-x-1/2">
        <FlowMenu />
      </div>

      {/* Right Section */}
      <div
        className={`relative left-3 z-30 flex shrink-0 items-center gap-3`}
        data-testid="header_right_section_wrapper"
      >
        {/* withhuman: no counts, notifications or separator; the engine menu only. */}
        <div className="flex">
          <CustomAccountMenu />
        </div>
      </div>
    </header>
  );
}
