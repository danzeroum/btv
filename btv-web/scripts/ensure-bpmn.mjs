// Garante o build da lib bpmn (submodule vendor/bpmn, pinado) antes do build
// do app — os aliases do vite/tsconfig apontam para os dist ESM dela. Se o
// dist já existe, não faz nada (barato em dev); no CI, o submodule é
// clonado e construído aqui.
import { spawnSync } from 'node:child_process'
import { existsSync } from 'node:fs'
import { resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const __dirname = fileURLToPath(new URL('.', import.meta.url))
const vendorRoot = resolve(__dirname, '..', '..', 'vendor', 'bpmn')

if (!existsSync(resolve(vendorRoot, 'package.json'))) {
  console.error(
    'vendor/bpmn vazio — rode `git submodule update --init` na raiz do repo (o Designer depende da lib).',
  )
  process.exit(1)
}

const marker = resolve(vendorRoot, 'packages', 'registry', 'dist', 'esm', 'index.js')
if (existsSync(marker)) {
  process.exit(0)
}

for (const args of [['install', '--frozen-lockfile'], ['build']]) {
  const r = spawnSync('pnpm', args, { cwd: vendorRoot, stdio: 'inherit' })
  if (r.status !== 0) process.exit(r.status ?? 1)
}
