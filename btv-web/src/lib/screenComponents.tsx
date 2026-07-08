import type { ComponentType } from 'react'
import type { ScreenId } from '../types/domain'
import { Inicio } from '../components/screens/user/Inicio'
import { Vivo } from '../components/screens/user/Vivo'
import { Biblioteca } from '../components/screens/user/Biblioteca'
import { Minhas } from '../components/screens/user/Minhas'
import { Personas } from '../components/screens/user/Personas'
import { Designer } from '../components/screens/user/Designer'
import { Telemetria } from '../components/screens/admin/Telemetria'
import { Ledger } from '../components/screens/admin/Ledger'
import { Providers } from '../components/screens/admin/Providers'
import { Permissoes } from '../components/screens/admin/Permissoes'
import { Modelos } from '../components/screens/admin/Modelos'
import { Usuarios } from '../components/screens/admin/Usuarios'


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
