import { defineConfig } from 'vite';
import { resolve, dirname } from 'path';
import { fileURLToPath } from 'url';
import { previewsJsonPlugin } from './previews-json-plugin.js';

const here = dirname(fileURLToPath(import.meta.url));

export default defineConfig({
  root: here,
  server: { port: 9090 },
  plugins: [previewsJsonPlugin(resolve(here, '../src'))],
  resolve: {
    // Array form: entries are tried in order, so the more specific path has to
    // come before the package-root prefix.
    alias: [
      {
        find: '@nigel/theme/css/nigel.css',
        replacement: resolve(here, '../../theme/dist/css/nigel.css'),
      },
      {
        find: '@nigel/theme',
        replacement: resolve(here, '../../theme/src/index.ts'),
      },
    ],
  },
});
