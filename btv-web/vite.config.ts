import { defineConfig } from 'vitest/config'
import react from '@vitejs/plugin-react'

// https://vite.dev/config/
export default defineConfig({
  plugins: [react()],
  server: {
    proxy: {
      // Aponta para o `forge dashboard` real rodando localmente — mesmo
      // padrão do console Forge (`web/`): toda rota /api é backend real.
      '/api': 'http://127.0.0.1:7878',
    },
  },
  test: {
    environment: 'jsdom',
    exclude: ['**/node_modules/**', '**/tests/e2e/**', '**/tests/e2e-integration/**'],
  },
})
