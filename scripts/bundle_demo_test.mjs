/**
 * Offline test bundle for the React demo. It goes through the same Vite pipeline
 * (React Compiler, Tailwind, path aliases) as the shipped build, so the browser
 * tests exercise compiled production code rather than a separate esbuild variant.
 * Output is a single IIFE with dynamic imports inlined so it can be injected into
 * a page without any network access.
 */
import { build } from 'vite';
import { fileURLToPath } from 'node:url';
import { resolve, dirname, basename } from 'node:path';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
if (!process.argv[2]) throw new Error('Pass an explicit output path');
const output = resolve(root, process.argv[2]);

await build({
  configFile: resolve(root, 'apps/demo/vite.config.ts'),
  logLevel: 'warn',
  // Library mode does not inline the environment the way an app build does.
  define: { 'process.env.NODE_ENV': JSON.stringify('production') },
  build: {
    outDir: dirname(output),
    emptyOutDir: false,
    copyPublicDir: false,
    cssCodeSplit: false,
    lib: {
      entry: resolve(root, 'apps/demo/tests/fixture-entry.tsx'),
      formats: ['iife'],
      name: 'LocalAIKitTestHost',
      fileName: () => basename(output),
    },
    // One file: the production vendor grouping is for the real build; the offline page needs a single IIFE.
    rolldownOptions: { output: { codeSplitting: false, entryFileNames: basename(output), assetFileNames: 'test-host.[ext]' } },
  },
});
console.log(`Bundled demo test host: ${output}`);
