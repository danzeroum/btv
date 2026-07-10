# Padrões da campanha DDD multitenant

> *O rito vale porque funciona nos dois sentidos.*
>
> Os dois momentos mais valiosos da campanha não foram código — foram
> escaladas: corrigir a premissa de que `skills` era um console (#54) e a de que
> os três grandes eram roteadores (#56). Em ambos, o recon provou que o plano
> media a coisa errada, e o rito respondeu consertando o **critério**, por
> escrito e com aceite — não fingindo a métrica. Um processo que só serve para
> confirmar o plano é cerimônia; um que também o corrige é método. Esta é a tese
> da campanha inteira.

Cada padrão abaixo tem uma **frase canônica** (o que carregar na memória), o
**episódio de origem** (onde nasceu, para poder reler o caso real) e **quando se
aplica**. São material de onboarding: a próxima campanha começa daqui.

---

## 1. Os três atos do estrangulamento

**Frase:** *O juiz existe antes da mudança que ele julga; o commit vermelho
declarado é o preço da regravação auditável.*

**Origem:** C3.1, o primeiro endpoint estrangulado (#31–#33). Formalizado em
todos os `btv.*`: ato 1 pina o corpo legado no golden **antes** de tocar o
handler; ato 2 estrangula (golden vermelho DE PROPÓSITO, declarado no commit);
ato 3 regrava a fixture isolada (uma linha `tenant`).

**Quando:** trocar um emissor/handler legado por uma porta, com um golden
vigiando o contrato. Nunca regravar e estrangular no mesmo commit — a regravação
tem que ser visível e isolada.

## 2. Prova-que-morde

**Frase:** *Um critério que não pode falhar não é critério.*

**Origem:** os goldens da Trilha T (#15) e a varredura de borda (E1s.4, #42).
Todo guarda mecânico é acompanhado da prova de que ele reprova sobre uma
violação real (golden mutado → vermelho; canário fora da allowlist → exit 1).

**Quando:** ao adicionar qualquer lint, golden, ou checagem de CI. Um guarda
verde que nunca foi visto vermelho é decorativo até prova em contrário.

## 3. A prova-que-morde envelhece com o progresso

**Frase:** *Cobaia consumida não aposenta a prova — re-mira na superfície
remanescente.*

**Origem:** C3.4a (#48). Estrangular a última rota sem extractor esvaziou a
cobaia da prova-que-morde da borda (`GET /api/btv/users` deixou de vazar); a
mordida re-mirou no `.fallback()` do SPA, o vazamento que resta por construção.

**Quando:** um guarda cujo espécime de teste é fechado pelo próprio progresso
que ele vigia. A pergunta certa é sempre "qual é o vazamento que AINDA existe por
construção?".

## 4. Helper multi-consumidor não se duplica

**Frase:** *Vai para o dono do tipo; "qual commit sou eu" em duas cópias é o bug
mais bobo esperando divergência.*

**Origem:** C4-2 (`git_sha` → `btv-verify`, #52) e C4-3 (leitores de config →
`btv-tools`, #54). Regra: leitor puro vai para o dono do TIPO que ele produz;
participante de política fica com o motor.

**Quando:** um helper usado por consumidores que se dividem entre crates. Nunca
duplicar; dar-lhe o lar compartilhado (o crate abaixo dos consumidores, dono do
conceito), export mínimo, decisão declarada no PR.

## 5. Juízes acompanham os julgados

**Frase:** *Um golden testando através de fronteira de crate é acoplamento novo
disfarçado de continuidade.*

**Origem:** C4 (#51–#55). Ao mover um módulo, seus `#[cfg(test)]` movem junto; o
sweep da borda fica com a borda, não com a rota que ele testa.

**Quando:** mudar um módulo de endereço. O teste vai com o código que ele julga.

## 6. Lição que morde duas vezes vira script

**Frase:** *Juiz mecânico > disciplina lembrada.*

**Origem:** C4-2 (#52). O mascaramento de exit-code por pipe (`... | tail && echo
OK` devolve o exit do `tail`) foi diagnosticado na E1s.3 e mordeu de novo dois
ciclos depois. Virou o alvo `just preflight` (#53): o pré-push canônico com exit
direto por construção, dogfood do `btv verify` do produto.

**Quando:** qualquer gate crítico que dependa da memória do operador falhar mais
de uma vez. Não se registra a lição — mecaniza-se, sem pipe entre o gate e a
decisão.

## 7. Contexto nasce na borda, nunca fabricado

**Frase:** *Contexto capturado da requisição, não derivado dos dados que ele
governa.*

**Origem:** E1s (o extractor de `TenantContext`, #40) e C3.4b (`registrar_entregas`,
#49) — a primeira operação de background: o `ctx` é clonado no spawn da task de
conclusão, nunca fabricado do `run.tenant` carregado depois. A leitura pela porta
(`RunRepository::get(ctx, …)`) torna o fail-closed de graça.

**Quando:** precisar de `TenantContext` fora do escopo de uma requisição
(webhook, cron, fila, task assíncrona). Captura na origem; proveniência = tenant
E actor de quem originou o trabalho.

## 8. Movimento puro não carrega redesenho de carona

**Frase:** *Se a separação for valiosa, ela merece PR próprio — depois.*

**Origem:** C4 inteiro. Mudança de endereço não regrava fixture nem separa
helper de console de brinde; qualquer regravação num PR de movimento puro é
achado, não carona.

**Quando:** mover/renomear código. A tentação de "aproveitar e limpar" é
diferida com gatilho, não anexada ao movimento — cada merge com uma razão só.

## 9. "Nada Fake" vale para METAS, não só para dados

**Frase:** *Conserta-se o critério — por escrito, com aceite — quando a
investigação prova que ele media a doença errada.*

**Origem:** C4/ADR 0031 (#56). Cumprir o T4 literal ("btv-cli não importa axum")
arrastaria o motor do produto para o dashboard: atingir a métrica traindo o
objetivo. O recon provou que a fronteira de julho (axum vs CLI) media a doença
errada — a real é console vs motor. Redefinir com ADR e guarda ativo é o oposto
de rebaixar a régua.

**Quando:** a satisfação literal de uma meta trairia o objetivo que ela media.
Cláusula que completa a regra "quando o critério morde, conserta-se o código":
*salvo quando a investigação prova que o critério media a doença errada.* A
mesma régua anti-fake dos goldens, apontada para as metas.

## 10. Descope com gatilho, não silêncio

**Frase:** *O que fica é decisão futura com dono, não sobra.*

**Origem:** toda a campanha (`pendencias.md`). Trabalho não feito entra com um
gatilho escrito ("quando X, isto paga a si mesmo") — nunca some silenciosamente
nem vira "quase pronto".

**Quando:** diferir qualquer trabalho. O gatilho transforma "não fiz" em "decidi
não fazer agora, e eis quando reabrir".

## 11. Olha o alvo — escala em vez de prosseguir

**Frase:** *Se não foste tu que criaste, ou se a premissa não bate, para e
escala.*

**Origem:** o commit de merge alheio não amendado (C3.4), e as duas escaladas da
epígrafe (`skills` #54, os três grandes #56). É o padrão que dá à campanha os
seus melhores momentos.

**Quando:** o recon contradiz a premissa do plano, ou a ação tocaria algo que
não é seu. A autoridade de corrigir o plano é do dono; a de apontar que ele
precisa de correção é de quem executa — e exercê-la é o trabalho, não desvio
dele.
