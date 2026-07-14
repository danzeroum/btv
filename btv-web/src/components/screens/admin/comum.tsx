import type { CSSProperties, ReactNode } from 'react'

/** Peças compartilhadas das telas de Administração (A1–A6), nos tokens do
 *  handoff (§9). */

export function StatCard({ k, v, delta, deltaCor }: { k: string; v: string; delta?: string; deltaCor?: string }) {
  return (
    <div style={{ background: 'var(--white)', border: '1px solid var(--line)', borderRadius: 13, padding: '18px 20px' }}>
      <div className="kicker" style={{ fontSize: 10, letterSpacing: '0.12em', color: 'var(--faint)' }}>{k}</div>
      <div style={{ fontFamily: 'var(--disp)', fontWeight: 800, fontSize: 26, marginTop: 7, letterSpacing: '-0.02em' }}>{v}</div>
      {delta && (
        <div className="mono" style={{ fontSize: 10.5, color: deltaCor ?? 'var(--faint)', marginTop: 3 }}>{delta}</div>
      )}
    </div>
  )
}

export function Pill({ tone, children }: { tone: 'ok' | 'warn' | 'muted' | 'erro'; children: ReactNode }) {
  const css: Record<string, CSSProperties> = {
    ok: { background: 'var(--ok-bg)', color: 'var(--ok-ink)' },
    warn: { background: 'var(--warn-bg)', color: 'var(--warn-ink)' },
    muted: { background: 'var(--paper)', color: 'var(--muted)' },
    erro: { background: 'var(--err-bg)', color: 'var(--err-ink)' },
  }
  return (
    <span className="mono" style={{ fontSize: 10, letterSpacing: '0.08em', borderRadius: 999, padding: '5px 12px', textAlign: 'center', ...css[tone] }}>
      {children}
    </span>
  )
}

export function Toggle({ on, onClick, label }: { on: boolean; onClick: () => void; label: string }) {
  return (
    <button
      onClick={onClick}
      aria-label={label}
      aria-pressed={on}
      style={{ width: 40, height: 22, borderRadius: 99, border: 'none', position: 'relative', background: on ? 'var(--brand)' : 'var(--line2)', flex: 'none' }}
    >
      <span style={{ position: 'absolute', top: 3, width: 16, height: 16, borderRadius: '50%', background: '#fff', transition: 'left .15s', left: on ? 21 : 3 }} />
    </button>
  )
}

export function ErroBox({ msg }: { msg: string }) {
  return (
    <div style={{ background: 'var(--err-bg)', border: '1px solid var(--err-line)', borderRadius: 12, padding: '14px 18px', color: 'var(--err-ink)', fontSize: 13 }}>
      {msg}
    </div>
  )
}

export function NotaHonesta({ children }: { children: ReactNode }) {
  return (
    <div style={{ background: 'var(--paper)', border: '1px solid var(--line)', borderRadius: 10, padding: '11px 16px', fontSize: 11.5, color: 'var(--muted)', lineHeight: 1.6 }}>
      {children}
    </div>
  )
}
