import { defineConfig } from "vite";
import { fileURLToPath } from "node:url";
// Every light theme of the public site is the night's stylesheets in its own colours, added at build time.
import { themePostcss } from "./src/public-research/themes.mjs";
// The public site's build writes the site's copy of the layers into its output.
import { siteLayers } from "./site-layers.mjs";

export default defineConfig(({ mode }) => {
  const publicResearch = mode === "public-research";
  return {
  base: "/__research/",
  plugins: [siteLayers()],
  css: { postcss: { plugins: [themePostcss()] } },
  server: {
    host: "127.0.0.1",
    fs: {
      // The browser entry imports these sibling source and generated modules.
      allow: [
        fileURLToPath(new URL(".", import.meta.url)),
        fileURLToPath(new URL("../shared", import.meta.url)),
        // The public stands read their bonsai from the repository root.
        fileURLToPath(new URL("../../bonsai", import.meta.url)),
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
        'zarathustra/index':fileURLToPath(new URL('./zarathustra/index.html',import.meta.url)),
        'antiquity/index':fileURLToPath(new URL('./antiquity/index.html',import.meta.url)),
        'not-found/index':fileURLToPath(new URL('./not-found/index.html',import.meta.url)),
        'home/index':fileURLToPath(new URL('./home/index.html',import.meta.url)),
      },
      output: {
        entryFileNames: chunk=>`assets/${chunk.name.replaceAll('/','-')}.js`,
        chunkFileNames: "assets/[name]-[hash].js",
        // Zarathustra's witnesses load a part of the book at a time; group them by part.
        manualChunks: id=>{
          const witness=/bonsai\/zarathustra\/witnesses\/(I|II|III|IV)\.\d+\.json$/.exec(id.replaceAll('\\','/'));
          return witness?`zarathustra-witnesses-${witness[1]}`:undefined;
        },
        assetFileNames: "assets/[name]-[hash][extname]",
      },
    },
  },
  };
});
