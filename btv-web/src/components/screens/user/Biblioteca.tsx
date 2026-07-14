import { useEffect } from 'react'
import { listDeliverables, deliverableDownloadUrl, type BtvDeliverable } from '../../../api/btv'
import { useTemplates } from '../../../state/TemplatesContext'
import { useAsyncAction } from '../../../hooks/useAsyncAction'
import { AsyncStatus } from '../../../components/primitives'

/** U4 · Biblioteca de entregas — artefatos REAIS gravados pelas ferramentas
 *  do squad, agrupados por modelo, com trilha de procedência e export
 *  honesto (formato binário desabilitado até existir conversor). */
export function Biblioteca() {
  const templates = useTemplates()
  // Estado assíncrono unificado (F1) pelo AsyncStatus.
  const { state, run: carregar } = useAsyncAction(listDeliverables)
  useEffect(() => {
    void carregar()
  }, [carregar])

  const byId = templates.status === 'ready' ? templates.byId : null

  return (
    <AsyncStatus state={state} onRetry={() => void carregar()} erroPrefixo="Não consegui carregar as entregas">
      {(items) => {
        if (items.length === 0) {
          return (
            <div style={{ background: 'var(--white)', border: '1px dashed var(--line2)', borderRadius: 14, padding: '28px 30px', color: 'var(--muted)', fontSize: 13.5, lineHeight: 1.6 }}>
              Nenhuma entrega ainda. Quando uma squad concluir com artefatos gravados de verdade
              (ferramenta <span className="mono">edit</span> do squad), eles aparecem aqui com a trilha
              completa de procedência.
            </div>
          )
        }
        const grupos = new Map<string, BtvDeliverable[]>()
        for (const d of items) {
          const list = grupos.get(d.template_id) ?? []
          list.push(d)
          grupos.set(d.template_id, list)
        }
        return (
          <>
            {[...grupos.entries()].map(([templateId, artefatos]) => {
        const template = byId?.get(templateId)
        const cor = template?.cor ?? 'var(--brand)'
        const binarioDe = (formato: string) =>
          template?.formatos.find((f) => f.nome === formato)?.binario ?? false
        // Formatos que exigem renderização/mídia real e ainda não têm conversor
        // honesto — o resto (DOCX/XLSX/PDF/SVG/MusicXML) é convertido no backend.
        const semConversor = (formato: string) =>
          ['png', 'midi'].includes(formato.toLowerCase())
        return (
          <div key={templateId} style={{ display: 'flex', flexDirection: 'column', gap: 10 }}>
            <div style={{ display: 'flex', alignItems: 'center', gap: 10, marginTop: 6 }}>
              <span style={{ width: 10, height: 10, borderRadius: 3, background: cor }} />
              <span style={{ fontFamily: 'var(--disp)', fontWeight: 700, fontSize: 15 }}>
                {template?.nome ?? templateId}
              </span>
              <span className="mono" style={{ fontSize: 10, color: 'var(--faint)' }}>
                {artefatos.length} entrega{artefatos.length > 1 ? 's' : ''}
              </span>
            </div>
            {artefatos.map((a) => {
              const emBreve = binarioDe(a.formato) && semConversor(a.formato)
              return (
                <div
                  key={a.id}
                  data-testid={`entrega-${a.id}`}
                  style={{ display: 'grid', gridTemplateColumns: '86px 1.3fr 1.6fr auto', gap: 16, alignItems: 'center', background: 'var(--white)', border: '1px solid var(--line)', borderRadius: 12, padding: '14px 18px' }}
                >
                  <span className="mono" style={{ fontSize: 10, fontWeight: 600, letterSpacing: '0.06em', background: '#f0ebdf', color: cor, borderRadius: 6, padding: '5px 0', textAlign: 'center' }}>
                    {a.formato}
                  </span>
                  <div style={{ display: 'flex', flexDirection: 'column', gap: 2, minWidth: 0 }}>
                    <span style={{ fontSize: 13.5, fontWeight: 600, whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }}>
                      {a.nome}
                    </span>
                    <span className="mono" style={{ fontSize: 10, color: 'var(--faint)' }}>
                      {a.versao} · {a.created_ts.slice(0, 10)}
                    </span>
                  </div>
                  <span className="mono" style={{ fontSize: 10.5, color: 'var(--muted)', lineHeight: 1.55 }}>{a.trilha}</span>
                  {emBreve ? (
                    <span
                      className="mono"
                      title="exige renderização/conversão de mídia real — sem conversor honesto ainda"
                      style={{ fontSize: 10.5, color: 'var(--faint)', border: '1px dashed var(--line2)', borderRadius: 8, padding: '7px 14px' }}
                    >
                      em breve
                    </span>
                  ) : (
                    <a
                      href={deliverableDownloadUrl(a.id)}
                      download={a.nome}
                      className="mono"
                      style={{ background: 'none', border: '1px solid var(--line2)', borderRadius: 8, padding: '7px 14px', fontSize: 10.5, color: 'var(--brand)', fontWeight: 600, textDecoration: 'none' }}
                    >
                      exportar ↓
                    </a>
                  )}
                </div>
              )
            })}
          </div>
        )
      })}
          </>
        )
      }}
    </AsyncStatus>
  )
}
