import { describe, expect, it } from 'vitest'

/** Lint de marca (identity-system.md §6/§7). Duas regras duras, executadas no
 *  job `btv-web` do CI junto do restante do vitest:
 *
 *   1. Terracota (`--decision` / #A85B3F / #8A4832 / classes .btn-decision e
 *      .status-gate) é EXCLUSIVA de decisão humana (gates, aprovações). Só pode
 *      aparecer nos arquivos da allowlist abaixo — cada um é um contexto de
 *      gate/aprovação, com o motivo anotado. Introduzir terracota em qualquer
 *      outro arquivo falha o teste: é o mecanismo de "exceção documentada" —
 *      para permitir, adicione o arquivo aqui COM a justificativa.
 *
 *   2. O logo nunca recebe `transform`/`filter` na UI (folha de identidade:
 *      "sem transform, sem filter, gate sempre no topo"). Vale para os
 *      elementos que renderizam os assets do logo — não para o desenho interno
 *      dos próprios SVGs (que podem usar transform para compor a arte). */

// Conteúdo de cada fonte TS/TSX como string (Vite `?raw`), sem depender de
// node:fs — roda igual no vitest/jsdom e no `tsc -b` (types: vite/client).
// Escopo TS/TSX de propósito: é onde a UI usa cor; o global.css apenas DEFINE
// os tokens e as classes .btn-decision/.status-gate (sempre permitido).
const RAW = import.meta.glob('./**/*.{ts,tsx}', {
  query: '?raw',
  import: 'default',
  eager: true,
}) as Record<string, string>

/** path relativo a src/ (sem './' e sem os próprios testes) → conteúdo. */
const SOURCES: Record<string, string> = Object.fromEntries(
  Object.entries(RAW)
    .map(([k, v]) => [k.replace(/^\.\//, ''), v] as const)
    .filter(([k]) => !/\.test\.tsx?$/.test(k)),
)

/** Arquivos onde terracota É permitida — cada um é contexto de gate/aprovação.
 *  A chave é o caminho relativo a src/; o valor documenta a exceção. */
const TERRACOTA_ALLOWLIST: Record<string, string> = {
  'components/screens/user/Vivo.tsx': 'gate humano ao vivo: ponto/cartão do gate, botão de aprovar, chip "gate humano"',
  'components/screens/user/Minhas.tsx': 'linha com gate aberto: ação "Revisar" e chip "aguardando você" (.status-gate)',
  'components/wizard/Wizard.tsx': 'passo "entregas & gates": marcador dos pontos de aprovação humana',
  'components/shell/Sidebar.tsx': 'badge "1 gate" — sinaliza gate aberto aguardando o humano',
  'components/screens/admin/Permissoes.tsx': 'confirmação de mudança de permissão — decisão humana governada',
  'components/screens/admin/Ledger.tsx': 'coluna de ator: decisão humana (gate/aprovação) vs. execução da máquina',
  'designer/btvPlugin.tsx': 'nó "Gate humano" do canvas BPMN — bloco de aprovação humana',
}

/** Casa terracota em qualquer forma de uso na UI: token, hex ou as classes
 *  utilitárias que só existem para decisão humana (case-insensitive). */
const TERRACOTA = /var\(--decision(ink)?\)|#a85b3f|#8a4832|\b(btn-decision|status-gate)\b/i

/** Assets do logo — usá-los com transform/filter é proibido. */
const LOGO_ASSET = /logo-(principal|reduzido|monocromatico|institucional|fundo-escuro)|favicon\.svg|avatar\.svg/

describe('lint de marca — terracota (--decision) só em gate/aprovação', () => {
  it('nenhum arquivo fora da allowlist usa terracota', () => {
    const infratores = Object.entries(SOURCES)
      .filter(([rel, src]) => !(rel in TERRACOTA_ALLOWLIST) && TERRACOTA.test(src))
      .map(([rel]) => rel)
    expect(
      infratores,
      `Terracota (--decision) é exclusiva de decisão humana. Fora de contexto de ` +
        `gate/aprovação em: ${infratores.join(', ')}. Se for realmente um gate, ` +
        `adicione o arquivo à TERRACOTA_ALLOWLIST com a justificativa.`,
    ).toEqual([])
  })

  it('a allowlist não tem entradas mortas (todo contexto declarado ainda usa terracota)', () => {
    const mortas = Object.keys(TERRACOTA_ALLOWLIST).filter((rel) => !TERRACOTA.test(SOURCES[rel] ?? ''))
    expect(mortas, `Entradas da allowlist sem terracota (remova): ${mortas.join(', ')}`).toEqual([])
  })
})

describe('lint de marca — logo sem transform/filter', () => {
  it('nenhum elemento que renderiza o logo carrega transform/filter', () => {
    const infratores: string[] = []
    for (const [rel, src] of Object.entries(SOURCES)) {
      if (!rel.endsWith('.tsx')) continue
      for (const tag of src.match(/<img\b[^>]*>/g) ?? []) {
        if (LOGO_ASSET.test(tag) && /\b(transform|filter)\b/.test(tag)) infratores.push(`${rel}: ${tag.trim()}`)
      }
    }
    expect(
      infratores,
      `Logo não pode receber transform/filter na UI (folha de identidade): ${infratores.join(' | ')}`,
    ).toEqual([])
  })
})
