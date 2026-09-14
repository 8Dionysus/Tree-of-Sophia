import { defineConfig } from "vite";

export default defineConfig({
  base: "/static/",
  server: {host: "127.0.0.1", proxy: {"/api": "http://127.0.0.1:44258"}},
  build: {
    chunkSizeWarningLimit: 900,
    manifest: false,
    outDir: "dist",
    emptyOutDir: true,
    rollupOptions: {
      output: {
        entryFileNames: "assets/tos-graph.js",
        chunkFileNames: "assets/[name]-[hash].js",
        assetFileNames: "assets/[name]-[hash][extname]",
      },
    },
  },
});

