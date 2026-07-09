import { useEffect, useRef, useState, type CSSProperties } from 'react'
import { useSquadRun } from '../../../state/SquadRunContext'
import { useAppDispatch } from '../../../state/AppContext'
import { listDeliverables } from '../../../api/btv'

/** Frases do papel ativo por etapa (copy do protótipo). */
const DOING: Record<string, string> = {
  Planejamento: 'está estruturando o plano de trabalho a partir do seu briefing',
  Produção: 'está produzindo a primeira versão completa',
  Revisão: 'está revisando estilo, consistência e clareza',
  Validação: 'está validando fatos e requisitos antes da entrega',
  Exportação: 'está gerando os arquivos finais',
}

const ghostBtn: CSSProperties = {
  background: 'none',
  border: '1px solid var(--line2)',
  borderRadius: 8,
  padding: '6px 13px',
  fontFamily: 'var(--mono)',
  fontSize: 10.5,
  color: 'var(--muted)',
}

export function Vivo() {
  const { run, view, feed, chat, aprovar, ajustar, enviarChat, encerrar } = useSquadRun()
  const dispatch = useAppDispatch()
  const [ajusteMode, setAjusteMode] = useState(false)
  const ajusteRef = useRef<HTMLTextAreaElement | null>(null)
  const chatRef = useRef<HTMLInputElement | null>(null)
  const chatBodyRef = useRef<HTMLDivElement | null>(null)

  // Só uma squad executa por vez (capacidade 1 do pool): uma ativação feita
  // enquanto outra roda fica na fila sem emitir evento nenhum. Sem sinal do
  // backend, inferimos honestamente: run ativo + feed vazio por alguns
  // segundos → provável fila.
  const hasFeed = feed.length > 0
  const runDone = view?.done ?? false
  const [filaHint, setFilaHint] = useState(false)
  useEffect(() => {
    if (!run || hasFeed || runDone) {
      setFilaHint(false)
      return
    }
    const timer = window.setTimeout(() => setFilaHint(true), 5000)
    return () => window.clearTimeout(timer)
  }, [run, hasFeed, runDone])

  // Ao concluir, conta as entregas REAIS desta task (arquivo gravado por
  // ferramenta). O insert é server-side na conclusão, então pode chegar logo
  // após o 'done' — daí um refetch curto antes de cravar "sem artefato".
  const taskId = run?.taskId
  const [artefatosDaTask, setArtefatosDaTask] = useState<number | null>(null)
  useEffect(() => {
    if (!runDone || !taskId) {
      setArtefatosDaTask(null)
      return
    }
    let cancel = false
    const contar = async (): Promise<number> => {
      try {
        const ds = await listDeliverables()
        return ds.filter((d) => d.task_id === taskId).length
      } catch {
        return 0
      }
    }
    void (async () => {
      let n = await contar()
      if (n === 0) {
        await new Promise((r) => setTimeout(r, 1400))
        if (!cancel) n = await contar()
      }
      if (!cancel) setArtefatosDaTask(n)
    })()
    return () => {
      cancel = true
    }
  }, [runDone, taskId])

  if (!run || !view) {
    return (
      <div
        style={{
          background: 'var(--white)',
          border: '1px dashed var(--line2)',
          borderRadius: 14,
          padding: '28px 30px',
          color: 'var(--muted)',
          fontSize: 13.5,
          lineHeight: 1.6,
        }}
      >
        Nenhuma squad rodando agora. Monte uma na{' '}
        <a
          href="#"
          onClick={(e) => {
            e.preventDefault()
            dispatch({ type: 'SET_SCREEN', screen: 'inicio' })
          }}
        >
          galeria de modelos
        </a>
        .
      </div>
    )
  }

  const cor = run.template.cor
  const etapas = run.etapas
  const etapaAtual = view.idx < etapas.length ? etapas[view.idx] : null
  const roster = [
    ...new Set(etapas.map((e) => e.papel).filter((p) => p !== 'Você' && p !== 'BuildToValue')),
  ]

  const enviarAjuste = async () => {
    const txt = ajusteRef.current?.value.trim() || 'ajustar conforme conversa'
    setAjusteMode(false)
    await ajustar(txt)
  }

  const sendChat = async () => {
    const el = chatRef.current
    const v = el?.value.trim()
    if (!v) return
    el!.value = ''
    await enviarChat(v)
    requestAnimationFrame(() => {
      const body = chatBodyRef.current
      if (body) body.scrollTop = body.scrollHeight
    })
  }

  return (
    <>
      {/* ── esteira ── */}
      <div
        data-testid="esteira"
        style={{ background: 'var(--white)', border: '1px solid var(--line)', borderRadius: 16, padding: '24px 26px 20px' }}
      >
        <div style={{ display: 'flex', alignItems: 'center', gap: 12, marginBottom: 20 }}>
          <span style={{ width: 12, height: 12, borderRadius: 4, background: cor }} />
          <span style={{ fontFamily: 'var(--disp)', fontWeight: 700, fontSize: 17 }}>{run.nome}</span>
          <span className="mono" style={{ fontSize: 10.5, color: 'var(--faint)', letterSpacing: '0.08em' }}>
            etapa {Math.min(view.idx + 1, etapas.length)} de {etapas.length}
            {run.teste ? ' · execução de teste' : ''}
          </span>
          <button
            onClick={() => void encerrar()}
            style={{ ...ghostBtn, marginLeft: 'auto' }}
            data-testid="encerrar-squad"
          >
            encerrar squad
          </button>
        </div>
        <div style={{ display: 'flex', alignItems: 'flex-start', gap: 0 }}>
          {etapas.map((e, i) => {
            const done = i < view.idx || view.done
            const active = i === view.idx && !view.done
            const base: CSSProperties = {
              width: 26,
              height: 26,
              borderRadius: '50%',
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
              fontSize: 11,
              flex: 'none',
              zIndex: 1,
            }
            let dotStyle: CSSProperties
            let dotIcon: string
            if (done) {
              dotStyle = { ...base, background: cor, color: '#fff', fontWeight: 700 }
              dotIcon = '✓'
            } else if (active && e.gate && view.gateOpen) {
              dotStyle = { ...base, background: 'var(--decision)', color: 'var(--card)', animation: 'btvPulse 1.6s infinite' }
              dotIcon = '✋'
            } else if (active) {
              dotStyle = {
                ...base,
                background: '#fff',
                outline: `2px solid ${cor}`,
                color: cor,
                fontWeight: 700,
                animation: 'btvPulse 1.8s infinite',
              }
              dotIcon = '●'
            } else {
              dotStyle = { ...base, background: 'var(--paper)', outline: '1px solid var(--line2)', color: 'var(--faint)' }
              dotIcon = String(i + 1)
            }
            return (
              <div key={e.nome + i} style={{ flex: 1, display: 'flex', flexDirection: 'column', gap: 8, position: 'relative', padding: '0 4px' }}>
                <div style={{ display: 'flex', alignItems: 'center', width: '100%' }}>
                  <span style={{ flex: 1, height: 2, background: i === 0 ? 'transparent' : done || active ? cor : 'var(--line)' }} />
                  <span style={dotStyle}>{dotIcon}</span>
                  <span style={{ flex: 1, height: 2, background: i === etapas.length - 1 ? 'transparent' : done ? cor : 'var(--line)' }} />
                </div>
                <div style={{ textAlign: 'center', display: 'flex', flexDirection: 'column', gap: 3 }}>
                  <span style={{ fontSize: 12, fontWeight: 600, color: done || active ? 'var(--ink)' : 'var(--faint)' }}>
                    {e.nome}
                  </span>
                  <span className="mono" style={{ fontSize: 9.5, color: 'var(--faint)' }}>{e.papel}</span>
                </div>
                {active && !e.gate && !view.erro && (
                  <div style={{ height: 4, background: 'var(--paper)', borderRadius: 99, overflow: 'hidden', margin: '0 12px' }}>
                    {/* Sem % fabricado: barra pulsante enquanto o trabalho real acontece. */}
                    <div style={{ height: '100%', background: cor, borderRadius: 99, animation: 'btvPulse 1.4s infinite' }} />
                  </div>
                )}
              </div>
            )
          })}
        </div>
      </div>

      <div style={{ display: 'grid', gridTemplateColumns: '1.5fr 1fr', gap: 14, alignItems: 'start' }}>
        <div style={{ display: 'flex', flexDirection: 'column', gap: 14 }}>
          {/* ── erro / kill-switch ── */}
          {view.erro && (
            <div
              style={{
                background: '#f7e7e3',
                border: '1px solid #e0b8ad',
                borderRadius: 16,
                padding: '18px 22px',
                color: '#a54334',
                fontSize: 13,
                lineHeight: 1.6,
              }}
            >
              <strong>Squad interrompida.</strong> {view.erro}
            </div>
          )}

          {/* ── gate humano ── */}
          {view.gateOpen && etapaAtual && (
            <div
              data-testid="gate-card"
              style={{ background: 'var(--white)', border: '2px solid var(--decision)', borderRadius: 16, padding: '22px 24px', display: 'flex', flexDirection: 'column', gap: 14 }}
            >
              <div style={{ display: 'flex', alignItems: 'center', gap: 10 }}>
                <span style={{ width: 26, height: 26, borderRadius: 8, background: 'var(--decision)', color: 'var(--card)', display: 'flex', alignItems: 'center', justifyContent: 'center', fontSize: 14 }}>
                  ✋
                </span>
                <span style={{ fontFamily: 'var(--disp)', fontWeight: 700, fontSize: 16 }}>
                  {etapaAtual.nome === 'Entrega'
                    ? 'Entrega final pronta para sua aprovação'
                    : 'Rascunho pronto para sua aprovação'}
                </span>
                <span className="status-gate" style={{ marginLeft: 'auto', fontSize: 10, letterSpacing: '0.1em' }}>
                  gate humano
                </span>
              </div>
              <div
                style={{ background: 'var(--paper)', border: '1px solid var(--line)', borderRadius: 10, padding: '16px 18px', fontSize: 13.5, lineHeight: 1.65 }}
              >
                <div className="mono" style={{ fontSize: 10, color: cor, letterSpacing: '0.1em', textTransform: 'uppercase', marginBottom: 8 }}>
                  o que o orquestrador reportou
                </div>
                {/* Sinais REAIS do stream — nada de checklist fabricado. */}
                {feed.slice(0, 3).map((f, i) => (
                  <div key={i} style={{ display: 'flex', gap: 8, alignItems: 'baseline' }}>
                    <span className="mono" style={{ fontSize: 10, color: 'var(--faint)' }}>{f.ts}</span>
                    <span>{f.txt}</span>
                  </div>
                ))}
              </div>
              {ajusteMode && (
                <textarea
                  ref={ajusteRef}
                  placeholder="Descreva o ajuste em uma frase — ela vira instrução para o papel certo…"
                  style={{ width: '100%', minHeight: 64, border: '1px solid var(--line2)', borderRadius: 10, padding: '12px 14px', fontSize: 13.5, background: 'var(--paper)', color: 'var(--ink)', resize: 'vertical' }}
                />
              )}
              <div style={{ display: 'flex', gap: 10 }}>
                <button
                  onClick={() => void aprovar()}
                  className="btn-decision"
                  style={{ padding: '11px 22px', borderRadius: 10, fontSize: 13.5, fontFamily: 'var(--sans)' }}
                >
                  Aprovar e continuar
                </button>
                <button
                  onClick={() => (ajusteMode ? void enviarAjuste() : setAjusteMode(true))}
                  style={{ background: 'none', border: '1px solid var(--line2)', borderRadius: 10, padding: '11px 18px', fontSize: 13.5, color: 'var(--muted)', fontFamily: 'var(--sans)' }}
                >
                  {ajusteMode ? 'Enviar ajuste' : 'Pedir ajuste'}
                </button>
              </div>
            </div>
          )}

          {/* ── papel ativo ── */}
          {!view.gateOpen && !view.done && !view.erro && etapaAtual && (
            <div
              data-testid="papel-ativo"
              style={{ background: 'var(--white)', border: '1px solid var(--brand)', borderRadius: 16, padding: '22px 24px', display: 'flex', gap: 16, alignItems: 'flex-start' }}
            >
              <div style={{ width: 44, height: 44, borderRadius: 14, background: cor, color: '#fff', display: 'flex', alignItems: 'center', justifyContent: 'center', fontFamily: 'var(--disp)', fontWeight: 800, fontSize: 17, flex: 'none' }}>
                {etapaAtual.papel[0] ?? 'B'}
              </div>
              <div style={{ display: 'flex', flexDirection: 'column', gap: 6, minWidth: 0 }}>
                <div style={{ display: 'flex', alignItems: 'center', gap: 10 }}>
                  <span style={{ fontFamily: 'var(--disp)', fontWeight: 700, fontSize: 15.5 }}>{etapaAtual.papel}</span>
                  <span className="mono" style={{ fontSize: 10, background: '#eaf1ec', color: 'var(--ok)', borderRadius: 999, padding: '3px 9px' }}>
                    trabalhando agora
                  </span>
                </div>
                <span style={{ fontSize: 13.5, color: 'var(--muted)', lineHeight: 1.6 }}>
                  {DOING[etapaAtual.nome] ?? `está trabalhando em "${etapaAtual.nome.toLowerCase()}"`}
                  <span style={{ animation: 'btvBlink 1s infinite', color: cor, fontWeight: 700 }}> ▮</span>
                </span>
                {view.inferida && (
                  <span className="mono" style={{ fontSize: 9.5, color: 'var(--faint)' }}>
                    posição da esteira inferida dos eventos do orquestrador
                  </span>
                )}
              </div>
            </div>
          )}

          {/* ── conclusão ── */}
          {view.done &&
            (artefatosDaTask === 0 ? (
              // Concluiu SEM gravar arquivo real: os agentes descreveram a
              // entrega sem usar a ferramenta de escrita → nada na Biblioteca.
              // Honestidade "Nada Fake": avisa em vez de apontar p/ tela vazia.
              <div
                data-testid="squad-done-sem-artefato"
                style={{ background: 'var(--paper)', border: '1px solid var(--line2)', borderRadius: 16, padding: '22px 24px', display: 'flex', alignItems: 'flex-start', gap: 14 }}
              >
                <span style={{ fontSize: 22 }}>⚠︎</span>
                <div>
                  <div style={{ fontFamily: 'var(--disp)', fontWeight: 700, fontSize: 15.5, color: 'var(--ink)' }}>
                    Concluída, mas sem artefato real
                  </div>
                  <div style={{ fontSize: 13, color: 'var(--muted)', marginTop: 3, lineHeight: 1.6 }}>
                    A squad terminou, mas <strong>nenhum arquivo foi gravado por ferramenta</strong> —
                    os agentes descreveram a entrega em texto sem chamar a ferramenta de escrita, então{' '}
                    <strong>nada foi para a Biblioteca</strong>. Tente novamente ou refine o briefing/modelo.
                  </div>
                </div>
              </div>
            ) : (
              <div
                data-testid="squad-done"
                style={{ background: '#e7efe9', border: '1px solid #bcd4c8', borderRadius: 16, padding: '22px 24px', display: 'flex', alignItems: 'center', gap: 14 }}
              >
                <span style={{ fontSize: 22 }}>◈</span>
                <div>
                  <div style={{ fontFamily: 'var(--disp)', fontWeight: 700, fontSize: 15.5, color: 'var(--brandink)' }}>
                    Entrega concluída
                  </div>
                  <div style={{ fontSize: 13, color: '#3f6355', marginTop: 3 }}>
                    Os artefatos estão na{' '}
                    <a
                      href="#"
                      onClick={(e) => {
                        e.preventDefault()
                        dispatch({ type: 'SET_SCREEN', screen: 'biblioteca' })
                      }}
                    >
                      Biblioteca de entregas
                    </a>
                    , com trilha completa de procedência.
                  </div>
                </div>
              </div>
            ))}

          {/* ── cockpit ── */}
          <div
            data-testid="cockpit"
            style={{ background: 'var(--white)', border: '1px solid var(--line)', borderRadius: 16, display: 'flex', flexDirection: 'column', overflow: 'hidden' }}
          >
            <div style={{ display: 'flex', alignItems: 'center', gap: 10, padding: '13px 18px', borderBottom: '1px solid var(--line)', flexWrap: 'wrap' }}>
              <span style={{ fontFamily: 'var(--disp)', fontWeight: 700, fontSize: 14 }}>Cockpit</span>
              <span className="mono" style={{ fontSize: 9.5, color: 'var(--faint)', letterSpacing: '0.1em' }}>
                você faz parte da squad
              </span>
              <span style={{ marginLeft: 'auto', display: 'flex', gap: 5, flexWrap: 'wrap' }}>
                {roster.map((r) => (
                  <span key={r} className="mono" style={{ fontSize: 9, letterSpacing: '0.05em', borderRadius: 999, padding: '3px 8px', background: 'var(--paper)', color: 'var(--muted)', border: '1px solid var(--line)' }}>
                    {r}
                  </span>
                ))}
                <span className="mono" style={{ fontSize: 9, letterSpacing: '0.05em', borderRadius: 999, padding: '3px 8px', background: 'var(--brand)', color: '#fff' }}>
                  Você
                </span>
              </span>
            </div>
            <div ref={chatBodyRef} style={{ display: 'flex', flexDirection: 'column', gap: 10, padding: '16px 18px', maxHeight: 250, overflowY: 'auto' }}>
              {chat.length === 0 && (
                <span className="mono" style={{ fontSize: 10.5, color: 'var(--faint)' }}>
                  os agentes falam aqui conforme trabalham — sua orientação entra no contexto do papel
                  ativo na próxima chamada
                </span>
              )}
              {chat.map((m, i) => {
                const me = m.author_role === 'HUMAN'
                return (
                  <div key={i} style={{ display: 'flex', justifyContent: me ? 'flex-end' : 'flex-start' }}>
                    <div
                      style={{
                        maxWidth: '85%',
                        padding: '9px 13px',
                        fontSize: 12.5,
                        lineHeight: 1.55,
                        ...(me
                          ? { background: 'var(--brand)', color: '#fff', borderRadius: '12px 12px 4px 12px' }
                          : { background: 'var(--paper)', color: 'var(--ink)', border: '1px solid var(--line)', borderRadius: '12px 12px 12px 4px' }),
                      }}
                    >
                      <div className="mono" style={{ fontSize: 9, letterSpacing: '0.08em', marginBottom: 3, opacity: 0.75 }}>
                        {m.author} · {m.ts}
                      </div>
                      {m.text}
                    </div>
                  </div>
                )
              })}
            </div>
            <div style={{ display: 'flex', gap: 8, padding: '12px 16px', borderTop: '1px solid var(--line)', background: 'var(--card)' }}>
              <input
                ref={chatRef}
                onKeyDown={(e) => {
                  if (e.key === 'Enter') void sendChat()
                }}
                placeholder="fale com a squad — direcione, pergunte, mude o rumo…"
                style={{ flex: 1, border: '1px solid var(--line2)', borderRadius: 10, padding: '10px 14px', fontSize: 13, background: 'var(--white)', color: 'var(--ink)', minWidth: 0 }}
              />
              <button
                onClick={() => void sendChat()}
                style={{ background: 'var(--brand)', color: '#fff', border: 'none', borderRadius: 10, padding: '0 18px', fontSize: 13, fontWeight: 600, fontFamily: 'var(--sans)' }}
              >
                enviar ↑
              </button>
            </div>
          </div>
        </div>

        {/* ── feed ── */}
        <div
          data-testid="feed"
          style={{ background: 'var(--card)', border: '1px solid var(--line)', borderRadius: 16, padding: '18px 20px', display: 'flex', flexDirection: 'column', gap: 2, maxHeight: 420, overflowY: 'auto' }}
        >
          <div className="kicker" style={{ fontSize: 10, color: 'var(--faint)', marginBottom: 10 }}>
            atividade
          </div>
          {feed.length === 0 && (
            <span className="mono" style={{ fontSize: 10.5, color: 'var(--faint)' }}>
              {filaHint
                ? 'sua squad está na fila — outra squad está trabalhando agora; esta começa sozinha assim que a anterior terminar.'
                : 'aguardando os primeiros eventos do orquestrador…'}
            </span>
          )}
          {feed.map((f, i) => (
            <div key={i} style={{ display: 'flex', gap: 10, padding: '7px 0', borderBottom: '1px dashed var(--line)', alignItems: 'baseline' }}>
              <span className="mono" style={{ fontSize: 10, color: 'var(--faint)', flex: 'none', width: 38 }}>{f.ts}</span>
              <span style={{ fontSize: 12.5, lineHeight: 1.5 }}>{f.txt}</span>
            </div>
          ))}
        </div>
      </div>
    </>
  )
}
