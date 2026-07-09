import { useEffect } from 'react'
import { StatTile } from '../../primitives/StatTile'
import { Badge } from '../../primitives/Badge'
import { Table } from '../../primitives/Table'
import { AsyncStatus } from '../../primitives/AsyncStatus'
import { useAsyncAction } from '../../../hooks/useAsyncAction'
import { fetchModelUsage } from '../../../api/modelUsage'

function hitRate(hits: number, misses: number): string {
  const total = hits + misses
  return total === 0 ? 'n/a' : `${((hits / total) * 100).toFixed(1)}%`
}

export function Modelos() {
  const state = useAsyncAction(fetchModelUsage)

  useEffect(() => {
    void state.run()
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  return (
    <AsyncStatus state={state.state} onRetry={() => void state.run()}>
      {(data) => {
        const usage = data.entries
        const totalCalls = usage.reduce((acc, u) => acc + u.calls, 0)
        const top = usage.slice().sort((a, b) => b.calls - a.calls)[0]
        const fmtUsd = (v: number) => `$${v.toFixed(v < 1 ? 4 : 2)}`

        return (
          <div className="stack">
            <div className="row">
              <StatTile value={String(totalCalls)} label="chamadas llm (todos os modelos)" />
              <StatTile value={top ? top.model : '—'} label="modelo mais usado" />
              <StatTile value={fmtUsd(data.total_estimated_cost_usd)} label="custo estimado (USD)" />
            </div>

            {usage.length === 0 ? (
              <div style={{ fontSize: 12, color: 'var(--faint)' }}>
                nenhum evento com <span className="mono">model</span> registrado ainda — aparece assim que uma
                sessão real chamar o gateway.
              </div>
            ) : (
              <Table
                rowKey={(u) => u.model}
                rows={usage}
                columns={[
                  { key: 'model', header: 'modelo', render: (u) => <span className="mono">{u.model}</span> },
                  { key: 'tier', header: 'tier', render: (u) => <Badge>{u.tier}</Badge> },
                  { key: 'calls', header: 'chamadas', render: (u) => u.calls },
                  {
                    key: 'tokens',
                    header: 'tokens (in/out)',
                    render: (u) => `${u.input_tokens.toLocaleString()} / ${u.output_tokens.toLocaleString()}`,
                  },
                  {
                    key: 'cost',
                    header: 'custo est. (USD)',
                    render: (u) =>
                      u.estimated_cost_usd === undefined ? (
                        <span style={{ color: 'var(--faint)' }} title="sem preço tabelado para este modelo">
                          sem preço
                        </span>
                      ) : (
                        fmtUsd(u.estimated_cost_usd)
                      ),
                  },
                  {
                    key: 'hit_rate',
                    header: 'hit rate',
                    render: (u) => hitRate(u.cache_hits, u.cache_misses),
                  },
                ]}
              />
            )}

            <div style={{ fontSize: 11, color: 'var(--faint)' }}>
              tier derivado de <span className="mono">model_tier::tier_from_id</span> · custo ={' '}
              <strong>estimativa</strong> a partir de tokens reais × tabela de preços estática (referência{' '}
              <span className="mono">{data.pricing_as_of}</span>, envelhece — não é custo cobrado). Modelo sem
              preço tabelado aparece como "sem preço", nunca com custo fabricado.
            </div>
          </div>
        )
      }}
    </AsyncStatus>
  )
}
