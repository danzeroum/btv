import { useCallback, useEffect, useState } from 'react'
import { Card } from '../../primitives/Card'
import { Button } from '../../primitives/Button'
import { Badge } from '../../primitives/Badge'
import { Gauge } from '../../primitives/Gauge'
import { ProgressBar } from '../../primitives/ProgressBar'
import { usePolling } from '../../../hooks/usePolling'
import { useToast } from '../../primitives/Toast'
import {
  fetchVerifyStatus,
  startVerifyRun,
  type ValueReview,
  type VerificationEvidence,
  type VerifyStatus,
} from '../../../api/verify'

const GATE_LABEL: Record<string, string> = {
  critical_finding: 'finding crítico na evidência',
  verify_fail: 'veredito fail do /verify',
  security_floor: 'segurança abaixo do piso',
}

function VerifyPoller({ runId, onUpdate }: { runId: string; onUpdate: (status: VerifyStatus) => void }) {
  const state = usePolling(() => fetchVerifyStatus(runId), 500)
  useEffect(() => {
    if (state.status === 'success') onUpdate(state.data)
  }, [state, onUpdate])
  return null
}

export function Verify() {
  const toast = useToast()
  const [activeRunId, setActiveRunId] = useState<string | null>(null)
  const [progress, setProgress] = useState<{ step: number; total: number } | null>(null)
  const [evidence, setEvidence] = useState<VerificationEvidence | null>(null)
  const [review, setReview] = useState<ValueReview | null>(null)
  const [starting, setStarting] = useState(false)
  const [expandedStep, setExpandedStep] = useState<string | null>(null)

  const handleStatusUpdate = useCallback(
    (status: VerifyStatus) => {
      if (status.status === 'running') {
        setProgress({ step: status.step, total: status.total })
        return
      }
      if (status.status === 'failed') {
        setProgress(null)
        setActiveRunId(null)
        toast.push('error', `pipeline /verify falhou internamente: ${status.message}`)
        return
      }
      setEvidence(status.evidence)
      setReview(status.review)
      setProgress(null)
      setActiveRunId(null)
      toast.push(status.evidence.verdict === 'pass' ? 'success' : 'error', `pipeline /verify: ${status.evidence.verdict}`)
    },
    [toast],
  )

  async function handleRun() {
    setStarting(true)
    try {
      const { run_id } = await startVerifyRun()
      setProgress({ step: 0, total: 0 })
      setActiveRunId(run_id)
    } catch {
      toast.push('error', 'falha ao iniciar /verify')
    } finally {
      setStarting(false)
    }
  }

  const isRunning = activeRunId !== null

  return (
    <div className="grid grid-2">
      {activeRunId && <VerifyPoller runId={activeRunId} onUpdate={handleStatusUpdate} />}
      <Card>
        <div className="row" style={{ justifyContent: 'space-between' }}>
          <strong>Pipeline /verify</strong>
          <Button onClick={() => void handleRun()} disabled={starting || isRunning}>
            {isRunning ? 'rodando…' : starting ? 'iniciando…' : 'rodar /verify'}
          </Button>
        </div>
        {isRunning && (
          <div style={{ fontSize: 12, color: 'var(--muted)', marginTop: 8 }}>
            {progress && progress.total > 0 ? `passo ${progress.step} de ${progress.total}…` : 'iniciando pipeline…'}
          </div>
        )}
        {!evidence && !isRunning && (
          <div style={{ fontSize: 12, color: 'var(--faint)', marginTop: 8 }}>
            nenhuma execução ainda nesta sessão do dashboard.
          </div>
        )}
        {evidence && (
          <div className="stack" style={{ marginTop: 10 }}>
            {evidence.steps.map((s) => (
              <div key={s.name}>
                <button
                  onClick={() => setExpandedStep(expandedStep === s.name ? null : s.name)}
                  className="row"
                  style={{
                    width: '100%',
                    justifyContent: 'space-between',
                    fontSize: 13,
                    background: 'transparent',
                    border: 'none',
                    color: 'var(--ink)',
                    padding: 0,
                  }}
                >
                  <span>
                    <span style={{ color: s.exit_code === 0 ? 'var(--ok)' : 'var(--red)' }}>
                      {s.exit_code === 0 ? '✓' : '✗'}
                    </span>{' '}
                    {s.name}
                  </span>
                  <span style={{ color: 'var(--muted)', fontSize: 12 }}>
                    {s.duration_ms}ms · {s.findings.length} finding(s) {expandedStep === s.name ? '▾' : '▸'}
                  </span>
                </button>
                {expandedStep === s.name && (
                  <pre
                    className="mono"
                    style={{
                      background: '#0a0d12',
                      border: '1px solid var(--line)',
                      borderRadius: 6,
                      padding: 8,
                      fontSize: 11,
                      marginTop: 4,
                      overflowX: 'auto',
                    }}
                  >
                    {JSON.stringify(s, null, 2)}
                  </pre>
                )}
              </div>
            ))}
            <div style={{ fontSize: 11, color: 'var(--faint)' }}>
              run <span className="mono">{evidence.run_id}</span> · <span className="mono">{evidence.git_sha.slice(0, 8)}</span> ·
              veredito: <strong>{evidence.verdict}</strong>
            </div>
          </div>
        )}
        <p style={{ fontSize: 11, color: 'var(--faint)', marginTop: 10 }}>
          self-hosting: este pipeline roda sobre o próprio BuildToValue (Fase 5). Job em memória — reinício do dashboard
          perde uma execução em andamento.
        </p>
      </Card>

      <Card>
        <strong>Review por valor</strong>
        {!review ? (
          <p style={{ fontSize: 12, color: 'var(--faint)', marginTop: 10 }}>
            rode o <span className="mono">/verify</span> — o review por valor é derivado da evidência real
            (não há mais números fixos aqui).
          </p>
        ) : (
          <>
            <div style={{ display: 'flex', justifyContent: 'center', margin: '12px 0' }}>
              <Gauge
                value={review.security}
                gate={0.5}
                label={`segurança · piso 0.50`}
              />
            </div>
            <div className="row" style={{ justifyContent: 'center', marginBottom: 10 }}>
              <Badge color={review.gates_passed ? 'var(--ok)' : 'var(--red)'}>
                {review.gates_passed ? 'GATES OK' : 'REPROVADO'}
              </Badge>
            </div>
            <div className="stack">
              <div>
                <div className="row" style={{ justifyContent: 'space-between', fontSize: 12 }}>
                  <span>technical (passos verdes)</span>
                  <span className="mono">{review.technical.toFixed(2)}</span>
                </div>
                <ProgressBar value={review.technical} />
              </div>
              <div>
                <div className="row" style={{ justifyContent: 'space-between', fontSize: 12 }}>
                  <span>security (1 − penalidade por finding)</span>
                  <span className="mono">{review.security.toFixed(2)}</span>
                </div>
                <ProgressBar value={review.security} />
              </div>
            </div>
            <p style={{ fontSize: 11, color: review.gates_passed ? 'var(--muted)' : 'var(--red)', marginTop: 8 }}>
              {review.reason}
              {review.gate_triggered && ` · gate: ${GATE_LABEL[review.gate_triggered] ?? review.gate_triggered}`}
            </p>
            <p style={{ fontSize: 11, color: 'var(--faint)', marginTop: 8 }}>
              derivado de <span className="mono">btv_schemas::review::ValueReview</span> sobre a evidência real:
              só as dimensões determinísticas (technical/security) + os gates duros. As dimensões{' '}
              <em>performance</em> e <em>value</em> do review completo dependem de avaliação de agente e{' '}
              <strong>não</strong> são fabricadas aqui — a certificação plena (média ponderada das 4) exige esse
              passo, ainda não wireado.
            </p>
          </>
        )}
      </Card>
    </div>
  )
}
