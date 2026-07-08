/** Utilitários compartilhados pela camada api/* — mesmo cliente comprovado do
 *  console Forge (`web/src/api/client.ts`), incluindo o tratamento de corpo
 *  vazio (202/204) que já pegou bug real lá. */

export class ApiError extends Error {
  code?: string
  constructor(message: string, code?: string) {
    super(message)
    this.name = 'ApiError'
    this.code = code
  }
}

/** Corpo de erro que toda rota real do backend devolve. */
interface ApiErrorBody {
  error?: string
  code?: string
}

export async function fetchJson<T>(url: string, init?: RequestInit): Promise<T> {
  let response: Response
  try {
    response = await fetch(url, init)
  } catch {
    throw new ApiError(`falha de rede em ${url}`, 'network_error')
  }
  if (!response.ok) {
    let body: ApiErrorBody = {}
    try {
      body = (await response.json()) as ApiErrorBody
    } catch {
      // corpo não-JSON (ex.: 404 de proxy) — segue com a mensagem genérica.
    }
    throw new ApiError(
      body.error ?? `${url} respondeu ${response.status}`,
      body.code ?? `http_${response.status}`,
    )
  }
  // Corpo vazio (204/202 fire-and-forget) nunca deve chamar `.json()` direto —
  // em conteúdo vazio ele lança `SyntaxError`, confundido com falha real.
  const text = await response.text()
  return (text ? JSON.parse(text) : undefined) as T
}
