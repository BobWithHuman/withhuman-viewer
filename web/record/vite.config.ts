import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import { viteSingleFile } from "vite-plugin-singlefile";

// One self-contained HTML file, so it opens from disk without a server.
export default defineConfig({ plugins: [react(), tailwindcss(), viteSingleFile()], base: "./" });
