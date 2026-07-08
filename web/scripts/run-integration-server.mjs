// Sobe o `btv dashboard` REAL (processo Rust, sqlite de verdade) para os
// e2e de integração em tests/e2e-integration/ (telemetria, permissões,
// squad, ledger). Não é vite dev + proxy — é o binário que roda em
// produção, servindo o build real de web/dist. Semeia dados via
// btv-store::Telemetry/LedgerStore (os mesmos caminhos reais que
// llm.call/tool.result e a CLI usam), nunca SQL cru.
//
// Chamado pelo `webServer.command` de playwright.integration.config.ts;
// Playwright espera a URL de health check e mata este processo (que repassa
// o sinal ao `cargo run` filho) ao final da suíte.

import { spawn, spawnSync } from 'node:child_process'
import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const __dirname = fileURLToPath(new URL('.', import.meta.url))
const repoRoot = resolve(__dirname, '..', '..')
const webDist = resolve(__dirname, '..', 'dist')
const port = process.env.BTV_E2E_PORT ?? '7999'

function run(cmd, args) {
  const result = spawnSync(cmd, args, { cwd: repoRoot, stdio: 'inherit' })
  if (result.status !== 0) {
    process.exit(result.status ?? 1)
  }
}

// 1. garante que o binário do CLI, os exemplos de seed e o fixture MCP
// (Fase 6 Onda 4, usado pelo teste do console MCP da Onda 7) estão compilados.
run('cargo', [
  'build', '-p', 'btv-cli', '-p', 'btv-store', '-p', 'btv-tools',
  '--example', 'seed_telemetry', '--example', 'seed_ledger', '--bin', 'btv_mcp_fixture',
])

// 2. diretório de trabalho isolado para o dashboard (.btv/telemetry.db e
// .btv/btv.db próprios, isolados de qualquer outra execução).
const workDir = mkdtempSync(join(tmpdir(), 'btv-e2e-'))
mkdirSync(join(workDir, '.btv'), { recursive: true })
const dbPath = join(workDir, '.btv', 'telemetry.db')
const ledgerPath = join(workDir, '.btv', 'btv.db')

// 3. semeia um evento real via o mesmo Telemetry::record usado em produção.
run('cargo', [
  'run', '-q', '-p', 'btv-store', '--example', 'seed_telemetry', '--',
  dbPath, 'llm.call', 'e2e-integration', '{"provider":"anthropic"}',
])

// 3c. semeia eventos com `model` (Fase 7 Onda 7, A5) — session_id dedicado
// (e2e-model-usage) para não inflar a contagem de linhas de
// "e2e-integration" que o teste de telemetria já conta; modelos com sufixo
// "-e2e" exclusivo desta suíte, mas ainda batendo nos regexes reais de
// `tier_from_id` ("haiku" -> small, "sonnet" -> large), para o teste de Uso
// por Modelo provar a agregação E a classificação de tier de ponta a ponta.
run('cargo', [
  'run', '-q', '-p', 'btv-store', '--example', 'seed_telemetry', '--',
  dbPath, 'llm.call', 'e2e-model-usage', '{"model":"claude-sonnet-5-e2e"}',
])
run('cargo', [
  'run', '-q', '-p', 'btv-store', '--example', 'seed_telemetry', '--',
  dbPath, 'cache.hit', 'e2e-model-usage', '{"model":"claude-sonnet-5-e2e"}',
])
run('cargo', [
  'run', '-q', '-p', 'btv-store', '--example', 'seed_telemetry', '--',
  dbPath, 'llm.call', 'e2e-model-usage', '{"model":"claude-haiku-4-5-e2e"}',
])

