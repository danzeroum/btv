// Garante o build da lib bpmn (submodule vendor/bpmn, pinado) antes do build
// do app — os aliases do vite/tsconfig apontam para os dist ESM dela. Se o
// dist já existe, não faz nada (barato em dev); no CI, o submodule é
// clonado e construído aqui.
import { spawnSync } from 'node:child_process'
import { existsSync } from 'node:fs'
import { resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const __dirname = fileURLToPath(new URL('.', import.meta.url))
const repoRoot = resolve(__dirname, '..', '..')
const vendorRoot = resolve(repoRoot, 'vendor', 'bpmn')

if (!existsSync(resolve(vendorRoot, 'package.json'))) {
  // F3 — clone fresco sem `--init` deixava o submodule vazio e o build do
  // Designer falhava. Tentamos inicializá-lo automaticamente antes de desistir;
  // só erramos (fail-closed com mensagem clara) se o git não resolver.
  console.error('vendor/bpmn vazio — inicializando o submodule (git submodule update --init)…')
  const init = spawnSync('git', ['submodule', 'update', '--init', 'vendor/bpmn'], {
    cwd: repoRoot,
    stdio: 'inherit',
  })
  if (init.status !== 0 || !existsSync(resolve(vendorRoot, 'package.json'))) {
    console.error(
      'não consegui inicializar vendor/bpmn — rode `git submodule update --init` na raiz do repo (o Designer depende da lib).',
    )
    process.exit(1)
  }
}

const marker = resolve(vendorRoot, 'packages', 'registry', 'dist', 'esm', 'index.js')
if (existsSync(marker)) {
  process.exit(0)
}

for (const args of [['install', '--frozen-lockfile'], ['build']]) {
  const r = spawnSync('pnpm', args, { cwd: vendorRoot, stdio: 'inherit' })
  if (r.status !== 0) process.exit(r.status ?? 1)
}
