import { useCallback, useEffect, useState } from 'react'
import { fetchLedger, fetchPublicacao, setPublicacao, type LedgerEntry } from '../../../api/admin'
import { useTemplates } from '../../../state/TemplatesContext'
import { ErroBox, Pill } from './comum'

const CAT_LABEL: Record<string, string> = {
  conteudo: 'Conteúdo',
  analise: 'Análise',
  criativa: 'Criativas',
  operacoes: 'Operações',
}

/** A5 · Modelos de squad — os 12 embutidos (squad-template.v1) com estado de
 *  publicação persistido (override no BtvStore, auditado) + fluxos salvos do
 *  Designer (ledger btv.flow_saved) no topo como rascunhos. */
export function Modelos() {
  const templates = useTemplates()
  const [pub, setPub] = useState<Map<string, boolean> | null>(null)
  const [fluxos, setFluxos] = useState<LedgerEntry[] | null>(null)
  const [erro, setErro] = useState<string | null>(null)

  const recarregar = useCallback(() => {
    Promise.all([fetchPublicacao(), fetchLedger(100)])
      .then(([p, l]) => {
        setPub(new Map(p.map((x) => [x.template_id, x.publicado])))
        setFluxos(l.filter((e) => e.kind === 'btv.flow_saved'))
      })
      .catch((e: Error) => setErro(e.message))
  }, [])
  useEffect(() => recarregar(), [recarregar])

  if (erro) return <ErroBox msg={`Não consegui carregar os modelos (${erro}).`} />
  if (templates.status !== 'ready' || !pub || !fluxos) {
    return <div className="mono" style={{ fontSize: 11.5, color: 'var(--faint)' }}>carregando…</div>
  }

  const grid = '1.6fr 90px 1fr 100px 120px 150px'

  return (
    <div style={{ background: 'var(--white)', border: '1px solid var(--line)', borderRadius: 14, overflowX: 'auto' }}>
      <div className="mono" style={{ minWidth: 780, display: 'grid', gridTemplateColumns: grid, gap: 14, padding: '12px 20px', background: 'var(--card)', borderBottom: '1px solid var(--line)', fontSize: 9.5, letterSpacing: '0.13em', textTransform: 'uppercase', color: 'var(--faint)' }}>
        <span>modelo</span><span>versão</span><span>categoria</span><span>origem</span><span>status</span><span>ação</span>
      </div>

      {fluxos.map((f) => (
        <div key={f.seq} style={{ minWidth: 780, display: 'grid', gridTemplateColumns: grid, gap: 14, padding: '13px 20px', borderBottom: '1px solid var(--line)', alignItems: 'center' }}>
          <span style={{ fontSize: 13.5, fontWeight: 600, display: 'flex', alignItems: 'center', gap: 9 }}>
            <span style={{ width: 9, height: 9, borderRadius: 3, background: '#2b7a8c' }} />
            {String(f.payload.nome ?? 'fluxo')} (Designer)
          </span>
          <span className="mono" style={{ fontSize: 11, color: 'var(--muted)' }}>
            {String(f.payload.versao_semantica ?? 'v0.1')}
          </span>
          <span style={{ fontSize: 12, color: 'var(--muted)' }}>Personalizada</span>
          <span className="mono" style={{ fontSize: 11, color: 'var(--muted)' }}>Designer</span>
          <Pill tone="warn">rascunho</Pill>
          <span className="mono" style={{ fontSize: 10, color: 'var(--faint)' }}>
            ledger #{f.seq} · {String(f.payload.snapshot_hash ?? '').slice(0, 8)}
          </span>
        </div>
      ))}

      {templates.templates.map((t) => {
        const publicado = pub.get(t.id) ?? t.publicado
        return (
          <div key={t.id} data-testid={`modelo-${t.id}`} style={{ minWidth: 780, display: 'grid', gridTemplateColumns: grid, gap: 14, padding: '13px 20px', borderBottom: '1px solid var(--line)', alignItems: 'center' }}>
            <span style={{ fontSize: 13.5, fontWeight: 600, display: 'flex', alignItems: 'center', gap: 9 }}>
              <span style={{ width: 9, height: 9, borderRadius: 3, background: t.cor }} />
              {t.nome}
            </span>
            <span className="mono" style={{ fontSize: 11, color: 'var(--muted)' }}>{t.versao}</span>
            <span style={{ fontSize: 12, color: 'var(--muted)' }}>{CAT_LABEL[t.categoria]}</span>
            <span className="mono" style={{ fontSize: 11, color: 'var(--muted)' }}>galeria</span>
            <Pill tone={publicado ? 'ok' : 'warn'}>{publicado ? 'publicado' : 'rascunho'}</Pill>
            <button
              onClick={() =>
                void setPublicacao(t.id, !publicado)
                  .then(recarregar)
                  .catch((e: Error) => setErro(e.message))
              }
              className="mono"
              style={{ background: 'none', border: '1px solid var(--line2)', borderRadius: 8, padding: '6px 12px', fontSize: 10, color: 'var(--brand)', fontWeight: 600, width: 'fit-content' }}
            >
              {publicado ? 'despublicar' : 'publicar'}
            </button>
          </div>
        )
      })}
    </div>
  )
}
