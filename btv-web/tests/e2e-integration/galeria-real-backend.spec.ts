import { expect, test } from '@playwright/test'

// Galeria (U1) + Wizard (U2) contra o `btv dashboard` REAL: os 12 modelos
// vêm de GET /api/btv/templates (squad-template.v1 embutido no binário),
// nunca de um catálogo duplicado no cliente.

test.describe('galeria e wizard com backend real', () => {
  test('galeria carrega os 12 modelos reais com filtro por categoria', async ({ page }) => {
    await page.goto('/')
    // Os 12 cards do contrato, na ordem da galeria.
    for (const id of [
      'editorial', 'pesquisa', 'bi', 'operacoes', 'sales', 'imagem',
      'educacao', 'design', 'juridico', 'musica', 'podcast', 'video',
    ]) {
      await expect(page.getByTestId(`card-${id}`)).toBeVisible()
    }
    // Conteúdo real do template no card (papéis e formatos do JSON).
    const editorial = page.getByTestId('card-editorial')
    await expect(editorial).toContainText('Fact-checker')
    await expect(editorial).toContainText('DOCX')

    // Filtro por categoria: "Análise" deixa só pesquisa e bi.
    await page.getByRole('button', { name: 'Análise', exact: true }).click()
    await expect(page.getByTestId('card-pesquisa')).toBeVisible()
    await expect(page.getByTestId('card-bi')).toBeVisible()
    await expect(page.getByTestId('card-editorial')).toHaveCount(0)
  })

  test('clicar num modelo abre o wizard com as perguntas da área', async ({ page }) => {
    await page.goto('/')
    await page.getByTestId('card-musica').click()
    const wizard = page.getByTestId('wizard-overlay')
    await expect(wizard).toBeVisible()
    await expect(wizard).toContainText('Montar squad · Música simbólica')
    // Perguntas na linguagem da área (do template real, não genéricas).
    await expect(wizard).toContainText('Forma e caráter')
    await expect(wizard).toContainText('Instrumentação')

    // Passo 2 — equipe com toggles.
    await wizard.getByRole('button', { name: 'Continuar →' }).click()
    await expect(wizard).toContainText('Compositor')
    await expect(wizard).toContainText('Desligue papéis que você mesmo fará.')
    await wizard.getByRole('button', { name: /Papel Copista/ }).click()

    // Passo 3 — formatos e gates do template.
    await wizard.getByRole('button', { name: 'Continuar →' }).click()
    await expect(wizard).toContainText('MusicXML')
    await expect(wizard).toContainText('Aprovar o rascunho antes da revisão')
    // Ativação real ligada (Onda 3) — exercitada em squad-real-backend.spec.ts.
    await expect(wizard.getByRole('button', { name: '⚑ Ativar squad' })).toBeEnabled()

    // Voltar preserva o fluxo; fechar sai do overlay.
    await wizard.getByRole('button', { name: '← Voltar' }).click()
    await expect(wizard).toContainText('Desligue papéis')
    await wizard.getByRole('button', { name: 'Fechar wizard' }).click()
    await expect(page.getByTestId('wizard-overlay')).toHaveCount(0)
  })

  test('referências do briefing viram chips removíveis', async ({ page }) => {
    await page.goto('/')
    await page.getByTestId('card-editorial').click()
    const wizard = page.getByTestId('wizard-overlay')
    await wizard.getByPlaceholder('cole um link de referência…').fill('https://exemplo.com/pauta')
    await wizard.getByRole('button', { name: 'adicionar' }).click()
    await expect(wizard).toContainText('↗ https://exemplo.com/pauta')
    await wizard.getByRole('button', { name: /Remover referência/ }).click()
    await expect(wizard).not.toContainText('exemplo.com/pauta')
  })
})
