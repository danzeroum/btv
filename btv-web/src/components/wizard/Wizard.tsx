import { useRef, useState, type CSSProperties } from 'react'
import { useAppDispatch, useAppState } from '../../state/AppContext'
import { useTemplates } from '../../state/TemplatesContext'
import { useSquadRun } from '../../state/SquadRunContext'
import type { SquadTemplate } from '../../api/templates'

/** Descrições genéricas por índice de papel — os 4 arquétipos do protótipo
 *  (abre o trabalho / produz / revisa / valida). */
export const PAPEL_DESCS = [
  'abre o trabalho e estrutura o plano',
  'produz a primeira versão',
  'refina qualidade e consistência',
  'valida antes da entrega',
]

interface RefItem {
  t: 'link' | 'arquivo'
  label: string
}

const STEP_NAMES = ['Briefing', 'Equipe', 'Entregas & gates']

function Toggle({ on, onClick, label }: { on: boolean; onClick: () => void; label: string }) {
  return (
    <button
      onClick={onClick}
      aria-label={label}
      aria-pressed={on}
      style={{
        marginLeft: 'auto',
        width: 40,
        height: 22,
        borderRadius: 99,
        border: 'none',
        position: 'relative',
        transition: 'background .15s',
        background: on ? 'var(--brand)' : 'var(--line2)',
        flex: 'none',
      }}
    >
      <span
        style={{
          position: 'absolute',
          top: 3,
          width: 16,
          height: 16,
          borderRadius: '50%',
          background: '#fff',
          transition: 'left .15s',
          left: on ? 21 : 3,
        }}
      />
    </button>
  )
}

