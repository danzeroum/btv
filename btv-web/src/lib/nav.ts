import type { NavItem, Persona, ScreenId } from '../types/domain'

/** Navegação contextual (handoff §5): máx. 5–6 itens visíveis por perfil.
 *  Ícones e hints verbatim do protótipo (classe Component, renderVals). */
export const USER_NAV: NavItem[] = [
  { id: 'inicio', icon: '⌂', label: 'Início', hint: 'galeria de modelos' },
  { id: 'minhas', icon: '≣', label: 'Minhas squads', hint: 'ativas e histórico' },
  { id: 'personas', icon: '☺', label: 'Personas', hint: 'papéis e prompts' },
  { id: 'biblioteca', icon: '▤', label: 'Biblioteca', hint: 'todas as entregas' },
  { id: 'designer', icon: '✎', label: 'Designer', hint: 'monte sua squad' },
]

export const ADMIN_NAV: NavItem[] = [
  { id: 'telemetria', icon: '◔', label: 'Telemetria', hint: 'custo e uso' },
  { id: 'ledger', icon: '≡', label: 'Ledger', hint: 'trilha auditável' },
  { id: 'providers', icon: '⇄', label: 'Providers', hint: 'modelos e limites' },
  { id: 'permissoes', icon: '⚿', label: 'Permissões', hint: 'skills, tools, MCP' },
  { id: 'modelos', icon: '❖', label: 'Modelos', hint: 'templates da galeria' },
  { id: 'usuarios', icon: '☷', label: 'Usuários', hint: 'acessos e papéis' },
]

export const NAV_BY_PERSONA: Record<Persona, NavItem[]> = {
  user: USER_NAV,
  admin: ADMIN_NAV,
}

export const DEFAULT_SCREEN: Record<Persona, ScreenId> = {
  user: 'inicio',
  admin: 'telemetria',
}

/** `vivo` (U3) pertence ao perfil usuário mas não ocupa item do menu GERAL —
 *  aparece na seção SQUAD ATIVA quando existe squad rodando (handoff §5). */
export function screenBelongsToPersona(persona: Persona, screen: ScreenId): boolean {
  if (screen === 'vivo') return persona === 'user'
  return NAV_BY_PERSONA[persona].some((n) => n.id === screen)
}
