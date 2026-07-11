/** Formata um timestamp ISO em `HH:MM` (fallback: primeiros 5 chars).
 *  Extraído de `esteira.ts`/`SquadRunContext.tsx`, onde estava duplicado. */
export function hhmm(ts: string): string {
  const m = ts.match(/T(\d{2}):(\d{2})/)
  return m ? `${m[1]}:${m[2]}` : ts.slice(0, 5)
}
