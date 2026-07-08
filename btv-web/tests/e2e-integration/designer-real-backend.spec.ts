import { expect, test } from '@playwright/test'

// U5 — Squad Designer sobre a lib bpmn (vendor/bpmn) contra o backend real:
// canvas com o fluxo inicial, esteira resultante da travessia, salvar como
// modelo (VersionRegistry da lib + btv.flow_saved no ledger BuildToValue com
// snapshot hash) e ▶ Testar squad rodando o fluxo no motor REAL do squad
// (execução de teste na tela Ao vivo).

test.setTimeout(180_000)

test('designer: canvas da lib, esteira resultante e salvar auditado', async ({ page }) => {
  await page.goto('/')
  await page.getByRole('button', { name: /Designer/ }).click()
  await expect(page.getByRole('heading', { name: 'Squad Designer' })).toBeVisible()

  // Canvas da lib bpmn com o fluxo inicial do protótipo (5 blocos).
  const canvas = page.getByTestId('designer-canvas')
  await expect(canvas.locator('svg').first()).toBeVisible()
  await expect(canvas).toContainText('Entrevistador')
  await expect(canvas).toContainText('Sua aprovação')
  // Paleta do plugin de domínio (grupos do produto, lib agnóstica).
  await expect(canvas).toContainText('Blocos da squad')
  await expect(canvas).toContainText('Gate humano')

  // Esteira resultante = travessia real do grafo.
  await expect(page.getByText('esteira resultante')).toBeVisible()
  await expect(page.getByText('⌂ Briefing')).toBeVisible()
  await expect(page.getByText(/☺ Entrevistador/)).toBeVisible()

  // Salvar: registry da lib + ledger BuildToValue com snapshot hash.
  await page.getByRole('button', { name: 'salvar como modelo' }).click()
  await expect(page.getByText(/✓ salvo e auditado — ledger #\d+/)).toBeVisible({ timeout: 15_000 })
  const ledger = await page.request.get('/api/ledger?limit=20')
  const entries = (await ledger.json()) as Array<{
    kind: string
    payload: { nome?: string; snapshot_hash?: string; diagram_sha256?: string; blocos?: number }
  }>
  const saved = entries.find((e) => e.kind === 'btv.flow_saved')
  expect(saved, 'btv.flow_saved no ledger').toBeTruthy()
  expect(saved!.payload.blocos).toBe(5)
  expect(saved!.payload.snapshot_hash).toMatch(/^[0-9a-f]{16,}$/)
  expect(saved!.payload.diagram_sha256).toMatch(/^[0-9a-f]{64}$/)

  // Auditoria do fluxo (AuditLedger hash-chained da lib) registrou o save.
  await expect(page.getByTestId('auditoria-fluxo')).toContainText('fluxo salvo como modelo')
})

test('▶ testar squad roda o fluxo desenhado no motor real', async ({ page }) => {
  await page.goto('/')
  await page.getByRole('button', { name: /Designer/ }).click()
  await page.getByRole('button', { name: '▶ Testar squad' }).click()

  // Navega para Ao vivo, marcado como execução de teste, com as etapas do fluxo.
  await expect(page.getByRole('heading', { name: 'Squad ao vivo' })).toBeVisible({ timeout: 60_000 })
  await expect(page.getByTestId('esteira')).toContainText('execução de teste')
  await expect(page.getByTestId('esteira')).toContainText('Entrevistador')
  await expect(page.getByTestId('esteira')).toContainText('Sua aprovação')

  // Motor real: propostas chegam; gate HITL real abre; aprovar conclui.
  await expect(page.getByTestId('feed')).toContainText('propôs', { timeout: 120_000 })
  const gate = page.getByTestId('gate-card')
  await expect(gate).toBeVisible({ timeout: 120_000 })
  await gate.getByRole('button', { name: 'Aprovar e continuar' }).click()
  await expect(page.getByTestId('squad-done')).toBeVisible({ timeout: 120_000 })
})
