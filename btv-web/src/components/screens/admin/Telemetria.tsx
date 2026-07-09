import { useEffect, useState } from 'react'
import { fetchSummary, fetchModelUsage, type TelemetrySummary, type ModelUsageResponse } from '../../../api/admin'
import { listRuns, listDeliverables, type BtvRun } from '../../../api/btv'
import { useTemplates } from '../../../state/TemplatesContext'
import { ErroBox, NotaHonesta, StatCard } from './comum'

/** A1 · Telemetria & custos — números REAIS (telemetria SQLite + runs do
 *  BtvStore). Custo monetário é uma ESTIMATIVA a partir de tokens reais ×
 *  tabela de preços estática (nunca fabricado; modelo sem preço não entra). */
export function Telemetria() {
  const [summary, setSummary] = useState<TelemetrySummary | null>(null)
  const [runs, setRuns] = useState<BtvRun[] | null>(null)
  const [entregas, setEntregas] = useState<number | null>(null)
  const [uso, setUso] = useState<ModelUsageResponse | null>(null)
  const [erro, setErro] = useState<string | null>(null)
  const templates = useTemplates()

  useEffect(() => {
    Promise.all([fetchSummary(), listRuns(), listDeliverables(), fetchModelUsage()])
      .then(([s, r, d, u]) => {
        setSummary(s)
        setRuns(r)
        setEntregas(d.length)
        setUso(u)
      })
      .catch((e: Error) => setErro(e.message))
  }, [])

  if (erro) return <ErroBox msg={`Não consegui carregar a telemetria (${erro}).`} />
  if (!summary || !runs) {
    return <div className="mono" style={{ fontSize: 11.5, color: 'var(--faint)' }}>carregando…</div>
  }

  const ativas = runs.filter((r) => r.status === 'ativa').length
  const porTemplate = new Map<string, number>()
  for (const r of runs) porTemplate.set(r.template_id, (porTemplate.get(r.template_id) ?? 0) + 1)
  const max = Math.max(1, ...porTemplate.values())
  const byId = templates.status === 'ready' ? templates.byId : null

  return (
    <>
      <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(160px, 1fr))', gap: 12 }}>
        <StatCard k="eventos de telemetria" v={String(summary.total_events)} />
        <StatCard
          k="chamadas llm"
          v={String(summary.by_name['llm.call'] ?? 0)}
          delta={
            summary.cache_hit_rate != null
              ? `cache hit ${Math.round(summary.cache_hit_rate * 100)}%`
              : 'sem chamadas com cache'
          }
          deltaCor="var(--ok)"
        />
        <StatCard k="squads ativadas" v={String(runs.length)} delta={`${ativas} rodando agora`} />
        <StatCard k="entregas exportadas" v={String(entregas ?? 0)} />
        <StatCard
          k="custo estimado (USD)"
          v={uso ? `$${uso.total_estimated_cost_usd.toFixed(uso.total_estimated_cost_usd < 1 ? 4 : 2)}` : '—'}
          delta={uso ? `tabela ${uso.pricing_as_of} · estimativa` : undefined}
          deltaCor="var(--muted)"
        />
      </div>
      <div style={{ background: 'var(--white)', border: '1px solid var(--line)', borderRadius: 14, padding: '22px 24px' }}>
        <div className="kicker" style={{ fontSize: 10, color: 'var(--faint)', marginBottom: 16 }}>
          execuções por squad
        </div>
        {porTemplate.size === 0 && (
          <span className="mono" style={{ fontSize: 10.5, color: 'var(--faint)' }}>nenhuma squad ativada ainda</span>
        )}
        <div style={{ display: 'flex', flexDirection: 'column', gap: 9 }}>
          {[...porTemplate.entries()]
            .sort((a, b) => b[1] - a[1])
            .map(([id, n]) => {
              const t = byId?.get(id)
              return (
                <div key={id} style={{ display: 'grid', gridTemplateColumns: '170px 1fr 74px', gap: 14, alignItems: 'center' }}>
                  <span style={{ fontSize: 12.5, fontWeight: 500, display: 'flex', alignItems: 'center', gap: 8 }}>
                    <span style={{ width: 8, height: 8, borderRadius: 3, background: t?.cor ?? 'var(--brand)' }} />
                    {t?.nome ?? id}
                  </span>
                  <div style={{ height: 10, background: 'var(--paper)', borderRadius: 99, overflow: 'hidden' }}>
                    <div style={{ height: '100%', width: `${Math.round((n / max) * 100)}%`, background: t?.cor ?? 'var(--brand)', borderRadius: 99 }} />
                  </div>
                  <span className="mono" style={{ fontSize: 11, color: 'var(--muted)', textAlign: 'right' }}>
                    {n} exec.
                  </span>
                </div>
              )
            })}
        </div>
      </div>
      <NotaHonesta>
        Custo é uma <strong>estimativa</strong>: tokens reais gravados na telemetria × uma tabela de
        preços por modelo/provider embutida (referência {uso?.pricing_as_of ?? '—'} — envelhece, não é
        o valor cobrado pelo provedor). Modelo sem preço tabelado não entra na soma — nunca um custo
        fabricado.
      </NotaHonesta>
    </>
  )
}
