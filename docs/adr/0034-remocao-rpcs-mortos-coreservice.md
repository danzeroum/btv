# ADR 0034 — remoção dos RPCs mortos do `CoreService` (`AppendLedger`/`Recall`/`Remember`)

- Status: proposto (implementação nesta PR; ratificação = merge do dono, por ser
  contrato — quebra de wire assinada, como os ADRs 0030 e 0033). O `buf breaking`
  do CI acusa por DESIGN; não é auto-mergeável verde.
- Data: 2026-07-14

## Contexto

O `CoreService` (`schemas/proto/core.proto`) é o canal reverso onde o **Rust
serve** e o **Python chama** — o sidecar nunca fala com provedor LLM nem toca
disco direto; tudo passa pelo core (keys, permissões, ledger). Três RPCs desse
serviço eram **stubs mortos** desde a Fase 4:

- `AppendLedger(LedgerAppend) → LedgerAck`
- `Recall(RecallRequest) → RecallResponse`
- `Remember(RememberRequest) → RememberAck`

O `crates/btv-sidecar/src/core_server.rs` respondia `Status::unimplemented` para
os três, e **nada os chamava** (nem Python, nem Rust — verificado). Eram a
**direção errada** para memória: memória do squad vive no Python (corpus
episódico + índice TF-IDF em `btv_squad`), então quem deve **servir** memória é o
Python, não o Rust. Isso foi resolvido pelo **`MemoryService`** (`memory.proto`,
Python serve / Rust chama; ADR 0022). O próprio `memory.proto` documenta:
"CoreService.Recall/Remember … ficam Unimplemented e não são usados — são a
direção errada, um stub abandonado da Fase 4."

Manter RPCs `Unimplemented` no contrato é uma **mentira de contrato**: confunde
quem lê o `.proto` (parece que há memória/ledger reverso disponível). A
`docs/documentacao/diagramas/09-analise-critica.md §9.2` registrou a limpeza
como oportunidade ("remover num `.v2` com ADR").

## Decisão

Remover os 3 RPCs do `CoreService` e as 6 mensagens usadas **só** por eles —
`LedgerAppend`, `LedgerAck`, `RecallRequest`, `RecallResponse`, `RememberRequest`,
`RememberAck` (todas em `btv.core.v1`). Ficam `Generate`, `RunTool`,
`RequestPermission` e as mensagens `ToolCall`/`ToolResult`/`PermissionRequest`/
`PermissionDecision`.

Segue o **precedente do ADR 0033** (remoção in-place assinada), **não** um
`core.v2` — duplicar o serviço inteiro para tirar stubs mortos é desproporcional.

Notas técnicas:

- **Protobuf não reserva RPCs de serviço** (só campos/valores de enum dentro de
  mensagem). Os RPCs são simplesmente deletados; as 6 mensagens são deletadas
  inteiras (não há campo a reservar).
- **Sem colisão com o `MemoryService`:** `memory.proto` define seus PRÓPRIOS
  `RecallRequest`/`RecallResponse` no pacote `btv.memory.v1` (campos `{query,k}`
  e `{repeated MemoryMatch}`), distintos dos de `core.proto` (`{agent,query,
  limit}` e `{repeated string}`). Deletar os do `core.proto` não toca a memória.

Stubs regenerados dos dois lados (`tonic-build`; `scripts/gen_proto_py.py`); as
3 impls `unimplemented` saem do `core_server.rs`.

## Consequências

- **2ª quebra de wire ASSINADA** da campanha do roadmap (depois da L2/ADR 0033).
  O job `buf` do CI (`buf breaking … --against main`, categoria `FILE`) **acusa a
  deleção de RPC + de mensagem por design** — o PR **não fica verde no `buf`** e
  **não é auto-mergeável**: exige sign-off do dono ("contrato = merge do dono").
  Rust e Python vivem no mesmo repo e sobem juntos (janela coordenada), sem skew
  de wire em produção local-first.
- **Sem perda de função:** nada chamava os 3 RPCs; remover não muda comportamento
  observável — só limpa o contrato. A memória real segue pelo `MemoryService`.
- **Reabertura:** se um dia o Rust precisar servir memória/ledger ao Python (o que
  contraria a fronteira atual), entram RPCs NOVOS com semântica lida de verdade —
  aditivo, sem reviver os nomes removidos.
