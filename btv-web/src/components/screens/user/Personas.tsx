import { useCallback, useEffect, useState, type CSSProperties } from 'react'
import {
  createCustomPersona,
  deleteCustomPersona,
  fetchPersonas,
  restoreAllPersonas,
  restorePersona,
  setPersonaOverride,
  updateCustomPersona,
  type PersonasResponse,
} from '../../../api/btv'
import { useTemplates } from '../../../state/TemplatesContext'
import { PAPEL_DESCS } from '../../wizard/Wizard'

const badgeBase: CSSProperties = {
  flex: 'none',
  fontFamily: 'var(--mono)',
  fontSize: 9.5,
  letterSpacing: '0.08em',
  borderRadius: 999,
  padding: '4px 10px',
}

/** U7 · Personas & prompts — os overrides são REAIS: o prompt efetivo
 *  (override ?? padrão) entra na descrição da próxima ativação e no hash de
 *  procedência do ledger. Debounce simples no salvar (onBlur). */
export function Personas() {
  const templates = useTemplates()
  const [templateId, setTemplateId] = useState('editorial')
  const [data, setData] = useState<PersonasResponse | null>(null)
  const [erro, setErro] = useState<string | null>(null)

  const recarregar = useCallback(() => {
    fetchPersonas(templateId)
      .then(setData)
      .catch((e: Error) => setErro(e.message))
  }, [templateId])

  useEffect(() => {
    setData(null)
    recarregar()
  }, [recarregar])

  if (templates.status !== 'ready') {
    return <div className="mono" style={{ color: 'var(--faint)', fontSize: 11.5 }}>carregando modelos…</div>
  }
  const template = templates.byId.get(templateId)
  const cor = template?.cor ?? 'var(--brand)'

  return (
    <>
      <div style={{ display: 'flex', gap: 7, flexWrap: 'wrap' }}>
        {templates.templates.map((m) => {
          const active = templateId === m.id
          return (
            <button
              key={m.id}
              onClick={() => setTemplateId(m.id)}
              style={{
                display: 'flex',
                alignItems: 'center',
                gap: 7,
                borderRadius: 999,
                padding: '7px 13px',
                fontSize: 12,
                fontWeight: 600,
                fontFamily: 'var(--sans)',
                border: `1px solid ${active ? m.cor : 'var(--line2)'}`,
                background: active ? 'var(--white)' : 'transparent',
                color: active ? 'var(--ink)' : 'var(--muted)',
              }}
            >
              <span style={{ width: 8, height: 8, borderRadius: 3, background: m.cor, flex: 'none' }} />
              {m.nome}
            </button>
          )
        })}
      </div>

      {erro && (
        <div style={{ background: '#f7e7e3', border: '1px solid #e0b8ad', borderRadius: 12, padding: '14px 18px', color: '#a54334', fontSize: 13 }}>
          {erro}
        </div>
      )}

      {data && (
        <>
          <div style={{ display: 'flex', alignItems: 'center', gap: 10 }}>
            <span style={{ fontSize: 12.5, color: 'var(--muted)' }}>
              {data.personas.length + data.proprias.length} personas · {template?.nome}
            </span>
            <button
              onClick={() => void restoreAllPersonas(templateId).then(recarregar)}
              className="mono"
              style={{ marginLeft: 'auto', background: 'none', border: '1px solid var(--line2)', borderRadius: 9, padding: '8px 14px', fontSize: 10.5, color: 'var(--muted)' }}
            >
              ↺ restaurar todos ao padrão
            </button>
            <button
              onClick={() =>
                void createCustomPersona(
                  templateId,
                  'Nova persona',
                  `Você é uma nova persona da squad ${template?.nome}. Descreva aqui o ofício, as responsabilidades e os limites deste papel — e quando ele deve passar o trabalho adiante.`,
                ).then(recarregar)
              }
              style={{ background: 'var(--brand)', color: '#fff', border: 'none', borderRadius: 9, padding: '9px 16px', fontSize: 12.5, fontWeight: 600, fontFamily: 'var(--sans)' }}
            >
              + Nova persona
            </button>
          </div>

          <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(330px, 1fr))', gap: 14 }}>
            {data.personas.map((p, i) => (
              <div key={p.papel} data-testid={`persona-${p.papel}`} style={{ minWidth: 0, background: 'var(--white)', border: '1px solid var(--line)', borderRadius: 14, padding: '18px 20px', display: 'flex', flexDirection: 'column', gap: 12 }}>
                <div style={{ display: 'flex', alignItems: 'center', gap: 11 }}>
                  <span style={{ width: 36, height: 36, borderRadius: 12, background: cor, color: '#fff', display: 'flex', alignItems: 'center', justifyContent: 'center', fontFamily: 'var(--disp)', fontWeight: 700, fontSize: 15, flex: 'none' }}>
                    {p.papel[0]}
                  </span>
                  <span style={{ display: 'flex', flexDirection: 'column', gap: 2, minWidth: 0, flex: 1 }}>
                    <span style={{ fontSize: 14, fontWeight: 600 }}>{p.papel}</span>
                    <span style={{ fontSize: 11, color: 'var(--faint)' }}>
                      {PAPEL_DESCS[Math.min(i, PAPEL_DESCS.length - 1)]}
                    </span>
                  </span>
                  <span
                    style={{
                      ...badgeBase,
                      ...(p.editado
                        ? { background: '#fdf3e3', color: '#9a6b14' }
                        : { background: 'var(--paper)', color: 'var(--faint)' }),
                    }}
                  >
                    {p.editado ? 'editado' : 'padrão'}
                  </span>
                </div>
                <textarea
                  key={`${templateId}-${p.papel}-${p.editado}`}
                  defaultValue={p.prompt}
                  onBlur={(e) => {
                    const v = e.target.value
                    if (v !== p.prompt) void setPersonaOverride(templateId, p.papel, v).then(recarregar)
                  }}
                  style={{ width: '100%', minHeight: 128, border: '1px solid var(--line)', borderRadius: 10, padding: '12px 14px', fontFamily: 'var(--mono)', fontSize: 11.5, lineHeight: 1.65, background: 'var(--paper)', color: 'var(--ink)', resize: 'vertical' }}
                />
                <div style={{ display: 'flex', gap: 8, minHeight: 28 }}>
                  {p.editado && (
                    <button
                      onClick={() => void restorePersona(templateId, p.papel).then(recarregar)}
                      className="mono"
                      style={{ background: 'none', border: '1px solid var(--line2)', borderRadius: 8, padding: '7px 13px', fontSize: 10, color: 'var(--brand)', fontWeight: 600 }}
                    >
                      ↺ restaurar padrão
                    </button>
                  )}
                </div>
              </div>
            ))}

            {data.proprias.map((c) => (
              <div key={c.id} style={{ minWidth: 0, background: 'var(--white)', border: '1px solid var(--line)', borderRadius: 14, padding: '18px 20px', display: 'flex', flexDirection: 'column', gap: 12 }}>
                <div style={{ display: 'flex', alignItems: 'center', gap: 11 }}>
                  <span style={{ width: 36, height: 36, borderRadius: 12, background: cor, color: '#fff', display: 'flex', alignItems: 'center', justifyContent: 'center', fontFamily: 'var(--disp)', fontWeight: 700, fontSize: 15, flex: 'none' }}>
                    {(c.nome[0] ?? 'P').toUpperCase()}
                  </span>
                  <span style={{ display: 'flex', flexDirection: 'column', gap: 2, minWidth: 0, flex: 1 }}>
                    <input
                      defaultValue={c.nome}
                      onBlur={(e) => {
                        if (e.target.value !== c.nome)
                          void updateCustomPersona(templateId, c.id, e.target.value, c.prompt).then(recarregar)
                      }}
                      style={{ border: 'none', borderBottom: '1px dashed var(--line2)', background: 'none', fontSize: 14, fontWeight: 600, fontFamily: 'var(--sans)', color: 'var(--ink)', padding: '2px 0', width: '100%' }}
                    />
                    <span style={{ fontSize: 11, color: 'var(--faint)' }}>persona criada por você</span>
                  </span>
                  <span style={{ ...badgeBase, background: '#e7efe9', color: '#2d6a50' }}>própria</span>
                </div>
                <textarea
                  defaultValue={c.prompt}
                  onBlur={(e) => {
                    if (e.target.value !== c.prompt)
                      void updateCustomPersona(templateId, c.id, c.nome, e.target.value).then(recarregar)
                  }}
                  style={{ width: '100%', minHeight: 128, border: '1px solid var(--line)', borderRadius: 10, padding: '12px 14px', fontFamily: 'var(--mono)', fontSize: 11.5, lineHeight: 1.65, background: 'var(--paper)', color: 'var(--ink)', resize: 'vertical' }}
                />
                <div style={{ display: 'flex', gap: 8, minHeight: 28 }}>
                  <button
                    onClick={() => void deleteCustomPersona(templateId, c.id).then(recarregar)}
                    className="mono"
                    style={{ marginLeft: 'auto', background: 'none', border: '1px solid var(--line2)', borderRadius: 8, padding: '7px 13px', fontSize: 10, color: 'var(--muted)' }}
                  >
                    remover
                  </button>
                </div>
              </div>
            ))}
          </div>
        </>
      )}
    </>
  )
}
