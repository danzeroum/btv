import { useEffect, useState, type CSSProperties } from 'react'
import { listRuns, type BtvRun } from '../../../api/btv'
import { useTemplates } from '../../../state/TemplatesContext'
import { useSquadRun } from '../../../state/SquadRunContext'
import { useAppDispatch } from '../../../state/AppContext'

const PILL: Record<string, CSSProperties> = {
  'em produção': { background: 'var(--paper)', color: 'var(--muted)' },
  'aguardando você': { background: '#fdf3e3', color: '#9a6b14' },
  concluída: { background: '#e7efe9', color: '#2d6a50' },
  encerrada: { background: 'var(--paper)', color: 'var(--faint)' },
  erro: { background: '#f7e7e3', color: '#a54334' },
}

/** U6 · Minhas squads — runs persistidos (backend real), a execução viva no
 *  topo com ação de abrir ao vivo; concluídas apontam para as entregas;
 *  encerradas reabrem o wizard do modelo. */
export function Minhas() {
  const [runs, setRuns] = useState<BtvRun[] | null>(null)
  const [erro, setErro] = useState<string | null>(null)
  const templates = useTemplates()
  const { run: liveRun, view, abrirRun } = useSquadRun()
  const dispatch = useAppDispatch()

  useEffect(() => {
    listRuns()
      .then(setRuns)
      .catch((e: Error) => setErro(e.message))
  }, [])

  if (erro) {
    return (
      <div style={{ background: '#f7e7e3', border: '1px solid #e0b8ad', borderRadius: 12, padding: '16px 20px', color: '#a54334', fontSize: 13 }}>
        Não consegui carregar as squads ({erro}).
      </div>
    )
  }
  if (!runs) {
    return <div className="mono" style={{ color: 'var(--faint)', fontSize: 11.5 }}>carregando…</div>
  }
  if (runs.length === 0) {
    return (
      <div style={{ background: 'var(--white)', border: '1px dashed var(--line2)', borderRadius: 14, padding: '28px 30px', color: 'var(--muted)', fontSize: 13.5 }}>
        Nenhuma squad ainda — ative a primeira pela galeria.
      </div>
    )
  }

  const byId = templates.status === 'ready' ? templates.byId : null

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
        const acao =
          r.status === 'ativa' && isLive
            ? { label: 'abrir ao vivo', on: () => dispatch({ type: 'SET_SCREEN', screen: 'vivo' }) }
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
            <span
              className="mono"
              style={{ fontSize: 10, letterSpacing: '0.06em', borderRadius: 999, padding: '5px 11px', textAlign: 'center', ...(PILL[status] ?? PILL['em produção']) }}
            >
              {status}
            </span>
            <div style={{ height: 7, background: 'var(--paper)', borderRadius: 99, overflow: 'hidden' }}>
              <div style={{ height: '100%', width: `${pct}%`, background: cor, borderRadius: 99 }} />
            </div>
            <button
              onClick={acao.on}
              className="mono"
              style={{ background: 'none', border: '1px solid var(--line2)', borderRadius: 8, padding: '7px 14px', fontSize: 10.5, color: 'var(--brand)', fontWeight: 600, whiteSpace: 'nowrap' }}
            >
              {acao.label}
            </button>
          </div>
        )
      })}
    </div>
  )
}
