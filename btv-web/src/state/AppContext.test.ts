import { describe, expect, it } from 'vitest'
import { appInitState, appReducer, type AppState } from './AppContext'

const base: AppState = { ...appInitState(), accent: null }

const squad = { nome: 'Editorial / SEO', cor: '#b8531f', status: 'em produção' as const, gateAberto: false }

describe('reducer do shell (comportamento do protótipo)', () => {
  it('voltar a Meu espaço com squad rodando cai em Ao vivo', () => {
    const comSquad = { ...base, persona: 'admin' as const, screen: 'telemetria' as const, squad }
    const next = appReducer(comSquad, { type: 'SET_PERSONA', persona: 'user' })
    expect(next.screen).toBe('vivo')
  })

  it('voltar a Meu espaço sem squad cai na galeria', () => {
    const semSquad = { ...base, persona: 'admin' as const, screen: 'telemetria' as const, squad: null }
    const next = appReducer(semSquad, { type: 'SET_PERSONA', persona: 'user' })
    expect(next.screen).toBe('inicio')
  })

  it('trocar para Administração numa tela de usuário abre Telemetria', () => {
    const next = appReducer({ ...base, screen: 'biblioteca' }, { type: 'SET_PERSONA', persona: 'admin' })
    expect(next.screen).toBe('telemetria')
  })

  it('encerrar a squad estando em Ao vivo devolve à galeria', () => {
    const vivo = { ...base, screen: 'vivo' as const, squad }
    const next = appReducer(vivo, { type: 'SET_SQUAD', squad: null })
    expect(next.screen).toBe('inicio')
    expect(next.squad).toBeNull()
  })

  it('encerrar a squad em outra tela não navega', () => {
    const biblioteca = { ...base, screen: 'biblioteca' as const, squad }
    const next = appReducer(biblioteca, { type: 'SET_SQUAD', squad: null })
    expect(next.screen).toBe('biblioteca')
  })
})
