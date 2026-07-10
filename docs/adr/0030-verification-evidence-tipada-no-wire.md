# ADR 0030 — evidência de verificação TIPADA no wire (o break assinado da janela G3)

- Status: proposta (aguardando o aceite do dono no PR — este ADR É o
  reconhecimento explícito do break; ver "Decisão")
- Data: 2026-07-10

## Contexto

O `SquadTask.verification_evidence_json` (`schemas/proto/squad.proto`) carregava
a `verification-evidence.v1` como **string JSON opaca**: o Rust
(`btv-cli::squad`) fazia `serde_json::to_string(&evidence)`, o Python
(`btv_squad/server.py::_parse_verification_evidence`) fazia `json.loads` para um
`dict[str, Any]` validado à mão com `isinstance`. Duas pontas do MESMO contrato
(`schemas/json/verification-evidence.v1.schema.json`) sem que o wire soubesse a
forma — exatamente o tipo de "campo viajou como texto, torça para ser o shape
certo" que o resto da campanha DDD eliminou nos outros contratos.

Este é o **único breaking de wire da campanha inteira** (D3, trilha do
`PLANO-DDD-MULTITENANT`), deliberadamente reservado para a janela **G3** — o
único ponto em que Rust e Python mudam o contrato JUNTOS. A regra de fronteira
do projeto (CLAUDE.md) diz "mudança breaking = novo arquivo `.v2` + ADR novo;
protos evoluem só aditivamente". A janela G3 é a exceção CONSCIENTE a esse
default aditivo: Rust e Python vivem no mesmo repositório e são lançados juntos,
então "deploy coordenado" aqui é **um merge**, não dois sistemas em produção a
sincronizar. Um `.v2` paralelo com os dois campos coexistindo seria dívida sem
retorno — não há consumidor externo do proto para proteger.

## Decisão

Trocar o campo 5 de `SquadTask` **in-place**:

```proto
- string verification_evidence_json = 5;
+ VerificationEvidence verification_evidence = 5;   // + VerificationStep,
+                                                   //   VerificationFinding, enum Verdict
```

espelhando 1:1 o schema `verification-evidence.v1`. `file`/`line` do achado são
`optional` (proto3) — presença explícita, espelho do `skip_serializing_if =
Option::is_none` do struct canônico `btv_schemas::verification`.

**O break é ASSINADO, não silencioso.** O job `buf` do CI (T5, semana 1) vai
acusar `FIELD_SAME_TYPE`/`FIELD_SAME_NAME` no campo 5 — CORRETAMENTE. Essa é a
função do job: transformar quebra silenciosa em quebra reconhecida. O **aceite
do dono no PR** (com o `buf` vermelho à vista) é a assinatura do break; este ADR
é o registro escrito dele. Não se "conserta" o `buf` — ele fez o trabalho.

Lados:

- **Rust** (`btv-cli::squad::evidence_to_proto`): mapeia o struct canônico →
  mensagem proto. Todos os call sites (`squad.rs`, `squad_agent.rs`, testes
  `squad_e2e.rs`) constroem `Some(VerificationEvidence{…})`/`None` em vez da
  string.
- **Python** (`btv_squad/verification.py`, novo): espelho Pydantic com
  `from_proto` (parse-don't-validate, como o `TenantContext` do D4t) e
  `to_wire_dict` (forma canônica do schema). `server.py` lê pela PRESENÇA
  (`HasField`) e valida CONTRA TIPO — **morre o `dict[str, Any]`**; um veredito
  `UNSPECIFIED`/ausente é recusado fail-closed, nunca vira dict malformado.
- **Fail-closed preservado:** campo não setado → `HasField` falso →
  `(None, True)`; o orquestrador reprova antes de custar uma chamada de LLM,
  exatamente como antes (o `verification_evidence_missing` não muda de
  semântica).
- **Paridade byte-a-byte:** a fixture `schemas/fixtures/verification-evidence.v1.example.json`
  é o juiz compartilhado — o teste Rust (`evidence_to_proto_espelha_a_fixture_v1`)
  e o Python (`test_verification.py`) provam que a evidência tipada reproduz o
  MESMO JSON canônico que a string carregava, então o prompt do auditor não
  muda.

## Não-escopo explícito

- **Downstream continua consumindo o `dict` canônico** (`to_wire_dict`) no
  payload do auditor — o `dict[str, Any]` que MORRE é o do PARSE opaco na borda;
  o dado que desce para o LLM é a forma canônica do schema, byte-idêntica. Tipar
  o orquestrador/auditor ponta-a-ponta ficaria fora do escopo desta onda (mudaria
  o prompt e o risco), e não é necessário para o objetivo (matar o parse
  não-tipado + tipar o wire).
- Nenhuma mudança no schema JSON `verification-evidence.v1` — o proto o
  espelha, não o substitui.
- `max_autonomy_level` segue ignorado ponta-a-ponta (ADR 0021), intocado aqui.

## Consequências

- Um caller Rust ANTIGO (workspace Python desatualizado, ou vice-versa) para de
  falar com o novo até `git pull` — custo: uma mensagem de erro clara de decode
  de proto, não corrupção silenciosa. Como Rust e Python sobem juntos no merge,
  a janela de incompatibilidade é só a de um dev com working tree parcial.
- **Rollback declarado antes do primeiro comando:** o revert do merge restaura
  `string verification_evidence_json = 5` (proto3 tolera a re-troca; o Python
  antigo volta a fazer `json.loads`). Sem migração de dados — o campo é
  transiente (viaja numa chamada RPC, não é persistido).
- Fecha o último item de WIRE da campanha DDD multitenant. O que resta na janela
  G3 é o C4 (mover os módulos axum de `btv-cli` para `btv-server`) — movimento
  estrutural interno, sem breaking de wire, executável como onda.
