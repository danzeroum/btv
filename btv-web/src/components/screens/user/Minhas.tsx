import { useEffect, type CSSProperties } from 'react'
import { listDeliverables, listRuns, type BtvDeliverable, type BtvRun } from '../../../api/btv'
import { useTemplates } from '../../../state/TemplatesContext'
import { useSquadRun } from '../../../state/SquadRunContext'
import { useAppDispatch } from '../../../state/AppContext'
import { useAsyncAction } from '../../../hooks/useAsyncAction'
import { AsyncStatus } from '../../../components/primitives'
import { runSemArtefatoReal } from '../../../lib/entregas'

// "aguardando você" NÃO está aqui de propósito: gate é decisão humana, sempre
// terracota (renderizado com .status-gate abaixo). Âmbar seria erro semântico.
const PILL: Record<string, CSSProperties> = {
  'em produção': { background: 'var(--paper)', color: 'var(--muted)' },
  concluída: { background: 'var(--ok-bg)', color: 'var(--ok-ink)' },
  encerrada: { background: 'var(--paper)', color: 'var(--faint)' },
  erro: { background: 'var(--err-bg)', color: 'var(--err-ink)' },
}

// Carrega runs + entregas juntos: as entregas são secundárias (só avisam
// quando uma run concluiu sem artefato), então uma falha nelas degrada para
// lista vazia sem derrubar a tela.
async function carregarMinhas(): Promise<{ runs: BtvRun[]; entregas: BtvDeliverable[] }> {
  const [runs, entregas] = await Promise.all([listRuns(), listDeliverables().catch(() => [] as BtvDeliverable[])])
  return { runs, entregas }
}

/** U6 · Minhas squads — runs persistidos (backend real), a execução viva no
 *  topo com ação de abrir ao vivo; concluídas apontam para as entregas;
 *  encerradas reabrem o wizard do modelo. */
export function Minhas() {
  const templates = useTemplates()
  const { run: liveRun, view, abrirRun } = useSquadRun()
  const dispatch = useAppDispatch()
  // Estado assíncrono unificado (F1): idle→loading→success|error pelo
  // AsyncStatus, sem re-implementar os três estados à mão.
  const { state, run: carregar } = useAsyncAction(carregarMinhas)
  useEffect(() => {
    void carregar()
  }, [carregar])

  const byId = templates.status === 'ready' ? templates.byId : null

  return (
    <AsyncStatus state={state} onRetry={() => void carregar()} erroPrefixo="Não consegui carregar as squads">
      {({ runs, entregas }) => {
        if (runs.length === 0) {
          return (
            <div style={{ background: 'var(--white)', border: '1px dashed var(--line2)', borderRadius: 14, padding: '28px 30px', color: 'var(--muted)', fontSize: 13.5 }}>
              Nenhuma squad ainda — ative a primeira pela galeria.
            </div>
          )
        }
        return (
          <div style={{ display: 'flex', flexDirection: 'column', gap: 10 }}>
            {runs.map((r) => {
        const template = byId?.get(r.template_id)
        const cor = template?.cor ?? 'var(--brand)'
        const isLive = liveRun?.taskId === r.task_id && r.status === 'ativa'
        const status =
          r.status === 'ativa'
            ? isLive && view?.gateOpen
              ? 'aguardando você'
              : 'em produção'
            : r.status === 'concluida'
              ? 'concluída'
              : r.status
        // Progresso REAL só existe para a execução viva desta sessão (posição
        // da esteira); concluída = 100%; demais ficam sem barra fabricada.
        const pct = isLive && view && liveRun
          ? Math.min(100, Math.round((view.idx / liveRun.etapas.length) * 100))
          : r.status === 'concluida'
            ? 100
            : 0
        const isGate = isLive && !!view?.gateOpen
        // Entregas REAIS desta run (arquivo gravado por ferramenta). Concluída
        // com zero = o modelo não chamou a ferramenta de escrita.
        const numEntregas = entregas.filter((e) => e.run_id === r.id).length
        const semArtefato = runSemArtefatoReal(r.status, numEntregas)
        const acao =
          r.status === 'ativa' && isLive
            ? { label: isGate ? 'Revisar' : 'abrir ao vivo', on: () => dispatch({ type: 'SET_SCREEN', screen: 'vivo' }) }
            : r.status === 'ativa'
              ? { label: 'reconectar', on: () => template && abrirRun(r, template) }
              : r.status === 'concluida'
                ? { label: 'ver entregas', on: () => dispatch({ type: 'SET_SCREEN', screen: 'biblioteca' }) }
                : {
                    label: 'reativar',
                    on: () => dispatch({ type: 'OPEN_WIZARD', templateId: r.template_id }),
                  }
        return (
          <div
            key={r.id}
            data-testid={`run-${r.id}`}
            style={{ display: 'grid', gridTemplateColumns: '1.5fr 1fr 140px 1fr auto', gap: 18, alignItems: 'center', background: 'var(--white)', border: '1px solid var(--line)', borderRadius: 12, padding: '15px 20px' }}
          >
            <span style={{ fontSize: 13.5, fontWeight: 600, display: 'flex', alignItems: 'center', gap: 10, minWidth: 0 }}>
              <span style={{ width: 9, height: 9, borderRadius: 3, background: cor, flex: 'none' }} />
              <span style={{ whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }}>{r.nome}</span>
            </span>
            <span className="mono" style={{ fontSize: 10.5, color: 'var(--faint)', whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }}>
              {template?.nome ?? r.template_id} · {r.template_versao}
            </span>
            {status === 'aguardando você' ? (
              <span className="status-gate" style={{ fontSize: 10, letterSpacing: '0.06em', justifyContent: 'center' }}>
                {status}
              </span>
            ) : (
              <div style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', gap: 4, minWidth: 0 }}>
                <span
                  className="mono"
                  style={{ fontSize: 10, letterSpacing: '0.06em', borderRadius: 999, padding: '5px 11px', textAlign: 'center', ...(PILL[status] ?? PILL['em produção']) }}
                >
                  {status}
                </span>
                {semArtefato && (
                  <span
                    data-testid={`sem-artefato-${r.id}`}
                    title="A run concluiu, mas nenhum arquivo foi gravado por ferramenta — o modelo pode ter apenas descrito a entrega. Nada aparece na Biblioteca."
                    className="mono"
                    style={{ fontSize: 9, letterSpacing: '0.03em', color: 'var(--muted)', background: 'var(--paper)', border: '1px solid var(--line2)', borderRadius: 999, padding: '2px 8px', whiteSpace: 'nowrap' }}
                  >
                    sem artefato real
                  </span>
                )}
              </div>
            )}
            <div style={{ height: 7, background: 'var(--paper)', borderRadius: 99, overflow: 'hidden' }}>
              <div style={{ height: '100%', width: `${pct}%`, background: cor, borderRadius: 99 }} />
            </div>
            <button
              onClick={acao.on}
              className={isGate ? 'btn-decision' : 'mono'}
              style={
                isGate
                  ? { padding: '7px 14px', fontSize: 11, fontWeight: 600, whiteSpace: 'nowrap', borderRadius: 8 }
                  : { background: 'none', border: '1px solid var(--line2)', borderRadius: 8, padding: '7px 14px', fontSize: 10.5, color: 'var(--brand)', fontWeight: 600, whiteSpace: 'nowrap' }
              }
            >
              {acao.label}
            </button>
          </div>
        )
            })}
          </div>
        )
      }}
    </AsyncStatus>
  )
}
