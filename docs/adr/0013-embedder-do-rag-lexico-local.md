# ADR 0013 — Embedder do RAG: recuperação léxica local (TF-IDF), zero-dependência

- Status: aceita
- Data: 2026-07-06

## Contexto

A Onda 6 dá recuperação semântica ao `recall` do squad. O `AgentMemorySystem.
recall_similar` era um **no-op na prática**: o `_FallbackCollection.query`
(`python/packages/forge-squad/src/forge_squad/memory.py`) devolvia listas vazias
sempre, e `chromadb` nunca foi dep declarada. A pergunta de contrato (PLANO §3):
qual embedder — **local vs API** — e onde vive o índice. Um dado duro pesou: o
ambiente Python **não tem nenhuma lib de ML** (sem numpy/sklearn/sentence-
transformers/torch/chromadb).

## Decisão — TF-IDF léxico em Rust... não: em Python puro (stdlib), local e offline

O retriever (`python/packages/forge-squad/src/forge_squad/recall.py`) é um índice
**TF-IDF esparso em Python puro** (só stdlib), ranqueado por cosseno, sobre o
corpus episódico persistido. Razões:

- **Offline-first de verdade:** nada sai da máquina, sem baixar modelo. Coerente
  com o princípio do produto.
- **Zero-dependência:** não infla o `uv.lock` nem arrisca supply-chain (a mesma
  disciplina do framing LSP hand-rolled da Onda 5).
- **A fronteira (ADR 0001) permite:** a regra proíbe o Python chamar *provedores
  LLM* / ter API keys — não proíbe **computação local**. Um embedder local não
  cruza essa fronteira; só um embedder **por API** teria que passar pelo gateway
  Rust (`CoreService.Generate`).

**Honestidade sobre o limite (a régua "Nada Fake"):** é recuperação **léxica**
(termos distintivos, com stopwords removidas e termos ubíquos descontados pelo
IDF), **não neural** (não faz ponte de sinônimo/paráfrase). O índice é derivado do
corpus persistido (`.forge/squad-memory/agent_memories.jsonl`) e reconstruído por
consulta (corpus pequeno); funciona entre sessões e dentro da sessão.

## O que foi provado, não só declarado

- Com **ground truth** de dois tópicos de vocabulário disjunto, a consulta de um
  tópico recupera **exatamente** as memórias daquele tópico — igualdade de
  conjunto, não "retornou algo". E o caminho antes-vazio (no-op) agora recupera as
  certas.
- O limite léxico foi *exposto por um teste que falhou de início*: uma consulta com
  "contêiner/docker" não casava a memória "sandbox" (sinônimos). A ground truth foi
  reescrita para relevância determinável lexicalmente — documentando o limite em
  vez de escondê-lo.

## Consequências

- Embeddings **neurais** (semântica de sinônimo) ficam para uma onda futura —
  exigiriam bundlar um modelo local (conflita com leveza/offline) ou rotear pelo
  gateway Rust (uma chamada de rede por recall). Decisão consciente, não omissão.
- O consumo do contexto recuperado no *planejamento* do squad (hoje o orquestrador
  registra a contagem do recall, agora real) fica como follow-up — é decisão de
  raciocínio do squad, fora da fronteira desta onda (correção da recuperação).
- O scaffolding `chromadb` (inativo) permanece como sink alternativo para um futuro
  vector DB real; o recall não depende mais dele.
