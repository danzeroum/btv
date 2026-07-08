import { activeEdges, activeNodes, type BpmnDiagram, type BpmnEdge, type BpmnNode } from '@bpmn-react/core'
import type { Etapa } from '../lib/esteira'

/** Travessia do fluxo (handoff §7): começa no Início sem seta de entrada
 *  (fallback: bloco sem entrada, senão o primeiro), segue setas de saída
 *  preferindo não-"nao", evita ciclos, anexa órfãos por ordem de x. */
export function ordemDoFluxo(diagram: BpmnDiagram): BpmnNode[] {
  const nodes = activeNodes(diagram)
  if (nodes.length === 0) return []
  const edges = activeEdges(diagram)
  const byId = new Map(nodes.map((n) => [n.id, n]))
  const out = new Map<string, BpmnEdge[]>()
  for (const e of edges) {
    const list = out.get(e.sourceId) ?? []
    list.push(e)
    out.set(e.sourceId, list)
  }
  const hasIn = new Set(edges.map((e) => e.targetId))
  const start =
    nodes.find((n) => n.type === 'startEvent' && !hasIn.has(n.id)) ??
    nodes.find((n) => !hasIn.has(n.id)) ??
    nodes[0]

  const seen = new Set<string>()
  const order: BpmnNode[] = []
  let cur: BpmnNode | undefined = start
  while (cur && !seen.has(cur.id)) {
    seen.add(cur.id)
    order.push(cur)
    const outs: BpmnEdge[] = (out.get(cur.id) ?? []).filter((e) => byId.has(e.targetId))
    const nx: BpmnEdge | undefined = outs.find((e) => e.type !== 'nao') ?? outs[0]
    cur = nx ? byId.get(nx.targetId) : undefined
  }
  for (const n of nodes.filter((n) => !seen.has(n.id)).sort((a, b) => a.x - b.x)) {
    order.push(n)
  }
  return order
}

/** Converte a travessia em etapas de esteira (▶ Testar squad) — mesma regra
 *  do protótipo: eventos início/fim saem; papel vira etapa própria; blocos
 *  técnicos ganham o papel genérico; gate humano para a esteira. */
export function etapasDoFluxo(diagram: BpmnDiagram): Etapa[] {
  const etapas: Etapa[] = [{ nome: 'Briefing', papel: 'Você' }]
  for (const n of ordemDoFluxo(diagram)) {
    switch (n.type) {
      case 'startEvent':
      case 'endEvent':
        break
      case 'squad:role':
        etapas.push({ nome: n.label, papel: n.label })
        break
      case 'squad:tool':
        etapas.push({ nome: n.label, papel: 'ferramenta' })
        break
      case 'squad:service':
        etapas.push({ nome: n.label, papel: 'API' })
        break
      case 'squad:data':
        etapas.push({ nome: n.label, papel: 'base de dados' })
        break
      case 'squad:approval':
        etapas.push({ nome: n.label, papel: 'Você', gate: true })
        break
      case 'exclusiveGateway':
        etapas.push({ nome: String(n.properties.condicao || n.label), papel: 'gateway ◇' })
        break
      case 'parallelGateway':
        etapas.push({ nome: n.label, papel: 'gateway ⧓' })
        break
      case 'squad:output':
        etapas.push({ nome: 'Exportação', papel: 'BuildToValue' })
        break
      default:
        etapas.push({ nome: n.label, papel: n.type })
    }
  }
  return etapas
}

/** Descrição REAL da tarefa de teste (▶ Testar squad) — o motor do squad
 *  recebe o fluxo desenhado como plano de trabalho. */
export function descricaoDoFluxo(nome: string, diagram: BpmnDiagram): string {
  const linhas = ordemDoFluxo(diagram)
    .filter((n) => n.type !== 'startEvent' && n.type !== 'endEvent')
    .map((n) => {
      const prompt = typeof n.properties.prompt === 'string' && n.properties.prompt.trim()
      const detalhe = prompt
        ? ` — ${String(n.properties.prompt).trim()}`
        : n.properties.endpoint
          ? ` — endpoint: ${String(n.properties.endpoint)}`
          : n.properties.fonte
            ? ` — fonte: ${String(n.properties.fonte)}`
            : ''
      return `- ${n.label} (${n.type})${detalhe}`
    })
  return `Execução de TESTE da squad "${nome}" desenhada no Squad Designer (fluxo BPMN).\n\n## Etapas do fluxo, em ordem\n${linhas.join('\n')}\n\nSiga o fluxo na ordem; nada é exportado sem aprovação humana.`
}
