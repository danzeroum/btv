import { createContext, useCallback, useContext, useRef, useState, type ReactNode } from 'react'

/** Feedback "fire and forget" da UI do produto. Portado do console `web/`
 *  (primitives/Toast) e adaptado aos tokens do btv-web (--card/--ok/--warn/
 *  --err). Substitui os `window.alert` — nada de diálogo nativo do browser. */

type ToastKind = 'success' | 'error' | 'warn'

interface ToastItem {
  id: number
  kind: ToastKind
  message: string
}

interface ToastContextValue {
  push: (kind: ToastKind, message: string) => void
}

const ToastContext = createContext<ToastContextValue | null>(null)

const LINHA: Record<ToastKind, string> = {
  success: 'var(--ok)',
  error: 'var(--err)',
  warn: 'var(--warn)',
}
const MARCA: Record<ToastKind, string> = { success: '✓ ', error: '✗ ', warn: '⚠ ' }

export function ToastProvider({ children }: { children: ReactNode }) {
  const [items, setItems] = useState<ToastItem[]>([])
  const idRef = useRef(0)

  const push = useCallback((kind: ToastKind, message: string) => {
    const id = ++idRef.current
    setItems((prev) => [...prev, { id, kind, message }])
    setTimeout(() => {
      setItems((prev) => prev.filter((i) => i.id !== id))
    }, 4500)
  }, [])

  return (
    <ToastContext.Provider value={{ push }}>
      {children}
      <div
        style={{
          position: 'fixed',
          bottom: 18,
          right: 18,
          display: 'flex',
          flexDirection: 'column',
          gap: 8,
          zIndex: 60,
        }}
      >
        {items.map((item) => (
          <div
            key={item.id}
            role="status"
            style={{
              background: 'var(--card)',
              border: `1px solid ${LINHA[item.kind]}`,
              color: 'var(--ink)',
              borderRadius: 10,
              padding: '10px 15px',
              fontSize: 13,
              maxWidth: 340,
              boxShadow: '0 6px 20px rgba(43, 43, 40, 0.12)',
            }}
          >
            {MARCA[item.kind]}
            {item.message}
          </div>
        ))}
      </div>
    </ToastContext.Provider>
  )
}

export function useToast(): ToastContextValue {
  const ctx = useContext(ToastContext)
  if (!ctx) throw new Error('useToast deve ser usado dentro de <ToastProvider>')
  return ctx
}
