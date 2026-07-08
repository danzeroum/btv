import type { ComponentType } from 'react'
import type { ScreenId } from '../types/domain'
import { Inicio } from '../components/screens/user/Inicio'
import { Vivo } from '../components/screens/user/Vivo'
import { Biblioteca } from '../components/screens/user/Biblioteca'
import { Minhas } from '../components/screens/user/Minhas'
import { Personas } from '../components/screens/user/Personas'

/** Placeholder honesto de tela ainda não construída — substituído onda a onda
 *  (ordem da seção 16 do handoff). Nunca simula dado ou comportamento. */
function EmConstrucao({ onda }: { onda: string }) {
  return (
    <div
      style={{
        background: 'var(--white)',
        border: '1px dashed var(--line2)',
        borderRadius: 14,
        padding: '28px 30px',
        color: 'var(--muted)',
        fontSize: 13.5,
        lineHeight: 1.6,
      }}
    >
      Esta tela chega na <strong>{onda}</strong> da implementação — o shell, a navegação e os ajustes
      rápidos já são reais.
    </div>
  )
}

const Designer = () => <EmConstrucao onda="Onda 5 (Squad Designer sobre bpmn-react)" />
const Telemetria = () => <EmConstrucao onda="Onda 6 (Admin)" />
const Ledger = () => <EmConstrucao onda="Onda 6 (Admin)" />
const Providers = () => <EmConstrucao onda="Onda 6 (Admin)" />
const Permissoes = () => <EmConstrucao onda="Onda 6 (Admin)" />
const Modelos = () => <EmConstrucao onda="Onda 6 (Admin)" />
const Usuarios = () => <EmConstrucao onda="Onda 6 (Admin)" />

export const SCREEN_COMPONENTS: Record<ScreenId, ComponentType> = {
  inicio: Inicio,
  vivo: Vivo,
  biblioteca: Biblioteca,
  designer: Designer,
  minhas: Minhas,
  personas: Personas,
  telemetria: Telemetria,
  ledger: Ledger,
  providers: Providers,
  permissoes: Permissoes,
  modelos: Modelos,
  usuarios: Usuarios,
}
