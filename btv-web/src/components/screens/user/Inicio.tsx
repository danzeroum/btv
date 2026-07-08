import { useState, type CSSProperties } from 'react'
import { useAppDispatch } from '../../../state/AppContext'
import { useTemplates } from '../../../state/TemplatesContext'
import type { CategoriaSquad, SquadTemplate } from '../../../api/templates'

const CATS: Array<[CategoriaSquad | 'todas', string]> = [
  ['todas', 'Todas'],
  ['conteudo', 'Conteúdo'],
  ['analise', 'Análise'],
  ['criativa', 'Criativas'],
  ['operacoes', 'Operações'],
]

const ONDA_LABEL: Record<number, string> = { 1: 'onda 1', 2: 'onda 2', 3: 'onda 3' }
const ONDA_CSS: Record<number, CSSProperties> = {
  1: { background: '#e7efe9', color: '#2d6a50' },
  2: { background: '#fdf3e3', color: '#9a6b14' },
  3: { background: '#f4e9ef', color: '#8d3f6a' },
}

function CardModelo({ template, onOpen }: { template: SquadTemplate; onOpen: () => void }) {
  const [hover, setHover] = useState(false)
  return (
    <button
      onClick={onOpen}
      onMouseEnter={() => setHover(true)}
      onMouseLeave={() => setHover(false)}
      data-testid={`card-${template.id}`}
      style={{
        minWidth: 0,
        textAlign: 'left',
        background: 'var(--white)',
        border: `1px solid ${hover ? 'var(--line2)' : 'var(--line)'}`,
        borderRadius: 14,
        padding: 20,
        display: 'flex',
        flexDirection: 'column',
        gap: 12,
        fontFamily: 'var(--sans)',
        color: 'var(--ink)',
        transition: 'transform .15s, box-shadow .15s, border-color .15s',
        transform: hover ? 'translateY(-3px)' : undefined,
        boxShadow: hover ? '0 14px 30px -18px #22140a55' : undefined,
      }}
    >
      <div style={{ display: 'flex', alignItems: 'center', gap: 10 }}>
        <span style={{ width: 12, height: 12, borderRadius: 4, background: template.cor, flex: 'none' }} />
        <span style={{ fontFamily: 'var(--disp)', fontWeight: 700, fontSize: 16, letterSpacing: '-0.01em' }}>
          {template.nome}
        </span>
        <span
          className="mono"
          style={{
            marginLeft: 'auto',
            fontSize: 9,
            letterSpacing: '0.1em',
            textTransform: 'uppercase',
            borderRadius: 999,
            padding: '3px 9px',
            ...ONDA_CSS[template.onda],
          }}
        >
          {ONDA_LABEL[template.onda]}
        </span>
      </div>
      <div style={{ fontSize: 12.5, color: 'var(--muted)', lineHeight: 1.55, minHeight: 38 }}>
        {template.descricao}
      </div>
      <div style={{ display: 'flex', flexWrap: 'wrap', gap: 5 }}>
        {template.papeis.map((p) => (
          <span
            key={p}
            style={{
              fontSize: 10.5,
              background: 'var(--paper)',
              border: '1px solid var(--line)',
              borderRadius: 999,
              padding: '3px 9px',
              color: 'var(--muted)',
            }}
          >
            {p}
          </span>
        ))}
      </div>
      <div
        style={{
          display: 'flex',
          alignItems: 'center',
          gap: 6,
          borderTop: '1px solid var(--line)',
          paddingTop: 11,
          marginTop: 2,
        }}
      >
        {template.formatos.map((f) => (
          <span
            key={f.nome}
            className="mono"
            style={{
              fontSize: 9.5,
              letterSpacing: '0.06em',
              background: '#f0ebdf',
              borderRadius: 5,
              padding: '3px 7px',
              color: template.cor,
              fontWeight: 600,
            }}
          >
            {f.nome}
          </span>
        ))}
        <span className="mono" style={{ marginLeft: 'auto', fontSize: 11, color: template.cor, fontWeight: 600 }}>
          montar →
        </span>
      </div>
    </button>
  )
}

export function Inicio() {
  const [cat, setCat] = useState<CategoriaSquad | 'todas'>('todas')
  const templates = useTemplates()
  const dispatch = useAppDispatch()

  if (templates.status === 'loading') {
    return (
      <div className="mono" style={{ color: 'var(--faint)', fontSize: 11.5 }}>
        carregando modelos…
      </div>
    )
  }
  if (templates.status === 'error') {
    return (
      <div
        style={{
          background: '#f7e7e3',
          border: '1px solid #e0b8ad',
          borderRadius: 12,
          padding: '16px 20px',
          color: '#a54334',
          fontSize: 13,
          lineHeight: 1.6,
        }}
      >
        Não consegui carregar os modelos ({templates.error.message}). O servidor local do
        BuildToValue não respondeu — abra o aplicativo de novo e recarregue esta página.
      </div>
    )
  }

  const visiveis = templates.templates.filter((t) => cat === 'todas' || t.categoria === cat)

  return (
    <>
      <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap' }}>
        {CATS.map(([id, label]) => {
          const active = cat === id
          return (
            <button
              key={id}
              onClick={() => setCat(id)}
              style={{
                borderRadius: 999,
                padding: '8px 16px',
                fontSize: 12.5,
                fontWeight: 600,
                fontFamily: 'var(--sans)',
                border: `1px solid ${active ? 'var(--brand)' : 'var(--line2)'}`,
                background: active ? 'var(--brand)' : 'var(--white)',
                color: active ? '#fff' : 'var(--muted)',
              }}
            >
              {label}
            </button>
          )
        })}
      </div>
      <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(250px, 1fr))', gap: 14 }}>
        {visiveis.map((t) => (
          <CardModelo
            key={t.id}
            template={t}
            onOpen={() => dispatch({ type: 'OPEN_WIZARD', templateId: t.id })}
          />
        ))}
      </div>
    </>
  )
}
