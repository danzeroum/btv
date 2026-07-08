import { fetchJson } from './client'

/** Rotas do produto BuildToValue (`btv-cli::btv_agent`). A ativação roda o
 *  MESMO motor do squad (`/api/squad/*`) — estas rotas somam a montagem do
 *  briefing, o ledger (`btv.*`) e a persistência de runs. */

export interface RespostaBriefing {
  label: string
  resposta: string
}

export interface AtivarSquadPayload {
  template_id: string
  nome?: string
  briefing: RespostaBriefing[]
  refs: string[]
  papeis_off: number[]
}

export interface AtivarSquadResponse {
  task_id: string
  run_id: number
}

export function ativarSquad(payload: AtivarSquadPayload): Promise<AtivarSquadResponse> {
  return fetchJson<AtivarSquadResponse>('/api/btv/squads', {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(payload),
  })
}

/** Espelho de `btv_store::BtvRun` (GET /api/btv/squads). */
export interface BtvRun {
  id: number
  task_id: string
  template_id: string
  template_versao: string
  nome: string
  briefing_json: string
  papeis_json: string
  status: 'ativa' | 'concluida' | 'encerrada' | 'erro'
  created_ts: string
  updated_ts: string
}

export function listRuns(): Promise<BtvRun[]> {
  return fetchJson<BtvRun[]>('/api/btv/squads')
}

/** Aprova o gate HITL pendente com auditoria (`btv.gate_approved`). */
export async function aprovarGate(taskId: string, etapa: string): Promise<void> {
  await fetchJson(`/api/btv/squads/${encodeURIComponent(taskId)}/gate`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ etapa }),
  })
}

/** "Pedir ajuste": a instrução vira orientação REAL do cockpit (injetada no
 *  próximo Generate do agente ativo) e o gate é liberado com ela em contexto
 *  (`btv.adjust_requested` no ledger). */
export async function pedirAjuste(taskId: string, instrucao: string, etapa: string): Promise<void> {
  await fetchJson(`/api/btv/squads/${encodeURIComponent(taskId)}/ajuste`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ instrucao, etapa }),
  })
}

/** Espelho de `btv_store::BtvDeliverable` (GET /api/btv/deliverables). */
export interface BtvDeliverable {
  id: number
  run_id: number
  task_id: string
  template_id: string
  nome: string
  path: string
  formato: string
  versao: string
  trilha: string
  created_ts: string
}

export function listDeliverables(): Promise<BtvDeliverable[]> {
  return fetchJson<BtvDeliverable[]>('/api/btv/deliverables')
}

export function deliverableDownloadUrl(id: number): string {
  return `/api/btv/deliverables/${id}/download`
}

// ── personas (U7) ──

export interface PersonaView {
  papel: string
  /** Prompt EFETIVO (override ?? padrão) — o que a próxima ativação usa. */
  prompt: string
  padrao: string
  editado: boolean
}

export interface CustomPersona {
  id: number
  template_id: string
  nome: string
  prompt: string
}

export interface PersonasResponse {
  template_id: string
  personas: PersonaView[]
  proprias: CustomPersona[]
}

export function fetchPersonas(templateId: string): Promise<PersonasResponse> {
  return fetchJson<PersonasResponse>(`/api/btv/personas/${encodeURIComponent(templateId)}`)
}

export async function setPersonaOverride(templateId: string, papel: string, prompt: string): Promise<void> {
  await fetchJson(`/api/btv/personas/${encodeURIComponent(templateId)}/${encodeURIComponent(papel)}`, {
    method: 'PUT',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ prompt }),
  })
}

export async function restorePersona(templateId: string, papel: string): Promise<void> {
  await fetchJson(`/api/btv/personas/${encodeURIComponent(templateId)}/${encodeURIComponent(papel)}`, {
    method: 'DELETE',
  })
}

export async function restoreAllPersonas(templateId: string): Promise<void> {
  await fetchJson(`/api/btv/personas/${encodeURIComponent(templateId)}`, { method: 'DELETE' })
}

export async function createCustomPersona(templateId: string, nome: string, prompt: string): Promise<number> {
  const r = await fetchJson<{ id: number }>(`/api/btv/personas/${encodeURIComponent(templateId)}/custom`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ nome, prompt }),
  })
  return r.id
}

export async function updateCustomPersona(templateId: string, id: number, nome: string, prompt: string): Promise<void> {
  await fetchJson(`/api/btv/personas/${encodeURIComponent(templateId)}/custom/${id}`, {
    method: 'PUT',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ nome, prompt }),
  })
}

export async function deleteCustomPersona(templateId: string, id: number): Promise<void> {
  await fetchJson(`/api/btv/personas/${encodeURIComponent(templateId)}/custom/${id}`, { method: 'DELETE' })
}
