// Sobe o `btv dashboard` REAL (processo Rust) para os e2e de integração do
// BuildToValue em tests/e2e-integration/. Não é vite dev + proxy — é o
// binário de produção servindo o build real de btv-web/dist na raiz (os 12
// modelos de squad vêm embutidos no binário, `GET /api/btv/templates`).
//
// Chamado pelo `webServer.command` de playwright.integration.config.ts;
// Playwright espera a URL de health check e mata este processo (que repassa
// o sinal ao `cargo run` filho) ao final da suíte. Mesmo desenho do harness
// do console BuildToValue (web/scripts/run-integration-server.mjs).

import { spawn, spawnSync } from 'node:child_process'
import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const __dirname = fileURLToPath(new URL('.', import.meta.url))
const repoRoot = resolve(__dirname, '..', '..')
const btvDist = resolve(__dirname, '..', 'dist')
// Porta própria (7998) — a suíte do console BuildToValue usa 7999; podem coexistir.
const port = process.env.BTV_E2E_PORT ?? '7998'

function run(cmd, args) {
  const result = spawnSync(cmd, args, { cwd: repoRoot, stdio: 'inherit' })
  if (result.status !== 0) {
    process.exit(result.status ?? 1)
  }
}

// 1. garante o binário do CLI e o exemplo de seed compilados.
run('cargo', ['build', '-p', 'btv-cli', '-p', 'btv-store', '--example', 'seed_btv'])

// 2. diretório de trabalho isolado (.btv/ próprio, longe de qualquer outra
// execução).
const workDir = mkdtempSync(join(tmpdir(), 'btv-e2e-'))
mkdirSync(join(workDir, '.btv'), { recursive: true })

// `btv.toml` com passos curtos e determinísticos: o squad roda /verify
// ANTES de cada tarefa (evidência para o auditor, ADR 0008) — sem isto, a
// ativação tentaria os passos default (cargo test/clippy reais) dentro do
// tmp dir. Mesma receita do harness do console (web/, Onda 11).
writeFileSync(
  join(workDir, 'btv.toml'),
  '[[step]]\nname = "passo-um"\nprogram = "sh"\nargs = ["-c", "sleep 0.1"]\n',
)

// 2b. semeia um run concluído + entregas REAIS (mesmo BtvStore de
// produção): um artefato MD exportável (arquivo de verdade no disco, o
// download serve o conteúdo real) e um DOCX (o texto é convertido para um
// DOCX real na exportação — serialização determinística, sem sandbox).
const artigoPath = join(workDir, 'artigo-seed.md')
writeFileSync(artigoPath, '# Artigo semeado\n\nconteúdo real do artefato para o download.\n')
const btvDb = join(workDir, '.btv', 'btv.db')
run('cargo', [
  'run', '-q', '-p', 'btv-store', '--example', 'seed_btv', '--',
  btvDb, 'editorial', 'Newsletter seed', artigoPath, 'MD',
])
const docxPath = join(workDir, 'minuta-seed.docx')
writeFileSync(docxPath, 'Minuta juridica seed\nclausula primeira\nclausula segunda')
run('cargo', [
  'run', '-q', '-p', 'btv-store', '--example', 'seed_btv', '--',
  btvDb, 'juridico', 'Minuta seed', docxPath, 'DOCX',
])

// 3. sobe o dashboard real. BTV_SCRIPTED=1 troca o gerador por respostas
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
  ['run', '-q', '--manifest-path', manifestPath, '-p', 'btv-cli', '--', 'dashboard', '--port', port],
  {
    cwd: workDir,
    env: {
      ...envWithoutProviderKeys,
      BTV_WEB_DIR: btvDist,
      BTV_SCRIPTED: '1',
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
