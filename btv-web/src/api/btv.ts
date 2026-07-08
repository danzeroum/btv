import { fetchJson } from './client'

/** Rotas do produto BuildToValue (`forge-cli::btv_agent`). A ativação roda o
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

/** Espelho de `forge_store::BtvRun` (GET /api/btv/squads). */
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
