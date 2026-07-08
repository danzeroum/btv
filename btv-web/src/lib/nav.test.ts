import { describe, expect, it } from 'vitest'
import { ADMIN_NAV, DEFAULT_SCREEN, USER_NAV, screenBelongsToPersona } from './nav'

describe('navegação contextual (handoff §5)', () => {
  it('respeita o teto de 5–6 itens por perfil', () => {
    expect(USER_NAV.length).toBeLessThanOrEqual(6)
    expect(ADMIN_NAV.length).toBeLessThanOrEqual(6)
  })

  it('perfil usuário abre na galeria; admin em telemetria', () => {
    expect(DEFAULT_SCREEN.user).toBe('inicio')
    expect(DEFAULT_SCREEN.admin).toBe('telemetria')
  })

  it('vivo (U3) pertence ao perfil usuário mesmo sem item no menu GERAL', () => {
    expect(USER_NAV.some((n) => n.id === 'vivo')).toBe(false)
    expect(screenBelongsToPersona('user', 'vivo')).toBe(true)
    expect(screenBelongsToPersona('admin', 'vivo')).toBe(false)
  })

  it('telas de admin não pertencem ao perfil usuário e vice-versa', () => {
    expect(screenBelongsToPersona('user', 'ledger')).toBe(false)
    expect(screenBelongsToPersona('admin', 'ledger')).toBe(true)
    expect(screenBelongsToPersona('admin', 'inicio')).toBe(false)
  })
})
