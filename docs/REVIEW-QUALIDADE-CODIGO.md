# Review de qualidade de código — BuildToValue (`mix_btv_code`)

> **Modo de entrega:** somente relatório. **Nenhuma** mudança foi aplicada, nada
> foi commitado ou pushado. Os diffs abaixo são propostas **prontas para
> `git apply`**, autoradas contra o `HEAD` atual e revalidadas relendo cada
> arquivo. Se a árvore tiver mudado, use `git apply --3way` (ou `--recount`).
>
> **Cobertura:** Rust (`crates/`), Python (`python/packages/`) e TypeScript
> (`btv-web/`, `web/`).
> **Restrições respeitadas:** sem breaking change de interface pública (todo
> helper novo é interno); comentários/docs/nomes de teste em português,
> identificadores em inglês; nenhum diff viola `scripts/arch-lint.sh`; estilo
> casa com o redor (`thiserror`/`#[cfg(test)]` inline no Rust,
> `from __future__ import annotations` no Python, `oxlint` no TS). Mudanças que
> tocam o contrato `prompt-cache-key.v1` ou config de build entram como
> **recomendação com alerta**, não como diff trivial.

---

## 1. Resumo executivo

O repositório é maduro e, no geral, disciplinado: o domínio é infra-free e
protegido por `arch-lint`; permissões, ledger hash-chain e tenant newtype são
fail-closed e bem testados; os seams de DI em Python (`Protocol` + `Scripted*`/
`Grpc*`, ADR 0005) são um bom exemplo de **DIP**. A dívida técnica **não** está
espalhada — ela se concentra em cinco bolsões, e quase tudo é auto-contido:

| # | Tema | Eixo do ebook | Onde | Severidade |
|---|------|---------------|------|-----------|
| C1 | Erro de store engolido no caminho de ativação | Clean Code / correção | `btv-cli/src/btv_agent.rs:213` | **Alta (correção)** |
| C2 | Regex gulosa `\{.*\}` corrompe parse com prosa à frente | Clean Code / correção | 6 arquivos de agente | **Alta (correção)** |
| C3 | Serialização divergente no guard de segurança (bypass) | Clean Code / segurança | `security.py:48` × `sandbox.py:99` | **Média (segurança)** |
| D1 | Parse JSON duplicado em 6 arquivos | SOLID (SRP) + DRY | `agents/*.py`, `planning.py` | Média |
| D2 | Boilerplate de resposta de erro (~65×) e lock-poison (~31×) | DRY / Design Pattern | `btv_agent.rs` | Média |
| D3 | Loop de retry LSP idêntico 2× + `300ms`/`20ms` mágicos | DRY / Extract Method | `lsp.rs`, `squad_agent.rs` | Média |
| D4 | Boilerplate de loading/error por tela + `hhmm()` duplicada | DRY | `btv-web/src/**` | Média |
| P1 | Corpus + IDF recomputados a cada recall (O(N)/chamada) | Big-O | `recall.py`, `memory.py` | Média (escala) |
| S1 | Mega-funções e god-objects | SOLID (SRP) | `btv_agent.rs`, `orchestrator.py`, `btv.rs`, `ports.rs` | Estrutural |
| G1 | Magic strings/numbers (`"claude-sonnet-5"` ~10×, thresholds) | Clean Code + 12-Factor | Python + TS | Baixa |
| G2 | Config de build frouxa (`strict` off, `oxlint` 2 regras) | 12-Factor / qualidade | `tsconfig.app.json`, `.oxlintrc.json` | Baixa (faseado) |
| L1 | Restrição numérica do hash v1 não é enforçada | Contrato / paridade | `hashing.py` × `canonical.rs` | Latente (recomendação) |

**Cobertura dos 5 princípios SOLID:** SRP (§2.1–2.4), OCP/DIP (seams `Protocol` +
`Scripted*`/`Grpc*`, ADR 0005 — exemplo positivo), **LSP** (§2.5 — a suíte de
contrato dual-adapter `btv-contract`, ADR 0026, é Liskov provado por construção) e
**ISP** (§2.6 — `UserRepository` examinado: tensão leve, sem achado acionável).
Ausência declarada, não silenciosa.

**Governança (alinhado ao repositório que o relatório avalia):** os itens
estruturais (§2.1/§2.3) **não** abrem backlog concorrente — reforçam as pendências
já registradas em `pendencias.md` (`:2182` decomposição dos 3 grandes; `:1829`
decisão B2 da API legada), com o gatilho de "segundo consumidor" que elas já fixam.
As três correções (§5.1 C1, §3.2 `wait_for_socket`, §5.3 C3) **não** entram no lote
mecânico: cada uma pede **PR próprio**, com o delta de comportamento em destaque e a
declaração do juiz — inclusive o C1, cujo caminho de falha **nenhum golden cobre**
(a prova é teste novo no PR). Cada diff aplicável do §6 nomeia o seu juiz (golden/
teste que, verde sem regravação, prova a neutralidade).

As seções 2–5 detalham cada item por eixo do ebook (justificativa + antes/depois
+ trade-off). Seção 6 lista os diffs completos e arquivos novos. Seção 7 traz os
testes. Seção 8, riscos e mitigação.

---

## 2. SOLID — coesão e acoplamento

### 2.1 SRP · `ativar_squad_handler` faz 8+ coisas (estrutural — esboço)

`crates/btv-cli/src/btv_agent.rs:161` tem ~244 linhas cobrindo: lookup de
template, filtro de papéis, carga de overrides/personas, hash de procedência,
montagem do roster, spawn da task, persistência + re-leitura do run, derivação
de evento e append no ledger. Isso é o clássico "handler que virou serviço".

**Justificativa:** uma função com 8 razões para mudar viola SRP — cada motivo
(mudou o formato do roster? mudou o schema do ledger?) força reabrir a mesma
função gigante, e ela é praticamente não-testável em unidade.

**Antes/depois (esboço, não diff aplicável):**

```rust
// depois — o handler orquestra; cada passo é uma fn testável isolada
pub(crate) async fn ativar_squad_handler(...) -> Response {
    let template = match resolver_template(&state, &body.template_id) { ... };
    let roster   = build_roster(&state, &ctx, &template, &body.papeis_off)?;   // personas + hash
    let run      = persist_and_derive_run(&state, &ctx, &template, &roster)?;
    append_activation_ledger(&state, &ctx, &run, &roster)?;
    spawn_and_respond(state, run, roster).await
}
```

**Trade-off:** ~5 funções novas e um struct `Roster` intermediário; +indireção em
troca de testabilidade e coesão. **Custo de refatoração alto** (toca o caminho
crítico de ativação) → recomendado fazer sob `just preflight` + os e2e de squad,
não às cegas. Por isso entra como recomendação, não como patch neste relatório.

