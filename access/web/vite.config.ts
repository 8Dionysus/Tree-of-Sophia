import { defineConfig, type Plugin } from "vite";
import { fileURLToPath } from "node:url";
import { createHash } from "node:crypto";
// The research core's revision is the same digest the history capture records for it.
import { readCore } from "../../bonsai/lib/history.mjs";
// Every light theme of the public site is the night's stylesheets in its own colours, added at build time.
import { themePostcss } from "./src/public-research/themes.mjs";
// The public site's build writes the site's copy of the layers into its output.
import { siteLayers } from "./site-layers.mjs";

// What a page would otherwise compute before its first view, known when it is built:
// - the research core's revision, the digest of its canonical form (bonsai/lib/history.mjs);
// - the build's identity, a digest of every file the build writes, which lets a page remember what it derived from
//   exactly this build. A page served from source carries no identity and remembers nothing.
const BUILD_FACTS = "virtual:tos-build-facts";
const IDENTITY_MARK = "tos-build-identity:";
const UNSET_IDENTITY = IDENTITY_MARK + "0".repeat(64);
function buildFacts(): Plugin {
  const stand = fileURLToPath(new URL("../../bonsai/zarathustra", import.meta.url));
  return {
    name: "tos-build-facts",
    resolveId: id => id === BUILD_FACTS ? "\0" + BUILD_FACTS : null,
    load(id) {
      if (id !== "\0" + BUILD_FACTS) return null;
      const {revision, paths} = readCore(stand);
      for (const path of paths) this.addWatchFile(path);
      return `export const researchRevision=${JSON.stringify(revision)};
const mark=${JSON.stringify(UNSET_IDENTITY)};
export const buildIdentity=/^${IDENTITY_MARK}(?!0{64}$)[0-9a-f]{64}$/.test(mark)?mark.slice(${IDENTITY_MARK.length}):null;`;
    },
    generateBundle(_options, bundle) {
      const hash = createHash("sha256");
      for (const name of Object.keys(bundle).sort()) {
        const item = bundle[name];
        hash.update(name).update("\0").update(item.type === "chunk" ? item.code : item.source).update("\0");
      }
      const identity = IDENTITY_MARK + hash.digest("hex");
      for (const item of Object.values(bundle)) if (item.type === "chunk" && item.code.includes(UNSET_IDENTITY)) item.code = item.code.replaceAll(UNSET_IDENTITY, identity);
    },
  };
}

export default defineConfig(({ mode }) => {
  const publicResearch = mode === "public-research";
  return {
  base: "/__research/",
  plugins: publicResearch ? [buildFacts(), siteLayers()] : [buildFacts()],
  css: { postcss: { plugins: [themePostcss()] } },
  server: {
    host: "127.0.0.1",
    fs: {
      // The browser entry imports these sibling source and generated modules.
      allow: [
        fileURLToPath(new URL(".", import.meta.url)),
        fileURLToPath(new URL("../shared", import.meta.url)),
        // The act journal reads its base kinds from the access contracts.
        fileURLToPath(new URL("../contracts", import.meta.url)),
        // The public stands read their bonsai from the repository root.
        fileURLToPath(new URL("../../bonsai", import.meta.url)),
        fileURLToPath(new URL("../deploy/cloudflare-worker/generated", import.meta.url)),
      ],
    },
    proxy: {"/api": process.env.TOS_DEV_API_TARGET ?? "http://127.0.0.1:44258"},
  },
  build: {
    chunkSizeWarningLimit: 900,
    // The build's own list of its files: a stand saved for working offline keeps every file its page may load.
    manifest: "asset-manifest.json",
    outDir: publicResearch ? "../deploy/cloudflare-worker/dist-public-research" : "dist",
    emptyOutDir: true,
    rollupOptions: {
      input: {
        'zarathustra/index':fileURLToPath(new URL('./zarathustra/index.html',import.meta.url)),
        'antiquity/index':fileURLToPath(new URL('./antiquity/index.html',import.meta.url)),
        'aesthetics/index':fileURLToPath(new URL('./aesthetics/index.html',import.meta.url)),
        'population/index':fileURLToPath(new URL('./population/index.html',import.meta.url)),
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
