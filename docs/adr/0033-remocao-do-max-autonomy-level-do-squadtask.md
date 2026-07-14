# ADR 0033 — remoção do campo `max_autonomy_level` do `SquadTask` (quebra de wire assinada)

- Status: proposto (implementação nesta PR; ratificação = merge do dono, por ser
  contrato — quebra de wire assinada exige sign-off, como o ADR 0030). O `buf
  breaking` do CI acusa por DESIGN; não é auto-mergeável verde.
- Data: 2026-07-14

## Contexto

O `SquadTask` (`schemas/proto/squad.proto`) carregava o campo
`uint32 max_autonomy_level = 4`. Desde a Fase 7 (ADR 0021) esse campo é
**ignorado ponta-a-ponta**: o orquestrador Python (`btv_squad/server.py::
ExecuteTask`) nunca lê `request.max_autonomy_level`; a autonomia real é decidida
**por agente**, via `ProgressiveAutonomyManager`/`agent_trust_scores`
(`hitl.py`), desconectada de qualquer teto de tarefa. Os produtores Rust
(`squad.rs`, `squad_agent.rs`) mandavam um `3` hardcoded; a tela Modelo do
console exibia `AUTONOMY_LEVELS` como texto **didático**, sem nunca enviar nada.

Ou seja: o campo era uma **mentira de contrato** — trafegava no wire mas não
tinha efeito. A `docs/documentacao/diagramas/09-analise-critica.md §9.2` e o
`docs/ROADMAP-MELHORIAS.md` (item L2) registraram a escolha binária: **wirear a
autonomia progressiva de verdade** ou **remover o campo numa janela de breaking**.

Decisão do dono (nesta campanha do roadmap): **remover**. É mais honesto — nada
finge existir — e a infraestrutura de autonomia real já vive por agente, sem
depender deste campo.

## Decisão

Remover `max_autonomy_level` do `SquadTask`, com a higiene protobuf correta:

```proto
reserved 4;
reserved "max_autonomy_level";
```

`reserved` impede que a tag `4` ou o nome sejam reutilizados por um campo futuro
com semântica diferente (o erro clássico de evolução de proto). Os stubs são
regenerados dos dois lados (Rust via `tonic-build`; Python via
`scripts/gen_proto_py.py`), e todos os sítios de construção Rust
(`squad.rs`, `squad_agent.rs`, `squad_e2e.rs`) deixam de setar o campo. Os
comentários do console (`web/`) passam a dizer que o campo foi removido.

## Consequências

- **Quebra de wire ASSINADA.** Como o ADR 0030 (evidência tipada), esta é uma
  quebra deliberada: o job `buf` do CI (`buf breaking … --against main`, categoria
  `FILE`) **acusa a remoção por design** — é o sinal de que a quebra é
  intencional, não silenciosa. Portanto o PR **não fica verde no `buf`** e **não
  é auto-mergeável**: exige sign-off do dono, na disciplina de "contrato = merge
  do dono". Rust e Python vivem no mesmo repo e sobem juntos (janela de deploy
  coordenada), então não há skew de wire em produção local-first.
- **ADR 0021 fica superseded na prática** para este campo: o "débito consciente"
  deixa de existir porque o campo deixa de existir. A autonomia progressiva real
  (por agente) permanece intacta e inalterada.
- **Sem perda de função:** nada lia o campo; remover não muda comportamento
  observável do squad — só limpa o contrato.
- **Reabertura:** se um dia a autonomia por-tarefa virar requisito real, entra
  como um campo NOVO (tag ≠ 4) com semântica lida de verdade pelo orquestrador —
  aditivo, sem reviver a tag reservada.
