import { fetchJson } from './client'

/** Clients das telas de Administração (A1–A6) — todas as rotas são reais e
 *  já existentes no BuildToValue, exceto publicação de template e perfis locais
 *  (rotas btv novas). */

// A1 · telemetria
export interface TelemetrySummary {
  total_events: number
  by_name: Record<string, number>
  cache_hit_rate: number | null
}
export const fetchSummary = () => fetchJson<TelemetrySummary>('/api/summary')

// A2 · ledger
export interface LedgerEntry {
  seq: number
  prev_hash: string
  entry_hash: string
  kind: string
  actor: string
  payload: Record<string, unknown>
  ts: string
}
export const fetchLedger = (limit = 40) => fetchJson<LedgerEntry[]>(`/api/ledger?limit=${limit}`)
export const verifyLedger = () =>
  fetchJson<{ ok: boolean; verified: number }>('/api/ledger/verify', { method: 'POST' })

// A3 · providers & rate limits
export interface ProviderInfo {
  id: string
  configured: boolean
}
export const fetchProviders = () => fetchJson<ProviderInfo[]>('/api/providers')
export interface RateLimitEntry {
  tier: string
  cap: number
  window_secs: number
}
export const fetchRateLimits = () => fetchJson<RateLimitEntry[]>('/api/ratelimit')

// A4 · permissões (matriz real de btv_core::{BUILD,PLAN} + overrides)
export type Decision = 'allow' | 'ask' | 'deny'
export interface MatrixRow {
  tool: string
  build: Decision
  plan: Decision
}
export const fetchMatrix = () => fetchJson<MatrixRow[]>('/api/permissions/matrix')
export interface RuleRecord {
  id: number
  profile: string
  tool: string
  scope_prefix?: string
  decision: Decision
  created_at: string
}
export const fetchRules = () => fetchJson<RuleRecord[]>('/api/permissions/rules')
export const setRule = (profile: string, tool: string, decision: Decision) =>
  fetchJson('/api/permissions/rules', {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ profile, tool, decision }),
  })
export const revokeRule = (id: number) =>
  fetchJson(`/api/permissions/rules/${id}`, { method: 'DELETE' })

// A5 · publicação de templates
export interface TemplatePub {
  template_id: string
  publicado: boolean
}
export const fetchPublicacao = () => fetchJson<TemplatePub[]>('/api/btv/templates/publicacao')
export const setPublicacao = (templateId: string, publicado: boolean) =>
  fetchJson(`/api/btv/templates/${encodeURIComponent(templateId)}/publicacao`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ publicado }),
  })

// A6 · perfis locais
export interface BtvUser {
  id: number
  nome: string
  email: string
  papel: string
  ativo: boolean
}
export const fetchUsers = () => fetchJson<BtvUser[]>('/api/btv/users')
export const createUser = (nome: string, email: string, papel: string) =>
  fetchJson<{ id: number }>('/api/btv/users', {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ nome, email, papel }),
  })
export const setUserAtivo = (id: number, ativo: boolean) =>
  fetchJson(`/api/btv/users/${id}/ativo`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ ativo }),
  })
