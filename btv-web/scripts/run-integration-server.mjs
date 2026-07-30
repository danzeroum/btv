// Sobe o `btv dashboard` REAL (processo Rust) para os e2e de integração do
// BuildToValue em tests/e2e-integration/. Não é vite dev + proxy — é o
// binário de produção servindo o build real de btv-web/dist na raiz (os 12
// modelos de squad vêm embutidos no binário, `GET /api/btv/templates`).
//
// Chamado pelo `webServer.command` de playwright.integration.config.ts, que
// declara `gracefulShutdown` com SIGTERM — sem isso o Playwright SIGKILLa o
// grupo e nada aqui roda. Ao receber o sinal, o teardown daqui mata a
// descendência INTEIRA por PID (ver `descendentesVistos` abaixo), não só o
// `cargo run` filho: o sidecar Python vive em grupo de processo próprio e
// vazaria. Mesmo desenho do harness do console BuildToValue
// (web/scripts/run-integration-server.mjs).

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

let encerrando = false

child.on('exit', (code) => {
  // Durante o teardown quem decide a hora de sair é `encerrar()`: o `cargo`
  // morre ANTES do sidecar, e sair aqui deixaria a descendência viva.
  if (encerrando) return
  cleanup()
  process.exit(code ?? 0)
})

/** Descendência do dashboard, rastreada ENQUANTO ele roda (pid -> linha de
 * comando vista no snapshot).
 *
 * Duas razões para rastrear em vez de varrer só no teardown:
 *
 * 1. O sidecar Python é spawnado pelo `btv-sidecar` com `process_group(0)`
 *    (supervisor.rs:46, squad_client.rs:141, memory_client.rs:118) — ele é
 *    líder do PRÓPRIO grupo, então o kill de grupo que o Playwright dispara no
 *    fim da suíte nunca o alcança. Alguém precisa matá-lo por PID.
 * 2. O `btv dashboard` não instala handler de sinal (não há
 *    `tokio::signal`/`with_graceful_shutdown` no workspace), então morre na
 *    ação default no MESMO instante que este script recebe o sinal, e a árvore
 *    é reparentada para o init. Uma varredura por PPID feita depois disso não
 *    acha mais nada — por isso o retrato é periódico, tirado enquanto a árvore
 *    ainda existe.
 *
 * Sem isso, `uv` + `python3` sobrevivem à suíte, seguram o lock do cache do uv
 * e o `uv cache prune` do post-step do `astral-sh/setup-uv` trava 5min e
 * reprova o job DEPOIS de todos os testes passarem (achado real: jobs
 * `web`/`btv-web`, runs 30421310025 e 30582693068).
 *
 * Nota: isto só roda porque a config declara `webServer.gracefulShutdown` com
 * SIGTERM. No default do Playwright o grupo leva SIGKILL e nenhum handler daqui
 * é executado.
 */
const descendentesVistos = new Map()

/** Um `ps` só: devolve pid -> linha de comando e registra a descendência viva. */
function snapshotDescendentes() {
  const argsPorPid = new Map()
  let saida = ''
  try {
    saida = execSync('ps -eo pid=,ppid=,args=', {
      stdio: ['ignore', 'pipe', 'ignore'],
      maxBuffer: 8 * 1024 * 1024,
    }).toString()
  } catch {
    return argsPorPid
  }
  const filhosPorPai = new Map()
  for (const linha of saida.split('\n')) {
    const campos = linha.match(/^\s*(\d+)\s+(\d+)\s+(.*\S)\s*$/)
    if (!campos) continue
    const pid = Number(campos[1])
    const ppid = Number(campos[2])
    argsPorPid.set(pid, campos[3])
    if (!filhosPorPai.has(ppid)) filhosPorPai.set(ppid, [])
    filhosPorPai.get(ppid).push(pid)
  }
  const fila = [child.pid]
  while (fila.length > 0) {
    for (const pid of filhosPorPai.get(fila.pop()) ?? []) {
      if (pid === process.pid) continue // impossível (somos ancestrais), mas nunca em nós mesmos
      descendentesVistos.set(pid, argsPorPid.get(pid) ?? '')
      fila.push(pid)
    }
  }
  return argsPorPid
}

const rastreio = setInterval(snapshotDescendentes, 500)
// não segura o event loop sozinho — quem mantém o processo vivo é o `child`
rastreio.unref()

function vivo(pid) {
  try {
    process.kill(pid, 0)
    return true
  } catch {
    return false
  }
}

function encerrar(sig) {
  if (encerrando) return
  encerrando = true
  clearInterval(rastreio)
  // último retrato, tirado antes de qualquer kill: pega o que nasceu depois do
  // tick anterior e dá a lista de comandos atual para conferir identidade.
  const atuais = snapshotDescendentes()

  // Só mata PID cuja linha de comando ainda é a registrada — guarda contra
  // reuso de PID entre o retrato e o teardown.
  const alvos = [...descendentesVistos]
    .filter(([pid, args]) => atuais.get(pid) === args)
    .map(([pid]) => pid)

  for (const pid of [...alvos, child.pid]) {
    try { process.kill(pid, sig) } catch { /* já morreu */ }
  }

  const inicio = Date.now()
  const relogio = setInterval(() => {
    const restantes = [...alvos, child.pid].filter(vivo)
    const decorrido = Date.now() - inicio
    if (restantes.length === 0 || decorrido >= 4000) {
      clearInterval(relogio)
      cleanup()
      process.exit(0)
    }
    // `uv` ignora SIGTERM enquanto espera o filho Python
    if (decorrido >= 1500) {
      for (const pid of restantes) {
        try { process.kill(pid, 'SIGKILL') } catch { /* já morreu */ }
      }
    }
  }, 100)
}

for (const sig of ['SIGTERM', 'SIGINT']) {
  process.on(sig, () => encerrar(sig))
}
