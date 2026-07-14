import { render, screen, fireEvent } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import { AsyncStatus } from './AsyncStatus'
import { ConfirmModal } from './Modal'
import type { AsyncState } from '../../hooks/useAsyncAction'

describe('AsyncStatus', () => {
  it('renderiza carregando no estado loading', () => {
    render(
      <AsyncStatus state={{ status: 'loading' }}>{() => <div>ok</div>}</AsyncStatus>,
    )
    expect(screen.getByText('carregando…')).toBeDefined()
  })

  it('mostra o erro com prefixo amigável e detalhe técnico + botão de retry', () => {
    const onRetry = vi.fn()
    const state: AsyncState<never> = { status: 'error', error: new Error('boom') }
    render(
      <AsyncStatus state={state} onRetry={onRetry} erroPrefixo="Não consegui carregar">
        {() => <div>nunca</div>}
      </AsyncStatus>,
    )
    expect(screen.getByText(/Não consegui carregar/)).toBeDefined()
    expect(screen.getByText(/boom/)).toBeDefined()
    fireEvent.click(screen.getByText('tentar de novo'))
    expect(onRetry).toHaveBeenCalledOnce()
  })

  it('renderiza os dados no estado success', () => {
    render(
      <AsyncStatus state={{ status: 'success', data: 42 }}>{(n) => <div>valor {n}</div>}</AsyncStatus>,
    )
    expect(screen.getByText('valor 42')).toBeDefined()
  })
})

describe('ConfirmModal', () => {
  it('não renderiza quando fechado', () => {
    render(
      <ConfirmModal aberto={false} mensagem="remover?" onConfirmar={() => {}} onCancelar={() => {}} />,
    )
    expect(screen.queryByText('remover?')).toBeNull()
  })

  it('dispara onConfirmar/onCancelar nos botões', () => {
    const onConfirmar = vi.fn()
    const onCancelar = vi.fn()
    render(
      <ConfirmModal
        aberto
        titulo="Remover perfil"
        mensagem="remover mesmo?"
        confirmarLabel="Remover"
        onConfirmar={onConfirmar}
        onCancelar={onCancelar}
      />,
    )
    fireEvent.click(screen.getByText('Remover'))
    expect(onConfirmar).toHaveBeenCalledOnce()
    fireEvent.click(screen.getByText('Cancelar'))
    expect(onCancelar).toHaveBeenCalledOnce()
  })
})
