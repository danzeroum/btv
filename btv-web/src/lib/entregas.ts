/** Uma run "concluída" que NÃO gerou nenhuma entrega real — isto é, nenhum
 *  arquivo gravado por ferramenta (`edit` exit 0) foi capturado pelo backend.
 *  Acontece quando o modelo apenas NARRA em prosa que "gravou o arquivo" sem
 *  chamar a ferramenta de escrita (ex.: um modelo fraco, ou uma tarefa sem
 *  ferramenta de renderização real). A tela usa isto para avisar em vez de
 *  apontar para uma Biblioteca vazia — honestidade "Nada Fake". */
export function runSemArtefatoReal(status: string, numEntregas: number): boolean {
  return status === 'concluida' && numEntregas === 0
}
