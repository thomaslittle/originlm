import { resolve } from 'node:path';
import { defineConfig } from 'vite';
import react, { reactCompilerPreset } from '@vitejs/plugin-react';
import babel from '@rolldown/plugin-babel';
import tailwindcss from '@tailwindcss/vite';

// Separate asset files (never inlined): the native helper and the fixture
// preview server both serve `script-src 'self'; style-src 'self'` CSP, and the
// native asset route only serves .html/.css/.js/.mjs/.svg under 2 MiB.
export default defineConfig({
  root: import.meta.dirname,
  plugins: [react(), babel({ presets: [reactCompilerPreset()] }), tailwindcss()],
  resolve: { tsconfigPaths: true },
  build: {
    outDir: resolve(import.meta.dirname, 'dist'),
    emptyOutDir: true,
    assetsDir: 'assets',
    assetsInlineLimit: 0,
    chunkSizeWarningLimit: 600,
    rolldownOptions: {
      output: {
        entryFileNames: 'assets/[name]-[hash].js',
        chunkFileNames: 'assets/[name]-[hash].js',
        assetFileNames: 'assets/[name]-[hash].[ext]',
        // Stable vendor chunks: application edits do not invalidate the framework and UI-kit
        // caches. Heavy, page-specific libraries (recharts, markdown) stay in their lazy chunks.
        codeSplitting: {
          groups: [
            { name: 'vendor-react', test: /node_modules[\/](react|react-dom|scheduler)[\/]/ },
            { name: 'vendor-router', test: /node_modules[\/]@tanstack[\/]/ },
            { name: 'vendor-ui', test: /node_modules[\/](@base-ui|class-variance-authority|clsx|tailwind-merge)[\/]/ },
          ],
        },
      },
    },
  },
});
