import { defineConfig } from '@playwright/test'

/** Config separada da padrão (playwright.config.ts): aqui não é vite dev +
 * proxy, é o `btv dashboard` real (Rust, sqlite de verdade) — ver
 * scripts/run-integration-server.mjs. Roda via `pnpm test:e2e:integration`.
 */
export default defineConfig({
  testDir: './tests/e2e-integration',
  timeout: 30_000,
  fullyParallel: false,
  reporter: 'list',
  use: {
    baseURL: 'http://127.0.0.1:7999',
    trace: 'retain-on-failure',
    // Ambientes com Chromium pré-instalado fora do cache do Playwright
    // apontam o binário por env var (mesmo mecanismo do btv-web).
    launchOptions: process.env.PW_EXECUTABLE_PATH
      ? { executablePath: process.env.PW_EXECUTABLE_PATH }
      : undefined,
  },
  webServer: {
    command: 'node scripts/run-integration-server.mjs',
    url: 'http://127.0.0.1:7999/api/summary',
    reuseExistingServer: false,
    timeout: 180_000,
    // Sem isto o Playwright faz `process.kill(-pid, 'SIGKILL')` no grupo
    // ("If unspecified, the process group is forcefully SIGKILLed" — types/
    // test.d.ts), o handler de teardown de scripts/run-integration-server.mjs
    // NUNCA roda, e o sidecar Python (que vive em grupo próprio) vaza. Com
    // SIGTERM o handler roda e varre a descendência por PID; os 15s são folga
    // sobre os ~4s de pior caso dele.
    gracefulShutdown: { signal: 'SIGTERM', timeout: 15_000 },
  },
})
