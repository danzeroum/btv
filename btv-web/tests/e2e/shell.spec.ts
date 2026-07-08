import { expect, test } from '@playwright/test'

test.describe('shell BuildToValue (Onda 1)', () => {
  test('abre na galeria com topbar, sidebar e tokens do handoff', async ({ page }) => {
    await page.goto('/')
    await expect(page.getByText('BuildToValue', { exact: true })).toBeVisible()
    await expect(page.getByRole('heading', { name: 'Monte uma squad, receba entregas' })).toBeVisible()
    await expect(page.getByText('perfil usuário · U1')).toBeVisible()

    // Tokens: fundo geral --paper (#efe9de) aplicado no root.
    const bg = await page
      .locator('#btv-root')
      .evaluate((el) => getComputedStyle(el).getPropertyValue('--paper').trim())
    expect(bg).toBe('#efe9de')

    // Sidebar do perfil usuário: 5 itens, sem seção de squad ativa.
    for (const label of ['Início', 'Minhas squads', 'Personas', 'Biblioteca', 'Designer']) {
      await expect(page.getByRole('button', { name: new RegExp(label) })).toBeVisible()
    }
    await expect(page.getByText('squad ativa')).toHaveCount(0)
  })

  test('toggle de perfil troca a navegação e a tela default', async ({ page }) => {
    await page.goto('/')
    await page.getByRole('button', { name: 'Administração' }).click()
    await expect(page.getByRole('heading', { name: 'Telemetria & custos' })).toBeVisible()
    for (const label of ['Telemetria', 'Ledger', 'Providers', 'Permissões', 'Modelos', 'Usuários']) {
      await expect(page.getByRole('button', { name: new RegExp(label) })).toBeVisible()
    }
    await page.getByRole('button', { name: 'Meu espaço' }).click()
    await expect(page.getByRole('heading', { name: 'Monte uma squad, receba entregas' })).toBeVisible()
  })

  test('navegação por item da sidebar muda a tela', async ({ page }) => {
    await page.goto('/')
    await page.getByRole('button', { name: /Biblioteca/ }).click()
    await expect(page.getByRole('heading', { name: 'Biblioteca de entregas' })).toBeVisible()
    await expect(page.getByText('perfil usuário · U4')).toBeVisible()
  })

  test('ajustes rápidos: swatch muda --brand na hora e persiste', async ({ page }) => {
    await page.goto('/')
    await page.getByRole('button', { name: 'Ajustes rápidos' }).click()
    await expect(page.getByTestId('gear-drawer')).toBeVisible()

    await page.getByRole('button', { name: 'Cor da marca #2b4a8c' }).click()
    const brand = await page
      .locator('#btv-root')
      .evaluate((el) => getComputedStyle(el).getPropertyValue('--brand').trim())
    expect(brand).toBe('#2b4a8c')

    // Persiste entre reloads (localStorage).
    await page.reload()
    const brandAfter = await page
      .locator('#btv-root')
      .evaluate((el) => getComputedStyle(el).getPropertyValue('--brand').trim())
    expect(brandAfter).toBe('#2b4a8c')
  })

  test('atalho do drawer navega para Personas', async ({ page }) => {
    await page.goto('/')
    await page.getByRole('button', { name: 'Ajustes rápidos' }).click()
    await page.getByRole('button', { name: /Personas & prompts/ }).click()
    await expect(page.getByRole('heading', { name: 'Personas & prompts' })).toBeVisible()
    await expect(page.getByTestId('gear-drawer')).toHaveCount(0)
  })
})
