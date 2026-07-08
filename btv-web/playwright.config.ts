import { defineConfig } from '@playwright/test'

export default defineConfig({
  testDir: './tests/e2e',
  timeout: 30_000,
  fullyParallel: true,
  reporter: 'list',
  use: {
    // Porta própria (5174) para poder coexistir com o dev server do console
    // BuildToValue (web/, 5173) na mesma máquina.
    baseURL: 'http://127.0.0.1:5174',
    trace: 'retain-on-failure',
    // Ambientes com Chromium pré-instalado fora do cache do Playwright (ex.:
    // /opt/pw-browsers/chromium) apontam o binário por env var; sem ela, o
    // Playwright resolve o browser normalmente (CI instala o próprio).
    launchOptions: process.env.PW_EXECUTABLE_PATH
      ? { executablePath: process.env.PW_EXECUTABLE_PATH }
      : undefined,
  },
  webServer: {
    // Nota: sem `--` antes de `--port` — o pnpm repassa flags direto ao vite;
    // com `--` o vite recebe o argumento como posicional e ignora a porta.
    command: 'pnpm dev --port 5174',
    url: 'http://127.0.0.1:5174',
    reuseExistingServer: !process.env.CI,
    timeout: 30_000,
  },
})
