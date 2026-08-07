import { defineConfig } from 'vite';
import { resolve, dirname } from 'path';
import { fileURLToPath } from 'url';

const here = dirname(fileURLToPath(import.meta.url));

export default defineConfig({
  root: here,
  build: {
    // Straight into the directory rust-embed bakes into the binary.
    // emptyOutDir has to be explicit because the target sits outside root.
    outDir: resolve(here, '../../dist'),
    emptyOutDir: true,
    target: 'esnext',
  },
  resolve: {
    // Array form: the more specific entry has to precede the package prefix.
    alias: [
      {
        find: '@nigel/theme/css/nigel.css',
        replacement: resolve(here, '../../packages/theme/dist/css/nigel.css'),
      },
      {
        find: '@nigel/theme',
        replacement: resolve(here, '../../packages/theme/src/index.ts'),
      },
      {
        find: '@nigel/ui',
        replacement: resolve(here, '../../packages/ui/src/index.ts'),
      },
    ],
  },
  server: {
    port: 5173,
    strictPort: true,
    // Both paths a browser needs from the backend: /auth to exchange the
    // printed token for the session cookie, /api for everything after.
    proxy: {
      '/api': { target: 'http://127.0.0.1:5731', changeOrigin: true },
      '/auth': { target: 'http://127.0.0.1:5731', changeOrigin: true },
    },
  },
  test: {
    // Node by default; only the tests that touch the DOM pay for jsdom.
    environment: 'node',
    globals: true,
    include: ['src/**/*.test.ts'],
    setupFiles: ['./src/test-setup.ts'],
    environmentMatchGlobs: [
      ['**/components/**', 'jsdom'],
      ['**/screens/**', 'jsdom'],
      ['**/state/**', 'jsdom'],
      ['**/api/**', 'jsdom'],
      // Cross-screen tests drive the whole app and need a DOM; the guard tests
      // beside them only read source text, and node is enough for those.
      ['**/__tests__/screen-freshness.test.ts', 'jsdom'],
    ],
    deps: {
      optimizer: {
        web: { include: ['lit', '@lit-labs/signals'] },
      },
    },
  },
});