// 3b. semeia 2 entradas reais no ledger (mesmo LedgerStore::append usado em
// produção) com um ator dedicado (e2e-ledger-seed) que nenhum outro spec
// usa — o teste de Ledger filtra por ele, então não importa a ordem em que
// os specs desta suíte rodam nem quantas outras entradas (squad/permissões)
// o mesmo btv.db acumular depois.
run('cargo', [
  'run', '-q', '-p', 'btv-store', '--example', 'seed_ledger', '--',
  ledgerPath, 'session.start', 'e2e-ledger-seed', '{"task":"e2e"}', '2026-01-01T00:00:00Z',
])
run('cargo', [
  'run', '-q', '-p', 'btv-store', '--example', 'seed_ledger', '--',
  ledgerPath, 'tool.run', 'e2e-ledger-seed', '{"tool":"bash"}', '2026-01-01T00:00:01Z',
])

// 3d. `.btv/mcp.toml` (Fase 7 Onda 7, A1) com 2 servidores: um apontando
// pro fixture MCP REAL (mesmo bin que `btv-tools/tests/mcp_integration.rs`
// usa — handshake de verdade, não um mock) e um comando inexistente, para o
// teste do console MCP provar os dois status (online/offline) num mesmo probe.
const mcpFixtureBin = join(repoRoot, 'target', 'debug', 'btv_mcp_fixture')
writeFileSync(
  join(workDir, '.btv', 'mcp.toml'),
  `[[server]]\nid = "vivo"\ncommand = "${mcpFixtureBin}"\nargs = []\n\n` +
    `[[server]]\nid = "morto"\ncommand = "/caminho/que/nao/existe/btv-mcp-x"\nargs = []\n`,
)

// 3e. corpus de memória do squad (Fase 7 Onda 8, A3) — semeado DIRETO no
// mesmo caminho relativo que `MemorySupervisor`/`SquadServicer` já usam em
// produção (`<python_workspace_dir>/.btv/squad-memory/agent_memories.jsonl`,
// já que `MemoryService` é construído com `memory_dir: None` — a mesma
// resolução do squad real, ver doc de `MemorySupervisor::spawn`). Um agente
// dedicado (e2e-memory-agent) evita colidir com o que já exista aí — e algo
// JÁ existe: o teste do squad real (`squad-real-backend.spec.ts`) roda um
// orquestrador de verdade, que chama `remember_decision` nesse MESMO
// arquivo. Não há cleanup: esse arquivo já persiste entre execuções hoje
// (`.btv/` é gitignored, então não afeta commits) — sobrescrever com
// `writeFileSync` (não `appendFileSync`) é idempotente por execução, o que
// basta.
const memoryCorpusDir = join(repoRoot, 'python', '.btv', 'squad-memory')
mkdirSync(memoryCorpusDir, { recursive: true })
const memoryCorpusPath = join(memoryCorpusDir, 'agent_memories.jsonl')
writeFileSync(
  memoryCorpusPath,
  `{"timestamp":"2026-01-01T00:00:00Z","agent":"e2e-memory-agent","decision":{"summary":"plano de arquitetura do gateway aprovado"},"confidence":0.9}\n`,
)

// 3f. semeia um experimento A/B real (Fase 7 Onda 9, A2): 2 variantes, 20
// amostras cada — o piso de `MIN_SAMPLES` em `btv_schemas::experiment`
// abaixo do qual o veredito vira `InsufficientData` em vez de decidir por
// significância. "controle" com 18/20 sucessos vs "tratamento" com 6/20 —
// diferença grande o bastante pro teste z ser `Significant` por construção,
// não por sorte. Nome dedicado (e2e-experiment) para o teste de UI buscar
// por ele sem depender de nenhum outro evento desta suíte.
function seedExperimentEvent(variant, success) {
  run('cargo', [
    'run', '-q', '-p', 'btv-store', '--example', 'seed_telemetry', '--',
    dbPath, 'llm.call', 'e2e-experiment',
    JSON.stringify({ experiment: 'e2e-experiment', variant, success }),
  ])
}
for (let i = 0; i < 20; i++) seedExperimentEvent('controle', i < 18)
for (let i = 0; i < 20; i++) seedExperimentEvent('tratamento', i < 6)

