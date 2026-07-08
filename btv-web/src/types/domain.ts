/** Tipos compartilhados do BuildToValue (btv-web). Espelhos de backend moram
 *  nos módulos de `api/` (mesma convenção do console Forge em `web/`). */

export type Persona = 'user' | 'admin'

/** Telas do produto (handoff §6/§9): U* no perfil usuário, A* no admin.
 *  `vivo` (U3) não tem item fixo no menu — entra pela seção SQUAD ATIVA. */
export type ScreenId =
  // perfil usuário
  | 'inicio' // U1 · galeria
  | 'vivo' // U3 · squad ao vivo
  | 'biblioteca' // U4
  | 'designer' // U5
  | 'minhas' // U6
  | 'personas' // U7
  // administração
  | 'telemetria' // A1
  | 'ledger' // A2
  | 'providers' // A3
  | 'permissoes' // A4
  | 'modelos' // A5
  | 'usuarios' // A6

export interface NavItem {
  id: ScreenId
  icon: string
  label: string
  hint: string
}

/** Resumo da squad ativa que o shell precisa exibir (chip da topbar, seção
 *  SQUAD ATIVA da sidebar). O estado completo da execução vive no contexto
 *  da squad (Onda 3); o shell só conhece este recorte. */
export interface ActiveSquadInfo {
  nome: string
  /** Cor de identidade do modelo (handoff §4 — cores por squad). */
  cor: string
  status: 'em produção' | 'aguardando você' | 'concluída'
  gateAberto: boolean
}
