import type { NodeTypeDefinition } from '@bpmn-react/core'
import type { BpmnPlugin, EdgeStyle, ShapeProps } from '@bpmn-react/react'

/** Plugin de domínio do PRODUTO (a lib bpmn permanece agnóstica — todo o
 *  vocabulário BuildToValue entra por aqui). Os 10 blocos do handoff §7
 *  mapeados a tags BPMN 2.0 interoperáveis; Decisão/Paralelo/Início/Fim
 *  usam os tipos built-in da lib (formas BPMN canônicas). */

export const BLOCO_META: Record<string, { cor: string; icon: string; label: string }> = {
  'squad:role': { cor: '#b8531f', icon: '☺', label: 'papel' },
  'squad:tool': { cor: '#6f675a', icon: '⚒', label: 'ferramenta' },
  'squad:service': { cor: '#2b7a8c', icon: '⇌', label: 'chamada de API' },
  'squad:data': { cor: '#8b8171', icon: '⛁', label: 'base de dados' },
  'squad:approval': { cor: '#a85b3f', icon: '✋', label: 'gate humano' },
  'squad:output': { cor: '#14614f', icon: '↧', label: 'exportador' },
  exclusiveGateway: { cor: '#9a6b14', icon: '◇', label: 'gateway · decisão' },
  parallelGateway: { cor: '#6b4fae', icon: '⧓', label: 'gateway · paralelo' },
  startEvent: { cor: '#3d8b4f', icon: '▷', label: 'evento · início' },
  endEvent: { cor: '#5b5344', icon: '◼', label: 'evento · fim' },
}

/** Card do handoff: retângulo 11px de raio, quadradinho de ícone na cor do
 *  tipo, nome + tipo em mono. API tem borda dupla; base de dados, topo de
 *  cilindro (double). */
function cardShape(tipo: string): (props: ShapeProps) => React.JSX.Element {
  const meta = BLOCO_META[tipo]
  return function BtvCardShape({ node, selected }: ShapeProps) {
    // Nó selecionado: borda de execução (--brand). Gate humano: borda terracota
    // (--decision) e fundo terracota-claro mesmo sem seleção (regra §5 Designer).
    const isGate = tipo === 'squad:approval'
    const stroke = selected ? '#14614f' : isGate ? '#a85b3f' : '#d2c7ae'
    const fill = isGate ? '#f6ebe4' : '#fbf8f1'
    const doubleBorder = tipo === 'squad:service'
    const cylinder = tipo === 'squad:data'
    return (
      <g>
        <rect
          width={node.width}
          height={node.height}
          rx={11}
          fill={fill}
          stroke={stroke}
          strokeWidth={doubleBorder ? 1.4 : 1.5}
        />
        {doubleBorder && (
          <rect
            x={3}
            y={3}
            width={node.width - 6}
            height={node.height - 6}
            rx={8}
            fill="none"
            stroke={stroke}
            strokeWidth={1.2}
          />
        )}
        {cylinder && (
          <>
            <line x1={2} y1={6} x2={node.width - 2} y2={6} stroke={stroke} strokeWidth={1.2} />
            <line x1={2} y1={9} x2={node.width - 2} y2={9} stroke={stroke} strokeWidth={1.2} />
          </>
        )}
        <rect x={9} y={(node.height - 22) / 2} width={22} height={22} rx={6} fill={meta.cor} />
        <text
          x={20}
          y={node.height / 2 + 4}
          textAnchor="middle"
          fontSize={11}
          fill="#ffffff"
          style={{ fontFamily: 'var(--sans, sans-serif)' }}
        >
          {meta.icon}
        </text>
        <text
          x={38}
          y={node.height / 2 - 2}
          fontSize={12}
          fontWeight={600}
          fill="#2b2b28"
          style={{ fontFamily: 'var(--sans, sans-serif)' }}
        >
          {node.label.length > 16 ? `${node.label.slice(0, 15)}…` : node.label}
        </text>
        <text
          x={38}
          y={node.height / 2 + 12}
          fontSize={9}
          fill="#a89e8b"
          style={{ fontFamily: 'var(--mono, monospace)' }}
        >
          {meta.label}
        </text>
      </g>
    )
  }
}

