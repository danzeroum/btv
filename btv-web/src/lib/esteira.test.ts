import { describe, expect, it } from 'vitest'
import { esteiraFromEvents, feedFromEvents, makeEtapas, type Etapa } from './esteira'
import type { SquadEventEnvelope, SquadEventPayload } from '../api/squad'
import type { SquadTemplate } from '../api/templates'

const template: SquadTemplate = {
  id: 'editorial',
  nome: 'Editorial / SEO',
  categoria: 'conteudo',
  cor: '#b8531f',
  onda: 1,
  versao: 'v1.4',
  publicado: true,
  descricao: '',
  papeis: ['Pauteiro', 'Redator', 'Revisor de estilo', 'Fact-checker'],
  formatos: [{ nome: 'MD', binario: false }],
  perguntas: [],
  gates: [],
}

function ev(payload: SquadEventPayload): SquadEventEnvelope {
  return { task_id: 't', ts: '2026-07-08T10:15:00Z', payload }
}

const consensus = ev({
  Consensus: { decision_maker: 'architect', strength: 0.5, decision_json: '{}', requires_human: true },
})
const hitl = ev({ Hitl: { reason: 'weak_consensus', confidence: 0.5 } })
const stepOk = ev({ Step: { step_id: '1', success: true, summary: 'publicar' } })
const stepFinal = ev({ Step: { step_id: 'final_validation', success: true, summary: 'ok' } })

describe('makeEtapas (regra do protótipo)', () => {
  it('gera as 8 etapas com papéis por índice e gates em Rascunho/Entrega', () => {
    const etapas = makeEtapas(template, [])
    expect(etapas.map((e) => e.nome)).toEqual([
      'Briefing', 'Planejamento', 'Produção', 'Rascunho', 'Revisão', 'Validação', 'Entrega', 'Exportação',
    ])
    expect(etapas[1].papel).toBe('Pauteiro')
    expect(etapas[3]).toMatchObject({ papel: 'Você', gate: true })
    expect(etapas[7].papel).toBe('BuildToValue')
  })

  it('papéis desligados reatribuem via p(i)=on[min(i,len-1)]', () => {
    const etapas = makeEtapas(template, [0, 1])
    expect(etapas[1].papel).toBe('Revisor de estilo')
    expect(etapas[2].papel).toBe('Fact-checker')
    expect(etapas[5].papel).toBe('Fact-checker')
  })
})

describe('esteiraFromEvents (mapeamento honesto de eventos reais)', () => {
  const etapas: Etapa[] = makeEtapas(template, [])

  it('ativação começa no Planejamento', () => {
    const v = esteiraFromEvents(etapas, [], [], false)
    expect(v).toMatchObject({ idx: 1, gateOpen: false, done: false, erro: null })
  })

  it('consenso avança para Produção (sinal direto)', () => {
    const v = esteiraFromEvents(etapas, [consensus], [], false)
    expect(v.idx).toBe(2)
    expect(v.inferida).toBe(false)
  })

  it('Hitl abre o primeiro gate (Rascunho) e eventos informativos não movem', () => {
    const v = esteiraFromEvents(etapas, [consensus, hitl, stepOk], [], false)
    expect(v.idx).toBe(3)
    expect(v.gateOpen).toBe(true)
  })

  it('aprovar o gate avança (posição inferida — o orquestrador não emite "gate resolvido")', () => {
    const v = esteiraFromEvents(etapas, [consensus, hitl], [{ kind: 'gate_aprovado', afterEventIndex: 2 }], false)
    expect(v.idx).toBe(4)
    expect(v.gateOpen).toBe(false)
    expect(v.inferida).toBe(true)
  })

  it('pedir ajuste REGRIDE a esteira 2 etapas visivelmente (aprovação obs. 1)', () => {
    const v = esteiraFromEvents(etapas, [consensus, hitl], [{ kind: 'ajuste', afterEventIndex: 2 }], false)
    expect(v.idx).toBe(1) // gate no 3 → volta para 1
    expect(v.gateOpen).toBe(false)
    expect(v.inferida).toBe(true)
  })

  it('após o ajuste, sinais reais voltam a avançar', () => {
    const v = esteiraFromEvents(
      etapas,
      [consensus, hitl, stepFinal],
      [{ kind: 'ajuste', afterEventIndex: 2 }],
      false,
    )
    expect(v.idx).toBe(6) // Validação concluída → parada seguinte (gate Entrega)
    expect(v.gateOpen).toBe(false) // Entrega só abre com Hitl real
  })

  it('segundo Hitl abre o segundo gate (Entrega)', () => {
    const v = esteiraFromEvents(
      etapas,
      [consensus, hitl, stepFinal, hitl],
      [{ kind: 'gate_aprovado', afterEventIndex: 2 }],
      false,
    )
    expect(v.idx).toBe(6)
    expect(v.gateOpen).toBe(true)
  })

  it('erro congela a esteira', () => {
    const v = esteiraFromEvents(etapas, [consensus, ev({ Error: 'kill-switch' })], [], true)
    expect(v.erro).toContain('kill-switch')
    expect(v.done).toBe(false)
  })

  it('fim do stream sem erro conclui tudo', () => {
    const v = esteiraFromEvents(etapas, [consensus, stepFinal], [], true)
    expect(v.done).toBe(true)
    expect(v.idx).toBe(etapas.length)
  })
})

describe('feedFromEvents', () => {
  it('deriva o feed dos eventos reais, mais recente primeiro', () => {
    const feed = feedFromEvents([
      ev({ Proposal: { agent: 'architect', confidence: 0.5, content_json: '{}' } }),
      hitl,
      ev({ Chat: { author: 'Você', author_role: 'HUMAN', text: 'foco no tom' } }),
    ])
    expect(feed).toHaveLength(3)
    expect(feed[0].txt).toContain('cockpit')
    expect(feed[2].txt).toContain('architect propôs')
    expect(feed[1].txt).toContain('✋ gate aberto')
    expect(feed[0].ts).toBe('10:15')
  })
})