> **Backlog único — não abre concorrente.** Este achado **reforça** a pendência
> já registrada em `pendencias.md:2182` (`[residual — decomposição dos 3 grandes,
> projeto pós-campanha]`), que nomeia exatamente `squad_agent`/`btv_agent`/
> `web_agent` como "redesenho de lógica, não movimento de endereço", adiado por
> decisão declarada. O papel deste relatório é só **somar evidência** ao caso
> dela (as ~244 linhas e as 8 responsabilidades enumeradas acima), não criar um
> segundo backlog. O **gatilho** já está fixado lá (`:2189-2190`): *"quando o
> motor precisar de um SEGUNDO consumidor (modo saas em processo separado, ou um
> worker headless), a extração paga a si mesma — até lá é custo sem comprador."*
> Não discordo do gatilho; registro o achado como evidência a favor dele.

### 2.2 SRP · `UnifiedOrchestrator.execute_complex_task` (~140 linhas)

`python/packages/btv-squad/src/btv_squad/orchestrator.py:165` faz roster,
recall, planejamento, propostas, consenso, narração de chat, gate HITL, execução
do plano, gate de verificação fail-closed, validação do auditor, emissão de
evento, registro de autonomia, persistência de memória e montagem do dict de
retorno. Extrair `_run_consensus_gate`, `_finalize_validation`, `_persist_outcome`
deixa o fluxo principal legível em ~30 linhas. Mesmo trade-off do 2.1 (indireção
× coesão/teste); **médio** custo de refatoração — o arquivo já tem boa cobertura.

### 2.3 SRP · `BtvStore`/`PgStore` god-object com API dupla

`crates/btv-store/src/btv.rs` (1612 linhas) tem uma struct dona de 6 agregados
(runs, deliverables, persona overrides, custom personas, publicação de template,
users) e **cada operação é escrita duas vezes**: método inerente
(`set_persona_override` L365) *e* impl de trait (`set_override` L928), espelhado
em `pg.rs`. É a maior violação de SRP+DRY da árvore.

**Recomendação:** dividir em repos por agregado (`RunRepo`, `PersonaRepo`, …),
mantendo `BtvStore` como fachada que os compõe (preserva a interface pública).
Remove centenas de linhas. **Trade-off:** churn grande em dois arquivos de 1.6k
linhas + `contract`/`golden` a revalidar → estrutural.

> **Duas ressalvas de governança (o relatório defere às decisões registradas):**
> 1. A **API dupla** (método inerente + impl de trait) **não** é dívida acidental
>    a "colapsar" livremente: `pendencias.md:1829` (`[decisão] B2 — API legada do
>    BtvStore = a porta do modo local`) declara a superfície inerente como a
>    **porta do modo local com escopo fixo no tenant LOCAL** (todo SELECT/UPDATE/
>    DELETE legado ganhou `WHERE tenant_id = LOCAL`). Colapsá-la sem preservar
>    esse escopo reintroduziria o vazamento cross-tenant que a decisão fecha. A
>    consolidação, se feita, herda essa restrição.
> 2. O split por agregado cai sob a **mesma** pendência do §2.1
>    (`pendencias.md:2182`, "decomposição dos 3 grandes") e o **mesmo gatilho**
>    (segundo consumidor do motor). Registro como evidência a favor dela, não como
>    backlog novo.

### 2.4 SRP · `ports.rs` mistura agregado + state machine + 6 traits (1003 linhas)

`crates/btv-domain/src/ports.rs` guarda o agregado `Run` (`activate` L322,
`approve_gate` L398, `transition_to` L438), o `RunStatus` com serde custom, **e**
as 6 traits de repositório, **e** os testes. Separar o agregado para `run.rs` e
deixar `ports.rs` só com as definições de trait alinha o nome do arquivo à sua
responsabilidade. Baixo risco (mover código + `pub use` de compatibilidade), mas
mexe num arquivo central — recomendação.

### 2.5 LSP (Liskov) · substituibilidade **provada por construção** — exemplo positivo

O melhor exemplo de Liskov da árvore é a **suíte de contrato dual-adapter**
`crates/btv-contract/src/lib.rs` (Trilha B, ADR 0026): o adapter SQLite (B2) e o
adapter Postgres (B4) implementam as **mesmas** traits de `btv-domain::ports` e
são passados **pelos mesmos** casos de teste — `suite_run_repository(|| adapter)`,
`suite_ledger_determinismo_cross_adapter`, etc. O cabeçalho do arquivo (`:7-8`)
enuncia o princípio na forma exata de LSP: *"Se um teste daqui só passa por
idiossincrasia de um adapter, ele está testando o adapter errado."* Ou seja: um
subtipo (PG) tem de honrar o contrato do supertipo (a trait) sob os mesmos
testes-juiz — substituibilidade comportamental, não só de assinatura.

**Veredito: sem achado — é o par positivo do DIP (ADR 0005).** Nenhuma mudança
recomendada; citado para fechar a cobertura dos 5 princípios com um exemplo de
Liskov correto, e como a rede que protege qualquer refatoração de storage.

### 2.6 ISP · `UserRepository` examinado — tensão leve, sem achado acionável

Candidato natural a violação de ISP: `crates/btv-domain/src/ports.rs:582`
`trait UserRepository` mistura **duas famílias de consumidor** que usam
subconjuntos disjuntos — o caminho de **auth** usa só `verify_pin` (`:613`, que
compara o `pin_hash` DENTRO do adapter e devolve `PinCheck` — o hash nunca sai,
`:579-580`), enquanto o **console** usa o CRUD (`list`/`set_pin`/`set_ativo`/
`delete`). Um consumidor que só autentica hoje depende da trait inteira, CRUD
incluído — o cheiro clássico de ISP.

**Veredito: verificado, tensão leve, sem achado acionável.** Dividir em
`UserAuthPort` + `UserCrudPort` seria purismo de ISP de baixo retorno aqui: (a) o
agregado `User` é pequeno e coeso (ADR 0024 o classifica como área "só tipagem");
(b) há uma família única de adapters (SQLite/PG) implementando ambos os lados, sem
consumidor que se beneficie de depender só de metade; (c) `arch-lint` já isola o
domínio. Registro a ausência **explicitamente** (ausência declarada > silenciosa):
se um terceiro serviço de auth headless surgir consumindo só `verify_pin`, o split
passa a pagar — mesmo critério de "segundo consumidor" da pendência `:2182`.

---

## 3. Design Patterns — problema resolvido × complexidade adicionada

