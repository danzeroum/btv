import type { CSSProperties } from 'react'
import { useAppDispatch, useAppState } from '../../state/AppContext'
import { NAV_BY_PERSONA } from '../../lib/nav'
import type { ScreenId } from '../../types/domain'

function itemStyle(active: boolean): CSSProperties {
  return {
    display: 'flex',
    alignItems: 'center',
    gap: 10,
    width: '100%',
    padding: '9px 10px',
    borderRadius: 10,
    border: 'none',
    fontFamily: 'var(--sans)',
    color: active ? 'var(--ink)' : 'var(--muted)',
    background: active ? '#ffffff' : 'transparent',
    boxShadow: active ? '0 1px 4px #22140a14' : undefined,
    outline: active ? '1px solid var(--line)' : undefined,
    textAlign: 'left',
  }
}

export function Sidebar() {
  const { persona, screen, squad } = useAppState()
  const dispatch = useAppDispatch()
  const nav = NAV_BY_PERSONA[persona]
  const go = (id: ScreenId) => dispatch({ type: 'SET_SCREEN', screen: id })

  const showSquadSection = persona === 'user' && squad !== null

  return (
    <nav
      style={{
        width: 236,
        flex: 'none',
        borderRight: '1px solid var(--line2)',
        background: 'var(--card)',
        padding: '18px 12px 14px',
        display: 'flex',
        flexDirection: 'column',
        gap: 3,
        overflowY: 'auto',
      }}
    >
      {showSquadSection && squad && (
        <>
          <div className="kicker" style={{ fontSize: 9.5, letterSpacing: '0.16em', color: squad.cor, padding: '2px 10px 8px' }}>
            squad ativa
          </div>
          <button onClick={() => go('vivo')} style={itemStyle(screen === 'vivo')}>
            <span className="mono" style={{ fontSize: 13, width: 20, textAlign: 'center', flex: 'none', color: squad.cor }}>
              ●
            </span>
            <span style={{ fontSize: 13.5, fontWeight: 500 }}>Ao vivo</span>
            {squad.gateAberto && (
              <span
                className="mono"
                style={{
                  marginLeft: 'auto',
                  background: 'var(--decision)',
                  color: 'var(--card)',
                  fontSize: 9.5,
                  borderRadius: 999,
                  padding: '2px 7px',
                }}
              >
                1 gate
              </span>
            )}
          </button>
          <button onClick={() => go('biblioteca')} style={itemStyle(false)}>
            <span className="mono" style={{ fontSize: 13, width: 20, textAlign: 'center', flex: 'none', color: 'var(--faint)' }}>
              ▤
            </span>
            <span style={{ fontSize: 13.5, fontWeight: 500 }}>Entregas</span>
          </button>
          <div style={{ height: 1, background: 'var(--line)', margin: '12px 6px' }} />
        </>
      )}

      <div className="kicker" style={{ fontSize: 9.5, letterSpacing: '0.16em', color: 'var(--faint)', padding: '2px 10px 8px' }}>
        {persona === 'user' ? 'geral' : 'administração'}
      </div>
      {nav.map((item) => {
        const active = screen === item.id
        return (
          <button key={item.id} onClick={() => go(item.id)} style={itemStyle(active)}>
            <span
              className="mono"
              style={{ fontSize: 13, width: 20, textAlign: 'center', flex: 'none', color: active ? 'var(--brand)' : 'var(--faint)' }}
            >
              {item.icon}
            </span>
            <span style={{ display: 'flex', flexDirection: 'column', gap: 2, textAlign: 'left', minWidth: 0 }}>
              <span style={{ fontSize: 13.5, fontWeight: 500 }}>{item.label}</span>
              <span
                className="mono"
                style={{ fontSize: 10.5, color: 'var(--faint)', whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }}
              >
                {item.hint}
              </span>
            </span>
          </button>
        )
      })}

      {/* Rodapé: perfil local. Nome/papel viram dados reais quando o store de
          perfis locais (A6, Onda 6) entrar — por ora é o placeholder do handoff. */}
      <div
        style={{
          marginTop: 'auto',
          borderTop: '1px solid var(--line)',
          padding: '12px 10px 2px',
          display: 'flex',
          alignItems: 'center',
          gap: 10,
        }}
      >
        <div
          style={{
            width: 28,
            height: 28,
            borderRadius: '50%',
            background: 'var(--brand)',
            color: '#fff',
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            fontFamily: 'var(--disp)',
            fontWeight: 700,
            fontSize: 12,
          }}
        >
          M
        </div>
        <div style={{ display: 'flex', flexDirection: 'column', gap: 1 }}>
          <span style={{ fontSize: 12.5, fontWeight: 500 }}>Marina L.</span>
          <span className="mono" style={{ fontSize: 10, color: 'var(--faint)' }}>
            {persona === 'user' ? 'perfil usuário' : 'perfil admin'}
          </span>
        </div>
      </div>
    </nav>
  )
}
