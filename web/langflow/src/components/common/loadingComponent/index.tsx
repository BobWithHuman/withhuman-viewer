import { useTranslation } from "react-i18next";
import { WithHumanLogo } from "@/withhuman/chrome/logo";
import type { LoadingComponentProps } from "../../../types/components";

export default function LoadingComponent({
  remSize,
}: LoadingComponentProps): JSX.Element {
  const { t } = useTranslation();
  return (
    <div role="status" className="flex flex-col items-center justify-center">
      {/* withhuman: the app's icon, and no "Loading..." line. */}
      <div
        className="animate-pulse"
        style={{
          width: `${remSize * 0.25}rem`,
          height: `${remSize * 0.25}rem`,
        }}
      >
        <WithHumanLogo className="h-full w-full" />
      </div>
      <span className="sr-only">{t("loading.loading")}</span>
    </div>
  );
}
