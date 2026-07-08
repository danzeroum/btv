import type { CSSProperties } from 'react'
import { useAppDispatch, useAppState } from '../../state/AppContext'

const tabBase: CSSProperties = {
  border: 'none',
  borderRadius: 8,
  padding: '7px 15px',
  fontSize: 12.5,
  fontWeight: 600,
  fontFamily: 'var(--sans)',
  whiteSpace: 'nowrap',
}

export function Topbar({ onToggleGear }: { onToggleGear: () => void }) {
  const { persona, squad } = useAppState()
  const dispatch = useAppDispatch()

  const tabStyle = (active: boolean): CSSProperties => ({
    ...tabBase,
    background: active ? 'var(--brand)' : 'transparent',
    color: active ? '#fff' : 'var(--muted)',
  })

  return (
    <header
      style={{
        display: 'flex',
        alignItems: 'center',
        gap: 22,
        padding: '12px 22px',
        borderBottom: '1px solid var(--line2)',
        background: 'var(--card)',
        flex: 'none',
        zIndex: 30,
      }}
    >
      <div style={{ display: 'flex', alignItems: 'center', gap: 10 }}>
        <div
          style={{
            width: 30,
            height: 30,
            borderRadius: 9,
            background: 'var(--brand)',
            color: 'var(--gold)',
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            fontFamily: 'var(--disp)',
            fontWeight: 800,
            fontSize: 16,
          }}
        >
          B
        </div>
        <div style={{ display: 'flex', flexDirection: 'column', gap: 1 }}>
          <span style={{ fontFamily: 'var(--disp)', fontWeight: 700, fontSize: 16, letterSpacing: '-0.01em', lineHeight: 1 }}>
            BuildToValue
          </span>
          <span
            className="mono"
            style={{ fontSize: 9.5, letterSpacing: '0.14em', textTransform: 'uppercase', color: 'var(--faint)', whiteSpace: 'nowrap' }}
          >
            squads de IA para cada ofício
          </span>
        </div>
      </div>

      <div
        style={{
          display: 'flex',
          flex: 'none',
          background: 'var(--paper)',
          border: '1px solid var(--line2)',
          borderRadius: 10,
          padding: 3,
          gap: 2,
        }}
      >
        <button onClick={() => dispatch({ type: 'SET_PERSONA', persona: 'user' })} style={tabStyle(persona === 'user')}>
          Meu espaço
        </button>
        <button onClick={() => dispatch({ type: 'SET_PERSONA', persona: 'admin' })} style={tabStyle(persona === 'admin')}>
          Administração
        </button>
      </div>

      <div
        className="mono"
        style={{
          marginLeft: 'auto',
          display: 'flex',
          alignItems: 'center',
          gap: 18,
          fontSize: 11,
          color: 'var(--muted)',
          minWidth: 0,
          overflow: 'hidden',
        }}
      >
        {squad && persona === 'user' && (
          <span
            data-testid="squad-chip"
            style={{
              display: 'flex',
              alignItems: 'center',
              gap: 8,
              background: 'var(--white)',
              border: '1px solid var(--line)',
              borderRadius: 999,
              padding: '5px 14px',
              whiteSpace: 'nowrap',
            }}
          >
            <span
              style={{
                width: 8,
                height: 8,
                borderRadius: '50%',
                background: squad.cor,
                animation: 'btvPulse 2s infinite',
              }}
            />
            <span style={{ color: 'var(--ink)', fontWeight: 500 }}>{squad.nome}</span>
            <span style={{ color: 'var(--faint)' }}>· {squad.status}</span>
          </span>
        )}
        <span style={{ display: 'flex', alignItems: 'center', gap: 7, whiteSpace: 'nowrap' }}>
          <span style={{ width: 7, height: 7, borderRadius: '50%', background: 'var(--ok)' }} />
          local · 127.0.0.1
        </span>
      </div>

      <button
        onClick={onToggleGear}
        title="Ajustes rápidos"
        aria-label="Ajustes rápidos"
        style={{
          flex: 'none',
          width: 34,
          height: 34,
          borderRadius: 10,
          border: '1px solid var(--line2)',
          background: 'var(--white)',
          color: 'var(--muted)',
          fontSize: 15,
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'center',
        }}
      >
        ⚙
      </button>
    </header>
  )
}
