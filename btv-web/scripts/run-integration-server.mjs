// Sobe o `forge dashboard` REAL (processo Rust) para os e2e de integração do
// BuildToValue em tests/e2e-integration/. Não é vite dev + proxy — é o
// binário de produção servindo o build real de btv-web/dist na raiz (os 12
// modelos de squad vêm embutidos no binário, `GET /api/btv/templates`).
//
// Chamado pelo `webServer.command` de playwright.integration.config.ts;
// Playwright espera a URL de health check e mata este processo (que repassa
// o sinal ao `cargo run` filho) ao final da suíte. Mesmo desenho do harness
// do console Forge (web/scripts/run-integration-server.mjs).

import { spawn, spawnSync } from 'node:child_process'
import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const __dirname = fileURLToPath(new URL('.', import.meta.url))
const repoRoot = resolve(__dirname, '..', '..')
const btvDist = resolve(__dirname, '..', 'dist')
// Porta própria (7998) — a suíte do console Forge usa 7999; podem coexistir.
const port = process.env.FORGE_E2E_PORT ?? '7998'

function run(cmd, args) {
  const result = spawnSync(cmd, args, { cwd: repoRoot, stdio: 'inherit' })
  if (result.status !== 0) {
    process.exit(result.status ?? 1)
  }
}

// 1. garante o binário do CLI compilado.
run('cargo', ['build', '-p', 'forge-cli'])

// 2. diretório de trabalho isolado (.forge/ próprio, longe de qualquer outra
// execução).
const workDir = mkdtempSync(join(tmpdir(), 'btv-e2e-'))
mkdirSync(join(workDir, '.forge'), { recursive: true })

// `forge.toml` com passos curtos e determinísticos: o squad roda /verify
// ANTES de cada tarefa (evidência para o auditor, ADR 0008) — sem isto, a
// ativação tentaria os passos default (cargo test/clippy reais) dentro do
// tmp dir. Mesma receita do harness do console (web/, Onda 11).
writeFileSync(
  join(workDir, 'forge.toml'),
  '[[step]]\nname = "passo-um"\nprogram = "sh"\nargs = ["-c", "sleep 0.1"]\n',
)

// 3. sobe o dashboard real. FORGE_SCRIPTED=1 troca o gerador por respostas
// determinísticas (sem API key) — o squad ativado pela UI roda o caminho
// real com o ScriptedSquadCoreBackend. Keys de provider isoladas do ambiente
// do runner (mesma razão do harness do console, Fase 7 Onda 12).
const {
  ANTHROPIC_API_KEY: _ignoredAnthropicKey,
  DEEPSEEK_API_KEY: _ignoredDeepseekKey,
  OPENAI_API_KEY: _ignoredOpenaiKey,
  ...envWithoutProviderKeys
} = process.env

const manifestPath = join(repoRoot, 'Cargo.toml')
const child = spawn(
  'cargo',
  ['run', '-q', '--manifest-path', manifestPath, '-p', 'forge-cli', '--', 'dashboard', '--port', port],
  {
    cwd: workDir,
    env: {
      ...envWithoutProviderKeys,
      FORGE_WEB_DIR: btvDist,
      FORGE_SCRIPTED: '1',
      ANTHROPIC_API_KEY: 'e2e-fake-anthropic-key',
    },
    stdio: 'inherit',
  },
)

function cleanup() {
  rmSync(workDir, { recursive: true, force: true })
}

child.on('exit', (code) => {
  cleanup()
  process.exit(code ?? 0)
})
for (const sig of ['SIGTERM', 'SIGINT']) {
  process.on(sig, () => child.kill(sig))
}
