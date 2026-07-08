/**
 * Fase 7 Onda 7 (A1): console MCP. `GET /api/mcp` mora no router mesclado de
 * `btv-cli` (precisa de `btv-tools`+`btv-core`) — sonda cada servidor
 * de `.btv/mcp.toml` de verdade e calcula o preview de política real
 * (override persistido da Onda 2, não um perfil mudo).
 */
import { fetchJson } from './client'
import type { PermissionMatrixDecision } from '../types/domain'

export interface McpToolPolicyPreview {
  build: PermissionMatrixDecision
  plan: PermissionMatrixDecision
}

export interface McpToolInfo {
  name: string
  description: string
  policy: McpToolPolicyPreview
}

export interface McpServerInfo {
  id: string
  command: string
  status: 'online' | 'offline'
  error?: string
  tools: McpToolInfo[]
}

export async function fetchMcpServers(): Promise<McpServerInfo[]> {
  return fetchJson<McpServerInfo[]>('/api/mcp')
}
