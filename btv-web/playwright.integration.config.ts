import { defineConfig } from '@playwright/test'

export default defineConfig({
  testDir: './tests/e2e-integration',
  timeout: 60_000,
  // O dashboard real compartilha um único .btv/ — specs em série, como na
  // suíte do console BuildToValue.
  fullyParallel: false,
  reporter: 'list',
  use: {
    baseURL: 'http://127.0.0.1:7998',
    trace: 'retain-on-failure',
    launchOptions: process.env.PW_EXECUTABLE_PATH
      ? { executablePath: process.env.PW_EXECUTABLE_PATH }
      : undefined,
  },
  webServer: {
    command: 'node scripts/run-integration-server.mjs',
    url: 'http://127.0.0.1:7998/api/btv/templates',
    reuseExistingServer: false,
    timeout: 180_000,
  },
})
