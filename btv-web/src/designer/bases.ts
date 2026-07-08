import {
  createDefaultRegistry,
  createDiagram,
  createEdge,
  createNode,
  type BpmnDiagram,
  type NodeTypeRegistry,
} from '@bpmn-react/core'
import type { SquadTemplate } from '../api/templates'
import { btvDesignerPlugin } from './btvPlugin'

function registryComDominio(): NodeTypeRegistry {
  const registry = createDefaultRegistry()
  for (const def of btvDesignerPlugin.nodeTypes ?? []) registry.register(def)
  return registry
}

interface BlocoSpec {
  tipo: string
  nome: string
  x: number
  y: number
  props?: Record<string, unknown>
}

function montar(nome: string, blocos: BlocoSpec[], cadeia = true): BpmnDiagram {
  const registry = registryComDominio()
  const diagram = createDiagram({ name: nome, createdBy: 'você' })
  const ids: string[] = []
  for (const b of blocos) {
    const node = createNode(
      { type: b.tipo, label: b.nome, x: b.x, y: b.y, properties: b.props },
      registry,
    )
    node.createdInVersion = diagram.version.id
    diagram.nodes[node.id] = node
    ids.push(node.id)
  }
  if (cadeia) {
    for (let i = 0; i < ids.length - 1; i++) {
      const edge = createEdge({
        sourceId: ids[i],
        targetId: ids[i + 1],
        versionId: diagram.version.id,
        createdBy: 'você',
      })
      diagram.edges[edge.id] = edge
    }
  }
  return diagram
}

/** Fluxo inicial do protótipo ("Estudo de caso": 5 blocos, 4 setas). */
export function baseInicial(): BpmnDiagram {
  return montar('Estudo de caso', [
    { tipo: 'squad:role', nome: 'Entrevistador', x: 40, y: 80 },
    { tipo: 'squad:tool', nome: 'Transcrição', x: 260, y: 190 },
    { tipo: 'squad:role', nome: 'Redator do caso', x: 480, y: 90 },
    { tipo: 'squad:approval', nome: 'Sua aprovação', x: 690, y: 210 },
    { tipo: 'squad:output', nome: 'DOCX + PDF', x: 900, y: 110 },
  ])
}

export function baseVazia(): BpmnDiagram {
  return montar('Estudo de caso', [], false)
}

/** Base derivada de um modelo da galeria (receita `loadBase` do protótipo):
 *  Início → papéis (zigue-zague) com gate após o 2º → gate final →
 *  exportador → Fim, tudo encadeado. */
export function baseDoModelo(template: SquadTemplate): BpmnDiagram {
  const blocos: BlocoSpec[] = []
  let x = 20
  blocos.push({ tipo: 'startEvent', nome: 'Início', x, y: 170 })
  x += 115
  template.papeis.forEach((p, i) => {
    blocos.push({ tipo: 'squad:role', nome: p, x, y: 70 + (i % 2) * 160 })
    x += 190
    if (i === 1) {
      blocos.push({ tipo: 'squad:approval', nome: 'Sua aprovação', x, y: 300 })
      x += 185
    }
  })
  blocos.push({ tipo: 'squad:approval', nome: 'Aprovação final', x, y: 110 })
  x += 180
  blocos.push({
    tipo: 'squad:output',
    nome: template.formatos.map((f) => f.nome).join(' + '),
    x,
    y: 240,
  })
  x += 175
  blocos.push({ tipo: 'endEvent', nome: 'Fim', x, y: 170 })
  const d = montar(`Derivada de ${template.nome}`, blocos)
  return d
}
