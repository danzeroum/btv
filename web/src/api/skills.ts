import { fetchJson, simulateLatency } from './client'
import type { McpServer, SkillEntry } from '../types/domain'

export let MCP_SERVERS: McpServer[] = [
  { id: 'filesystem', status: 'ok' },
  { id: 'git', status: 'ok' },
  { id: 'postgres', status: 'pendente' },
]

/**
 * Fase 6 Onda 3: lista as skills com o status REAL do vetter, do endpoint
 * `/api/skills` (forge-server → `forge-verify::vetter::list_skill_statuses`).
 * O status é read-only: o vetter decide (fail-closed), o usuário não sobrepõe.
 * Fase 7 Onda 2: sem fallback silencioso — uma falha de rede/backend vira
 * erro real (`AsyncStatus`), não um array mock disfarçado de dado real.
 */
export async function fetchSkills(): Promise<SkillEntry[]> {
  return fetchJson<SkillEntry[]>('/api/skills')
}

/** // TODO: backend Fase 7 Onda 7 (A1) — reconecta o servidor MCP real via forge-tools, atualiza a saúde do sidecar. */
export async function reconnectMcp(id: string): Promise<McpServer> {
  await simulateLatency(400)
  MCP_SERVERS = MCP_SERVERS.map((s) => (s.id === id ? { ...s, status: 'ok' } : s))
  const found = MCP_SERVERS.find((s) => s.id === id)
  if (!found) throw new Error('servidor MCP não encontrado')
  return found
}
