// withhuman: the 훅 and 엔진 settings pages under /settings.
import { settingsRoutes } from "@/withhuman/chrome/settings-items";

export const CustomRoutesStore = () => {
  return settingsRoutes();
};

export default CustomRoutesStore;
