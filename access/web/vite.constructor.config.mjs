import { defineConfig } from 'vite';
export default defineConfig({base:'./',publicDir:false,build:{emptyOutDir:false,chunkSizeWarningLimit:900,rollupOptions:{input:'constructor.html'}}});
