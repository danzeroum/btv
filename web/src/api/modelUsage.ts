/**
 * Fase 7 Onda 7 (A5): uso por modelo. `GET /api/models/usage` mora direto em
 * `btv-server` (só depende do que o crate já depende — `btv-store` +
 * `btv-llm`, este último já usado pelo bin `loadgen`). Nome do módulo não
 * é `models.ts` — já ocupado pela tela `modelo` de usuário (seleção de
 * tier/agente).
 */
import { fetchJson } from './client'
import type { ModelTierId } from '../types/domain'

export interface ModelUsageEntry {
  model: string
  tier: ModelTierId
  calls: number
  cache_hits: number
  cache_misses: number
  input_tokens: number
  output_tokens: number
  /** Provider do preço tabelado; ausente quando o modelo não tem preço. */
  provider?: string
  /** Custo estimado (USD) = tokens reais × preço tabelado; ausente sem preço. */
  estimated_cost_usd?: number
}

export interface ModelUsageResponse {
  entries: ModelUsageEntry[]
  total_estimated_cost_usd: number
  /** Data de referência da tabela de preços estática (a estimativa envelhece). */
  pricing_as_of: string
}

export async function fetchModelUsage(): Promise<ModelUsageResponse> {
  return fetchJson<ModelUsageResponse>('/api/models/usage')
}
