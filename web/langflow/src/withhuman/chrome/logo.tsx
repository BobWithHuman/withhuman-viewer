// WithHuman's app icon (icons/128x128@2x.png, inlined by the single-file build) in place of
// Langflow's mark. The icon has a transparent margin around its rounded square; scaled up so the
// square fills the box it is given.

import { cn } from "@/utils/utils";
import icon from "./withhuman.png";

export function WithHumanLogo({ className }: { className?: string }) {
  return (
    <span className={cn("inline-flex shrink-0 items-center justify-center overflow-hidden", className)} aria-hidden="true">
      <img src={icon} alt="" draggable={false} className="h-full w-full scale-[1.22] object-contain" />
    </span>
  );
}

export default WithHumanLogo;
