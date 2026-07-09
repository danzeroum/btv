/**
 * Fase 7 Onda 11: pipeline `/verify` real, rodando em background no `btv
 * dashboard`. `POST /api/verify/run` dispara o job (mesma config que `btv
 * verify`: `btv.toml` na raiz, ou `default_steps()` — espelha o job
 * `rust` do CI) e devolve um `run_id`; `GET /api/verify/:id` (polling)
 * acompanha o progresso real passo a passo até o veredito final. Execuções
 * concorrentes são serializadas: um segundo `POST` com job ativo devolve
 * `409` com o `run_id` já em andamento — o cliente trata 202 e 409 igual
 * (os dois dão um `run_id` para acompanhar via polling).
 */
import { ApiError, fetchJson } from './client'

/** Review por valor DERIVADO da evidência real do `/verify` — espelha
 * `btv_schemas::review::ValueReview`. Só as dimensões determinísticas
 * (technical/security) + os gates duros; performance/value dependem de
 * agente e NÃO são fabricadas aqui. */
export type GateTriggered = 'critical_finding' | 'verify_fail' | 'security_floor'

export interface ValueReview {
  technical: number
  security: number
  gates_passed: boolean
  gate_triggered?: GateTriggered
  reason: string
}

export interface Finding {
  tool: string
  severity: string
  message: string
  file?: string
  line?: number
}

export interface VerificationStep {
  name: string
  tool: string
  exit_code: number
  duration_ms: number
  findings: Finding[]
}

export type Verdict = 'pass' | 'fail' | 'skipped'

/** Espelha `btv_schemas::verification::VerificationEvidence`. */
export interface VerificationEvidence {
  run_id: string
  git_sha: string
  steps: VerificationStep[]
  verdict: Verdict
  produced_at: string
}

export interface VerifyRunStarted {
  run_id: string
}

export type VerifyStatus =
  | { status: 'running'; run_id: string; step: number; total: number }
  | { status: 'done'; run_id: string; evidence: VerificationEvidence; review: ValueReview }
  // Panic interno no pipeline (capturado no servidor via catch_unwind) —
  // terminal como 'done', mas sem evidência para mostrar.
  | { status: 'failed'; run_id: string; message: string }

export async function startVerifyRun(): Promise<VerifyRunStarted> {
  let response: Response
  try {
    response = await fetch('/api/verify/run', { method: 'POST' })
  } catch {
    throw new ApiError('falha de rede em /api/verify/run', 'network_error')
  }
  if (response.status === 202 || response.status === 409) {
    return (await response.json()) as VerifyRunStarted
  }
  throw new ApiError(`/api/verify/run respondeu ${response.status}`, `http_${response.status}`)
}

export async function fetchVerifyStatus(runId: string): Promise<VerifyStatus> {
  return fetchJson<VerifyStatus>(`/api/verify/${encodeURIComponent(runId)}`)
}
