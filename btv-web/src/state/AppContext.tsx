import { createContext, useContext, useReducer, type Dispatch, type ReactNode } from 'react'
import type { ActiveSquadInfo, Persona, ScreenId } from '../types/domain'
import { DEFAULT_SCREEN, screenBelongsToPersona } from '../lib/nav'

export const ACCENT_STORAGE_KEY = 'btv_accent'

/** Swatches de marca dos Ajustes rápidos (handoff §4 — `--brand` tweakável). */
export const BRAND_SWATCHES = ['#14614f', '#8a3b2a', '#2b4a8c', '#5b3f8c', '#9a6b14'] as const

export interface AppState {
  persona: Persona
  screen: ScreenId
  /** Sobreposição de `--brand` escolhida no ⚙; null = padrão do tema. */
  accent: string | null
  /** Recorte da squad ativa que o shell exibe; null = nenhuma squad rodando.
   *  O estado completo da execução (esteira, feed, chat) chega na Onda 3. */
  squad: ActiveSquadInfo | null
}

export type AppAction =
  | { type: 'SET_PERSONA'; persona: Persona }
  | { type: 'SET_SCREEN'; screen: ScreenId }
  | { type: 'SET_ACCENT'; accent: string | null }
  | { type: 'SET_SQUAD'; squad: ActiveSquadInfo | null }

function readPersistedAccent(): string | null {
  try {
    return localStorage.getItem(ACCENT_STORAGE_KEY) || null
  } catch {
    // localStorage indisponível (modo privado/sandbox) — degrada para o padrão.
    return null
  }
}

function initState(): AppState {
  return { persona: 'user', screen: DEFAULT_SCREEN.user, accent: readPersistedAccent(), squad: null }
}

function reducer(state: AppState, action: AppAction): AppState {
  switch (action.type) {
    case 'SET_PERSONA': {
      // Comportamento do protótipo: voltar a "Meu espaço" com squad rodando
      // cai direto em Ao vivo; sem squad, na galeria. Admin abre em Telemetria.
      const screen =
        action.persona === 'user'
          ? state.squad
            ? 'vivo'
            : screenBelongsToPersona('user', state.screen)
              ? state.screen
              : DEFAULT_SCREEN.user
          : screenBelongsToPersona('admin', state.screen)
            ? state.screen
            : DEFAULT_SCREEN.admin
      return { ...state, persona: action.persona, screen }
    }
    case 'SET_SCREEN':
      return { ...state, screen: action.screen }
    case 'SET_ACCENT':
      try {
        if (action.accent) localStorage.setItem(ACCENT_STORAGE_KEY, action.accent)
        else localStorage.removeItem(ACCENT_STORAGE_KEY)
      } catch {
        // ok ignorar — ver readPersistedAccent
      }
      return { ...state, accent: action.accent }
    case 'SET_SQUAD': {
      // Encerrar a squad estando em Ao vivo devolve à galeria (o protótipo faz
      // o mesmo em `encerrarSquad`).
      const screen = !action.squad && state.screen === 'vivo' ? DEFAULT_SCREEN.user : state.screen
      return { ...state, squad: action.squad, screen }
    }
  }
}

const AppStateContext = createContext<AppState | null>(null)
const AppDispatchContext = createContext<Dispatch<AppAction> | null>(null)

export function AppProvider({ children }: { children: ReactNode }) {
  const [state, dispatch] = useReducer(reducer, undefined, initState)
  return (
    <AppStateContext.Provider value={state}>
      <AppDispatchContext.Provider value={dispatch}>{children}</AppDispatchContext.Provider>
    </AppStateContext.Provider>
  )
}

export function useAppState(): AppState {
  const ctx = useContext(AppStateContext)
  if (!ctx) throw new Error('useAppState deve ser usado dentro de <AppProvider>')
  return ctx
}

export function useAppDispatch(): Dispatch<AppAction> {
  const ctx = useContext(AppDispatchContext)
  if (!ctx) throw new Error('useAppDispatch deve ser usado dentro de <AppProvider>')
  return ctx
}

export { reducer as appReducer, initState as appInitState }
