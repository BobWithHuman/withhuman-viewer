import { WithHumanLogo } from "@/withhuman/chrome/logo";

/**
 * Seam for the mark inside the header's home button.
 *
 * withhuman: the app's own icon in place of Langflow's logo (this ships as
 * WithHuman's app).
 *
 * The button around it owns the navigation and the accessible name, so this
 * renders decoration only and stays `aria-hidden`.
 */
export function CustomHeaderLogo({ className }: { className?: string }) {
  return <WithHumanLogo className={className} />;
}

export default CustomHeaderLogo;
