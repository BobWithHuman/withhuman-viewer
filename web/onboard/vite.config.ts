import { defineConfig } from "vite";
import { viteSingleFile } from "vite-plugin-singlefile";
import tailwindcss from "@tailwindcss/vite";
import { resolve } from "node:path";

// One self-contained HTML (scripts, styles, fonts and images inlined): the app loads it as the
// first-launch window's srcdoc, with no server and no network.
export default defineConfig({
  plugins: [tailwindcss(), viteSingleFile()],
  resolve: { alias: { "@": resolve(import.meta.dirname, "src") } },
  build: { target: "es2020", assetsInlineLimit: 100000000 },
});
