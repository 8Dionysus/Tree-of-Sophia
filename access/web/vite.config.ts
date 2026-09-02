import { defineConfig } from "vite";

export default defineConfig({
  base: "/static/",
  build: {
    chunkSizeWarningLimit: 900,
    manifest: false,
    outDir: "dist",
    emptyOutDir: true,
    rollupOptions: {
      output: {
        entryFileNames: "assets/tos-graph.js",
        chunkFileNames: "assets/[name].js",
        assetFileNames: "assets/tos-graph[extname]",
      },
    },
  },
});

