import type { SquadEventEnvelope } from '../api/squad'
import type { SquadTemplate } from '../api/templates'
import { hhmm } from './time'

/** Etapa da esteira (handoff §6 U3). `gate: true` = ponto onde a squad para
 *  e espera o humano. */
export interface Etapa {
  nome: string
  papel: string
  gate?: boolean
}

/** Constrói as 8 etapas do modelo (regra exata do protótipo, `makeEtapas`):
 *  papéis desligados no wizard reatribuem via `p(i) = on[min(i, len-1)]`. */
export function makeEtapas(template: SquadTemplate, papeisOff: number[]): Etapa[] {
  const on = template.papeis.filter((_, i) => !papeisOff.includes(i))
  const p = (i: number) => on[Math.min(i, on.length - 1)] ?? 'Você'
  return [
    { nome: 'Briefing', papel: 'Você' },
    { nome: 'Planejamento', papel: p(0) },
    { nome: 'Produção', papel: p(1) },
    { nome: 'Rascunho', papel: 'Você', gate: true },
    { nome: 'Revisão', papel: p(2) },
    { nome: 'Validação', papel: p(3) },
    { nome: 'Entrega', papel: 'Você', gate: true },
    { nome: 'Exportação', papel: 'BuildToValue' },
  ]
}

/** Ação local do usuário que afeta a posição da esteira, ordenada em relação
 *  aos eventos do stream por `afterEventIndex` (quantos eventos já tinham
 *  chegado quando a ação aconteceu). */
export interface AcaoLocal {
  kind: 'gate_aprovado' | 'ajuste'
  afterEventIndex: number
}

export interface EsteiraView {
  /** Índice da etapa ativa; `etapas.length` quando tudo concluído. */
  idx: number
  gateOpen: boolean
  done: boolean
  /** Kill-switch ou erro do orquestrador — esteira congelada. */
  erro: string | null
  /** true quando a posição atual foi INFERIDA dos eventos (não veio de um
   *  sinal direto do orquestrador) — a UI rotula (aprovação obs. 4). */
  inferida: boolean
}

/**
 * Mapeia os eventos REAIS do orquestrador (proto `SquadEvent`) para a posição
 * da esteira de apresentação — função pura, honestidade por construção:
 *
 * - sinais DIRETOS: `Hitl` (abre o próximo gate ainda não passado),
 *   `Consensus` (planejamento decidido → produção), `Error` (congela),
 *   `Step` com sucesso (produção/validação), fim do stream (tudo concluído).
 * - sinais INFERIDOS (rotulados na UI): avanço pós-aprovação de gate (o
 *   orquestrador não emite "gate resolvido" — a aprovação é ação local) e a
 *   regressão visual do "pedir ajuste" (o trabalho retoma 2 etapas atrás,
 *   aprovação obs. 1).
 * - a posição nunca regride EXCETO no ajuste (regra do protótipo).
 */
export function esteiraFromEvents(
  etapas: Etapa[],
  events: SquadEventEnvelope[],
  acoes: AcaoLocal[],
  streamEnded: boolean,
): EsteiraView {
  const gateIdxs = etapas.map((e, i) => (e.gate ? i : -1)).filter((i) => i >= 0)
  let idx = Math.min(1, etapas.length - 1) // ativação: 1ª etapa após o briefing
  let gateOpen = false
  let done = false
  let erro: string | null = null
  let inferida = false
  let gatesPassados = 0

  const proximoGate = () => gateIdxs.find((g) => g > idx || (g === idx && !gateOpen))

  // Intercala eventos e ações locais na ordem em que aconteceram.
  type Item = { at: number; ev?: SquadEventEnvelope; acao?: AcaoLocal }
  const itens: Item[] = [
    ...events.map((ev, i) => ({ at: i, ev })),
    ...acoes.map((acao) => ({ at: acao.afterEventIndex - 0.5, acao })),
  ].sort((a, b) => a.at - b.at)

  const avancar = (para: number, foiInferido: boolean) => {
    if (para > idx) {
      idx = Math.min(para, etapas.length)
      inferida = foiInferido
    }
  }

  for (const item of itens) {
    if (erro) break
    if (item.acao) {
      if (item.acao.kind === 'gate_aprovado' && gateOpen) {
        gatesPassados += 1
        gateOpen = false
        avancar(idx + 1, true)
      } else if (item.acao.kind === 'ajuste') {
        // Regressão visual: o trabalho volta 2 etapas a partir do gate
        // (protótipo); o gate fecha e a squad retoma com a instrução em
        // contexto. Única situação em que idx diminui.
        if (gateOpen) gatesPassados += 1
        gateOpen = false
        idx = Math.max(1, idx - 2)
        inferida = true
      }
      continue
    }
    const payload = item.ev?.payload
    if (!payload) continue
    if ('Error' in payload) {
      erro = payload.Error
      gateOpen = false
      break
    }
    if ('Hitl' in payload) {
      const g = gateIdxs[Math.min(gatesPassados, gateIdxs.length - 1)]
      if (g !== undefined && g >= idx) {
        idx = g
        gateOpen = true
        inferida = false
      }
      continue
    }
    // Gate aberto: eventos de TRABALHO (Step/Consensus) só acontecem depois
    // do HITL resolvido — se chegam, o gate foi aprovado fora desta sessão
    // (replay de snapshot após recarregar a página). Fecha por inferência.
    if (gateOpen) {
      if ('Step' in payload || 'Consensus' in payload) {
        gatesPassados += 1
        gateOpen = false
        avancar(idx + 1, true)
      } else {
        continue // Proposal/Handoff/Chat: informativos, não movem
      }
    }
    if ('Consensus' in payload) {
      avancar(2, false)
      continue
    }
    if ('Step' in payload) {
      if (payload.Step.step_id === 'final_validation' && payload.Step.success) {
        // Validação concluída → próxima parada é o gate de Entrega (que só
        // ABRE com um Hitl real) ou o fim.
        const validacaoIdx = etapas.findIndex((e) => e.nome === 'Validação')
        if (validacaoIdx >= 0) avancar(validacaoIdx + 1, false)
      } else if (payload.Step.success) {
        avancar(2, true)
      }
      continue
    }
    // Proposal/Handoff/Chat: informativos — alimentam feed/chat, não a posição.
  }

  if (!erro && streamEnded && !gateOpen) {
    done = true
    idx = etapas.length
  }
  if (proximoGate() === undefined && done) gateOpen = false

  return { idx, gateOpen, done, erro, inferida }
}