### 3.1 Extract Method + constante nomeada · `retry_until_ready` no LSP  ✅ diff pronto

`crates/btv-tools/src/lsp.rs:378-390` (`position_query`) e `:400-416` (`symbol`)
têm o **mesmo** loop de retry byte-a-byte, e o intervalo `300ms` está inline nos
dois. Extrair um combinador `retry_until_ready(|| …)` resolve a duplicação e dá
um único lugar para raciocinar sobre a política de retry (a spec LSP manda
re-tentar em `ContentModified`/`ServerCancelled`).

**Antes** (`symbol`, idêntico em `position_query`):

```rust
let start = Instant::now();
loop {
    match proc.request("workspace/symbol", json!({ "query": name }), REQUEST_TIMEOUT) {
        Err(e) if is_retryable_lsp_error(&e) && start.elapsed() <= READY_TIMEOUT => {}
        Err(e) => return Err(e),
        Ok(res) => {
            if !is_empty(&res) || start.elapsed() > READY_TIMEOUT {
                return Ok(res);
            }
        }
    }
    std::thread::sleep(Duration::from_millis(300));
}
```

**Depois:**

```rust
Self::retry_until_ready(|| {
    proc.request("workspace/symbol", json!({ "query": name }), REQUEST_TIMEOUT)
})
```

**Trade-off:** +1 função e +1 constante (`LSP_POLL_INTERVAL`) contra −24 linhas
duplicadas e uma política de retry única. Custo de refatoração baixíssimo; sem
mudança de comportamento. Diff completo em §6.1.

### 3.2 Extract + fail-closed · `wait_for_socket`  ✅ diff pronto

`crates/btv-cli/src/squad_agent.rs:599-604` faz `for _ in 0..100 { if exists break; sleep(20ms) }`
— com dois problemas: os mágicos `100`/`20ms` (sem constante) e, pior, **se o
socket nunca aparecer o código segue mesmo assim** e só falha depois, opaco, no
`connect`. O mesmo loop é copiado no helper de teste `service.rs:490`.

**Justificativa (Design Pattern = guard clause + timeout nomeado):** transformar
"espera cega + segue" em "espera com veredito" torna a falha explícita e
localizada, e o timeout vira config em um lugar.

**Antes:**

```rust
let core_task = tokio::spawn(serve_core(backend, core_sock.clone()));
for _ in 0..100 {
    if core_sock.exists() {
        break;
    }
    tokio::time::sleep(Duration::from_millis(20)).await;
}
```

**Depois:**

```rust
let core_task = tokio::spawn(serve_core(backend, core_sock.clone()));
wait_for_socket(&core_sock, SOCKET_READY_TIMEOUT).await
    .map_err(|e| anyhow::anyhow!("core-server não subiu: {e}"))?;
```

**Trade-off:** o caminho passa a **poder** retornar erro cedo (antes seguia
silenciosamente) — é uma mudança de comportamento *desejada* (fail-closed
coerente com o resto do projeto), mas precisa de teste do caminho de timeout.
Diff em §6.2. **Nota honesta:** só há **um** site de produção (o resto é teste),
então o ganho maior aqui é robustez, não volume.

> **Rito (correção com delta de comportamento):** **PR próprio**, com o delta em
> destaque na descrição — *"ganha erro onde hoje segue calado se o socket nunca
> aparece"*. **Juiz:** nenhum teste atual exercita o caminho de socket-ausente
> (o `wait_for_socket` de produção sempre vê o socket subir) — **este delta não
> tem juiz automático; a prova é o teste novo de timeout do PR** (§7). Não
> misturar no lote mecânico do §3.3.

### 3.3 Factory de resposta + helper de lock · `btv_agent.rs` (mecânico, alto volume)

Dois idiomas se repetem no arquivo:

```rust
// ~65× — varia só status/code/msg:
return (StatusCode::INTERNAL_SERVER_ERROR,
        Json(ErrorBody::new("store_error", e.to_string()))).into_response();
// ~31× — recuperação de mutex envenenado:
let store = state.store.lock().unwrap_or_else(|e| e.into_inner());
```

**Depois** (dois helpers privados no módulo):

```rust
fn erro(status: StatusCode, code: &str, msg: impl Into<String>) -> Response {
    (status, Json(ErrorBody::new(code, msg.into()))).into_response()
}
fn lock_store(state: &AppState) -> std::sync::MutexGuard<'_, BtvStore> {
    state.store.lock().unwrap_or_else(|e| e.into_inner())
}
// uso:
return erro(StatusCode::INTERNAL_SERVER_ERROR, "store_error", e.to_string());
let store = lock_store(&state);
```

**Justificativa:** DRY + um único ponto para raciocinar sobre política de erro e
sobre poison-recovery. **Trade-off:** ~96 sites a trocar (mecânico, grande, mas
zero risco semântico) — é o típico "cleanup de alto volume/baixo risco". Por ser
volumoso e não-verificável linha-a-linha aqui, entrego o **padrão** + 1 bloco
representativo (§6.3), não os 96 hunks.

> **Juiz (por que este diff é seguro):** os corpos que o `erro()` colapsa são
> pinados **byte a byte com a mensagem em pt** pelo golden
> `schemas/fixtures/http/squad_activation_errors.golden.json` — exercitado por
> `btv_agent_golden.rs:297` (`golden_squad_activation_errors` →
> `btv_golden::check("squad_activation_errors", …)` na L357). O `erro()` é seguro
> **precisamente porque** esse golden prova a neutralidade: **verde sem regravação
> do golden = o helper não mudou nenhum wire.** Um comando: `cargo test -p btv-cli
> golden`. (Corpos de ledger idem, via `ledger_bodies.golden.json`.) Este é o
> lote **mecânico** — separado das correções com delta (§5.1, §3.2, §5.3).

---

## 4. Big-O — estrutura pelo padrão de acesso, tempo × espaço

### 4.1 Recall relê e re-tokeniza o corpus inteiro a cada chamada (P1)

`python/packages/btv-squad/src/btv_squad/recall.py:89` (`rank`) tokeniza **todos**
os docs e recomputa o IDF a cada query; e `memory.py:_load_corpus` relê+parseia o
JSONL episódico inteiro a cada `recall_similar`/`list_memories`. Custo por chamada
= **O(N·L)** (N docs, L tokens), recomeçando do zero sempre. Está documentado como
"corpus pequeno" (`recall.py:19`), mas é um penhasco de escala: quando a memória
do squad cresce, cada turno paga a releitura de tudo.

