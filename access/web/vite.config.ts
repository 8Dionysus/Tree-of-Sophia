import { defineConfig } from "vite";
import { fileURLToPath } from "node:url";

export default defineConfig(({ mode }) => {
  const publicResearch = mode === "public-research";
  return {
  base: publicResearch ? "/__research/" : "/static/",
  server: {
    host: "127.0.0.1",
    fs: {
      // The browser entry imports these sibling source and generated modules.
      allow: [
        fileURLToPath(new URL(".", import.meta.url)),
        fileURLToPath(new URL("../shared", import.meta.url)),
        fileURLToPath(new URL("../deploy/cloudflare-worker/generated", import.meta.url)),
      ],
    },
    proxy: {"/api": process.env.TOS_DEV_API_TARGET ?? "http://127.0.0.1:44258"},
  },
  build: {
    chunkSizeWarningLimit: 900,
    manifest: false,
    outDir: publicResearch ? "../deploy/cloudflare-worker/dist-public-research" : "dist",
    emptyOutDir: true,
    rollupOptions: {
      input: {
        index:fileURLToPath(new URL('./index.html',import.meta.url)),
        research:fileURLToPath(new URL('./research.html',import.meta.url)),
        'zarathustra/index':fileURLToPath(new URL('./zarathustra/index.html',import.meta.url)),
        'antiquity/index':fileURLToPath(new URL('./antiquity/index.html',import.meta.url)),
      },
      output: {
        entryFileNames: chunk=>chunk.name==='index'?'assets/tos-graph.js':chunk.name==='research'?'assets/tos-research.js':`assets/${chunk.name.replaceAll('/','-')}.js`,
        chunkFileNames: "assets/[name]-[hash].js",
        assetFileNames: "assets/[name]-[hash][extname]",
      },
    },
  },
  };
});
