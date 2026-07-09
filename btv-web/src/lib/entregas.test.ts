import { describe, expect, it } from 'vitest'
import { runSemArtefatoReal } from './entregas'

describe('runSemArtefatoReal', () => {
  it('concluída com 0 entregas → true (artefato só narrado, não gravado)', () => {
    expect(runSemArtefatoReal('concluida', 0)).toBe(true)
  })
  it('concluída com entregas reais → false', () => {
    expect(runSemArtefatoReal('concluida', 2)).toBe(false)
  })
  it('só avisa em concluída — outros status → false', () => {
    expect(runSemArtefatoReal('ativa', 0)).toBe(false)
    expect(runSemArtefatoReal('erro', 0)).toBe(false)
    expect(runSemArtefatoReal('encerrada', 0)).toBe(false)
  })
})
