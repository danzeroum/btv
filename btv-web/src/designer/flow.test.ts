import { describe, expect, it } from 'vitest'
import { createEdge } from '@bpmn-react/core'
import { baseDoModelo, baseInicial, baseVazia } from './bases'
import { descricaoDoFluxo, etapasDoFluxo, ordemDoFluxo } from './flow'
import type { SquadTemplate } from '../api/templates'

const editorial: SquadTemplate = {
  id: 'editorial',
  nome: 'Editorial / SEO',
  categoria: 'conteudo',
  cor: '#b8531f',
  onda: 1,
  versao: 'v1.4',
  publicado: true,
  descricao: '',
  papeis: ['Pauteiro', 'Redator', 'Revisor de estilo', 'Fact-checker'],
  formatos: [
    { nome: 'DOCX', binario: true },
    { nome: 'MD', binario: false },
  ],
  perguntas: [],
  gates: [],
}

describe('travessia do fluxo (regra do handoff §7)', () => {
  it('base inicial: 5 blocos encadeados na ordem das setas', () => {
    const ordem = ordemDoFluxo(baseInicial())
    expect(ordem.map((n) => n.label)).toEqual([
      'Entrevistador', 'Transcrição', 'Redator do caso', 'Sua aprovação', 'DOCX + PDF',
    ])
  })

  it('base vazia devolve travessia vazia e esteira só com o briefing', () => {
    expect(ordemDoFluxo(baseVazia())).toEqual([])
    expect(etapasDoFluxo(baseVazia())).toEqual([{ nome: 'Briefing', papel: 'Você' }])
  })

  it('base do modelo começa no Início sem entrada e termina no Fim', () => {
    const ordem = ordemDoFluxo(baseDoModelo(editorial))
    expect(ordem[0].type).toBe('startEvent')
    expect(ordem.at(-1)?.type).toBe('endEvent')
    expect(ordem.map((n) => n.label)).toContain('Sua aprovação')
  })

  it('prefere a seta não-"nao" e anexa órfãos por x', () => {
    const d = baseInicial()
    const ids = ordemDoFluxo(d).map((n) => n.id)
    // Desvio "nao" do papel 1 direto para o exportador: a travessia deve
    // continuar pela sequência normal (transcrição), não pelo desvio.
    const desvio = createEdge({ sourceId: ids[0], targetId: ids[4], versionId: d.version.id })
    desvio.type = 'nao'
    d.edges[desvio.id] = desvio
    const ordem = ordemDoFluxo(d)
    expect(ordem[1].label).toBe('Transcrição')
  })

  it('etapas do fluxo: gate humano para a esteira; exportador vira Exportação', () => {
    const etapas = etapasDoFluxo(baseInicial())
    expect(etapas[0]).toEqual({ nome: 'Briefing', papel: 'Você' })
    const gate = etapas.find((e) => e.gate)
    expect(gate).toMatchObject({ nome: 'Sua aprovação', papel: 'Você' })
    expect(etapas.at(-1)).toMatchObject({ nome: 'Exportação', papel: 'BuildToValue' })
    expect(etapas.find((e) => e.papel === 'ferramenta')?.nome).toBe('Transcrição')
  })

  it('a descrição de teste lista as etapas em ordem, sem eventos', () => {
    const d = descricaoDoFluxo('Estudo de caso', baseDoModelo(editorial))
    expect(d).toContain('Execução de TESTE')
    expect(d).toContain('- Pauteiro (squad:role)')
    expect(d).not.toContain('(startEvent)')
  })
})
