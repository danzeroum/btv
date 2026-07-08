import { expect, test, type Page } from '@playwright/test'

// U3 — Squad ao vivo de PONTA A PONTA contra o motor real: wizard → POST
// /api/btv/squads → orquestrador Python de verdade (uv run, 5 agentes,
// BTV_SCRIPTED=1 → consenso fraco de propósito) → gate HITL real →
// cockpit real (ChatMessage no stream + injeção no próximo Generate) →
// conclusão + ledger (btv.squad_activated / btv.gate_approved /
// btv.adjust_requested). SquadPool tem capacidade 1: os testes rodam em
// série (config) e cada um leva a squad até o fim antes do próximo.

test.describe.configure({ mode: 'serial' })

// Sobe sidecar Python + /verify + orquestrador — folga real.
test.setTimeout(180_000)

async function ativarPelaGaleria(page: Page, cardId: string, resposta: string) {
  await page.goto('/')
  await page.getByTestId(`card-${cardId}`).click()
  const wizard = page.getByTestId('wizard-overlay')
  await wizard.locator('input').first().fill(resposta)
  await wizard.getByRole('button', { name: 'Continuar →' }).click()
  await wizard.getByRole('button', { name: 'Continuar →' }).click()
  await wizard.getByRole('button', { name: '⚑ Ativar squad' }).click()
  // Ativação real navega para Ao vivo.
  await expect(page.getByRole('heading', { name: 'Squad ao vivo' })).toBeVisible({ timeout: 60_000 })
}

test('ativar pela galeria roda squad real: esteira, gate, cockpit, conclusão e ledger', async ({ page }) => {
  await ativarPelaGaleria(page, 'editorial', 'tendências de logística verde no Brasil')

  // Esteira com o nome do modelo e chip da squad na topbar.
  await expect(page.getByTestId('esteira')).toContainText('Editorial / SEO')
  await expect(page.getByTestId('squad-chip')).toBeVisible()

  // O orquestrador real emite propostas (feed) e abre o gate HITL
  // (consenso fraco de propósito no modo roteirizado).
  await expect(page.getByTestId('feed')).toContainText('propôs', { timeout: 120_000 })
  const gate = page.getByTestId('gate-card')
  await expect(gate).toBeVisible({ timeout: 120_000 })
  await expect(gate).toContainText('gate humano')
  // Badge "1 gate" na sidebar (seção SQUAD ATIVA).
  await expect(page.getByText('1 gate')).toBeVisible()

  // Cockpit ANTES de aprovar: a fala é ecoada como ChatMessage(HUMAN) no
  // stream e entra na inbox real (injetada no próximo Generate).
  await page.getByPlaceholder('fale com a squad — direcione, pergunte, mude o rumo…').fill('priorize dados de 2026')
  await page.getByRole('button', { name: 'enviar ↑' }).click()
  await expect(page.getByTestId('cockpit')).toContainText('priorize dados de 2026', { timeout: 15_000 })
  await expect(page.getByTestId('feed')).toContainText('você orientou a squad pelo cockpit')

  // Os agentes também têm voz real no chat (propostas narradas).
  await expect(page.getByTestId('cockpit')).toContainText('·')

  // Aprova o gate → a squad retoma e conclui (stream termina).
  await gate.getByRole('button', { name: 'Aprovar e continuar' }).click()
  await expect(page.getByTestId('gate-card')).toHaveCount(0, { timeout: 30_000 })
  await expect(page.getByTestId('squad-done')).toBeVisible({ timeout: 120_000 })
  await expect(page.getByTestId('squad-done')).toContainText('Entrega concluída')

  // Ledger REAL: ativação com hash de prompts + aprovação do gate.
  const ledger = await page.request.get('/api/ledger?limit=50')
  expect(ledger.ok()).toBeTruthy()
  const entries = (await ledger.json()) as Array<{ kind: string; actor: string; payload: unknown }>
  const ativacao = entries.find((e) => e.kind === 'btv.squad_activated')
  expect(ativacao, 'btv.squad_activated no ledger').toBeTruthy()
  expect(ativacao!.actor).toBe('web:btv')
  const payload = ativacao!.payload as { template_id: string; prompt_hashes: Array<{ prompt_sha256: string }> }
  expect(payload.template_id).toBe('editorial')
  expect(payload.prompt_hashes.length).toBeGreaterThan(0)
  expect(payload.prompt_hashes[0].prompt_sha256).toMatch(/^[0-9a-f]{64}$/)
  expect(entries.some((e) => e.kind === 'btv.gate_approved')).toBeTruthy()

  // Run persistido transiciona para "concluida" (watcher de status).
  await expect
    .poll(
      async () => {
        const runs = (await (await page.request.get('/api/btv/squads')).json()) as Array<{ status: string }>
        return runs[0]?.status
      },
      { timeout: 20_000 },
    )
    .toBe('concluida')
})

test('pedir ajuste no gate: instrução vira orientação real e fica no ledger', async ({ page }) => {
  await ativarPelaGaleria(page, 'musica', 'peça AABA, lírica, ~2 min')

  const gate = page.getByTestId('gate-card')
  await expect(gate).toBeVisible({ timeout: 120_000 })

  await gate.getByRole('button', { name: 'Pedir ajuste' }).click()
  await gate
    .getByPlaceholder('Descreva o ajuste em uma frase — ela vira instrução para o papel certo…')
    .fill('andamento mais lento no B')
  await gate.getByRole('button', { name: 'Enviar ajuste' }).click()

  // O gate fecha e a instrução aparece como fala do humano no cockpit
  // (echo real do stream) — e a squad segue até concluir.
  await expect(page.getByTestId('gate-card')).toHaveCount(0, { timeout: 30_000 })
  await expect(page.getByTestId('cockpit')).toContainText('andamento mais lento no B', { timeout: 15_000 })
  await expect(page.getByTestId('squad-done')).toBeVisible({ timeout: 120_000 })

  const ledger = await page.request.get('/api/ledger?limit=50')
  const entries = (await ledger.json()) as Array<{ kind: string; payload: { instrucao?: string } }>
  const ajuste = entries.find((e) => e.kind === 'btv.adjust_requested')
  expect(ajuste, 'btv.adjust_requested no ledger').toBeTruthy()
  expect(ajuste!.payload.instrucao).toContain('andamento mais lento')
})
