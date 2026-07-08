import { useLayoutEffect, type RefObject } from 'react'

/** Aplica a cor de marca escolhida no ⚙ sobre os tokens `--brand`/`--brandink`
 *  do root — mesmo mecanismo do protótipo (`applyBrand`): a sobreposição vale
 *  para as duas variáveis (o hover herda a própria cor). `null` remove a
 *  sobreposição e devolve os valores padrão de global.css. */
export function useBrand(rootRef: RefObject<HTMLElement | null>, accent: string | null) {
  useLayoutEffect(() => {
    const el = rootRef.current
    if (!el) return
    if (accent) {
      el.style.setProperty('--brand', accent)
      el.style.setProperty('--brandink', accent)
    } else {
      el.style.removeProperty('--brand')
      el.style.removeProperty('--brandink')
    }
  }, [rootRef, accent])
}
