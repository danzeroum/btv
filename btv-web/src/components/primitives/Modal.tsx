import type { ReactNode } from 'react'

/** Diálogo modal do produto — overlay + cartão centralizado. Usado para
 *  confirmações destrutivas (substitui `window.confirm`) e conteúdo curto.
 *  Sem terracota: `danger` usa a cor de erro (--err), não a de decisão. */
export function Modal({
  aberto,
  titulo,
  children,
  onFechar,
}: {
  aberto: boolean
  titulo?: string
  children: ReactNode
  onFechar: () => void
}) {
  if (!aberto) return null
  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-label={titulo}
      onClick={onFechar}
      style={{
        position: 'fixed',
        inset: 0,
        background: 'rgba(43, 43, 40, 0.4)',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        zIndex: 70,
        padding: 20,
      }}
    >
      <div
        onClick={(e) => e.stopPropagation()}
        style={{
          background: 'var(--card)',
          border: '1px solid var(--line2)',
          borderRadius: 14,
          padding: '22px 24px',
          maxWidth: 420,
          width: '100%',
          boxShadow: '0 16px 44px rgba(43, 43, 40, 0.24)',
        }}
      >
        {titulo && (
          <h3 style={{ margin: '0 0 12px', fontFamily: 'var(--disp)', fontSize: 18, color: 'var(--ink)' }}>
            {titulo}
          </h3>
        )}
        {children}
      </div>
    </div>
  )
}

/** Confirmação destrutiva pronta: mensagem + cancelar/confirmar. */
export function ConfirmModal({
  aberto,
  titulo,
  mensagem,
  confirmarLabel = 'Confirmar',
  onConfirmar,
  onCancelar,
}: {
  aberto: boolean
  titulo?: string
  mensagem: string
  confirmarLabel?: string
  onConfirmar: () => void
  onCancelar: () => void
}) {
  return (
    <Modal aberto={aberto} titulo={titulo} onFechar={onCancelar}>
      <p style={{ margin: '0 0 18px', fontSize: 13.5, color: 'var(--muted)', lineHeight: 1.55 }}>{mensagem}</p>
      <div style={{ display: 'flex', gap: 10, justifyContent: 'flex-end' }}>
        <button
          onClick={onCancelar}
          style={{
            background: 'none',
            border: '1px solid var(--line2)',
            borderRadius: 9,
            padding: '9px 16px',
            fontSize: 13,
            color: 'var(--muted)',
          }}
        >
          Cancelar
        </button>
        <button
          onClick={onConfirmar}
          style={{
            background: 'var(--err)',
            color: 'var(--card)',
            border: 'none',
            borderRadius: 9,
            padding: '9px 16px',
            fontSize: 13,
            fontWeight: 600,
          }}
        >
          {confirmarLabel}
        </button>
      </div>
    </Modal>
  )
}