const HANDOFF_LABEL: Record<number, string> = {
  0: 'handoff',
  1: 'iniciou handoff para',
  2: 'confirmou handoff de',
  3: 'concluiu handoff para',
  4: 'falhou handoff para',
}

export interface FeedItem {
  ts: string
  txt: string
}

/** Deriva o feed de atividade (coluna direita de U3) dos eventos reais —
 *  mais recente primeiro. Mostra o AGENTE REAL do motor (architect/developer/
 *  auditor/ops/…), NÃO um rótulo do template: o orquestrador é um squad de
 *  engenharia genérico, e disfarçar seus agentes de "Pauteiro/Redator"
 *  esconderia por que uma tarefa não-software se comporta como se comporta
 *  (rodar `cargo test`, pedir estratégia de deploy, etc.). */
export function feedFromEvents(events: SquadEventEnvelope[]): FeedItem[] {
  const out: FeedItem[] = []
  for (const ev of events) {
    const ts = hhmm(ev.ts)
    const p = ev.payload
    if (!p) continue
    if ('Proposal' in p) {
      out.push({
        ts,
        txt: `${p.Proposal.agent} propôs (confiança ${Math.round(p.Proposal.confidence * 100)}%)`,
      })
    } else if ('Consensus' in p) {
      out.push({
        ts,
        txt: `consenso de ${p.Consensus.decision_maker || 'squad'} (força ${Math.round(p.Consensus.strength * 100)}%)${p.Consensus.requires_human ? ' — aguarda humano' : ''}`,
      })
    } else if ('Handoff' in p) {
      out.push({
        ts,
        txt: `${p.Handoff.from_agent} ${HANDOFF_LABEL[p.Handoff.phase] ?? 'handoff'} ${p.Handoff.to_agent}`,
      })
    } else if ('Hitl' in p) {
      out.push({ ts, txt: `✋ gate aberto — aguarda sua decisão (${p.Hitl.reason})` })
    } else if ('Step' in p) {
      out.push({
        ts,
        txt: `${p.Step.success ? '✓' : '✕'} passo ${p.Step.step_id}: ${p.Step.summary}`,
      })
    } else if ('Error' in p) {
      out.push({ ts, txt: `⚠ ${p.Error}` })
    } else if ('Chat' in p && p.Chat.author_role === 'HUMAN') {
      out.push({ ts, txt: '💬 você orientou a squad pelo cockpit' })
    }
  }
  return out.reverse()
}

export interface AtividadeAtual {
  /** Agente REAL do motor (architect/developer/auditor/ops/…) que começou por
   *  último — não um papel do template. */
  agente: string
  /** Quando começou (HH:MM) — se ficou parado, a distância até agora salta aos
   *  olhos. */
  desde: string
}

/** Qual agente do orquestrador está (ou ficou por último) ativo e desde quando,
 *  derivado do último `Handoff`→agente (≠ orchestrator) ou `Proposal`. Torna
 *  VISÍVEL uma congelada: "developer · desde 17:30" enquanto o relógio anda. */
export function atividadeAtual(events: SquadEventEnvelope[]): AtividadeAtual | null {
  let atual: AtividadeAtual | null = null
  for (const ev of events) {
    const p = ev.payload
    if (!p) continue
    if ('Handoff' in p && p.Handoff.to_agent && p.Handoff.to_agent !== 'orchestrator') {
      atual = { agente: p.Handoff.to_agent, desde: hhmm(ev.ts) }
    } else if ('Proposal' in p && p.Proposal.agent) {
      atual = { agente: p.Proposal.agent, desde: hhmm(ev.ts) }
    }
  }
  return atual
}
