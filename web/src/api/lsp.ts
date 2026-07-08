/**
 * Fase 7 Onda 10 (A7): language servers declarados em `.btv/lsp.toml`.
 * `GET /api/lsp` mora no router mesclado de `btv-cli` (precisa de
 * `btv-tools`). **Zero probe sob demanda**: enumera a config, nunca sobe
 * um processo para checar status — cada servidor é sempre "declarado, não
 * iniciado" (não há como saber se algum OUTRO processo já o usou sem
 * introspectar estado entre processos, fora do escopo desta fase).
 */
import { fetchJson } from './client'

export interface LspServerInfo {
  id: string
  command: string
  args: string[]
}

export async function fetchLspServers(): Promise<LspServerInfo[]> {
  return fetchJson<LspServerInfo[]>('/api/lsp')
}
