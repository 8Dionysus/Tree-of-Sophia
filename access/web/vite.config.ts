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
        index:fileURLToPath(new URL('./index.html',import.meta.url)),
        research:fileURLToPath(new URL('./research.html',import.meta.url)),
        'zarathustra/index':fileURLToPath(new URL('./zarathustra/index.html',import.meta.url)),
        'antiquity/index':fileURLToPath(new URL('./antiquity/index.html',import.meta.url)),
      },
      output: {
        entryFileNames: chunk=>chunk.name==='index'?'assets/tos-graph.js':chunk.name==='research'?'assets/tos-research.js':`assets/${chunk.name.replaceAll('/','-')}.js`,
        chunkFileNames: "assets/[name]-[hash].js",
        // Zarathustra's witnesses load a part of the book at a time, so the software archive keeps
        // four members for them rather than one per chapter.
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
