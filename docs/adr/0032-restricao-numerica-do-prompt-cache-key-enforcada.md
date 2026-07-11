# ADR 0032 — restrição numérica do `prompt-cache-key.v1` enforçada (rejeição de floats com fração zero)

- Status: proposto (implementação nesta PR; ratificação = merge do dono, por ser
  contrato — o auto-merge da rodada de qualidade foi exceção com escopo, não vale
  para contrato/ADR).
- Data: 2026-07-11

## Contexto

O contrato `prompt-cache-key.v1` (herdado do prompte, `api/src/hash.js`) tem
implementação dupla — `crates/btv-schemas/src/canonical.rs` (Rust) e
`python/packages/btv-promptforge/src/btv_promptforge/hashing.py` (Python) — cuja
paridade é garantida pelas fixtures em `schemas/fixtures/prompt-cache-key.v1.json`.

Os dois lados documentavam, **só em prosa**, uma restrição: floats com parte
fracionária zero (ex.: `1.0`) são proibidos nas entradas, porque o produtor JS
os serializa como `1` enquanto Rust/Python emitem `1.0`. Nada enforçava a regra:
`request_hash(messages, temperature)` aceitava `temperature=1.0` e produzia,
silenciosamente, uma chave que **diverge** da que o produtor JS geraria. O
sintoma é um cache-miss cross-produtor silencioso — custo real de API pago sem
sinal. Não é incêndio (hoje o gateway Rust e o sidecar Python usam a MESMA impl,
parity-testada, então concordam entre si), mas tem vencimento: cada novo produtor
de cache-key aumenta o custo do buraco.

Auditoria da rodada de qualidade aprovou fechá-lo como "o próximo trabalho de
mérito" (único deferido com impacto de *correção*), no rito completo: ADR +
validador compartilhado + regenerar fixtures pelo método não-circular + os dois
testes de paridade como juízes.

## Decisão

1. **Enforçar por rejeição, não por normalização.** O v1 *proíbe* a entrada; não
   a reescreve. Um validador compartilhado (`reject_forbidden_numbers`)
   recusa, recursivamente por todo o valor:
   - floats com `fract() == 0` / `is_integer()` (ex.: `1.0`, `0.0`, `3.0`);
   - números não-finitos (NaN/Inf).
   Exposto como `validate_cache_key(messages, temperature)` nos dois lados, e
   chamado no início de `request_hash`.
2. **`request_hash` passa a poder falhar.** Rust: `-> Result<String, CacheKeyError>`
   (erro tipado, novo). Python: levanta `CacheKeyError`. É mudança de assinatura
   **interna ao workspace** (btv-schemas é crate de path, não publicado) — sem
   impacto de wire/proto. Os 2 chamadores Rust foram atualizados:
   `LlmRequest::cache_key` e `CachedGenerator::cache_key` propagam o `Result`; o
   `CachedGenerator` **degrada pulando o cache** quando a chave é proibida (um
   request sem chave válida não é cacheável — melhor que gerar chave divergente).
3. **`canonical_json` fica intacto.** O guard vive só no `request_hash` (o
   contrato do cache-key), não no `canonical_json` — que também serializa corpos
   do ledger (`btv-schemas::ledger`) e evidências de certificação
   (`btv_review.certification`), onde a restrição numérica do cache-key não se
   aplica.
4. **Fixtures ganham `reject_cases`.** Além dos `cases` válidos (hashes
   inalterados — o guard não muda a serialização de entrada válida), a fixture
   agora tem `reject_cases`: entradas proibidas representáveis em JSON
   cross-language (floats de fração zero). NaN/Inf **não** entram na fixture —
   não são JSON válido e o `serde_json` nem constrói um `Number` não-finito
   (`from_f64` → `None`), então o caso não-finito é coberto por teste inline do
   lado Python (onde `float('inf')` é um float real). `gen_fixtures.py`
   auto-verifica que a impl de referência recusa cada `reject_case`.

## Consequências

- **Correção:** entradas que divergiriam entre produtores param de gerar chave
  silenciosamente; no runtime, degradam para "sem cache" em vez de "cache
  divergente". Fail-closed coerente com o resto da plataforma.
- **Paridade preservada e ampliada:** os hashes válidos são byte-idênticos
  (regeneração não-circular confirmou); os dois testes de paridade agora também
  provam que ambos os lados **recusam** os mesmos `reject_cases`.
- **Superfície:** +1 tipo público (`CacheKeyError`) e +1 função
  (`validate_cache_key`) em cada lado; `request_hash` vira falível. Chamadores
  internos atualizados; nenhum contrato de wire/proto muda.
- **Trade-off:** um caller que hoje passe `temperature=1.0` deixa de cachear
  aquele request (antes cacheava com chave divergente). É o comportamento
  correto sob o v1; se algum caller depende de `1.0`, deve passar `1` (int) ou
  um decimal não inteiro.
