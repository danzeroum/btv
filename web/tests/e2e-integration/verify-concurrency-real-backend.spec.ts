import { test, expect } from '@playwright/test'

/** Validação de pendencias.md (Fase 7 Onda 2, cenário 2 — "duas abas
 * concorrentes → 409 claro"): antes só existia como teste Rust
 * (`segundo_post_verify_com_job_ativo_recebe_409`). Aqui a prova é de
 * NAVEGADOR, disparando dois `POST /api/verify/run` concorrentes da MESMA
 * origem (como duas abas fariam) contra o backend real: um job serializa em
 * `202` (aceito) e o segundo recebe `409` com o MESMO `run_id` em andamento —
 * nunca dois pipelines disputando o mesmo `target/`.
 *
 * `btv.toml` do harness (run-integration-server.mjs) declara 2 passos de
 * `sleep 0.2` (~0.4s) — janela sobrada para o 2º POST chegar durante o 1º job.
 */
test('dois POST /api/verify/run concorrentes: um 202, o outro 409 com o mesmo run_id', async ({ page }) => {
  await page.goto('/')

  const resultado = await page.evaluate(async () => {
    const dispara = () =>
      fetch('/api/verify/run', { method: 'POST' }).then(async (r) => ({
        status: r.status,
        body: (await r.json()) as { run_id: string },
      }))
    // Dois pedidos concorrentes, como duas abas clicando quase juntas.
    const [a, b] = await Promise.all([dispara(), dispara()])
    return { a, b }
  })

  const statuses = [resultado.a.status, resultado.b.status].sort()
  expect(statuses).toEqual([202, 409])

  // Os dois carregam o MESMO run_id — o 409 aponta o job já em andamento,
  // não um erro genérico nem um job novo.
  expect(resultado.a.body.run_id).toBe(resultado.b.body.run_id)
  expect(resultado.a.body.run_id).toMatch(/.+/)
})
