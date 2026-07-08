import { defineConfig } from 'vitest/config'
import react from '@vitejs/plugin-react'

// https://vite.dev/config/
export default defineConfig({
  // Assets relativos: o console Forge passou a ser servido sob `/dev` pelo
  // `forge dashboard` (a SPA raiz agora é o BuildToValue, `btv-web/`), mas os
  // testes de integração continuam servindo este build na raiz — `./` funciona
  // nos dois pontos de montagem (o app não usa roteamento por URL).
  base: './',
  plugins: [react()],
  server: {
    proxy: {
      // Aponta para `forge dashboard` real rodando localmente (telemetria é o
      // único domínio com backend de verdade nesta fase).
      '/api': 'http://127.0.0.1:7878',
    },
  },
  test: {
    environment: 'jsdom',
    exclude: ['**/node_modules/**', '**/tests/e2e/**', '**/tests/e2e-integration/**'],
  },
})
