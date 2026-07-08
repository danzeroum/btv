import { defineConfig } from 'vitest/config'
import react from '@vitejs/plugin-react'
import { fileURLToPath } from 'node:url'

const vendor = (p: string) => fileURLToPath(new URL(`../vendor/bpmn/packages/${p}`, import.meta.url))

// https://vite.dev/config/
export default defineConfig({
  plugins: [react()],
  resolve: {
    // O dist da lib importa 'react'/'react-dom' e, sem dedupe, o bundler
    // resolveria o React 18 de vendor/bpmn/node_modules — duas cópias de
    // React no bundle (React error #525 em produção). dedupe força a cópia
    // única do app (React 19).
    dedupe: ['react', 'react-dom'],
    // A lib bpmn (submodule vendor/bpmn, pinada) é consumida pelo BUILD dela
    // (dist ESM) via alias — sem publicar no npm, sem acoplar os workspaces
    // pnpm. `scripts/ensure-bpmn.mjs` garante o dist. A ordem importa: os
    // subpaths (styles.css) antes dos pacotes.
    alias: [
      { find: '@bpmn-react/react/styles.css', replacement: vendor('react/styles.css') },
      { find: '@bpmn-react/core', replacement: vendor('core/dist/esm/index.js') },
      { find: '@bpmn-react/react', replacement: vendor('react/dist/esm/index.js') },
      { find: '@bpmn-react/registry', replacement: vendor('registry/dist/esm/index.js') },
    ],
  },
  server: {
    proxy: {
      // Aponta para o `btv dashboard` real rodando localmente — mesmo
      // padrão do console BuildToValue (`web/`): toda rota /api é backend real.
      '/api': 'http://127.0.0.1:7878',
    },
  },
  test: {
    environment: 'jsdom',
    exclude: ['**/node_modules/**', '**/tests/e2e/**', '**/tests/e2e-integration/**'],
  },
})
