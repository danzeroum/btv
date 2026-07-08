import { useEffect, useState } from 'react'
import { fetchLedger, verifyLedger, type LedgerEntry } from '../../../api/admin'
import { useTemplates } from '../../../state/TemplatesContext'
import { ErroBox, Pill } from './comum'

const KIND_LABEL: Record<string, string> = {
  'btv.squad_activated': 'Squad ativada',
  'btv.gate_approved': 'Gate aprovado',
  'btv.adjust_requested': 'Ajuste solicitado no gate',
  'btv.export_generated': 'Exportação gerada',
  'btv.flow_saved': 'Fluxo salvo no Designer',
  'btv.persona_updated': 'Persona editada',
  'btv.template_published': 'Publicação de modelo',
  'session.start': 'Sessão iniciada',
  'session.end': 'Sessão encerrada',
  'squad.consensus': 'Consenso do squad',
  'squad.tool_run': 'Chamada de ferramenta',
}

/** A2 · Ledger de auditoria — hash-chain real (`.btv/btv.db`), com
 *  verificação de integridade sob demanda (nunca afirmada sem rodar). */
export function Ledger() {
  const [entries, setEntries] = useState<LedgerEntry[] | null>(null)
  const [erro, setErro] = useState<string | null>(null)
  const [verificado, setVerificado] = useState<string | null>(null)
  const templates = useTemplates()

  useEffect(() => {
    fetchLedger(40)
      .then(setEntries)
      .catch((e: Error) => setErro(e.message))
  }, [])

  if (erro) return <ErroBox msg={`Não consegui carregar o ledger (${erro}).`} />
  if (!entries) return <div className="mono" style={{ fontSize: 11.5, color: 'var(--faint)' }}>carregando…</div>

  const byId = templates.status === 'ready' ? templates.byId : null
  const squadDe = (e: LedgerEntry): { nome: string; cor: string } => {
    const tid = typeof e.payload.template_id === 'string' ? e.payload.template_id : null
    const t = tid ? byId?.get(tid) : null
    return t ? { nome: t.nome, cor: t.cor } : { nome: '—', cor: 'var(--faint)' }
  }

  return (
    <>
      <div style={{ display: 'flex', alignItems: 'center', gap: 10 }}>
        <span className="mono" style={{ fontSize: 10.5, color: 'var(--muted)' }}>
          {entries.length} entradas mais recentes ·{' '}
          {verificado ?? 'integridade ainda não verificada nesta sessão'}
        </span>
        <button
          onClick={() =>
            void verifyLedger()
              .then((r) => setVerificado(r.ok ? `✓ cadeia íntegra (${r.verified} entradas verificadas)` : '✗ CADEIA VIOLADA'))
              .catch((e: Error) => setVerificado(`erro ao verificar: ${e.message}`))
          }
          className="mono"
          style={{ marginLeft: 'auto', background: 'none', border: '1px solid var(--line2)', borderRadius: 9, padding: '8px 14px', fontSize: 10.5, color: 'var(--brand)', fontWeight: 600 }}
        >
          verificar integridade
        </button>
      </div>
      <div style={{ background: 'var(--white)', border: '1px solid var(--line)', borderRadius: 14, overflow: 'hidden' }}>
        <div className="mono" style={{ display: 'grid', gridTemplateColumns: '120px 1.7fr 1fr 1fr 110px', gap: 14, padding: '12px 20px', background: 'var(--card)', borderBottom: '1px solid var(--line)', fontSize: 9.5, letterSpacing: '0.13em', textTransform: 'uppercase', color: 'var(--faint)' }}>
          <span>quando</span><span>evento</span><span>squad</span><span>ator</span><span>hash</span>
        </div>
        {entries.map((e) => {
          const squad = squadDe(e)
          // Ator humano (decisão) vs agente/sistema (execução) — derivado dos
          // campos reais: kinds de gate são sempre humanos; senão, pelo ator.
          const humano =
            e.kind === 'btv.gate_approved' ||
            e.kind === 'btv.adjust_requested' ||
            /human|você|voc[eê]|usu[aá]ri|marina/i.test(e.actor)
          return (
            <div key={e.seq} style={{ display: 'grid', gridTemplateColumns: '120px 1.7fr 1fr 1fr 110px', gap: 14, padding: '12px 20px', borderBottom: '1px solid var(--line)', alignItems: 'baseline' }}>
              <span className="mono" style={{ fontSize: 10.5, color: 'var(--faint)' }}>
                {e.ts.slice(5, 10)} {e.ts.slice(11, 16)}
              </span>
              <span style={{ fontSize: 13 }}>{KIND_LABEL[e.kind] ?? e.kind}</span>
              <span style={{ fontSize: 12, color: squad.cor, fontWeight: 500 }}>{squad.nome}</span>
              <span className="mono" style={{ fontSize: 11, color: humano ? 'var(--decision)' : 'var(--brand)', fontWeight: 500 }}>{e.actor}</span>
              <span className="mono" style={{ fontSize: 10, color: 'var(--faint)' }}>
                {e.entry_hash.slice(0, 4)}…{e.entry_hash.slice(-4)}
              </span>
            </div>
          )
        })}
        {entries.length === 0 && (
          <div className="mono" style={{ padding: 20, fontSize: 10.5, color: 'var(--faint)' }}>ledger vazio</div>
        )}
      </div>
      {verificado != null && <Pill tone={verificado.startsWith('✓') ? 'ok' : 'erro'}>{verificado}</Pill>}
    </>
  )
}
