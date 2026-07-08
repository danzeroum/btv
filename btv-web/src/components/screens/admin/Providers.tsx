import { useEffect, useState } from 'react'
import { fetchProviders, fetchRateLimits, type ProviderInfo, type RateLimitEntry } from '../../../api/admin'
import { ErroBox, NotaHonesta, Pill } from './comum'

const PROVIDER_LABEL: Record<string, string> = {
  anthropic: 'Anthropic',
  deepseek: 'DeepSeek',
  openai: 'OpenAI',
}

/** A3 · Providers & rate limits — `Gateway::from_env` real (ordem fixa de
 *  fallback anthropic→deepseek→openai) + tetos por tier. Uso ao vivo do
 *  limite não existe entre processos (o limitador vive na sessão) — dito na
 *  nota, não fabricado em barra. */
export function Providers() {
  const [providers, setProviders] = useState<ProviderInfo[] | null>(null)
  const [limits, setLimits] = useState<RateLimitEntry[] | null>(null)
  const [erro, setErro] = useState<string | null>(null)

  useEffect(() => {
    Promise.all([fetchProviders(), fetchRateLimits()])
      .then(([p, l]) => {
        setProviders(p)
        setLimits(l)
      })
      .catch((e: Error) => setErro(e.message))
  }, [])

  if (erro) return <ErroBox msg={`Não consegui carregar providers (${erro}).`} />
  if (!providers || !limits) {
    return <div className="mono" style={{ fontSize: 11.5, color: 'var(--faint)' }}>carregando…</div>
  }

  return (
    <>
      <div style={{ display: 'flex', flexDirection: 'column', gap: 12 }}>
        {providers.map((p, i) => (
          <div key={p.id} style={{ display: 'grid', gridTemplateColumns: '1.2fr 1.8fr auto', gap: 20, alignItems: 'center', background: 'var(--white)', border: '1px solid var(--line)', borderRadius: 13, padding: '18px 22px' }}>
            <div style={{ display: 'flex', flexDirection: 'column', gap: 3 }}>
              <span style={{ fontFamily: 'var(--disp)', fontWeight: 700, fontSize: 15 }}>
                {PROVIDER_LABEL[p.id] ?? p.id}
              </span>
              <span className="mono" style={{ fontSize: 10, color: 'var(--faint)' }}>
                API direta · prioridade {i + 1} no fallback
              </span>
            </div>
            <span style={{ fontSize: 12.5, color: 'var(--muted)', lineHeight: 1.5 }}>
              {p.configured
                ? 'chave presente no ambiente do processo Rust (keys nunca saem dele)'
                : 'sem chave no ambiente — pulado pelo fallback'}
            </span>
            <Pill tone={p.configured ? 'ok' : 'muted'}>{p.configured ? 'configurado' : 'sem key'}</Pill>
          </div>
        ))}
      </div>
      <div style={{ background: 'var(--white)', border: '1px solid var(--line)', borderRadius: 14, padding: '20px 24px' }}>
        <div className="kicker" style={{ fontSize: 10, color: 'var(--faint)', marginBottom: 12 }}>
          rate limiting por tier (janela deslizante)
        </div>
        <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
          {limits.map((l) => (
            <div key={l.tier} style={{ display: 'grid', gridTemplateColumns: '120px 1fr', gap: 14, alignItems: 'baseline' }}>
              <span className="mono" style={{ fontSize: 11.5, fontWeight: 600, color: 'var(--brand)' }}>{l.tier}</span>
              <span style={{ fontSize: 12.5, color: 'var(--muted)' }}>
                {l.cap} requisições / {Math.round(l.window_secs / 60)} min
              </span>
            </div>
          ))}
        </div>
      </div>
      <NotaHonesta>
        Uso ao vivo do limite não é exibido: o limitador vive no processo de cada sessão, não neste
        dashboard — uma barra de consumo aqui seria fabricada (mesma constatação da Fase 7 do
        console).
      </NotaHonesta>
    </>
  )
}
