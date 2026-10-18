import { defineConfig } from 'vite';

// GitHub Pages serves the project under /physicall/.
export default defineConfig(({ command }) => ({
  base: command === 'build' ? '/physicall/' : '/',
  build: { target: 'es2022' },
  server: { fs: { allow: ['..'] } },
}));
