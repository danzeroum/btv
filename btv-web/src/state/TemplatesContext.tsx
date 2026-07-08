import { createContext, useContext, useEffect, useMemo, useState, type ReactNode } from 'react'
import { fetchTemplates, type SquadTemplate } from '../api/templates'

/** Os 12 modelos vêm do backend real (`GET /api/btv/templates`, embutidos no
 *  binário a partir de `schemas/squad-templates/`) — carregados uma vez e
 *  compartilhados entre galeria (U1), wizard (U2), personas (U7) e admin (A5).
 *  Nada de catálogo duplicado no cliente. */
export type TemplatesState =
  | { status: 'loading' }
  | { status: 'error'; error: Error }
  | { status: 'ready'; templates: SquadTemplate[]; byId: Map<string, SquadTemplate> }

const TemplatesContext = createContext<TemplatesState | null>(null)

export function TemplatesProvider({ children }: { children: ReactNode }) {
  const [raw, setRaw] = useState<{ templates?: SquadTemplate[]; error?: Error }>({})

  useEffect(() => {
    let cancelled = false
    fetchTemplates()
      .then((templates) => {
        if (!cancelled) setRaw({ templates })
      })
      .catch((error: Error) => {
        if (!cancelled) setRaw({ error })
      })
    return () => {
      cancelled = true
    }
  }, [])

  const value: TemplatesState = useMemo(() => {
    if (raw.templates) {
      return {
        status: 'ready',
        templates: raw.templates,
        byId: new Map(raw.templates.map((t) => [t.id, t])),
      }
    }
    if (raw.error) return { status: 'error', error: raw.error }
    return { status: 'loading' }
  }, [raw])

  return <TemplatesContext.Provider value={value}>{children}</TemplatesContext.Provider>
}

export function useTemplates(): TemplatesState {
  const ctx = useContext(TemplatesContext)
  if (!ctx) throw new Error('useTemplates deve ser usado dentro de <TemplatesProvider>')
  return ctx
}
