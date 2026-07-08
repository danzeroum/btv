import { useCallback, useEffect, useState } from 'react'
import { fetchMatrix, fetchRules, revokeRule, setRule, type Decision, type MatrixRow, type RuleRecord } from '../../../api/admin'
import { ErroBox, NotaHonesta, Pill } from './comum'

const TOOL_DESC: Record<string, string> = {
  read: 'Leitura de arquivos do workspace.',
  grep: 'Busca por conteúdo (ripgrep).',
  edit: 'Escrita/edição de arquivos — é o que gera as entregas.',
  bash: 'Execução de comandos no shell.',
  webfetch: 'Busca e captura de conteúdo da rede.',
}

/** A4 · Permissões — matriz EFETIVA real (`btv_core::{BUILD,PLAN}` +
 *  overrides persistidos no RuleStore, auditados no ledger). Negar aqui vale
 *  imediatamente para as squads (o RunTool avalia o mesmo motor). */
export function Permissoes() {
  const [matrix, setMatrix] = useState<MatrixRow[] | null>(null)
  const [rules, setRules] = useState<RuleRecord[] | null>(null)
  const [erro, setErro] = useState<string | null>(null)
  const [confirmar, setConfirmar] = useState<{ tool: string; profile: string; decision: Decision } | null>(null)

  const recarregar = useCallback(() => {
    Promise.all([fetchMatrix(), fetchRules()])
      .then(([m, r]) => {
        setMatrix(m)
        setRules(r)
      })
      .catch((e: Error) => setErro(e.message))
  }, [])
  useEffect(() => recarregar(), [recarregar])

  if (erro) return <ErroBox msg={`Não consegui carregar as permissões (${erro}).`} />
  if (!matrix || !rules) {
    return <div className="mono" style={{ fontSize: 11.5, color: 'var(--faint)' }}>carregando…</div>
  }

  const proximo = (d: Decision): Decision => (d === 'allow' ? 'deny' : d === 'deny' ? 'ask' : 'allow')
  const tone = (d: Decision) => (d === 'allow' ? 'ok' : d === 'deny' ? 'erro' : 'warn')

  return (
    <>
      <div style={{ display: 'flex', flexDirection: 'column', gap: 10 }}>
        {matrix.map((row) => (
          <div key={row.tool} style={{ display: 'grid', gridTemplateColumns: '160px 1.7fr auto auto', gap: 18, alignItems: 'center', background: 'var(--white)', border: '1px solid var(--line)', borderRadius: 12, padding: '15px 20px' }}>
            <span className="mono" style={{ fontSize: 12, fontWeight: 600 }}>{row.tool}</span>
            <span style={{ fontSize: 12.5, color: 'var(--muted)', lineHeight: 1.5 }}>
              {TOOL_DESC[row.tool] ?? '—'}
            </span>
            {(['build', 'plan'] as const).map((profile) => (
              <button
                key={profile}
                onClick={() => setConfirmar({ tool: row.tool, profile, decision: proximo(row[profile]) })}
                style={{ background: 'none', border: 'none', display: 'flex', flexDirection: 'column', gap: 3, alignItems: 'center' }}
                title={`mudar decisão de ${row.tool} no perfil ${profile}`}
              >
                <span className="mono" style={{ fontSize: 9, color: 'var(--faint)', textTransform: 'uppercase', letterSpacing: '0.1em' }}>{profile}</span>
                <Pill tone={tone(row[profile])}>{row[profile]}</Pill>
              </button>
            ))}
          </div>
        ))}
      </div>

      {confirmar && (
        <div data-testid="confirmar-permissao" style={{ background: 'var(--white)', border: '2px solid var(--gold)', borderRadius: 14, padding: '18px 22px', display: 'flex', alignItems: 'center', gap: 14, flexWrap: 'wrap' }}>
          <span style={{ fontSize: 13.5 }}>
            Mudar <strong className="mono">{confirmar.tool}</strong> no perfil{' '}
            <strong className="mono">{confirmar.profile}</strong> para{' '}
            <strong className="mono">{confirmar.decision}</strong>? Vale imediatamente e fica no
            ledger.
          </span>
          <span style={{ marginLeft: 'auto', display: 'flex', gap: 8 }}>
            <button
              onClick={() =>
                void setRule(confirmar.profile, confirmar.tool, confirmar.decision)
                  .then(() => {
                    setConfirmar(null)
                    recarregar()
                  })
                  .catch((e: Error) => setErro(e.message))
              }
              style={{ background: 'var(--brand)', color: '#fff', border: 'none', borderRadius: 9, padding: '9px 16px', fontSize: 12.5, fontWeight: 600, fontFamily: 'var(--sans)' }}
            >
              Confirmar
            </button>
            <button
              onClick={() => setConfirmar(null)}
              style={{ background: 'none', border: '1px solid var(--line2)', borderRadius: 9, padding: '9px 14px', fontSize: 12.5, color: 'var(--muted)', fontFamily: 'var(--sans)' }}
            >
              Cancelar
            </button>
          </span>
        </div>
      )}

      {rules.length > 0 && (
        <div style={{ background: 'var(--card)', border: '1px solid var(--line)', borderRadius: 12, padding: '14px 18px', display: 'flex', flexDirection: 'column', gap: 8 }}>
          <div className="kicker" style={{ fontSize: 10, color: 'var(--faint)' }}>overrides persistidos</div>
          {rules.map((r) => (
            <div key={r.id} className="mono" style={{ display: 'flex', gap: 10, alignItems: 'center', fontSize: 11, color: 'var(--muted)' }}>
              <span>{r.profile} · {r.tool}{r.scope_prefix ? ` · ${r.scope_prefix}` : ''} → {r.decision}</span>
              <button
                onClick={() => void revokeRule(r.id).then(recarregar)}
                style={{ marginLeft: 'auto', background: 'none', border: '1px solid var(--line2)', borderRadius: 7, padding: '4px 10px', fontSize: 9.5, color: 'var(--warn)', fontFamily: 'var(--mono)' }}
              >
                revogar
              </button>
            </div>
          ))}
        </div>
      )}
      <NotaHonesta>
        A matriz é a decisão efetiva dos perfis de permissão da plataforma (build/plan) +
        os overrides gravados acima — o MESMO motor que as ferramentas das squads consultam.
        Allow-list por modelo de squad é extensão futura.
      </NotaHonesta>
    </>
  )
}