const NODE_TYPES: NodeTypeDefinition[] = [
  { type: 'squad:role', label: 'Papel', category: 'custom', defaultSize: { width: 160, height: 46 }, xml: { tag: 'userTask' }, visual: { shadow: true } },
  { type: 'squad:tool', label: 'Ferramenta', category: 'custom', defaultSize: { width: 160, height: 46 }, xml: { tag: 'serviceTask' }, visual: { shadow: true } },
  { type: 'squad:service', label: 'Chamada de API', category: 'custom', defaultSize: { width: 170, height: 48 }, xml: { tag: 'serviceTask' }, visual: { shadow: true } },
  { type: 'squad:data', label: 'Base de dados', category: 'custom', defaultSize: { width: 165, height: 48 }, xml: { tag: 'dataObjectReference' }, visual: { shadow: true } },
  { type: 'squad:approval', label: 'Gate humano', category: 'custom', defaultSize: { width: 160, height: 46 }, xml: { tag: 'manualTask' }, visual: { shadow: true } },
  { type: 'squad:output', label: 'Exportador', category: 'custom', defaultSize: { width: 160, height: 46 }, xml: { tag: 'sendTask' }, visual: { shadow: true } },
]

/** Setas por finalidade (handoff §4/§7): sequência cinza, sim verde, não
 *  rust, dados azul tracejada. */
export const EDGE_STYLES: Record<string, EdgeStyle> = {
  sequenceFlow: { stroke: '#8b8171', strokeWidth: 1.8, marker: 'filled' },
  sim: { stroke: '#3d8b4f', strokeWidth: 1.8, marker: 'filled' },
  nao: { stroke: '#b8531f', strokeWidth: 1.8, marker: 'filled' },
  dados: { stroke: '#6f675a', strokeWidth: 1.8, dash: '6,5', marker: 'open' },
}

export const btvDesignerPlugin: BpmnPlugin = {
  id: 'buildtovalue/squad-designer',
  name: 'Vocabulário BuildToValue do Squad Designer',
  nodeTypes: NODE_TYPES,
  shapes: {
    'squad:role': cardShape('squad:role'),
    'squad:tool': cardShape('squad:tool'),
    'squad:service': cardShape('squad:service'),
    'squad:data': cardShape('squad:data'),
    'squad:approval': cardShape('squad:approval'),
    'squad:output': cardShape('squad:output'),
  },
  edgeStyles: EDGE_STYLES,
  paletteGroups: [
    {
      id: 'squad',
      label: 'Blocos da squad',
      headerColor: 'var(--brand, #14614f)',
      itemBackground: '#ffffff',
      itemHoverBackground: '#f0ebdf',
    },
    {
      id: 'fluxo',
      label: 'Fluxo BPMN',
      headerColor: '#8b8171',
      itemBackground: '#ffffff',
      itemHoverBackground: '#f0ebdf',
    },
  ],
  paletteItems: [
    { id: 'sq-role', label: 'Papel', nodeType: 'squad:role', icon: '☺', group: 'squad', defaultProperties: { prompt: '' } },
    { id: 'sq-tool', label: 'Ferramenta', nodeType: 'squad:tool', icon: '⚒', group: 'squad' },
    { id: 'sq-service', label: 'Chamada de API', nodeType: 'squad:service', icon: '⇌', group: 'squad', defaultProperties: { endpoint: '' } },
    { id: 'sq-data', label: 'Base de dados', nodeType: 'squad:data', icon: '⛁', group: 'squad', defaultProperties: { fonte: '' } },
    { id: 'sq-approval', label: 'Gate humano', nodeType: 'squad:approval', icon: '✋', group: 'squad' },
    { id: 'sq-output', label: 'Exportador', nodeType: 'squad:output', icon: '↧', group: 'squad' },
    { id: 'sq-decision', label: 'Decisão ◇', nodeType: 'exclusiveGateway', icon: '◇', group: 'fluxo', defaultProperties: { condicao: '' } },
    { id: 'sq-parallel', label: 'Paralelo ⧓', nodeType: 'parallelGateway', icon: '⧓', group: 'fluxo' },
    { id: 'sq-start', label: 'Início ○', nodeType: 'startEvent', icon: '▷', group: 'fluxo' },
    { id: 'sq-end', label: 'Fim ◉', nodeType: 'endEvent', icon: '◼', group: 'fluxo' },
  ],
}