**Recomendação (troca espaço por tempo):** cachear o corpus parseado + o índice
IDF, invalidando no append. Um `MemoryStore` com `self._corpus` e `self._idf`
memoizados e um `mtime`/contador de versão derruba o custo amortizado por query
para **O(L_query · N)** apenas no `_cosine`, sem re-parse nem re-IDF.

**Esboço (não diff — `memory.py` não foi patch-verificado linha-a-linha):**

```python
def _corpus(self) -> list[str]:
    stamp = self._path.stat().st_mtime_ns if self._path.exists() else 0
    if stamp != self._corpus_stamp:          # invalida só quando o arquivo mudou
        self._corpus_cache = self._parse_jsonl()
        self._idf_cache = _idf([_tokenize(d) for d in self._corpus_cache])
        self._corpus_stamp = stamp
    return self._corpus_cache
```

**Trade-off:** +memória (corpus e IDF residentes) e +complexidade de invalidação
(o `stat().st_mtime_ns` cobre o append local; um segundo escritor exigiria versão
explícita). Ganho: recall deixa de ser O(N) por chamada. **Medição sugerida:**
`pytest` com um corpus sintético de 10k entradas comparando latência antes/depois.

### 4.2 SSE copia o array inteiro por evento (nota TS, sem diff obrigatório)

`btv-web/src/state/SquadRunContext.tsx:78` (`{ ...r, events: [...r.events, event] }`)
e `web/.../Squad.tsx:125` (`setEvents(prev => [...prev, event])`) copiam O(n) por
evento e depois `esteiraFromEvents`/os 6 `useMemo` re-escaneiam tudo → **O(n²)**
no stream. Aceitável no volume atual; para runs longos, acumular incrementalmente
(reduzir o estado derivado por evento em vez de recomputar do zero) remove o
quadrático. Fica como recomendação de evolução.

---

## 5. Clean Code + 12-Factor — legibilidade/portabilidade × custo

### 5.1 (C1) Erro de store engolido ativa squad sem as personas do usuário ⚠️ correção

`crates/btv-cli/src/btv_agent.rs:213`:

```rust
let overrides = store.list_overrides(&ctx, &template.id).unwrap_or_default()...;
let proprias  = store.list_custom(&ctx, &template.id).unwrap_or_default();
```

Um erro de banco aqui é **silenciado** e o squad é ativado como se o usuário não
tivesse **nenhum** override/persona — não é estilo, é correção: o run roda com o
prompt errado e a procedência (hash) mente. **Depois:** propagar o erro para uma
resposta 500 (usando o helper `erro` do §3.3) em vez de `unwrap_or_default()`.
**Trade-off:** uma falha transitória de DB passa a abortar a ativação em vez de
degradar silenciosamente — que é exatamente o contrato fail-closed do projeto.

