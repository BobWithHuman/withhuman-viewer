// withhuman: builds Langflow's frontend as ONE self-contained HTML file that
// runs with no backend (src/withhuman/mock answers the API in-page) and from
// an iframe `srcdoc`. It reuses Langflow's own vite.config.mts and only adds:
//   1. the entry src/withhuman/main.tsx (mock + local fonts, then src/index.tsx),
//   2. a memory router instead of the browser router (srcdoc has no real
//      path; start path from the URL hash, see src/withhuman/router.ts),
//   3. no remote font links / favicon / manifest in index.html; its title
//      WithHuman and its lang ko,
//   4. vite-plugin-singlefile (all JS, CSS, fonts and images inlined; the
//      voice-mode audio worklet, which Langflow marks `no-inline`, too).
// Langflow's source files stay untouched; see src/withhuman/README.md.

import { defineConfig, mergeConfig, type Plugin, type UserConfig } from "vite";
import { viteSingleFile } from "vite-plugin-singlefile";
import langflowConfig from "./vite.config.mts";

function withhuman(): Plugin {
  return {
    name: "withhuman-single",
    enforce: "pre",
    transformIndexHtml: {
      order: "pre",
      handler(html) {
        return html
          .replace(/\s*<link rel="(icon|manifest|preconnect)"[^>]*\/>/g, "")
          .replace(/\s*<link\s+href="https:\/\/fonts\.googleapis\.com[^>]*\/>/g, "")
          .replace(/<title>[^<]*<\/title>/, "<title>WithHuman</title>")
          .replace(/<html lang="[^"]*"/, '<html lang="ko"')
          .replace('src="/src/index.tsx"', 'src="/src/withhuman/main.tsx"');
      },
    },
    // The templates modal's six "Get started" pictures (~9.6 MB inlined) are starter-project
    // content the record viewer never shows: a 1x1 transparent GIF stands in for each.
    load(id) {
      if (/\/src\/assets\/temp-pat-(m-)?\d\.png$/.test(id.replace(/\\/g, "/").split("?")[0]))
        return `export default "data:image/gif;base64,R0lGODlhAQABAIAAAAAAAP///yH5BAEAAAAALAAAAAABAAEAAAIBRAA7";`;
      return null;
    },
    transform(code, id) {
      // srcdoc frames cannot use path-based history; route in memory.
      if (id.replace(/\\/g, "/").endsWith("/src/routes.tsx")) {
        const out = code.replace(
          /\bcreateBrowserRouter\(/,
          "createWithhumanRouter(",
        );
        if (out === code) this.error("routes.tsx: createBrowserRouter( not found");
        return {
          code: `import { createWithhumanRouter } from "@/withhuman/router";\n${out}`,
          map: null,
        };
      }
      // The voice-mode audio worklet is the one asset Langflow keeps out of
      // line; inline it too so dist/ holds nothing but index.html.
      if (code.includes("audio-worklet-processor.js?url&no-inline")) {
        return {
          code: code.replace("audio-worklet-processor.js?url&no-inline", "audio-worklet-processor.js?url"),
          map: null,
        };
      }
      return null;
    },
  };
}

export default defineConfig(async (env) => {
  const base = (await (typeof langflowConfig === "function"
    ? langflowConfig(env)
    : langflowConfig)) as UserConfig;
  return mergeConfig(base, {
    publicDir: false,
    plugins: [withhuman(), viteSingleFile()],
    build: {
      outDir: "dist",
      emptyOutDir: true,
      chunkSizeWarningLimit: 100_000,
      reportCompressedSize: false,
    },
  } satisfies UserConfig);
});
