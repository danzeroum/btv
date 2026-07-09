import { expect, test } from '@playwright/test'

// A1–A6 contra o backend real: telemetria/ledger/providers/permissões/
// modelos/usuários. O harness roda com só ANTHROPIC_API_KEY (fake) no env —
// providers prova exatamente 1 configurado; o seed do BtvStore alimenta a
// telemetria de squads.

async function irParaAdmin(page: import('@playwright/test').Page, tela: string) {
  await page.goto('/')
  await page.getByRole('button', { name: 'Administração' }).click()
  if (tela) await page.getByRole('button', { name: new RegExp(tela) }).click()
}

test('telemetria mostra números reais e execuções por squad', async ({ page }) => {
  await irParaAdmin(page, '')
  await expect(page.getByRole('heading', { name: 'Telemetria & custos' })).toBeVisible()
  await expect(page.getByText('squads ativadas')).toBeVisible()
  // Seed do harness: 2 runs (editorial + juridico).
  await expect(page.getByText('execuções por squad')).toBeVisible()
  // Custo agora é estimativa real (tokens × tabela de preços), com nota honesta.
  await expect(page.getByText('custo estimado (USD)')).toBeVisible()
  await expect(page.getByText(/Custo é uma.*estimativa/)).toBeVisible()
})

test('ledger lista entradas reais e verifica a integridade sob demanda', async ({ page }) => {
  await irParaAdmin(page, 'Ledger')
  await expect(page.getByText('integridade ainda não verificada nesta sessão')).toBeVisible()
  await page.getByRole('button', { name: 'verificar integridade' }).click()
  await expect(page.getByText(/✓ cadeia íntegra/).first()).toBeVisible({ timeout: 10_000 })
})

test('providers reflete o env real (1 configurado, 2 sem key) + tiers', async ({ page }) => {
  await irParaAdmin(page, 'Providers')
  await expect(page.getByText('Anthropic')).toBeVisible()
  await expect(page.getByText('configurado', { exact: true })).toHaveCount(1)
  await expect(page.getByText('sem key', { exact: true })).toHaveCount(2)
  await expect(page.getByText('rate limiting por tier')).toBeVisible()
})

test('permissões: override real com confirmação, ledger e revogação', async ({ page }) => {
  await irParaAdmin(page, 'Permissões')
  await expect(page.getByRole('heading', { name: /Permissões/ })).toBeVisible()
  // Matriz real dos perfis BUILD/PLAN.
  await expect(page.getByText('bash', { exact: true })).toBeVisible()

  // Clique numa célula → modal de confirmação explícito (nunca opaco).
  await page.getByTitle('mudar decisão de webfetch no perfil build').click()
  const modal = page.getByTestId('confirmar-permissao')
  await expect(modal).toContainText('webfetch')
  await modal.getByRole('button', { name: 'Confirmar' }).click()

  // Override aparece na lista e no ledger (permission_rule.set).
  await expect(page.getByText(/overrides persistidos/)).toBeVisible()
  const ledger = await page.request.get('/api/ledger?limit=20')
  const entries = (await ledger.json()) as Array<{ kind: string }>
  expect(entries.some((e) => e.kind === 'permission_rule.set')).toBeTruthy()

  // Revogar limpa (deixa o estado como estava para os outros specs).
  await page.getByRole('button', { name: 'revogar' }).first().click()
  await expect(page.getByText(/overrides persistidos/)).toHaveCount(0)
})

test('modelos: publicar/despublicar persiste e audita', async ({ page }) => {
  await irParaAdmin(page, 'Modelos')
  const linha = page.getByTestId('modelo-video')
  await expect(linha).toContainText('rascunho') // publicado: false no template
  await linha.getByRole('button', { name: 'publicar' }).click()
  await expect(linha).toContainText('publicado', { timeout: 10_000 })

  // Persiste entre reloads (override no BtvStore).
  await page.reload()
  await page.getByRole('button', { name: 'Administração' }).click()
  await page.getByRole('button', { name: /Modelos/ }).click()
  await expect(page.getByTestId('modelo-video')).toContainText('publicado')

  const ledger = await page.request.get('/api/ledger?limit=20')
  const entries = (await ledger.json()) as Array<{ kind: string; payload: { template_id?: string } }>
  expect(entries.some((e) => e.kind === 'btv.template_published' && e.payload.template_id === 'video')).toBeTruthy()
})

test('usuários: perfis locais criam, listam e suspendem', async ({ page }) => {
  await irParaAdmin(page, 'Usuários')
  await expect(page.getByText('o primeiro criado entra como admin')).toBeVisible()
  await page.getByPlaceholder('nome do perfil…').fill('Marina Lopes')
  await page.getByPlaceholder('e-mail (opcional)').fill('marina@exemplo.com')
  await page.getByRole('button', { name: '+ Adicionar perfil' }).click()
  const linha = page.getByTestId('user-1')
  await expect(linha).toContainText('Marina Lopes')
  await expect(linha).toContainText('admin')
  await expect(linha).toContainText('ativo')
  await linha.getByRole('button', { name: /acesso de Marina/ }).click()
  await expect(linha).toContainText('suspenso')
})
