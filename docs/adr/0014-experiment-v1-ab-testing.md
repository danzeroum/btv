# ADR 0014 — `experiment.v1`: relatório de A/B testing, Rust-only, veredito honesto

- Status: aceita
- Data: 2026-07-06

## Contexto

A Onda 7 realiza o critério de conclusão nº 2 da fase: o A/B testing gera
relatório. Isso introduz um schema novo (`experiment.v1`) e duas perguntas de
contrato: (1) o relatório vive no Rust ou no Python; (2) como a significância
estatística é computada sem inventar vencedor. A telemetria (`forge-store`,
`telemetry-event.v1`) já aceita `props` arbitrários — a base do A/B — mas seu
`summary` só agrupa por nome, não fatia por props.

## Decisão 1 — o relatório é Rust; sem paridade Python

O relatório A/B vive em Rust (`forge experiment` em `forge-cli`, tipos em
`forge-schemas/src/experiment.rs`, agregação em `forge-store::telemetry`). É
**agregação determinística** sobre a telemetria SQLite (Rust-owned: "storage" é
Rust pela ADR 0001), **não raciocínio de agente** — mesma natureza de
`summary`/`dashboard`/`verify`. Uma consulta nova (`experiment_variants`) agrupa
por variante via `json_extract` (extensão JSON1 do SQLite bundled). Atribuição por
`props.experiment` / `props.variant` / `props.success` — sem mudar a escrita.

Sobre paridade cross-language: pela regra de contrato (CLAUDE.md), **só**
`prompt-cache-key.v1` exige implementação dupla (Rust×Python), por ser o único
contrato com dois produtores independentes que precisam bater byte-a-byte.
`experiment.v1` segue o precedente `telemetry-event.v1`: schema hand-written + tipo
`schemars` + fixture golden, **sem consumidor Python** a parear. `gen_fixtures.py`
**não** é tocado.

## Decisão 2 — significância hand-rolled; veredito derivado dos dados

Não há crate de estatística no workspace. O **teste z de duas proporções**
(variância pooled) é hand-rolled em Rust puro, com CDF normal via aproximação de
`erf` (Abramowitz-Stegun 7.1.26, |erro| ≤ 1.5e-7) — suficiente para um p-valor de
decisão (precedente de matemática pequena embutida: `cache_hit_rate`,
`derive_verdict`). O veredito é **derivado dos dados**, com três estados honestos:
`Significant` (p<α, **com** vencedor = maior taxa), `Inconclusive` (amostra ok mas
sem diferença — **sem vencedor**), `InsufficientData` (< `MIN_SAMPLES` por
variante). O vencedor **só** existe quando há significância — nunca fabricado. É a
régua "Nada Fake" aplicada a estatística.

## O que foi provado, não só declarado

- Com telemetria semeada e rodando o binário: `exp-sig` (A 90% × B 50%) →
  "VENCEDOR A (p≈7e-10)"; `exp-tie` (A 50% × B 52%) → "SEM SIGNIFICÂNCIA (p=0.78) —
  sem vencedor". O gêmeo honesto prova que variantes empatadas não produzem
  conclusão fabricada.
- Fixture golden `experiment.v1` valida contra o schema e desserializa no tipo
  Rust; o caso inválido (sem `verdict`) reprova o schema.

## Consequências

- A/B multivariante (>2 variantes, com correção de comparações múltiplas) e outras
  métricas (latência P95, custo) ficam como extensão futura; hoje é 2 variantes
  sobre `success_rate`.
- O relatório pode ser registrado no ledger (payload opaco, como a certificação da
  Fase 5) para auditoria — produzido em Rust, registrável.