> **Rito (correção, não limpeza):** **PR próprio**, delta de comportamento em
> destaque na descrição (*"erro de store na leitura de personas passa a retornar
> 500 em vez de ativar com roster vazio"*). **Ponto crítico do juiz:** o comentário
> em `btv_agent.rs:210` diz "golden de ativação é o juiz" — mas isso vale para o
> **wire** (descrição/procedência **imóvel** no caminho feliz). O golden
> `squad_activation` roda sobre um store em memória que **funciona**, então ele
> **nunca dispara** o ramo `unwrap_or_default()` de `:215/:219`. Ou seja: **nenhum
> golden cobre o caminho de falha de store — este delta não tem juiz automático;
> a prova é teste novo no PR** (um `PersonaRepository` fake que devolve `Err`,
> §7). Sem isso, o C1 vira item de lote de limpeza na mão de quem aplicar — e não
> é.

### 5.2 (C2/D1) Parse JSON: regex gulosa + duplicação em 6 arquivos ✅ diff pronto

`_JSON_BLOCK = re.compile(r"\{.*\}", re.DOTALL)` aparece **idêntico** em
`agents/{architect,auditor,developer,ops,designer}.py` e `planning.py`, cada um
com sua variação do bloco "search / json.loads / isinstance dict / warning". Dois
problemas num só:

- **Correção (C2):** `\{.*\}` com `DOTALL` é **guloso** — captura do primeiro `{`
  até a **última** `}` da resposta inteira. Se o modelo escrever `{...} obrigado! :}`
  ou qualquer prosa com `}`, o parse corrompe.
- **SRP/DRY (D1):** a mesma lógica defensiva está copiada 6×; dois arquivos
  (`auditor`, `planning`) já tinham extraído um `_extract_json` privado — os
  outros quatro inlinam.

**Depois:** um único `extract_json_object(text)` em `btv_squad/_json.py` que varre
a partir do primeiro `{` e deixa o `raw_decode` do decoder achar o fim **real** do
objeto (não-guloso, correto), mantendo o mesmo contrato defensivo (retorna `{}` +
warning em falha). Arquivo novo em §6.4; patches dos 6 sites em §6.5.

**Trade-off:** um import a mais por arquivo e um módulo novo; em troca, a correção
da gulosidade vale para os 6 de uma vez e some a duplicação. Comportamento
preservado site-a-site (ex.: `developer._parse_react_action` continua devolvendo
`{"action": "parse_error"}`).

### 5.3 (C3) Guard de segurança com duas serializações (bypass) ⚠️ segurança

`SecurityConfig.validate_tool_call` casa os `FORBIDDEN_PATTERNS` contra
`str(params)` (`security.py:48`), enquanto `SecureToolSandbox._validate_params_safety`
casa os **mesmos** padrões contra `json.dumps(params, ensure_ascii=False)`
(`sandbox.py:99`). Duas formas textuais do mesmo objeto → um payload pode casar
uma e escapar da outra (ex.: aspas/escapes que `repr` e `json` renderizam
diferente). **Depois:** um canonicalizador único
(`_canonical_params(params) -> str`) usado pelos dois. **Trade-off:** acoplar as
duas classes a um helper compartilhado (pequeno aumento de dependência interna)
para fechar o buraco de bypass — claramente positivo num guard de segurança.

> **Rito (correção de segurança com delta — NÃO é DRY):** unificar a serialização
> **muda o que os `FORBIDDEN_PATTERNS` alcançam** nos dois guards — um payload que
> hoje casa `str(params)` mas escapa de `json.dumps` (ou vice-versa) passa a ser
> bloqueado nos dois. Isso é mudança de superfície de segurança, não
> de-duplicação cosmética. **PR próprio**, delta em destaque. **Juiz:** o teste
> `payload_escapa_um_guard_mas_nao_o_outro` do §7 é a prova — não há golden/teste
> atual que exercite a divergência (por isso ela passou despercebida). Não
> misturar com o lote mecânico do §3.3.

### 5.4 (D4) Front: `hhmm()` duplicada + boilerplate de loading/error ✅ diff parcial

`hhmm(ts)` está definida **verbatim** em `btv-web/src/lib/esteira.ts:173` e
`btv-web/src/state/SquadRunContext.tsx:64`. Extrair para `btv-web/src/lib/time.ts`
e importar nos dois (diff em §6.6).

Além disso, ~22 telas de `btv-web` repetem `useState<T|null>(null)` +
`useEffect(fetch → set/catch)` + `if (erro) …; if (!x) 'carregando…'`. O SPA irmão
`web/` **já** resolveu isso com `hooks/usePolling.ts` + `primitives/AsyncStatus.tsx`;
`btv-web` copiou `useAsyncAction.ts` mas não esses dois. **Recomendação:** portar
`AsyncStatus` + um hook `useAsyncData(fn, deps)` e trocar as ~22 ocorrências
(`admin/Ledger.tsx:28`, `admin/Telemetria.tsx:18`, `user/Biblioteca.tsx:13`,
`user/Minhas.tsx:28`, …). **Trade-off:** um helper novo + varredura ampla (mecânica)
contra ~120 linhas de boilerplate a menos e um único lugar para o estado de
loading/erro. Entrego o hook novo (§6.7) e o padrão; a troca das 22 telas é
mecânica.

### 5.5 (G1) Magic strings/numbers

- **Python `"claude-sonnet-5"`** repetido ~10× como default de argumento
  (`agents/*.py`, `orchestrator.py:89`, `server.py:144,206,236,253`). Extrair
  `DEFAULT_MODEL` (uma constante em `btv_squad/__init__.py` ou `config.py`), com
  override 12-Factor por env: `DEFAULT_MODEL = os.getenv("BTV_SQUAD_MODEL", "claude-sonnet-5")`.
  Um lugar para trocar o modelo padrão; hoje é impossível sem sed.
- **Python thresholds de autonomia** (`hitl.py:55-61` `0.4/0.6/0.8`, `:109-116`
  `+0.02`/`-0.1`) sem nome — contraste com o bem-nomeado
  `HITL_ESCALATION_THRESHOLD` (`consensus.py:22`). Diff em §6.8.
- **TS timeouts** `5000`/`1400`/`500` inline (`Vivo.tsx:46,72`; `web/Squad.tsx:109`;
  `web/Verify.tsx:24`) → constantes nomeadas.
- **`VERSION = "0.1.0"`** duplicado nos 3 sidecars → `importlib.metadata.version`.

**Trade-off (12-Factor):** cada constante/env-var adiciona uma linha de setup em
troca de config em um lugar e portabilidade (mesmo binário, comportamento por
ambiente). Custo de refatoração baixo.

### 5.6 (G2) Config de build frouxa — recomendação faseada

- **`tsconfig.app.json` (ambos SPAs) não tem `"strict": true`.** O código é
  escrito *como se* `strictNullChecks`/`noImplicitAny` estivessem ligados (daí os
  `!` não-guardados), mas o compilador não verifica. Ligar `strict` é o maior
  ganho de qualidade do front — porém **vai expor erros hoje ocultos**, então é
  faseado (diff em §6.9, aplicar e corrigir o fallout).
- **`.oxlintrc.json` (ambos) só ativa 2 regras.** Adicionar `no-explicit-any`,
  `react-hooks/exhaustive-deps` e `no-non-null-assertion` faz os `!` de
  `Designer.tsx:74,77,89`, `Vivo.tsx:130`, `Wizard.tsx:89` etc. virarem lint. Sem
  Prettier no projeto — adicionar é opcional.

**Trade-off:** endurecer a config paga dívida acumulada de uma vez (fallout de
erros/lints) em troca de uma rede de segurança permanente. Por isso: **faseado**,
não num único commit.

### 5.7 (L1) Restrição numérica do hash v1 não enforçada — recomendação com alerta

`request_hash(messages, temperature: Any)` (`hashing.py:28`) aceita `1.0`, `NaN`,
`Inf` sem guarda, embora o docstring (ambos lados) diga que floats com fração zero
são proibidos no v1 (JS emite `1`, Python/Rust emitem `1.0` → chaves divergentes
entre produtores). Hoje a regra é **só prosa**.

**Recomendação:** um validador compartilhado que rejeite float com fração zero /
não-finito, chamado dos **dois** lados (`hashing.py` e `canonical.rs`). **Alerta:**
isso (a) muda comportamento (passa a levantar onde antes aceitava) e (b) toca o
contrato `prompt-cache-key.v1` → exige **ADR novo** + regenerar `schemas/fixtures/`
+ os 2 testes de paridade verdes. **Não** é um patch trivial; é trabalho de
contrato. **Trade-off:** guardar cedo (fail-fast, evita cache-miss cross-impl
silencioso) × risco de quebrar chamadores que hoje passam `1.0`.

---

## 6. Diffs completos e arquivos novos

> Todos os diffs abaixo foram autorados relendo o `HEAD` atual. Onde marco
> "✅ diff pronto", o hunk reflete o texto exato lido. Use `git apply --3way` se
> a árvore divergir.

### 6.1 `retry_until_ready` no LSP  ✅

```diff
--- a/crates/btv-tools/src/lsp.rs
+++ b/crates/btv-tools/src/lsp.rs
@@
+/// Intervalo de polling entre tentativas enquanto o server LSP indexa.
+const LSP_POLL_INTERVAL: Duration = Duration::from_millis(300);
+
 impl LspTool {
+    /// Re-tenta `request` enquanto o server indexa (resultado vazio) ou emite
+    /// erros transitórios (ContentModified/ServerCancelled), até assentar ou
+    /// estourar `READY_TIMEOUT`. Centraliza o loop idêntico de `position_query`
+    /// e `symbol`.
+    fn retry_until_ready(
+        mut request: impl FnMut() -> Result<Value, String>,
+    ) -> Result<Value, String> {
+        let start = Instant::now();
+        loop {
+            match request() {
+                Err(e) if is_retryable_lsp_error(&e) && start.elapsed() <= READY_TIMEOUT => {}
+                Err(e) => return Err(e),
+                Ok(res) => {
+                    if !is_empty(&res) || start.elapsed() > READY_TIMEOUT {
+                        return Ok(res);
+                    }
+                }
+            }
+            std::thread::sleep(LSP_POLL_INTERVAL);
+        }
+    }
```

E os dois call-sites passam a delegar:

```diff
@@ fn position_query
-        let start = Instant::now();
-        loop {
-            match proc.request(method, params.clone(), REQUEST_TIMEOUT) {
-                Err(e) if is_retryable_lsp_error(&e) && start.elapsed() <= READY_TIMEOUT => {}
-                Err(e) => return Err(e),
-                Ok(res) => {
-                    if !is_empty(&res) || start.elapsed() > READY_TIMEOUT {
-                        return Ok(res);
-                    }
-                }
-            }
-            std::thread::sleep(Duration::from_millis(300));
-        }
+        Self::retry_until_ready(|| proc.request(method, params.clone(), REQUEST_TIMEOUT))
@@ fn symbol
-        let start = Instant::now();
-        loop {
-            match proc.request(
-                "workspace/symbol",
-                json!({ "query": name }),
-                REQUEST_TIMEOUT,
-            ) {
-                Err(e) if is_retryable_lsp_error(&e) && start.elapsed() <= READY_TIMEOUT => {}
-                Err(e) => return Err(e),
-                Ok(res) => {
-                    if !is_empty(&res) || start.elapsed() > READY_TIMEOUT {
-                        return Ok(res);
-                    }
-                }
-            }
-            std::thread::sleep(Duration::from_millis(300));
-        }
+        Self::retry_until_ready(|| {
+            proc.request("workspace/symbol", json!({ "query": name }), REQUEST_TIMEOUT)
+        })
```

> Nota de empréstimo: `proc` é `&mut` de `guard.as_mut()`; a closure `FnMut` o
> captura mutavelmente e ambos os sites retornam logo após, então não há conflito
> de borrow. `Instant`/`Duration` já estão importados no arquivo.
>
> **Juiz:** `cargo test -p btv-tools` (os testes de `definition`/`symbol` contra o
> rust-analyzer real, job `lsp` do CI) verdes sem mudança = a política de retry foi
> preservada; `clippy -D warnings` pega qualquer regressão de borrow.

### 6.2 `wait_for_socket` + timeout nomeado  ✅

```diff
--- a/crates/btv-cli/src/squad_agent.rs
+++ b/crates/btv-cli/src/squad_agent.rs
@@
+/// Prazo para o core-server (gRPC sobre UDS) criar o socket antes de desistir.
+const SOCKET_READY_TIMEOUT: Duration = Duration::from_secs(2);
+
+/// Espera o arquivo de socket aparecer, com veredito explícito. Diferente do
+/// loop cego anterior (`for _ in 0..100`), FALHA se o socket nunca surgir em
+/// vez de seguir e estourar opaco no connect.
+async fn wait_for_socket(path: &std::path::Path, timeout: Duration) -> Result<(), String> {
+    let start = Instant::now();
+    while !path.exists() {
+        if start.elapsed() > timeout {
+            return Err(format!("socket {} não apareceu em {timeout:?}", path.display()));
+        }
+        tokio::time::sleep(Duration::from_millis(20)).await;
+    }
+    Ok(())
+}
@@ dentro de run_squad_task_inner
     let core_task = tokio::spawn(serve_core(backend, core_sock.clone()));
-    for _ in 0..100 {
-        if core_sock.exists() {
-            break;
-        }
-        tokio::time::sleep(Duration::from_millis(20)).await;
-    }
+    wait_for_socket(&core_sock, SOCKET_READY_TIMEOUT)
+        .await
+        .map_err(|e| anyhow::anyhow!("core-server não subiu: {e}"))?;
```

> `Instant` precisa entrar no `use std::time::{Duration, Instant};` do arquivo se
> ainda não estiver. A função retornante já é `Result<_, anyhow::Error>` no
> caminho de `run_squad_task_inner` (confirmar o tipo de erro local; se for
> `String`, trocar o `map_err`).

### 6.3 Helpers `erro`/`lock_store` (padrão — bloco representativo)

```diff
--- a/crates/btv-cli/src/btv_agent.rs
+++ b/crates/btv-cli/src/btv_agent.rs
@@
+/// Constrói uma resposta de erro JSON padronizada (colapsa ~65 blocos
+/// `(...).into_response()` idênticos a menos de status/code/msg).
+fn erro(status: StatusCode, code: &str, msg: impl Into<String>) -> Response {
+    (status, Json(ErrorBody::new(code, msg.into()))).into_response()
+}
+
+/// Trava o store recuperando de mutex envenenado (idioma repetido ~31×).
+fn lock_store(state: &AppState) -> std::sync::MutexGuard<'_, BtvStore> {
+    state.store.lock().unwrap_or_else(|e| e.into_inner())
+}
```

Uso representativo (aplicar o mesmo padrão aos demais sites):

```diff
-    let store = state.store.lock().unwrap_or_else(|e| e.into_inner());
+    let store = lock_store(&state);
...
-            return (StatusCode::INTERNAL_SERVER_ERROR,
-                    Json(ErrorBody::new("store_error", e.to_string()))).into_response();
+            return erro(StatusCode::INTERNAL_SERVER_ERROR, "store_error", e.to_string());
```

> `AppState` é o tipo do `State` do módulo; ajustar o nome se diferir. A troca dos
> ~96 sites é mecânica e sem risco semântico, mas volumosa — não listada hunk a
> hunk aqui.

### 6.4 Arquivo novo — `python/packages/btv-squad/src/btv_squad/_json.py`  ✅

```python
"""Extração robusta de um único objeto JSON de respostas de modelo.

Centraliza o parse que estava duplicado nos agentes (`architect`, `auditor`,
`developer`, `ops`, `designer`) e no `planning`. Além de DRY, corrige a regex
gulosa `\\{.*\\}` (com `re.DOTALL` capturava do primeiro `{` até a ÚLTIMA `}`
da resposta inteira — qualquer chave em prosa posterior corrompia o parse):
aqui varremos a partir do primeiro `{` e deixamos o `raw_decode` do decoder
achar o fim REAL do objeto.
"""

from __future__ import annotations

import json
import logging
from typing import Any

logger = logging.getLogger(__name__)

_DECODER = json.JSONDecoder()


def extract_json_object(raw_text: str, *, context: str = "") -> dict[str, Any]:
    """Extrai o primeiro objeto JSON de `raw_text`.

    Retorna `{}` (com log de aviso) quando não há objeto, quando o JSON é
    inválido ou quando o valor de topo não é um objeto — o mesmo contrato
    defensivo que cada agente já usava: uma resposta malformada nunca
    derruba o agente.
    """

    rotulo = f" ({context})" if context else ""
    inicio = raw_text.find("{")
    if inicio == -1:
        logger.warning("Resposta do modelo%s não contém um bloco JSON: %r", rotulo, raw_text[:200])
        return {}
    try:
        candidate, _ = _DECODER.raw_decode(raw_text[inicio:])
    except json.JSONDecodeError:
        logger.warning("Resposta do modelo%s não é JSON válido: %r", rotulo, raw_text[:200])
        return {}
    return candidate if isinstance(candidate, dict) else {}
```

### 6.5 Patches dos 6 call-sites de parse JSON  ✅ (representativos)

Padrão comum: adicionar `from btv_squad._json import extract_json_object`,
remover `_JSON_BLOCK` e o bloco inline, e — se `re`/`json` ficarem sem uso no
arquivo — remover esses imports (trivial, sem efeito).

```diff
--- a/python/packages/btv-squad/src/btv_squad/agents/architect.py
+++ b/python/packages/btv-squad/src/btv_squad/agents/architect.py
@@
-import json
-import logging
-import re
+import logging
 from datetime import datetime, timezone
 from typing import Any
+
+from btv_squad._json import extract_json_object
@@
-_JSON_BLOCK = re.compile(r"\{.*\}", re.DOTALL)
-
-
 class ArchitectAgent(BaseAgent):
@@ def _parse_reasoning
-        parsed: dict[str, Any] = {}
-        match = _JSON_BLOCK.search(raw_text)
-        if match:
-            try:
-                candidate = json.loads(match.group(0))
-                if isinstance(candidate, dict):
-                    parsed = candidate
-            except json.JSONDecodeError:
-                logger.warning("Resposta do modelo não é JSON válido: %r", raw_text[:200])
-        else:
-            logger.warning("Resposta do modelo não contém um bloco JSON: %r", raw_text[:200])
+        parsed = extract_json_object(raw_text)
```

```diff
--- a/python/packages/btv-squad/src/btv_squad/agents/developer.py
+++ b/python/packages/btv-squad/src/btv_squad/agents/developer.py
@@ def _parse_react_action
-        match = _JSON_BLOCK.search(raw_text)
-        if not match:
-            logger.warning("Resposta do modelo (ReAct) não contém um bloco JSON: %r", raw_text[:200])
-            return {"action": "parse_error"}
-        try:
-            candidate = json.loads(match.group(0))
-        except json.JSONDecodeError:
-            logger.warning("Resposta do modelo (ReAct) não é JSON válido: %r", raw_text[:200])
-            return {"action": "parse_error"}
-        if not isinstance(candidate, dict) or candidate.get("action") not in {"tool_call", "final_answer"}:
-            return {"action": "parse_error"}
-        return candidate
+        candidate = extract_json_object(raw_text, context="ReAct")
+        if candidate.get("action") not in {"tool_call", "final_answer"}:
+            return {"action": "parse_error"}
+        return candidate
```

Os demais (`auditor._extract_json`, `planning._extract_json`, `ops._parse_plan`,
`designer._parse_design`, `developer._parse_result`) seguem o mesmo padrão:
`auditor`/`planning` passam a `return extract_json_object(raw_text)` (mantendo o
nome do método privado como fachada — preserva chamadas internas); `ops`/`designer`/
`developer._parse_result` trocam o bloco inline por `parsed = extract_json_object(raw_text)`.

> **Juiz:** `uv run pytest` nos `tests/` de cada agente (comportamento preservado
> site-a-site) **mais** os testes de regressão novos do §7 — em especial
> `test_ignora_prosa_com_chave_no_fim`, que é o **único** que prova a correção da
> gulosidade (C2) e que a suíte atual não cobre. Sem esse teste novo, o C2 não tem
> juiz.

### 6.6 Extrair `hhmm()` no front  ✅

Arquivo novo `btv-web/src/lib/time.ts`:

```ts
export function hhmm(ts: string): string {
  const m = ts.match(/T(\d{2}):(\d{2})/)
  return m ? `${m[1]}:${m[2]}` : ts.slice(0, 5)
}
```

```diff
--- a/btv-web/src/lib/esteira.ts
+++ b/btv-web/src/lib/esteira.ts
@@
-function hhmm(ts: string): string {
-  const m = ts.match(/T(\d{2}):(\d{2})/)
-  return m ? `${m[1]}:${m[2]}` : ts.slice(0, 5)
-}
+import { hhmm } from './time'
```

```diff
--- a/btv-web/src/state/SquadRunContext.tsx
+++ b/btv-web/src/state/SquadRunContext.tsx
@@
-function hhmm(ts: string): string {
-  const m = ts.match(/T(\d{2}):(\d{2})/)
-  return m ? `${m[1]}:${m[2]}` : ts.slice(0, 5)
-}
+import { hhmm } from '../lib/time'
```

> Mover o `import` para o topo do arquivo (junto dos demais), não no meio — os
> hunks acima mostram só a remoção/substituição.
>
> **Juiz:** `pnpm -C btv-web test` (vitest de `esteira`/`SquadRunContext`) +
> `pnpm -C btv-web build` (`tsc -b`) verdes = a função extraída é idêntica e nada
> mais referenciava a cópia local.

### 6.7 Arquivo novo — `btv-web/src/hooks/useAsyncData.ts` (mata o boilerplate de tela)

```ts
import { useEffect, useState } from 'react'

/** Estado de uma carga assíncrona única (não-polling). Espelha o padrão de
 *  `AsyncStatus` do SPA `web/`, que `btv-web` ainda não tinha adotado. */
export interface AsyncData<T> {
  data: T | null
  erro: string | null
  carregando: boolean
}

export function useAsyncData<T>(fn: () => Promise<T>, deps: unknown[] = []): AsyncData<T> {
  const [estado, setEstado] = useState<AsyncData<T>>({ data: null, erro: null, carregando: true })
  useEffect(() => {
    let vivo = true
    setEstado({ data: null, erro: null, carregando: true })
    fn()
      .then((d) => vivo && setEstado({ data: d, erro: null, carregando: false }))
      .catch((e: Error) => vivo && setEstado({ data: null, erro: e.message, carregando: false }))
    return () => {
      vivo = false
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, deps)
  return estado
}
```

### 6.8 Constantes nomeadas de autonomia (`hitl.py`)  ✅

```diff
--- a/python/packages/btv-squad/src/btv_squad/hitl.py
+++ b/python/packages/btv-squad/src/btv_squad/hitl.py
@@
+# Limiares de score de confiança que definem o nível de autonomia (0..3).
+_AUTONOMY_THRESHOLDS = (0.4, 0.6, 0.8)
+# Ajuste do score após cada ação real — assimétrico de propósito: confiança
+# sobe devagar e cai rápido.
+_TRUST_REWARD = 0.02
+_TRUST_PENALTY = 0.1
@@ def _get_autonomy_level
-        score = self.agent_trust_scores.get(agent, 0.5)
-        if score < 0.4:
-            return 0
-        if score < 0.6:
-            return 1
-        if score < 0.8:
-            return 2
-        return 3
+        score = self.agent_trust_scores.get(agent, 0.5)
+        return sum(1 for limiar in _AUTONOMY_THRESHOLDS if score >= limiar)
@@ def _update_score
-        if success:
-            score = min(1.0, score + 0.02)
-        else:
-            score = max(0.0, score - 0.1)
+        if success:
+            score = min(1.0, score + _TRUST_REWARD)
+        else:
+            score = max(0.0, score - _TRUST_PENALTY)
```

> Equivalência verificada: `score=0.5→1`, `0.3→0`, `0.7→2`, `0.8→3` batem com o
> encadeamento original.
>
> **Juiz:** os testes de autonomia/HITL existentes (`uv run pytest` em
> `btv-squad/tests`) verdes **sem** ajuste = a extração de constantes é pura
> refatoração (mesmos valores, mesmos limiares).

### 6.9 `tsconfig` strict (recomendação faseada)

```diff
--- a/btv-web/tsconfig.app.json
+++ b/btv-web/tsconfig.app.json
@@
     /* Linting */
+    "strict": true,
     "noUnusedLocals": true,
```

(idem `web/tsconfig.app.json`). **Aplicar junto da correção do fallout** — não
isolado, senão o `tsc -b` do CI quebra.

```diff
--- a/btv-web/.oxlintrc.json
+++ b/btv-web/.oxlintrc.json
@@
   "rules": {
     "react/rules-of-hooks": "error",
-    "react/only-export-components": ["warn", { "allowConstantExport": true }]
+    "react/only-export-components": ["warn", { "allowConstantExport": true }],
+    "typescript/no-explicit-any": "warn",
+    "react-hooks/exhaustive-deps": "warn",
+    "typescript/no-non-null-assertion": "warn"
   }
```

---

## 7. Testes propostos

Nenhum foi criado no disco (modo relatório). Casos a adicionar junto de cada
mudança, no estilo do projeto (Rust `#[cfg(test)]` inline; Python `tests/` por
pacote; nomes em português):

- **`_json.extract_json_object` (regressão C2):**
  `test_ignora_prosa_com_chave_no_fim` — entrada `'{"a":1} obrigado :}'` deve
  retornar `{"a": 1}` (a regex antiga retornava `{}` ou lixo).
  `test_sem_bloco_retorna_vazio`, `test_json_invalido_retorna_vazio`,
  `test_topo_nao_objeto_retorna_vazio` (entrada `'[1,2]'` → `{}`).
- **`developer._parse_react_action`:** `test_acao_desconhecida_vira_parse_error`
  (garante que o comportamento foi preservado).
- **`retry_until_ready` (Rust):** teste com uma closure que devolve `is_empty`
  N vezes e depois um valor — assenta; e uma que só devolve erro retryable até o
  `READY_TIMEOUT` — retorna o último Ok/Err como antes. (Pode ser feito sem LSP
  real, injetando a closure.)
- **`wait_for_socket` (Rust):** `socket_inexistente_estoura_timeout` (path que
  nunca aparece → `Err` dentro do prazo) e `socket_aparece_ok`.
- **`hitl` constantes:** os testes existentes de autonomia continuam verdes
  (equivalência já verificada); adicionar `nivel_por_limiar` cobrindo os 4 níveis.
- **`recall`/`memory` cache (P1):** `recall_nao_reparseia_sem_append` (mockar o
  parser e assertar que é chamado 1× em 3 recalls sem append) + benchmark
  informal com corpus grande.
- **Segurança C3:** `payload_escapa_um_guard_mas_nao_o_outro` — um input que casa
  `str(params)` mas não `json.dumps` (ou vice-versa) hoje passa; após unificar,
  ambos bloqueiam.

---

## 8. Riscos e mitigação

| Mudança | Risco | Mitigação |
|---|---|---|
| §6.1 `retry_until_ready` | Borrow de `proc` na closure `FnMut` | Verificado: ambos os sites retornam logo após; `clippy -D warnings` pega qualquer regressão. Rodar `cargo test -p btv-tools` + o job `lsp` do CI. |
| §6.2 `wait_for_socket` | Passa a **falhar** onde antes seguia; tipo de erro local pode ser `String` e não `anyhow` | É fail-closed desejado; ajustar o `map_err` ao tipo real da fn. Teste de timeout obrigatório. |
| §6.3 helpers erro/lock | 96 sites mecânicos — risco de erro humano na varredura | Aplicar com busca/substituição verificável + `cargo test` da crate; sem mudança semântica. |
| §6.4/6.5 `_json` | Comportamento site-a-site pode divergir sutilmente | Preservado explicitamente (ex.: `parse_error` no developer); cobrir com os testes do §7 antes de aplicar. Remover `import re/json` só se realmente sem uso (checar cada arquivo). |
| §5.1 (C1) propagar erro de store | Falha transitória de DB passa a abortar ativação | É o contrato fail-closed do projeto; alternativa é logar+degradar explicitamente, nunca silencioso. |
| §5.3 (C3) guard unificado | Acoplar duas classes a um helper | Helper puro/pequeno; cobre o buraco de bypass — ganho > custo. |
| §6.9 `strict`/`oxlint` | Expõe muitos erros/lints de uma vez | **Faseado**: aplicar e corrigir o fallout no mesmo PR; começar por um SPA. `warn` (não `error`) nos lints novos para não travar o CI de imediato. |
| §5.7 (L1) guard do hash | Muda contrato `prompt-cache-key.v1` | **Não** aplicar sem ADR + regenerar `schemas/fixtures/` + os 2 testes de paridade verdes. |
| §2.x estruturais | Churn grande em arquivos centrais | Fora do escopo deste relatório; fazer isoladamente, um agregado/handler por vez, sob `just preflight`. |

### Como validar, se depois aplicar um subconjunto
- Rust: `just preflight` (espelha o CI: `cargo test`/`clippy -D warnings`/`fmt`/`arch-lint`).
- Python: `cd python && uv run pytest` (+ paridade de hash se tocar `hashing.py`/`canonical.rs`).
- TS: `pnpm -C btv-web build && pnpm -C btv-web test && pnpm -C btv-web lint` (idem `web/`).

---

### Apêndice — o que **não** foi feito (honestidade)
Nenhum arquivo de código foi alterado; nenhum commit/push; nenhum fixture
regenerado; os diffs estruturais são **esboços** rotulados como tais (não finjo
patch pronto para um handler de 244 linhas). Os itens marcados "✅ diff pronto"
foram autorados relendo o `HEAD` — ainda assim, revalide com `git apply --check`
antes de aplicar.
