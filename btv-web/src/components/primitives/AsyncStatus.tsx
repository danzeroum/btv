import type { ReactNode } from 'react'
import type { AsyncState } from '../../hooks/useAsyncAction'

/** Wrapper padrão idle → loading → success | error para o btv-web. Portado do
 *  console `web/` (primitives/AsyncStatus) e adaptado aos tokens do produto.
 *  Nenhuma ação assíncrona de tela deve renderizar seus estados à mão: passe
 *  por aqui (ou pelo Toast, para "fire and forget"). Assim loading/erro/vazio
 *  ficam consistentes em todas as telas. */
export function AsyncStatus<T>({
  state,
  onRetry,
  children,
  idleFallback,
  erroPrefixo,
}: {
  state: AsyncState<T>
  onRetry?: () => void
  children: (data: T) => ReactNode
  idleFallback?: ReactNode
  /** Texto amigável antes do detalhe técnico do erro (ex.: "Não consegui
   *  carregar as squads"). O detalhe (`error.message`) entra entre parênteses. */
  erroPrefixo?: string
}) {
  switch (state.status) {
    case 'idle':
      return <>{idleFallback ?? null}</>
    case 'loading':
      return (
        <div className="mono" style={{ color: 'var(--faint)', fontSize: 11.5 }}>
          carregando…
        </div>
      )
    case 'error':
      return (
        <div
          style={{
            background: 'var(--err-bg)',
            border: '1px solid var(--err-line)',
            borderRadius: 12,
            padding: '16px 20px',
            color: 'var(--err-ink)',
            fontSize: 13,
            display: 'flex',
            alignItems: 'center',
            gap: 12,
          }}
        >
          <span>
            {erroPrefixo ? `${erroPrefixo} ` : ''}
            ({state.error.message})
          </span>
          {onRetry && (
            <button
              onClick={onRetry}
              style={{
                marginLeft: 'auto',
                background: 'none',
                border: '1px solid var(--err-line)',
                borderRadius: 8,
                padding: '5px 12px',
                fontSize: 11.5,
                color: 'var(--err-ink)',
                fontFamily: 'var(--mono)',
              }}
            >
              tentar de novo
            </button>
          )}
        </div>
      )
    case 'success':
      return <>{children(state.data)}</>
  }
}
