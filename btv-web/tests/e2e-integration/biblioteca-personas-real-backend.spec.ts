import { expect, test } from '@playwright/test'

// U4 (Biblioteca), U6 (Minhas squads) e U7 (Personas) contra o backend real.
// O harness semeia via os MESMOS caminhos de produção (seed_btv →
// BtvStore::insert_run/insert_deliverable): um MD exportável com arquivo
// real no disco e um DOCX binário (export honesto: "em breve").

test('biblioteca agrupa entregas reais com trilha e export honesto', async ({ page }) => {
  await page.goto('/')
  await page.getByRole('button', { name: /Biblioteca/ }).click()
  await expect(page.getByRole('heading', { name: 'Biblioteca de entregas' })).toBeVisible()

  // Grupo Editorial com a entrega MD e trilha de procedência real.
  await expect(page.getByText('Editorial / SEO', { exact: true })).toBeVisible()
  await expect(page.getByText('artigo-seed.md')).toBeVisible()
  await expect(page.getByText(/Redator → Revisor de estilo · 1 gate/).first()).toBeVisible()

  // Export do MD baixa o CONTEÚDO REAL do arquivo.
  const download = await page.request.get('/api/btv/deliverables/1/download')
  expect(download.ok()).toBeTruthy()
  expect(await download.text()).toContain('conteúdo real do artefato')

  // DOCX binário: sem botão de export — "em breve" (sem conversor real).
  await expect(page.getByText('minuta-seed.docx')).toBeVisible()
  await expect(page.getByText('em breve')).toBeVisible()
  const negado = await page.request.get('/api/btv/deliverables/2/download')
  expect(negado.status()).toBe(422)
})

test('minhas squads lista runs persistidos com status e ação contextual', async ({ page }) => {
  await page.goto('/')
  await page.getByRole('button', { name: /Minhas squads/ }).click()
  await expect(page.getByText('Newsletter seed')).toBeVisible()
  await expect(page.getByText('concluída').first()).toBeVisible()
  await expect(page.getByRole('button', { name: 'ver entregas' }).first()).toBeVisible()
})

test('personas: override real com badge, restaurar e auditoria', async ({ page }) => {
  await page.goto('/')
  await page.getByRole('button', { name: /Personas/ }).click()
  await expect(page.getByRole('heading', { name: 'Personas & prompts' })).toBeVisible()

  // 4 papéis do editorial com prompt padrão do arquétipo.
  const card = page.getByTestId('persona-Redator')
  await expect(card).toBeVisible()
  await expect(card).toContainText('padrão')
  await expect(card.locator('textarea')).toHaveValue(/Produza a primeira versão completa/)

  // Editar (blur salva) → badge "editado" + restaurar aparece.
  await card.locator('textarea').fill('Você é o Redator. Escreva SEMPRE em voz ativa.')
  await card.locator('textarea').blur()
  await expect(card).toContainText('editado')
  await expect(card.getByRole('button', { name: '↺ restaurar padrão' })).toBeVisible()

  // Auditoria real do override no ledger.
  const ledger = await page.request.get('/api/ledger?limit=20')
  const entries = (await ledger.json()) as Array<{ kind: string; payload: { papel?: string } }>
  const upd = entries.find((e) => e.kind === 'btv.persona_updated')
  expect(upd, 'btv.persona_updated no ledger').toBeTruthy()
  expect(upd!.payload.papel).toBe('Redator')

  // Restaurar volta ao padrão.
  await card.getByRole('button', { name: '↺ restaurar padrão' }).click()
  await expect(card).toContainText('padrão')
  await expect(card.locator('textarea')).toHaveValue(/Produza a primeira versão completa/)

  // Persona própria: criar e remover.
  await page.getByRole('button', { name: '+ Nova persona' }).click()
  await expect(page.getByText('persona criada por você')).toBeVisible()
  await page.getByRole('button', { name: 'remover' }).click()
  await expect(page.getByText('persona criada por você')).toHaveCount(0)
})