function WizardInner({ template, onClose }: { template: SquadTemplate; onClose: () => void }) {
  const [step, setStep] = useState(0)
  const [papeisOff, setPapeisOff] = useState<Record<number, boolean>>({})
  const [refs, setRefs] = useState<RefItem[]>([])
  const [answers, setAnswers] = useState<string[]>(() => template.perguntas.map(() => ''))
  const [ativando, setAtivando] = useState(false)
  const [erroAtivacao, setErroAtivacao] = useState<string | null>(null)
  const linkRef = useRef<HTMLInputElement | null>(null)
  const fileRef = useRef<HTMLInputElement | null>(null)
  const { ativar } = useSquadRun()

  const ativarSquadReal = async () => {
    setAtivando(true)
    setErroAtivacao(null)
    try {
      await ativar(template, {
        briefing: template.perguntas.map((q, i) => ({ label: q.label, resposta: answers[i] ?? '' })),
        refs: refs.map((r) => (r.t === 'arquivo' ? `arquivo: ${r.label}` : r.label)),
        papeis_off: Object.entries(papeisOff)
          .filter(([, off]) => off)
          .map(([i]) => Number(i)),
      })
      // `ativar` fecha o wizard e navega para Ao vivo.
    } catch (e) {
      setErroAtivacao(e instanceof Error ? e.message : String(e))
      setAtivando(false)
    }
  }

  const addLink = () => {
    const v = linkRef.current?.value.trim()
    if (!v) return
    linkRef.current!.value = ''
    setRefs((r) => [...r, { t: 'link', label: v }])
  }

  const inputStyle: CSSProperties = {
    border: '1px solid var(--line2)',
    borderRadius: 10,
    padding: '11px 14px',
    fontSize: 13.5,
    background: 'var(--white)',
    color: 'var(--ink)',
  }

  return (
    <div
      data-testid="wizard-overlay"
      style={{
        position: 'fixed',
        inset: 0,
        background: '#221d1580',
        zIndex: 50,
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        padding: 30,
      }}
    >
      <div
        style={{
          width: 640,
          maxWidth: '100%',
          maxHeight: '88vh',
          overflowY: 'auto',
          background: 'var(--card)',
          borderRadius: 20,
          boxShadow: '0 40px 90px -30px #000a',
          padding: '32px 36px',
          display: 'flex',
          flexDirection: 'column',
          gap: 20,
        }}
      >
        <div style={{ display: 'flex', alignItems: 'center', gap: 12 }}>
          <span style={{ width: 13, height: 13, borderRadius: 4, background: template.cor }} />
          <span style={{ fontFamily: 'var(--disp)', fontWeight: 700, fontSize: 19, letterSpacing: '-0.01em' }}>
            Montar squad · {template.nome}
          </span>
          <button
            onClick={onClose}
            aria-label="Fechar wizard"
            style={{ marginLeft: 'auto', background: 'none', border: 'none', fontSize: 17, color: 'var(--faint)' }}
          >
            ✕
          </button>
        </div>

        <div style={{ display: 'flex', gap: 6 }}>
          {STEP_NAMES.map((label, i) => (
            <div key={label} style={{ flex: 1, display: 'flex', flexDirection: 'column', gap: 6 }}>
              <div
                style={{
                  height: 4,
                  borderRadius: 99,
                  background: i <= step ? template.cor : 'var(--line)',
                }}
              />
              <span
                className="mono"
                style={{
                  fontSize: 9.5,
                  letterSpacing: '0.1em',
                  textTransform: 'uppercase',
                  color: i === step ? 'var(--ink)' : 'var(--faint)',
                }}
              >
                {label}
              </span>
            </div>
          ))}
        </div>

        {step === 0 && (
          <div style={{ display: 'flex', flexDirection: 'column', gap: 14 }}>
            <p style={{ fontSize: 13.5, color: 'var(--muted)', margin: 0, lineHeight: 1.6 }}>
              Conte para a squad o que você precisa — nas palavras da sua área. Nada aqui é técnico.
            </p>
            {template.perguntas.map((q, i) => (
              <label key={q.label} style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
                <span style={{ fontSize: 13, fontWeight: 600 }}>{q.label}</span>
                <input
                  placeholder={q.placeholder}
                  value={answers[i]}
                  onChange={(e) =>
                    setAnswers((a) => a.map((v, j) => (j === i ? e.target.value : v)))
                  }
                  style={inputStyle}
                />
              </label>
            ))}
            <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
              <span style={{ fontSize: 13, fontWeight: 600 }}>
                Referências e materiais <span style={{ color: 'var(--faint)', fontWeight: 400 }}>· opcional</span>
              </span>
              <div style={{ display: 'flex', gap: 8 }}>
                <input
                  ref={linkRef}
                  placeholder="cole um link de referência…"
                  onKeyDown={(e) => {
                    if (e.key === 'Enter') addLink()
                  }}
                  style={{ ...inputStyle, flex: 1, padding: '10px 14px', fontSize: 13, minWidth: 0 }}
                />
                <button
                  onClick={addLink}
                  style={{
                    background: 'none',
                    border: '1px solid var(--line2)',
                    borderRadius: 10,
                    padding: '0 16px',
                    fontSize: 12.5,
                    fontWeight: 600,
                    color: 'var(--brand)',
                    fontFamily: 'var(--sans)',
                  }}
                >
                  adicionar
                </button>
              </div>
              <input
                ref={fileRef}
                type="file"
                multiple
                style={{ display: 'none' }}
                onChange={(e) => {
                  const files = Array.from(e.target.files ?? [])
                  if (files.length) {
                    setRefs((r) => [...r, ...files.map((f) => ({ t: 'arquivo' as const, label: f.name }))])
                  }
                  e.target.value = ''
                }}
              />
              <button
                onClick={() => fileRef.current?.click()}
                style={{
                  border: '1.5px dashed var(--line2)',
                  borderRadius: 10,
                  padding: 13,
                  background: 'none',
                  color: 'var(--faint)',
                  fontSize: 12,
                  fontFamily: 'var(--sans)',
                }}
              >
                ⇪ arraste arquivos aqui ou clique para anexar
              </button>
              {refs.length > 0 && (
                <div style={{ display: 'flex', flexWrap: 'wrap', gap: 6 }}>
                  {refs.map((r, i) => (
                    <span
                      key={`${r.label}-${i}`}
                      className="mono"
                      style={{
                        display: 'flex',
                        alignItems: 'center',
                        gap: 6,
                        fontSize: 10.5,
                        background: 'var(--paper)',
                        border: '1px solid var(--line)',
                        borderRadius: 999,
                        padding: '5px 7px 5px 12px',
                        color: 'var(--muted)',
                      }}
                    >
                      {(r.t === 'arquivo' ? '⎘ ' : '↗ ') + r.label}
                      <button
                        onClick={() => setRefs((all) => all.filter((_, j) => j !== i))}
                        aria-label={`Remover referência ${r.label}`}
                        style={{ background: 'none', border: 'none', color: 'var(--faint)', fontSize: 12, padding: '0 4px' }}
                      >
                        ×
                      </button>
                    </span>
                  ))}
                </div>
              )}
            </div>
          </div>
        )}

        {step === 1 && (
          <div style={{ display: 'flex', flexDirection: 'column', gap: 10 }}>
            <p style={{ fontSize: 13.5, color: 'var(--muted)', margin: 0, lineHeight: 1.6 }}>
              Esta é a equipe do modelo. Desligue papéis que você mesmo fará.
            </p>
            {template.papeis.map((nome, i) => {
              const off = !!papeisOff[i]
              return (
                <div
                  key={nome}
                  style={{
                    display: 'flex',
                    alignItems: 'center',
                    gap: 14,
                    background: 'var(--white)',
                    border: '1px solid var(--line)',
                    borderRadius: 12,
                    padding: '13px 16px',
                  }}
                >
                  <span
                    style={{
                      width: 34,
                      height: 34,
                      borderRadius: 11,
                      background: off ? 'var(--faint)' : template.cor,
                      color: '#fff',
                      display: 'flex',
                      alignItems: 'center',
                      justifyContent: 'center',
                      fontFamily: 'var(--disp)',
                      fontWeight: 700,
                      fontSize: 14,
                    }}
                  >
                    {nome[0]}
                  </span>
                  <span style={{ display: 'flex', flexDirection: 'column', gap: 1 }}>
                    <span style={{ fontSize: 13.5, fontWeight: 600 }}>{nome}</span>
                    <span style={{ fontSize: 11.5, color: 'var(--faint)' }}>
                      {PAPEL_DESCS[Math.min(i, PAPEL_DESCS.length - 1)]}
                    </span>
                  </span>
                  <Toggle
                    on={!off}
                    label={`Papel ${nome} ${off ? 'desligado' : 'ligado'}`}
                    onClick={() => setPapeisOff((p) => ({ ...p, [i]: !off }))}
                  />
                </div>
              )
            })}
          </div>
        )}

        {step === 2 && (
          <div style={{ display: 'flex', flexDirection: 'column', gap: 16 }}>
            <div>
              <div style={{ fontSize: 13, fontWeight: 600, marginBottom: 8 }}>Formatos de entrega</div>
              <div style={{ display: 'flex', gap: 7, flexWrap: 'wrap' }}>
                {template.formatos.map((f) => (
                  <span
                    key={f.nome}
                    className="mono"
                    style={{
                      fontSize: 11,
                      fontWeight: 600,
                      background: '#f0ebdf',
                      color: template.cor,
                      border: '1px solid var(--line)',
                      borderRadius: 8,
                      padding: '7px 13px',
                    }}
                  >
                    {f.nome}
                  </span>
                ))}
              </div>
            </div>
            <div>
              <div style={{ fontSize: 13, fontWeight: 600, marginBottom: 8 }}>
                Pontos onde a squad vai esperar por você
              </div>
              <div style={{ display: 'flex', flexDirection: 'column', gap: 7 }}>
                {template.gates.map((g) => (
                  <span
                    key={g}
                    style={{ fontSize: 13, color: 'var(--muted)', display: 'flex', gap: 9, alignItems: 'center' }}
                  >
                    <span style={{ color: 'var(--gold)', fontSize: 14 }}>✋</span>
                    {g}
                  </span>
                ))}
              </div>
            </div>
            <div
              style={{
                background: 'var(--paper)',
                border: '1px solid var(--line)',
                borderRadius: 10,
                padding: '13px 16px',
                fontSize: 12,
                color: 'var(--muted)',
                lineHeight: 1.6,
              }}
            >
              As ferramentas desta squad já foram liberadas pela administração. Você pode acompanhar
              tudo na tela <strong>Ao vivo</strong> e pedir ajustes a qualquer momento.
            </div>
          </div>
        )}

        <div style={{ display: 'flex', gap: 10, borderTop: '1px solid var(--line)', paddingTop: 18, alignItems: 'center' }}>
          {step > 0 && (
            <button
              onClick={() => setStep((s) => s - 1)}
              style={{
                background: 'none',
                border: '1px solid var(--line2)',
                borderRadius: 10,
                padding: '11px 18px',
                fontSize: 13.5,
                color: 'var(--muted)',
                fontFamily: 'var(--sans)',
              }}
            >
              ← Voltar
            </button>
          )}
          {erroAtivacao && (
            <span style={{ marginLeft: 'auto', fontSize: 11.5, color: '#a54334', maxWidth: 280, lineHeight: 1.4 }}>
              {erroAtivacao}
            </span>
          )}
          <button
            onClick={() => {
              if (step < 2) setStep((s) => s + 1)
              else void ativarSquadReal()
            }}
            disabled={ativando}
            style={{
              marginLeft: erroAtivacao ? 0 : 'auto',
              background: ativando ? 'var(--line2)' : 'var(--brand)',
              color: '#fff',
              border: 'none',
              borderRadius: 10,
              padding: '11px 26px',
              fontSize: 13.5,
              fontWeight: 600,
              fontFamily: 'var(--sans)',
              cursor: ativando ? 'wait' : 'pointer',
            }}
          >
            {step < 2 ? 'Continuar →' : ativando ? 'ativando…' : '⚑ Ativar squad'}
          </button>
        </div>
      </div>
    </div>
  )
}

/** Overlay do wizard — renderizado pelo Shell quando `wizardTemplateId` está
 *  setado (galeria U1 ou "reativar" de U6). */
export function WizardOverlay() {
  const { wizardTemplateId } = useAppState()
  const dispatch = useAppDispatch()
  const templates = useTemplates()
  if (!wizardTemplateId || templates.status !== 'ready') return null
  const template = templates.byId.get(wizardTemplateId)
  if (!template) return null
  return <WizardInner template={template} onClose={() => dispatch({ type: 'CLOSE_WIZARD' })} />
}
