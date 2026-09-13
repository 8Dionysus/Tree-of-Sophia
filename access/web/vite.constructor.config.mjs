import { defineConfig } from 'vite';
export default defineConfig({base:'./',publicDir:false,
  cacheDir:process.env.TOS_WEB_CACHE_DIR??'node_modules/.vite-constructor',
  server:{host:'127.0.0.1',port:44257,strictPort:true,proxy:{'/api':'http://127.0.0.1:44258'}},
  build:{emptyOutDir:false,chunkSizeWarningLimit:900,rollupOptions:{input:'constructor.html'}}});
