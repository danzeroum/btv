import { fetchJson } from './client'

/** Espelho de `btv_schemas::squad_template::SquadTemplate`
 *  (`squad-template.v1`) — servido por `GET /api/btv/templates` a partir dos
 *  12 JSONs embutidos de `schemas/squad-templates/`. */
export type CategoriaSquad = 'conteudo' | 'analise' | 'criativa' | 'operacoes'

export interface FormatoEntrega {
  nome: string
  /** true = exportação direta ainda indisponível (exige conversor na sandbox
   *  — onda futura). A UI desabilita, nunca finge exportar. */
  binario: boolean
}

export interface PerguntaBriefing {
  label: string
  placeholder: string
}

export interface SquadTemplate {
  id: string
  nome: string
  categoria: CategoriaSquad
  cor: string
  onda: 1 | 2 | 3
  versao: string
  publicado: boolean
  descricao: string
  papeis: string[]
  formatos: FormatoEntrega[]
  perguntas: PerguntaBriefing[]
  gates: string[]
}

export function fetchTemplates(): Promise<SquadTemplate[]> {
  return fetchJson<SquadTemplate[]>('/api/btv/templates')
}
