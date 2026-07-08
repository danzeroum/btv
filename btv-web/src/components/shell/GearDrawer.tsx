import { BRAND_SWATCHES, useAppDispatch, useAppState } from '../../state/AppContext'

/** Ajustes rápidos (engrenagem ⚙) — handoff §6 "Ajustes rápidos": drawer de
 *  320px à direita. Nesta onda entram só os ajustes que já são reais: marca
 *  (troca `--brand` na hora) e atalhos. "Ritmo da esteira" e "Aprovar
 *  rascunhos por mim" entram quando a esteira real existir (Onda 3) — e o
 *  auto-gate será política de backend (auto-approve de HITL registrado no
 *  ledger), nunca clique simulado no frontend. */
export function GearDrawer({ onClose }: { onClose: () => void }) {
  const { accent } = useAppState()
  const dispatch = useAppDispatch()
  const current = accent ?? BRAND_SWATCHES[0]

  const goTo = (screen: 'personas' | 'minhas') => {
    dispatch({ type: 'SET_PERSONA', persona: 'user' })
    dispatch({ type: 'SET_SCREEN', screen })
    onClose()
  }

  return (
    <>
      <div onClick={onClose} style={{ position: 'fixed', inset: 0, background: '#221d1526', zIndex: 55 }} />
      <aside
        data-testid="gear-drawer"
        style={{
          position: 'fixed',
          top: 0,
          right: 0,
          bottom: 0,
          width: 320,
          background: 'var(--card)',
          borderLeft: '1px solid var(--line2)',
          boxShadow: '-24px 0 60px -34px #22140a66',
          zIndex: 56,
          padding: 22,
          display: 'flex',
          flexDirection: 'column',
          gap: 24,
          overflowY: 'auto',
        }}
      >
        <div style={{ display: 'flex', alignItems: 'center', gap: 10 }}>
          <span style={{ fontSize: 15 }}>⚙</span>
          <span style={{ fontFamily: 'var(--disp)', fontWeight: 700, fontSize: 16 }}>Ajustes rápidos</span>
          <button
            onClick={onClose}
            aria-label="Fechar ajustes"
            style={{ marginLeft: 'auto', background: 'none', border: 'none', fontSize: 16, color: 'var(--faint)' }}
          >
            ✕
          </button>
        </div>

        <div style={{ display: 'flex', flexDirection: 'column', gap: 10 }}>
          <span className="kicker" style={{ fontSize: 10, color: 'var(--faint)' }}>
            marca
          </span>
          <div style={{ display: 'flex', gap: 8 }}>
            {BRAND_SWATCHES.map((c) => (
              <button
                key={c}
                aria-label={`Cor da marca ${c}`}
                onClick={() => dispatch({ type: 'SET_ACCENT', accent: c })}
                style={{
                  width: 26,
                  height: 26,
                  borderRadius: '50%',
                  background: c,
                  border: `2px solid ${current === c ? 'var(--ink)' : 'var(--card)'}`,
                  outline: '1px solid var(--line2)',
                }}
              />
            ))}
          </div>
        </div>

        <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
          <span className="kicker" style={{ fontSize: 10, color: 'var(--faint)' }}>
            atalhos
          </span>
          <button
            onClick={() => goTo('personas')}
            style={{
              display: 'flex',
              alignItems: 'center',
              gap: 9,
              background: 'var(--white)',
              border: '1px solid var(--line)',
              borderRadius: 10,
              padding: '10px 13px',
              fontSize: 12.5,
              fontWeight: 600,
              color: 'var(--ink)',
              fontFamily: 'var(--sans)',
              textAlign: 'left',
            }}
          >
            ☺ Personas &amp; prompts
          </button>
          <button
            onClick={() => goTo('minhas')}
            style={{
              display: 'flex',
              alignItems: 'center',
              gap: 9,
              background: 'var(--white)',
              border: '1px solid var(--line)',
              borderRadius: 10,
              padding: '10px 13px',
              fontSize: 12.5,
              fontWeight: 600,
              color: 'var(--ink)',
              fontFamily: 'var(--sans)',
              textAlign: 'left',
            }}
          >
            ≣ Minhas squads
          </button>
        </div>

        <div
          style={{
            marginTop: 'auto',
            fontSize: 11.5,
            color: 'var(--faint)',
            lineHeight: 1.6,
            borderTop: '1px solid var(--line)',
            paddingTop: 14,
          }}
        >
          Estes ajustes valem só para você e são aplicados na hora — nada aqui exige a administração.
        </div>
      </aside>
    </>
  )
}
