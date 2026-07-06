import { simulateLatency } from './client'

export type ToolCallStatus = 'running' | 'ok' | 'error'

export interface TranscriptTurn {
  id: string
  kind: 'user' | 'agent' | 'tool' | 'diff' | 'lint'
  text: string
  toolStatus?: ToolCallStatus
}

export interface SessionHeader {
  model: string
  agent: string
  provider: string
  cacheOn: boolean
  sessionId: string
}

export interface ToolPolicy {
  tool: string
  policy: 'allow' | 'ask'
}

export const TOOL_POLICIES: ToolPolicy[] = [
  { tool: 'read', policy: 'allow' },
  { tool: 'grep', policy: 'allow' },
  { tool: 'edit', policy: 'ask' },
  { tool: 'bash', policy: 'ask' },
  { tool: 'webfetch', policy: 'ask' },
]

/** // TODO: backend Fase 5 — persiste a política de permissões por ferramenta no forge-core. */
export async function toggleToolPolicy(tool: string): Promise<ToolPolicy> {
  await simulateLatency(200)
  const found = TOOL_POLICIES.find((p) => p.tool === tool)
  if (!found) throw new Error(`ferramenta desconhecida: ${tool}`)
  found.policy = found.policy === 'allow' ? 'ask' : 'allow'
  return found
}

export const SESSION_HEADER: SessionHeader = {
  model: 'claude-sonnet-5',
  agent: 'build',
  provider: 'anthropic',
  cacheOn: true,
  sessionId: 's7f3a1',
}
