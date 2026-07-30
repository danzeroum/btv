// Sobe o `btv dashboard` REAL (processo Rust) para os e2e de integração do
// BuildToValue em tests/e2e-integration/. Não é vite dev + proxy — é o
// binário de produção servindo o build real de btv-web/dist na raiz (os 12
// modelos de squad vêm embutidos no binário, `GET /api/btv/templates`).
//
// Chamado pelo `webServer.command` de playwright.integration.config.ts;
// Playwright espera a URL de health check e mata este processo ao final da
// suíte — e o teardown daqui varre a descendência INTEIRA (ver
// `descendentes` abaixo), não só o `cargo run` filho: o sidecar Python vive
// em grupo de processo próprio e vazaria. Mesmo desenho do harness do
// console BuildToValue (web/scripts/run-integration-server.mjs).

import { spawn, spawnSync, execSync } from 'node:child_process'
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

/** PIDs de TODA a descendência de `raiz`, do mais fundo para o mais raso.
 *
 * Andar por PPID (e não sinalizar um grupo) é obrigatório aqui: o sidecar
 * Python é spawnado pelo `btv-sidecar` com `process_group(0)`
 * (supervisor.rs:46, squad_client.rs:141, memory_client.rs:118), ou seja, ele
 * é líder do PRÓPRIO grupo e nenhum kill de grupo mirado no `cargo` o alcança.
 * E o `btv dashboard` não instala handler de SIGTERM/SIGINT — não existe
 * `tokio::signal`/`with_graceful_shutdown` no workspace — então os `impl Drop`
 * que fariam essa limpeza (supervisor.rs:110 e pares) nunca rodam quando o
 * processo morre por sinal. Sem esta varredura, `uv` + `python3` sobrevivem à
 * suíte, seguram o lock do cache do uv, e o `uv cache prune` do post-step do
 * `astral-sh/setup-uv` trava 5min e reprova o job — DEPOIS de todos os testes
 * passarem (achado real: jobs `web`/`btv-web`, run 30421310025).
 */
function descendentes(raiz) {
  const encontrados = []
  const fila = [raiz]
  while (fila.length > 0) {
    const pai = fila.pop()
    let saida = ''
    try {
      saida = execSync(`ps -o pid= --ppid ${pai}`, { stdio: ['ignore', 'pipe', 'ignore'] }).toString()
    } catch {
      // sem filhos (ps sai 1) — nada a coletar
    }
    for (const linha of saida.split('\n')) {
      const pid = Number(linha.trim())
      if (Number.isInteger(pid) && pid > 0) {
        encontrados.push(pid)
        fila.push(pid)
      }
    }
  }
  // do mais fundo para o mais raso: mata o neto antes do pai, para não perder
  // a árvore por reparent no meio da varredura
  return encontrados.reverse()
}

let encerrando = false
for (const sig of ['SIGTERM', 'SIGINT']) {
  process.on(sig, () => {
    if (encerrando) return
    encerrando = true
    // Coletar ANTES de matar: assim que o `cargo`/`dashboard` morre, a árvore
    // é reparentada para o init e deixa de ser rastreável por PPID.
    const alvos = descendentes(child.pid)
    for (const pid of alvos) {
      try { process.kill(pid, sig) } catch { /* já morreu */ }
    }
    try { child.kill(sig) } catch { /* já morreu */ }
    // Carência curta e então SIGKILL no que insistir (o `uv` ignora SIGTERM
    // enquanto espera o filho Python).
    setTimeout(() => {
      for (const pid of [...alvos, child.pid]) {
        try { process.kill(pid, 'SIGKILL') } catch { /* já morreu */ }
      }
      cleanup()
      process.exit(0)
    }, 2000)
  })
}
