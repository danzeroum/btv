/** Utilitários compartilhados pela camada api/*. Todo módulo mock usa
 * `simulateLatency()` para nunca parecer instantâneo/travado, e lança
 * `ApiError` para exercitar o estado `error` de `useAsyncAction`.
 */

export class ApiError extends Error {
  code?: string
  constructor(message: string, code?: string) {
    super(message)
    this.name = 'ApiError'
    this.code = code
  }
}

export function simulateLatency(ms = 300 + Math.random() * 400): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms))
}

/** Lança ApiError com a taxa dada — use em ações que devem, às vezes, exercitar o caminho de erro. */
export function maybeFail(rate: number, message: string): void {
  if (Math.random() < rate) {
    throw new ApiError(message)
  }
}

/** Corpo de erro que toda rota real desta fase devolve (Fase 7 Onda 1). */
interface ApiErrorBody {
  error?: string
  code?: string
}

/**
 * Cliente HTTP real para as rotas ligadas ao backend (Fase 7). Checa `r.ok` e
 * lança `ApiError` com o `code` do corpo `{error, code}` em caso de falha —
 * fim do padrão "assume sucesso" dos módulos ainda mock. `init` aceita os
 * mesmos campos de `fetch` (method/body/headers/signal...).
 */
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
  if (response.status === 204) {
    return undefined as T
  }
  return (await response.json()) as T
}