// 3g. `.btv/lsp.toml` (Fase 7 Onda 10, A7) com um comando INEXISTENTE —
// prova que a tela enumera o declarado sem nunca tentar subir o processo
// (mesma prova que `skills.rs`'s teste de registro lazy já faz, agora pela
// rota HTTP e pelo browser).
writeFileSync(
  join(workDir, '.btv', 'lsp.toml'),
  '[[server]]\nid = "rust"\ncommand = "comando-lsp-inexistente-xyz"\nargs = ["--stdio"]\n',
)

// 3h. skill de TERCEIRO real (Fase 7 Onda 10, A6) em `.btv/skills/` — vetada
// e aprovada (sem padrão perigoso), para a tela de sandbox mostrar a lista
// real via `/api/skills` (filtrando `source === 'third-party'`), não uma
// lista vazia.
const thirdPartySkillDir = join(workDir, '.btv', 'skills', 'eco-terceiro')
mkdirSync(thirdPartySkillDir, { recursive: true })
writeFileSync(
  join(thirdPartySkillDir, 'skill.toml'),
  'name = "eco-terceiro"\ndescription = "eco simples para prova da tela de sandbox"\n' +
    'entrypoint = \'echo "oi"\'\npermissions = []\n',
)

// 3i. `btv.toml` na RAIZ do workDir (Fase 7 Onda 11) — não em `.btv/`:
// `/api/verify/run` resolve `btv.toml` contra `state.root`, que é o `cwd`
// do processo do dashboard (`workDir` aqui), o mesmo lugar que `btv
// verify` (CLI) já olha. Passos curtos e determinísticos (não os comandos
// reais de `default_steps()`, que levariam minutos) — o teste prova
// progresso real via polling, não que o cargo real roda de novo aqui.
writeFileSync(
  join(workDir, 'btv.toml'),
  '[[step]]\nname = "passo-um"\nprogram = "sh"\nargs = ["-c", "sleep 0.2"]\n\n' +
    '[[step]]\nname = "passo-dois"\nprogram = "sh"\nargs = ["-c", "sleep 0.2"]\n',
)

// 4. sobe o dashboard real apontando pro build da SPA, servindo o evento semeado.
// --manifest-path resolve o workspace a partir de workDir (cargo não muda o
// cwd do processo filho); run_dashboard lê `.btv/telemetry.db` relativo ao
// cwd real do binário, por isso `cwd: workDir` aqui. Fase 7 Onda 15 (fecho):
// as rotas de sessão/permissão/matriz/squad vêm HABILITADAS por padrão agora
// (antes exigia `--web-agent` explícito, Ondas 1-4) — nenhuma flag extra
// necessária aqui. `BTV_SCRIPTED=1` troca o
// gerador por respostas determinísticas (sem API key) tanto na sessão de chat
// quanto no squad (`ScriptedSquadCoreBackend`, mesma confiança 0.5 uniforme
// do teste Rust — consenso fraco de propósito, exercita o gate HITL real);
// nenhum teste de integração hoje envia mensagem de chat, então isso não
// muda o comportamento observado pelos specs existentes.
// Fase 7 Onda 12: as 3 chaves de provider são removidas do env herdado e só
// ANTHROPIC_API_KEY é redefinida com um valor fake — determinístico
// independente do que o runner (CI ou dev local) tenha de verdade no
// ambiente, para `GET /api/providers` (`Gateway::from_env`) provar
// exatamente 1 `configured: true` (anthropic) e 2 `false`.
const {
  ANTHROPIC_API_KEY: _ignoredAnthropicKey,
  DEEPSEEK_API_KEY: _ignoredDeepseekKey,
  OPENAI_API_KEY: _ignoredOpenaiKey,
  ...envWithoutProviderKeys
} = process.env

const manifestPath = join(repoRoot, 'Cargo.toml')
const child = spawn(
  'cargo',
  [
    'run', '-q', '--manifest-path', manifestPath, '-p', 'btv-cli', '--',
    'dashboard', '--port', port,
  ],
  {
    cwd: workDir,
    env: {
      ...envWithoutProviderKeys,
      BTV_WEB_DIR: webDist,
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
