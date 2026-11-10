import { defineConfig, type Plugin } from "vite";
import { fileURLToPath } from "node:url";
import { join, resolve } from "node:path";
import { placeLayers } from "../../bonsai/lib/layers.mjs";
// Every light theme of the public site is the night's stylesheets in its own colours, added at build time.
import { themePostcss } from "./src/public-research/themes.mjs";

// A public-research build carries the site's copy of the layers under layers/sha256/<hex> of its output: the
// stands' word layers, the public corpus and the stand copies of media files, from every home of the checkout.
// Every build writes them after its pages, so a page never addresses a layer its site lacks.
function siteLayers(): Plugin {
  let outDir = "";
  return {
    name: "tos-site-layers",
    apply: "build",
    configResolved(config) { outDir = resolve(config.root, config.build.outDir); },
    async closeBundle() {
      const repo = fileURLToPath(new URL("../..", import.meta.url));
      const { homes, files } = await placeLayers(repo, join(outDir, "layers"), { log: (line: string) => console.log(line) });
      console.log(`layers: ${files} files of ${homes.join(", ")}`);
    },
  };
}

export default defineConfig(({ mode }) => {
  const publicResearch = mode === "public-research";
  return {
  base: publicResearch ? "/__research/" : "/static/",
  css: { postcss: { plugins: [themePostcss()] } },
  plugins: publicResearch ? [siteLayers()] : [],
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
      // The public site holds its two stands and the page for an address it does not hold; the older atlas
      // and research pages stay in the access build only.
      input: publicResearch ? {
        'zarathustra/index':fileURLToPath(new URL('./zarathustra/index.html',import.meta.url)),
        'antiquity/index':fileURLToPath(new URL('./antiquity/index.html',import.meta.url)),
        'not-found/index':fileURLToPath(new URL('./not-found/index.html',import.meta.url)),
        // The site's own address greets the reader with its bonsai to choose from.
        'home/index':fileURLToPath(new URL('./home/index.html',import.meta.url)),
      } : {
        index:fileURLToPath(new URL('./index.html',import.meta.url)),
        research:fileURLToPath(new URL('./research.html',import.meta.url)),
        'zarathustra/index':fileURLToPath(new URL('./zarathustra/index.html',import.meta.url)),
        'antiquity/index':fileURLToPath(new URL('./antiquity/index.html',import.meta.url)),
        'home/index':fileURLToPath(new URL('./home/index.html',import.meta.url)),
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
