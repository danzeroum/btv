# 09 — Auditoria LGPD e Privacy by Design (relatório técnico)

**Pergunta:** onde o sistema trata dado pessoal em desacordo com a Lei 13.709/2018, e o
que exatamente precisa mudar?
**Entrada:** leitura estática de `crates/` (14), `python/` (4 pacotes), `web/`, `btv-web/`,
`infra/`, `.github/workflows/` e `schemas/`.
**Base:** 100% estático. Cruze com [dados sensíveis (08)](08-dados-sensiveis-e-seguranca.md),
[modelo de dados (11)](../diagramas/11-modelo-de-dados.md) e
[endpoints HTTP (14)](../referencia/14-endpoints-http.md).
**Documento irmão:** o [RIPD](../RIPD.md) consolida estes achados em relatório de impacto.

> **Commit auditado:** `a3e14f4` · **Método:** 100% análise estática, com comandos
> reproduzíveis em §9.7 — sem pentest, sem execução contra ambiente de produção.
> **Não é parecer jurídico:** é levantamento técnico que instrumenta um. A qualificação
> de artigo é a leitura do revisor de engenharia e deve ser confirmada pelo encarregado
> (que ainda não existe — Risco-030).
>
> As referências `arquivo:linha` envelhecem. Onde a linha divergir, o nome da função ou o
> trecho citado é a âncora — e o comando de evidência de §9.7 revalida o achado no commit
> corrente.

---

## 9.1 Como ler este documento

### Os dois cenários

Todo risco declara **em qual cenário ele morde**. Os dois existem no repositório hoje:

| | Cenário | O que é | Onde está declarado |
|---|---|---|---|
| **A** | **Local-first** | `btv dashboard` com bind `127.0.0.1:7878`, máquina única, sem auth por desenho. | `crates/btv-cli/src/main.rs:136-141` (default do `--host`) |
| **B** | **Hospedado** | `btv dashboard --host 0.0.0.0`, na bridge interna de um compose, atrás de um nginx com basic auth **que não está neste repositório**. | `infra/docker/docker-compose.prod.yml:42` |

A distinção importa porque quase toda a postura de privacidade do sistema — ausência de
autenticação, de CSP, de sanitização de erro, de rate limit — é **justificada** pelo
cenário A e **injustificável** no cenário B. O cenário B não é hipotético: o compose
existe, documenta `BTV_TRUSTED_ORIGINS=squad.buildtovalue.cloud` e é acompanhado de um
README com as instruções de subida.

### Titular considerado

**O usuário da plataforma**: os perfis do A6 (`nome`, `email`, `papel`, `pin_hash`) e o
conteúdo que ele mesmo escreve — briefing, instruções de ajuste, prompts, mensagens de
chat, personas.

**Fora de escopo, deliberadamente:** dados de **terceiros** mencionados *dentro* do
conteúdo submetido (um briefing de RH com currículos, um contrato com nome de cliente). O
sistema não sabe distinguir esse conteúdo, e tratá-lo exigiria classificação de dado na
ingestão — que não existe. O descope está registrado no [RIPD §2.4](../RIPD.md) porque é
um pressuposto que, se cair, muda a classificação de vários riscos daqui (ver Risco-003).

### Severidade

- **P0** — o tratamento é ilícito ou o direito do titular é inexequível. Exige decisão
  antes de qualquer uso com dado pessoal real.
- **P1** — o controle de segurança exigido pelo Art. 46 está ausente.
- **P2** — endurecimento, transparência e prevenção de regressão.

### Os 7 princípios PbD (Cavoukian)

Referenciados por número nas tabelas: **(1)** Proativo e Preventivo · **(2)** Privacy by
Default · **(3)** Privacidade no Design · **(4)** Soma Positiva · **(5)** Transparência ·
**(6)** Segurança Ponta-a-Ponta · **(7)** Centrado no Usuário.

### Numeração

Os IDs `Risco-NNN` são **handles estáveis**, atribuídos na ordem em que os achados
surgiram — não na ordem de severidade. A severidade é dada pela seção (§9.2 = P0,
§9.3 = P1, §9.4 = P2). Um ID nunca é reciclado nem renumerado, para que referências
externas (PRs, ADRs, o [RIPD](../RIPD.md)) não apodreçam.

### Placar

| | Quantidade |
|---|---|
| **P0** | 8 |
| **P1** | 13 |
| **P2** | 9 |
| **Conforme** (créditos verificados) | 15 |
| **Não aplicável** (com razão registrada) | 7 |

---

## 9.2 P0 — tratamento ilícito ou direito inexequível

### Risco-001 · Nenhuma autenticação; todo GET é isento até do guard de Origin

| Campo | Conteúdo |
|---|---|
| **Localização** | `crates/btv-server/src/guard.rs:22-36`; `crates/btv-cli/src/tenant_extractor.rs:163-166,187,194-204`; `crates/btv-cli/src/main.rs:414` |
| **Categoria** | Controle de acesso |
| **Base legal violada** | Art. 46 (medidas de segurança); Art. 6º, VII (segurança) |
| **Princípio PbD** | (2) Privacy by Default, (6) Segurança Ponta-a-Ponta |
| **Impacto** | **Alto** — cenário B crítico, cenário A alto |

**Problema.** O único controle sempre ativo é o guard de `Origin`/`Host` (ADR 0015), e ele
tem dois furos por desenho:

```rust
// crates/btv-server/src/guard.rs:22
pub(crate) async fn require_local_origin(req: Request, next: Next) -> Response {
    if req.method() != Method::GET {                     // ← GET não é checado
        if let Some(origin) = req.headers().get(header::ORIGIN) {  // ← sem Origin, passa
            ...
```

O próprio doc-comment (`guard.rs:16-17`) declara: *"Sem `Origin` (curl/CLI) passa — o
cabeçalho só existe em requisições de navegador."* Isso é correto para o que o guard se
propõe (CSRF de navegador) e o ADR 0015 é explícito que **não substitui autenticação**.

A camada que deveria autenticar é inerte. `resolver_contexto` em modo local nunca recusa:

```rust
// crates/btv-cli/src/tenant_extractor.rs:163
Mode::Local => Ok(TenantContext::local(
    ActorId::new(ACTOR_LOCAL).expect("actor local fixo válido"),
)),
```

E `Mode::Local` é o default (qualquer coisa ≠ `BTV_MODE=saas`). O modo saas existe, é
fail-closed e está corretamente desenhado (ADR 0029) — mas `main.rs:414` injeta
`TenantResolucao::from_env(None)`, ou seja **sem resolver**, e `ROTAS_LIVRES` está vazio
(`tenant_extractor.rs:187`) porque não há rota de login HTTP. Na prática o modo saas
devolveria 500 em tudo.

Não há conceito de autorização em lugar nenhum. O campo `papel` (`admin`/`usuario`) é
rótulo de exibição — nenhum handler o consulta para decidir acesso.

**Consequência.** **Nenhuma** rota GET tem proteção — e são a maioria da superfície
(ver o inventário completo em [referencia/14](../referencia/14-endpoints-http.md)).
Entre elas:

| Rota | O que entrega |
|---|---|
| `GET /api/btv/users` | nome + e-mail de todos os perfis |
| `GET /api/btv/deliverables/{id}/download` | os **bytes** do arquivo produzido |
| `GET /api/btv/squads` | os briefings verbatim (`briefing_json`) |
| `GET /api/ledger` | a trilha de auditoria inteira, incluindo texto livre |
| `GET /api/session/{id}/events`, `GET /api/squad/{id}/events` | a transcrição completa da execução |

**Cenário de dano (B).** O nginx do ingress cai, é reconfigurado sem `auth_basic`, ou uma
rota é adicionada ao `location` sem a diretiva. Nada no código do BuildToValue percebe: a
aplicação serve PII em massa a quem pedir. Todo o controle de acesso do produto mora num
arquivo que não está neste repositório e não é testado por este CI.

**Recomendação.** Ordenadas por custo:
1. **Curto prazo, cenário B:** documentar que o ingress é o único controle de acesso, e
   adicionar um teste de fumaça que falhe se o compose de produção subir sem basic auth
   configurado.
2. **Médio prazo:** estender o guard de `Origin` aos GET que retornam dado pessoal (a
   isenção faz sentido para assets estáticos, não para `/api/btv/users`).
3. **Correção real:** completar a E1s — rota de login, `SessionResolver` wired em
   `main.rs:414`, primeira entrada em `ROTAS_LIVRES`. O desenho já está aceito no ADR 0029;
   falta o fio.

**Evidência.**
```sh
sed -n '20,40p'  crates/btv-server/src/guard.rs
sed -n '158,205p' crates/btv-cli/src/tenant_extractor.rs
grep -n 'TenantResolucao::from_env' crates/btv-cli/src/main.rs
grep -rn 'papel' crates/btv-cli/src/btv_agent.rs   # nunca em decisão de autorização
```

---

### Risco-002 · IDOR por construção: ids sequenciais + replay completo no SSE

| Campo | Conteúdo |
|---|---|
| **Localização** | `crates/btv-cli/src/squad_agent.rs:146,206-214`; `crates/btv-cli/src/web_agent.rs:222-227`; `crates/btv-cli/src/btv_agent.rs:804,1378` |
| **Categoria** | Controle de acesso / anti-enumeração |
| **Base legal violada** | Art. 46; Art. 6º, VII |
| **Princípio PbD** | (2) Privacy by Default, (3) Privacidade no Design |
| **Impacto** | **Alto** — cenário B crítico, cenário A médio |

**Problema.** Nenhuma rota que recebe um id valida se o chamador é dono do recurso
(consequência direta do Risco-001: não há "dono"). E os ids são adivinháveis:

```rust
// crates/btv-cli/src/squad_agent.rs:146
let task_id = format!("sq{:x}", self.next_task_seq.fetch_add(1, Ordering::Relaxed));
```

Isso produz `sq1`, `sq2`, … `sqa`, `sqb`. Todos os demais ids de recurso são rowid
sequencial do SQLite (`users`, `deliverables`, `prompts`, `custom_personas`,
`permission_rules`).

Agrava: o `subscribe` do SSE devolve o **snapshot inteiro** do log ao assinante, e usa
`or_insert_with` — de modo que assinar um id inexistente **cria** o estado:

```rust
// crates/btv-cli/src/squad_agent.rs:206
let state = tasks.entry(task_id.to_string()).or_insert_with(SquadTaskState::new);
(state.log.clone(), state.tx.as_ref().map(|tx| tx.subscribe()))
```

O checklist de revisão pede **UUID no path** e **404, nunca 403** para recurso não
autorizado. O segundo já existe para cross-tenant (`btv_agent.rs:19-23`, ver §9.5); o
primeiro não.

**Cenário de dano (B).** Um usuário autenticado no basic auth do ingress itera
`sq1..sq100` e lê a transcrição completa de todas as execuções de squad de todos os
colegas — briefings, decisões dos agentes, conteúdo dos arquivos tocados. Não é escalada
de privilégio: é o comportamento projetado, exercido por quem já entrou.

**Recomendação.** Trocar o contador por UUID v4 no `new_task` (mudança local, o id é
opaco para o frontend); trocar `or_insert_with` por `get` no `subscribe` (assinar id
inexistente deve dar 404, não criar estado); e, quando houver identidade (Risco-001),
checar posse antes do replay.

**Evidência.**
```sh
sed -n '140,150p' crates/btv-cli/src/squad_agent.rs
sed -n '204,216p' crates/btv-cli/src/squad_agent.rs
sed -n '218,230p' crates/btv-cli/src/web_agent.rs
```

---

### Risco-003 · Transferência internacional de conteúdo sem mecanismo válido

| Campo | Conteúdo |
|---|---|
| **Localização** | `crates/btv-llm/src/gateway.rs:60-90`; `crates/btv-llm/src/anthropic.rs:12`; `crates/btv-llm/src/openai.rs:12-13` |
| **Categoria** | Transferência internacional / gestão de fornecedor |
| **Base legal violada** | **Art. 33** (hipóteses de transferência); Art. 34 (adequação); Art. 35 (cláusulas-padrão) |
| **Princípio PbD** | (5) Transparência, (6) Segurança Ponta-a-Ponta |
| **Impacto** | **Alto** — cenários A e B |

**Problema.** Todo conteúdo submetido pelo titular — tarefa, briefing, prompt, mensagem,
trechos de arquivo lidos pelas ferramentas — é enviado a um provedor de LLM no exterior. A
escolha do provedor é feita pela mera presença de variável de ambiente, numa cadeia fixa:

```rust
// crates/btv-llm/src/gateway.rs:61
let candidates = [
    (ProviderId::Anthropic, "ANTHROPIC_API_KEY", anthropic::DEFAULT_BASE_URL),
    (ProviderId::Deepseek,  "DEEPSEEK_API_KEY",  openai::DEEPSEEK_BASE_URL),
    (ProviderId::Openai,    "OPENAI_API_KEY",    openai::OPENAI_BASE_URL),
];
```

Destinos: `api.anthropic.com` (EUA), `api.deepseek.com` (**China**), `api.openai.com`
(EUA).

O que não existe em lugar nenhum do repositório:
- registro de **DPA** com qualquer um dos três;
- **cláusulas-padrão contratuais (SCC)** ou invocação de decisão de adequação da ANPD —
  e não há decisão de adequação da ANPD para os EUA nem para a China;
- **allowlist por classificação de dado** (nada impede que o pior conteúdo vá ao destino
  com menor garantia);
- **opt-out de treino** documentado por provedor;
- consentimento ou aviso ao titular de que o conteúdo sai do país.

A ordem da cadeia é especialmente relevante: se só `DEEPSEEK_API_KEY` estiver setada, todo
o conteúdo vai para a China sem que nada no produto sinalize a mudança de jurisdição.

**Cenário de dano.** Um profissional escreve no briefing detalhes do próprio negócio e de
sua operação. Esse texto sai do Brasil para uma jurisdição sem decisão de adequação, sob
os termos de uso padrão do provedor, sem contrato de tratamento e sem que o titular tenha
sido informado. Não há vazamento nem falha técnica — a transferência é o funcionamento
normal do produto.

**Recomendação.**
1. Registrar em `docs/` a decisão de fornecedor: qual provedor, sob qual base do Art. 33,
   com qual documento (DPA/SCC) e qual hash desse documento.
2. Tornar a cadeia de fallback **explícita e configurável**, não derivada de presença de
   env var — um `BTV_PROVIDER_POLICY` que falhe fechado se o provedor pretendido não
   estiver liberado.
3. Registrar em `AGENTS.md`/README, visível ao usuário, quais destinos recebem o conteúdo.
4. Se dado sensível (Art. 11) entrar no escopo, a decisão muda de figura: exige modelo
   on-premise ou contrato específico — ver a ressalva do descope em §9.1.

**Evidência.**
```sh
sed -n '58,95p' crates/btv-llm/src/gateway.rs
grep -rn "DEFAULT_BASE_URL\|OPENAI_BASE_URL\|DEEPSEEK_BASE_URL" crates/btv-llm/src/
grep -rniE "dpa|scc|cláusulas|adequa" docs/ --include=*.md   # nenhum registro de fornecedor
```

---

### Risco-004 · Google Fonts a cada carregamento do SPA

| Campo | Conteúdo |
|---|---|
| **Localização** | `btv-web/index.html:9-14` |
| **Categoria** | Script de terceiro / transferência internacional |
| **Base legal violada** | Art. 7º (falta de base legal); Art. 33 |
| **Princípio PbD** | (2) Privacy by Default, (5) Transparência |
| **Impacto** | **Alto** — cenários A e B |

**Problema.** O SPA do produto faz duas conexões e duas requisições ao Google LLC a cada
carregamento, antes de qualquer ação do usuário:

```html
<!-- btv-web/index.html:9 -->
<link rel="preconnect" href="https://fonts.googleapis.com" />
<link rel="preconnect" href="https://fonts.gstatic.com" crossorigin />
<link href="https://fonts.googleapis.com/css2?family=Bricolage+Grotesque:..." rel="stylesheet" />
```

Isso transmite **IP, User-Agent e Referer** do titular a um terceiro nos EUA. Sem
consentimento, sem base legal declarada, sem DPA, e — porque não há `Referrer-Policy`
(Risco-013) — com o caminho completo da página no `Referer`.

É a **única** requisição a host não-loopback disparada pelos frontends. Todo o resto do
tráfego é same-origin relativo (`/api/...`), o que torna este item fácil de perder numa
revisão de código: ele não está em nenhum `fetch`.

Contradiz frontalmente o que o próprio servidor afirma em `crates/btv-server/src/lib.rs:5`
(*"Nada sai da máquina do usuário"*) — ver Risco-026.

**Recomendação.** Auto-hospedar as três famílias em `btv-web/public/`. O custo é baixo e
resolve três coisas de uma vez: elimina a transferência, elimina a única lacuna de SRI, e
torna um `default-src 'self'` trivialmente alcançável (Risco-013). O padrão correto já
existe no repositório: o console `web/` usa `src: local('Space Grotesk')`
(`web/src/styles/global.css:1-4`) e não contata host externo algum. O comentário em
`btv-web/index.html:7-8` já reconhece que há fallback local.

**Não** recomendo resolver isso com um banner de consentimento: pedir consentimento para
uma transferência evitável é o inverso de privacy by default.

**Evidência.**
```sh
grep -rn "https://" btv-web/index.html web/index.html
sed -n '1,6p' web/src/styles/global.css     # o padrão correto, no mesmo repo
```

---

### Risco-005 · Direito à eliminação é estruturalmente impossível no ledger

| Campo | Conteúdo |
|---|---|
| **Localização** | `crates/btv-schemas/src/ledger.rs:60-78`; `crates/btv-store/src/ledger.rs:5-7`; `crates/btv-cli/src/session.rs:33`; `crates/btv-domain/src/ports.rs:162-222` |
| **Categoria** | Retenção / direitos do titular |
| **Base legal violada** | **Art. 18, VI** (eliminação) |
| **Princípio PbD** | (7) Centrado no Usuário, (3) Privacidade no Design |
| **Impacto** | **Alto** — cenários A e B |

**Problema.** O ledger é append-only com hash-chain por tenant (ADR 0027) e o hash cobre o
payload:

```rust
// crates/btv-schemas/src/ledger.rs:76
pub fn chain_hash(&self, prev_hash: &str) -> String {
    sha256_hex(&format!("{prev_hash}{}", self.hash_body()))
}
```

Apagar ou redigir o payload de uma entrada invalida seu `entry_hash` **e o de toda a
cadeia subsequente daquele tenant** — `verify_chain` passaria a reportar corrupção. Não há
`DELETE FROM ledger` no crate, por desenho.

Isso seria aceitável se o payload contivesse só metadados. Não contém:

| Origem | Payload | Conteúdo |
|---|---|---|
| `crates/btv-cli/src/session.rs:33` | `session.start` | **a tarefa inteira do usuário**, verbatim |
| `crates/btv-domain/src/ports.rs:192-197` | `AdjustRequested.instruction` | texto livre digitado no gate |
| `crates/btv-domain/src/ports.rs:164-183` | `SquadActivated.name`, `.refs` | nome dado pelo usuário; URLs/caminhos livres |
| `crates/btv-cli/src/session.rs:69,75,78` | `tool.run`, `tool.result`, `tool.denied` | caminhos de arquivo e resumos de saída |

O remédio previsto pelo produto (`ledger.rs:5-7`) é que uma correção vira **nova entrada
marcada** — o que registra a retificação (Art. 18, III) mas **não elimina** (Art. 18, VI).
Não há crypto-shredding, tokenização, nem blob de conteúdo destacável.

**Cenário de dano.** O titular exerce o direito de eliminação. O operador consegue apagar
a linha em `users`, mas a tarefa que ele escreveu, as instruções que digitou e os caminhos
de seus arquivos permanecem no ledger — permanentemente, por construção, e legíveis via
`GET /api/ledger`, que não tem autenticação (Risco-001).

**Recomendação.** A correção precisa nascer no schema, não num job:

- **Crypto-shredding** é o caminho compatível com a cadeia: cifrar os campos de texto
  livre com uma chave por titular, encadear o hash sobre o **ciphertext**, e implementar a
  eliminação como destruição da chave. A cadeia continua íntegra e verificável; o texto
  vira irrecuperável. É a única opção que preserva a garantia do ADR 0027.
- **Alternativa mais barata:** parar de colocar texto livre no ledger. O próprio sistema
  já mostra como — `user.turn` grava só a contagem de caracteres (`main.rs:837`) e
  `PersonaUpdated` grava só o `sha256` do prompt. Aplicar a mesma disciplina a
  `session.start.task` e `AdjustRequested.instruction` resolveria a maior parte do
  problema **para entradas futuras** (não para as já gravadas).

Qualquer decisão aqui merece ADR próprio: é mudança de contrato do ledger.

**Evidência.**
```sh
sed -n '56,80p'  crates/btv-schemas/src/ledger.rs
sed -n '28,40p'  crates/btv-cli/src/session.rs
sed -n '162,200p' crates/btv-domain/src/ports.rs
grep -n "DELETE FROM ledger" crates/btv-store/src/ledger.rs   # vazio, por desenho
```

---

### Risco-006 · Zero retenção e zero expurgo em todo o sistema

| Campo | Conteúdo |
|---|---|
| **Localização** | ausente em todo o repositório; `crates/btv-core/src/session.rs:120+` (compactação que não apaga) |
| **Categoria** | Retenção / ciclo de vida |
| **Base legal violada** | **Art. 15** (término do tratamento); **Art. 16** (eliminação após o término) |
| **Princípio PbD** | (1) Proativo e Preventivo, (2) Privacy by Default |
| **Impacto** | **Alto** — cenários A e B |

**Problema.** Não existe TTL, job de expurgo, cap de tamanho, política de idade nem
atributo `retencao_ate` em nenhuma tabela. O checklist pede que retenção seja **atributo
estrutural**, não tarefa eventual — aqui ela não é nem tarefa eventual.

O que cresce sem limite e sem política:

| Armazenamento | Conteúdo | Caminho de exclusão |
|---|---|---|
| `telemetry_event` (`.btv/telemetry.db`) | métrica por chamada | **nenhum** — sem `DELETE`/`UPDATE` no crate |
| `ledger` (`.btv/btv.db`) | trilha + texto livre | **nenhum**, por desenho (Risco-005) |
| `event` / `event_sequence` (`.btv/events.db`) | **cada turno de chat, verbatim** | **nenhum** |
| `prompt_cache` (`.btv/cache.db`) | **a completion inteira do modelo** | só `INSERT OR REPLACE`; `created_at` é gravado mas nunca lido para expirar |
| `.btv/tool-outputs/*.txt` | saída de ferramenta (overflow) | **nenhum** |
| `.btv/evidence/<run_id>.json` | evidência de verify | **nenhum** |
| `.btv/squad-memory/agent_memories.jsonl` | **o dict de tarefa inteiro** por decisão | **nenhum**, append-only sem rotação |

As únicas exclusões que existem são CRUD iniciado pelo usuário (`prompt_library`,
`permission_rules`, `persona_overrides`, `custom_personas`, `users`) — nenhuma é retenção.

Detalhe que engana: a **compactação de sessão** (`btv-core/src/session.rs:120+`)
*acrescenta* um marco `epoch.started.1` e limpa a lista **em memória**. As mensagens
originais continuam no banco para sempre.

A pendência é conhecida pelo projeto — `docs/adr/0027-...md:80` adia "Retenção/eliminação
por tenant (LGPD)" para a Trilha E. Este relatório apenas datiza a dívida.

**Recomendação.** A ordem importa, porque expurgo sem catálogo apaga a coisa errada:
1. Definir, por tabela, `finalidade` / `base_legal` / `prazo`. Esse é o insumo do
   [RIPD §2](../RIPD.md).
2. Calcular `retencao_ate` na ingestão (o `created_ts` já existe em quase toda tabela).
3. Só então implementar o job — em batch, com registro de evidência do que foi excluído.
4. O ledger é o caso especial: ver Risco-005.

**Evidência.**
```sh
# Deve retornar apenas documentação e os deadlines da sessão SaaS — nenhum job:
grep -rniE "VACUUM|retention|retencao_ate|\bttl\b|expire|purge|prune|older_than" \
  --include=*.rs --include=*.py crates/ python/
grep -rn "DELETE FROM" --include=*.rs crates/btv-store/src/    # só CRUD de usuário
```

---

### Risco-007 · Nenhum endpoint de direito do titular existe

| Campo | Conteúdo |
|---|---|
| **Localização** | ausente; exceção parcial em `crates/btv-cli/src/btv_agent.rs:1376-1412` |
| **Categoria** | Direitos do titular |
| **Base legal violada** | **Art. 18, I a VI** |
| **Princípio PbD** | (7) Centrado no Usuário, (5) Transparência |
| **Impacto** | **Alto** — cenários A e B |

**Problema.** Aplicando a regra do checklist — *se o direito não tem endpoint com prazo,
autenticação e log, ele não existe no sistema* — o placar é:

| Art. 18 | Direito | Endpoint | Prazo | Autenticação | Log |
|---|---|---|---|---|---|
| I | Confirmação de tratamento | ✗ | ✗ | ✗ | ✗ |
| II | Acesso aos dados | ✗ | ✗ | ✗ | ✗ |
| III | Correção | ✗ (só CRUD de perfil) | ✗ | ✗ | parcial |
| IV | Anonimização / bloqueio | ✗ | ✗ | ✗ | ✗ |
| V | Portabilidade | ✗ | ✗ | ✗ | ✗ |
| VI | Eliminação | **parcial** | ✗ | ✗ | ✓ |
| — | Oposição | ✗ | ✗ | ✗ | ✗ |

A exceção parcial é `DELETE /api/btv/users/{id}`. Ela apaga a linha em `users` e grava
`btv.user_removed{user_id}` no ledger — o que é a coisa certa para o que faz. Mas **não
cascateia**: `runs.briefing_json`, `deliverables`, `persona_overrides`, `custom_personas`,
`ledger.body`, `event.data`, `prompt_cache.response`, `prompt_library.rendered`,
`telemetry_event` e `agent_memories.jsonl` continuam com o conteúdo do titular. O
relatório não chama isso de "eliminação parcial": é exclusão da linha do perfil, e nada
mais.

**Recomendação.** Ordem de dependência: nenhum desses endpoints faz sentido sem
autenticação (Risco-001) — um endpoint de "acesso aos dados" sem auth *é* o vazamento. A
sequência mínima é: identidade → catálogo de dados (que campos existem por titular) →
endpoints. E a eliminação depende da decisão do Risco-005.

**Evidência.**
```sh
sed -n '1376,1412p' crates/btv-cli/src/btv_agent.rs
grep -rn "api/me\|/portabilidade\|/meus-dados\|/titular" crates/   # vazio
```

---

### Risco-027 · `BTV_MODE` ausente no compose de produção: tenant único e ator fixo

| Campo | Conteúdo |
|---|---|
| **Localização** | `infra/docker/docker-compose.prod.yml` (**ausência** de `BTV_MODE`) × `crates/btv-cli/src/tenant_extractor.rs:30,51-56,163-166` |
| **Categoria** | Segregação / auditoria |
| **Base legal violada** | Art. 37 (registro das operações); Art. 46; Art. 6º, VII |
| **Princípio PbD** | (2) Privacy by Default, (3) Privacidade no Design |
| **Impacto** | **Alto** — cenário B |

**Problema.** O compose de produção não define `BTV_MODE`. E o default é local:

```rust
// crates/btv-cli/src/tenant_extractor.rs:30
const ACTOR_LOCAL: &str = "web:btv";
// :163
Mode::Local => Ok(TenantContext::local(
    ActorId::new(ACTOR_LOCAL).expect("actor local fixo válido"),
)),
```

Consequência no cenário B, onde há **N titulares** por trás de uma basic auth
compartilhada no ingress:

- todos operam no **mesmo tenant** `{local}` — a segregação multitenant construída nos
  ADRs 0026/0027 não é exercida, porque nada seleciona um tenant;
- **todo evento do ledger é atribuído ao mesmo ator**, `web:btv`. A trilha registra *que*
  algo aconteceu, nunca *quem* fez;
- o filtro `?actor=` de `GET /api/ledger`, que existe justamente para segmentar por ator,
  não segmenta nada.

Isso é o que transforma o Risco-001 (sem autenticação) em algo pior que "sem controle de
acesso": mesmo que o ingress autentique corretamente, **o produto não sabe quem é o
usuário e não pode registrar o que ele fez**. Um sistema com tabela `users` contendo nome
e e-mail, mas sem qualquer atribuição individual de ato, é o pior dos dois mundos — coleta
identificação sem produzir accountability.

Nota justa: isto não é bug do `tenant_extractor`, que faz exatamente o que o ADR 0029
decidiu. É a lacuna entre o desenho (aceito) e o fio (não construído) — a mesma do
Risco-001 — vista pelo lado do compose.

**Recomendação.** Enquanto o modo saas não estiver completo, **documentar no compose** que
o cenário B roda com tenant e ator únicos, e que a trilha do ledger não individualiza. É
informação que muda a resposta a "quem acessou este dado?" — pergunta que a ANPD faz.

**Evidência.**
```sh
grep -n "BTV_MODE" infra/docker/docker-compose.prod.yml   # vazio = confirmação
sed -n '28,32p'   crates/btv-cli/src/tenant_extractor.rs
sed -n '160,170p' crates/btv-cli/src/tenant_extractor.rs
```

---

## 9.3 P1 — controles de segurança do Art. 46 ausentes

### Risco-008 · Nenhuma criptografia em repouso; `.btv/` com umask padrão

| Campo | Conteúdo |
|---|---|
| **Localização** | `crates/btv-cli/src/session.rs:26,114`; `crates/btv-cli/src/main.rs:345,508,586`; `crates/btv-tools/src/lib.rs:86`; `python/packages/btv-squad/src/btv_squad/memory.py:29-31` |
| **Categoria** | Criptografia |
| **Base legal violada** | Art. 46 |
| **Princípio PbD** | (6) Segurança Ponta-a-Ponta |
| **Impacto** | **Alto** — cenários A e B |

**Problema.** Duas ausências que se somam.

*Sem cifra.* Nenhum SQLCipher, nenhum AES, nenhum KMS, nenhum envelope encryption. Os
`.btv/*.db` e o JSONL são texto/SQLite em claro. O único primitivo criptográfico do
código é `sha256_hex` (`crates/btv-schemas/src/canonical.rs:102-105`), usado para
integridade e para o `pin_hash` — nunca para confidencialidade.

*Sem permissão restritiva.* Toda criação de `.btv/` usa `std::fs::create_dir_all` sem
`set_permissions`; o Python usa `mkdir(parents=True, exist_ok=True)` sem `mode`. Diretório
e bancos herdam o umask do processo — tipicamente `0755`/`0644`, **legíveis por qualquer
conta do host**.

O checklist pede envelope encryption com DEK por registro e chave própria por campo
sensível. O sistema está três degraus abaixo disso: não há sequer cifra de volume no
escopo da aplicação.

**Cenário de dano.** Num host compartilhado (VPS do cenário B, ou uma máquina de trabalho
com múltiplas contas), qualquer usuário local lê `.btv/btv.db` e obtém e-mails, `pin_hash`
de todos os perfis, prompts de persona em claro, briefings — e `.btv/cache.db` com as
completions inteiras. Nenhuma exploração necessária: `cat` basta.

**Recomendação.** Em ordem de custo/benefício:
1. **`0o700` no diretório e `0o600` nos bancos.** Uma linha por call site, sem mudança de
   contrato, e fecha o vetor mais provável.
2. Documentar que a confidencialidade em repouso depende de cifra de disco do host.
3. Cifra a nível de aplicação (SQLCipher ou campo a campo) só se o cenário B se
   consolidar — e aí o desenho deve casar com o crypto-shredding do Risco-005, não ser
   feito em separado.

**Evidência.**
```sh
grep -rniE "sqlcipher|encrypt|cipher|\baes\b|argon2|bcrypt|pbkdf2" \
  --include=*.rs --include=*.py crates/ python/     # só prosa em docs/
grep -rn "set_permissions\|PermissionsExt\|0o700" --include=*.rs crates/   # vazio
grep -rn "create_dir_all" --include=*.rs crates/ | head
```

---

### Risco-009 · PIN com SHA-256 puro, salt derivável, e verificação sem throttle

| Campo | Conteúdo |
|---|---|
| **Localização** | `crates/btv-store/src/btv.rs:1247-1255`; `crates/btv-cli/src/btv_agent.rs:1485-1513` |
| **Categoria** | Criptografia / autenticação |
| **Base legal violada** | Art. 46 |
| **Princípio PbD** | (6) Segurança Ponta-a-Ponta |
| **Impacto** | **Médio-Alto** — cenários A e B |

**Problema.**

```rust
// crates/btv-store/src/btv.rs:1253
pub(crate) fn pin_hash(created_ts: &str, email: &str, nome: &str, pin: &str) -> String {
    btv_schemas::sha256_hex(&format!("{created_ts}|{email}|{nome}|{pin}"))
}
```

Três defeitos que se compõem:
1. **Não é KDF.** SHA-256 de passagem única, sem fator de custo. Bilhões de tentativas por
   segundo em GPU comum.
2. **O salt não é secreto.** `email` e `nome` vêm do próprio `GET /api/btv/users`, que não
   tem autenticação (Risco-001); `created_ts` está na mesma linha do banco. Contra um
   atacante com o arquivo, o salt não adiciona trabalho algum.
3. **Sem rate limit.** `POST /api/btv/users/{id}/verify-pin` não passa por throttle nenhum
   (Risco-012), então a força bruta **online** de um PIN de 4–6 dígitos também é viável.

O código é honesto sobre o item 1 — o doc-comment em `btv.rs:1251` diz que não é um KDF
pesado, e o ADR 0029 chama o PIN de "conveniência de perfil, não autenticação". Este
relatório concorda com a caracterização e registra a consequência: **o PIN não deve ser
apresentado ao usuário como proteção de acesso**, e a UI hoje o apresenta assim.

**Recomendação.** Se o PIN continuar sendo conveniência: dizer isso na UI, não sugerir
proteção. Se virar credencial: Argon2id (ou bcrypt), salt aleatório por registro em coluna
própria, e throttle com backoff no `verify-pin`.

**Evidência.**
```sh
sed -n '1245,1256p' crates/btv-store/src/btv.rs
sed -n '1485,1513p' crates/btv-cli/src/btv_agent.rs
grep -rn "RateLimitLayer\|governor" --include=*.rs crates/   # vazio
```

---

### Risco-010 · Entidade de domínio direto no wire, sem DTO por finalidade

| Campo | Conteúdo |
|---|---|
| **Localização** | `crates/btv-domain/src/user.rs:24-36`; `crates/btv-cli/src/btv_agent.rs:1326-1338`; `btv-web/src/components/screens/admin/Usuarios.tsx:103,131` |
| **Categoria** | Minimização / limitação de finalidade |
| **Base legal violada** | Art. 6º, I (finalidade); **Art. 6º, III (necessidade)** |
| **Princípio PbD** | (2) Privacy by Default, (3) Privacidade no Design |
| **Impacto** | **Médio** — cenários A e B |

**Problema.** O handler serializa a entidade de domínio diretamente:

```rust
// crates/btv-cli/src/btv_agent.rs:1332
match store.list(&ctx) {
    Ok(users) => Json(users).into_response(),
```

`User` deriva `Serialize` (`crates/btv-domain/src/user.rs:24`) e leva `email: String`. O
resultado é o e-mail completo de **todos** os perfis, sem paginação, sem propósito
declarado, sem mascaramento — a rota não sabe para que o chamador quer os dados.

Crédito onde é devido: o tipo já toma duas decisões corretas — `tenant` é
`#[serde(skip_serializing)]` e o hash vira `has_pin: bool` em vez de vazar. Isso mostra que
a disciplina existe; falta aplicá-la ao e-mail.

Agravante de necessidade: o campo é **opcional na UI** (`placeholder="e-mail (opcional)"`,
`Usuarios.tsx:103`), é exibido sem máscara (`:131`), e não alimenta funcionalidade alguma —
não há envio de e-mail no produto. Coletar dado que nenhuma finalidade consome é
exatamente o que o Art. 6º, III veda.

**Recomendação.** Decidir primeiro **se o e-mail tem finalidade**. Se não tiver, removê-lo
é a correção mais forte e mais barata (minimização vence mascaramento). Se tiver, criar um
DTO de resposta com allowlist explícito, mascarar por padrão (`d***@exemplo.com`) e exigir
propósito para a versão completa.

**Evidência.**
```sh
sed -n '24,36p'   crates/btv-domain/src/user.rs
sed -n '1326,1340p' crates/btv-cli/src/btv_agent.rs
grep -rn "email" btv-web/src/components/screens/admin/Usuarios.tsx
grep -rn "smtp\|sendmail\|mailer" --include=*.rs crates/   # vazio: nada consome o e-mail
```

---

### Risco-011 · Nenhuma paginação, nenhum teto, nenhum cursor

| Campo | Conteúdo |
|---|---|
| **Localização** | `crates/btv-server/src/handlers/telemetria.rs:16,23`; `crates/btv-server/src/handlers/ledger.rs:12,32`; `crates/btv-store/src/btv.rs:1098-1119`; `crates/btv-store/src/prompt_library.rs:76-90` |
| **Categoria** | Minimização / anti-enumeração |
| **Base legal violada** | Art. 6º, III |
| **Princípio PbD** | (2) Privacy by Default |
| **Impacto** | **Médio** — cenário B |

**Problema.** Onde há `?limit=`, ele vai direto para o `LIMIT` do SQL sem teto — `?limit=
4294967295` despeja a tabela inteira. E seis endpoints de listagem não têm nem parâmetro:

| Endpoint | `limit` | Teto | Total exposto |
|---|---|---|---|
| `GET /api/events` | default 50 | **não** | não |
| `GET /api/ledger` | default 50 | **não** | não |
| `GET /api/memory` | default 50 | **não** | não |
| `GET /api/prompts` | **ausente** | — | não |
| `GET /api/btv/users` | **ausente** | — | não |
| `GET /api/btv/deliverables` | **ausente** | — | não |
| `GET /api/btv/squads` | **ausente** | — | não |
| `GET /api/permissions/rules` | **ausente** | — | não |
| `GET /api/btv/personas/{t}` | **ausente** | — | não |

Nenhuma resposta usa envelope com `total`/`next_cursor` — são arrays nus. O checklist pede
paginação obrigatória com hard limit e preferência por cursor.

Detalhe adjacente de eficiência que também é exposição: `get_deliverable` é
`list_deliverables()?.into_iter().find(...)` (`crates/btv-store/src/btv.rs:359-361`) — todo
download carrega a tabela inteira em memória antes de escolher uma linha.

**Recomendação.** Teto duro no parser do `limit` (ex.: 100), `limit` obrigatório nas seis
rotas sem ele, e paginação por cursor onde a listagem é de dado pessoal (`users`,
`deliverables`, `squads`).

**Evidência.**
```sh
sed -n '10,30p' crates/btv-server/src/handlers/telemetria.rs
sed -n '1095,1122p' crates/btv-store/src/btv.rs
sed -n '355,365p' crates/btv-store/src/btv.rs
```

---

### Risco-012 · Zero rate limiting HTTP, e bypass do cap de sessões

| Campo | Conteúdo |
|---|---|
| **Localização** | `Cargo.toml:57`; `crates/btv-cli/src/web_agent.rs:222-227`; `crates/btv-cli/src/squad_agent.rs:206-214`; `crates/btv-server/src/handlers/admin.rs:76-89` |
| **Categoria** | Disponibilidade / anti-abuso |
| **Base legal violada** | Art. 46 |
| **Princípio PbD** | (1) Proativo e Preventivo |
| **Impacto** | **Médio** — cenário B |

**Problema.** Não há `tower::limit`, `governor`, `TimeoutLayer` nem `DefaultBodyLimit` em
lugar nenhum — `tower-http` entra só com a feature `fs`, para servir arquivos. O checklist
pede rate limit **por ator** em todo endpoint de listagem; aqui não há nem por IP.

Cuidado com um falso positivo: `GET /api/ratelimit` existe, mas reporta os caps do gateway
**LLM**, não uso HTTP — e o próprio doc-comment do handler admite que não é uso ao vivo
(cada requisição constrói um `RateLimiter` novo e vazio).

Os dois controles que existem parecem limites mas não são por chamador:
`SessionHub.max_sessions` (default 8, → 429) e o slot único do `/api/verify/run` (→ 409).

E o primeiro tem bypass: `subscribe` usa `entry().or_insert_with()`, então
`GET /api/session/<qualquer-string>/events` **cria estado de sessão sem consultar
`max_sessions`** — crescimento de memória não autenticado, no mesmo call site do Risco-002.

Consequência direta: `verify-pin` (Risco-009), `set_rule`, `send_message` e `squad/run`
são todos irrestritos.

**Recomendação.** `DefaultBodyLimit` e `TimeoutLayer` globais (baratos, sem decisão de
produto); rate limit por ator quando houver identidade (Risco-001); e trocar
`or_insert_with` por `get` no `subscribe`, que já é a correção do Risco-002.

**Evidência.**
```sh
grep -rn "RateLimitLayer\|governor\|TimeoutLayer\|DefaultBodyLimit" --include=*.rs crates/  # vazio
grep -n 'tower-http' Cargo.toml
sed -n '204,216p' crates/btv-cli/src/squad_agent.rs
sed -n '74,92p' crates/btv-server/src/handlers/admin.rs
```

---

### Risco-013 · Ausência total de CSP, SRI e cabeçalhos de segurança

| Campo | Conteúdo |
|---|---|
| **Localização** | `crates/btv-server/src/lib.rs:78-161`; `crates/btv-cli/src/web_agent.rs:924-948`; `btv-web/index.html`; `web/index.html`; `infra/` |
| **Categoria** | Frontend / cabeçalhos |
| **Base legal violada** | Art. 46; Art. 50 (boas práticas) |
| **Princípio PbD** | (6) Segurança Ponta-a-Ponta |
| **Impacto** | **Médio** — cenário B crítico, cenário A baixo |

**Problema.** Nenhum `Content-Security-Policy` em lugar nenhum: nem `<meta http-equiv>` nos
dois `index.html`, nem header no servidor, nem no vite, nem no ingress documentado. O
router tem exatamente uma camada — o guard de `Origin` — que **rejeita**, não adiciona
header. `tower-http` nem compila o middleware de header (só a feature `fs`).

Ausentes também: `Referrer-Policy`, `X-Content-Type-Options`, `X-Frame-Options` /
`frame-ancestors`, `Permissions-Policy`, `Strict-Transport-Security`, `Cross-Origin-Opener-Policy`.

Sem SRI no único subrecurso externo — o `<link>` do Google Fonts (Risco-004).

A falta de `Referrer-Policy` **agrava** o Risco-004: sem política, o `Referer` completo vai
para `fonts.googleapis.com`.

**Recomendação.** Sequenciar com o Risco-004: auto-hospedar as fontes torna
`default-src 'self'` trivialmente alcançável. Depois, uma camada de headers no router
(`SetResponseHeaderLayer`, exige habilitar a feature `set-header` do `tower-http`) cobre
os dois consumidores — `btv-server` e o `merged_router` do `web_agent` — de uma vez. Os
headers de transporte (HSTS) pertencem ao ingress e devem ser documentados lá.

**Evidência.**
```sh
grep -rniE "content-security-policy|referrer-policy|x-content-type-options|integrity=" \
  crates/ web/ btv-web/ infra/     # vazio
grep -n 'tower-http' Cargo.toml
sed -n '155,165p' crates/btv-server/src/lib.rs
```

---

### Risco-014 · Erros crus — inclusive mensagem de panic — devolvidos ao cliente

| Campo | Conteúdo |
|---|---|
| **Localização** | `crates/btv-server/src/handlers/mod.rs:58-64`; `crates/btv-cli/src/btv_agent.rs` (24 ocorrências de `store_error`); `crates/btv-server/src/handlers/verify.rs:132-140,167-172`; `btv-web/src/api/client.ts:34-37` |
| **Categoria** | Vazamento de informação |
| **Base legal violada** | Art. 46 |
| **Princípio PbD** | (6) Segurança Ponta-a-Ponta |
| **Impacto** | **Médio** — cenário B |

**Problema.** O contrato de erro é `{error, code}`. O `code` é seguro; o `error` é, com
frequência, o `Display` do erro interno:

```rust
// crates/btv-server/src/handlers/mod.rs:58
pub(crate) fn db_error(message: impl std::fmt::Display) -> Response {
    (StatusCode::INTERNAL_SERVER_ERROR,
     Json(ErrorBody::new("prompt_library_error", message.to_string()))).into_response()
}
```

Três famílias:
- **Erro cru de SQLite** — `ErrorBody::new("store_error", e.to_string())`, 24 ocorrências
  só em `btv_agent.rs`.
- **Erro de I/O com caminho absoluto** — `btv_agent.rs:844-853`:
  `format!("arquivo da entrega não encontrado: {e}")`, onde `e` é o `std::io::Error` de
  `fs::read(&deliverable.path)`.
- **Mensagem de panic** — `verify.rs:132-140` faz downcast do payload do `catch_unwind` e
  `:167-172` devolve o texto no corpo JSON.

O client re-lança a string do servidor como `Error.message`
(`btv-web/src/api/client.ts:34-37`) e as telas renderizam verbatim — inclusive a tela
inicial (`Inicio.tsx:146`). O checklist pede `errorId` rastreável internamente, nunca o
detalhe.

**Severidade honesta:** no cenário A isso é quase uma feature — quem lê o erro é o dono da
máquina, e mensagem honesta ajuda a depurar. Vira achado real no cenário B, onde caminho
de arquivo e erro de banco atravessam a rede até um navegador.

**Recomendação.** Manter a cópia amigável da UI e trocar o detalhe por um `errorId`:
`{code, error_id}` na resposta, detalhe completo no log do servidor. Sem perder a
depurabilidade local — o log continua no terminal do dono.

**Evidência.**
```sh
sed -n '55,66p'   crates/btv-server/src/handlers/mod.rs
sed -n '844,853p' crates/btv-cli/src/btv_agent.rs
sed -n '130,142p' crates/btv-server/src/handlers/verify.rs
grep -c 'store_error' crates/btv-cli/src/btv_agent.rs      # 24
```

---

### Risco-015 · Sem framework de log, sem redaction, sem níveis

| Campo | Conteúdo |
|---|---|
| **Localização** | `crates/btv-cli/src/main.rs:332,1042,1049`; `crates/btv-cli/src/squad.rs:469,563`; `python/packages/btv-squad/src/btv_squad/_json.py:34,39`; `.../agents/base.py:72` |
| **Categoria** | Logging |
| **Base legal violada** | Art. 46 |
| **Princípio PbD** | (6) Segurança Ponta-a-Ponta |
| **Impacto** | **Médio** — cenários A e B |

**Problema.** Não há dependência de `tracing`/`log`/`slog` em nenhum crate. Toda a saída
Rust é `println!`/`eprintln!` — **148 call sites** em `crates/` no commit auditado, sem
nível, sem sink configurável e portanto **sem ponto único onde aplicar redaction**.

Os que carregam conteúdo:

| Local | O que sai |
|---|---|
| `main.rs:332` | **o token de sessão SaaS em claro no stdout** (intencional: o operador o vê uma vez — mas vai para histórico de shell, scrollback e log de CI) |
| `main.rs:1049` | cada delta de completion do modelo |
| `main.rs:1042` | o prompt renderizado inteiro |
| `squad.rs:469` | a descrição da tarefa do usuário |
| `squad.rs:563` | o corpo das mensagens entre agentes |
| `gateway.rs:130-136` | URL + os primeiros 300 chars do corpo de erro do provedor |

No Python (usa `logging` do stdlib, nível de `BTV_LOG_LEVEL`, default INFO):

| Local | O que sai |
|---|---|
| `_json.py:34,39` | 200 chars do output cru do modelo, em **WARNING**, a cada falha de parse |
| `agents/base.py:72` | o **dict de decisão inteiro** via `extra={"decision": entry}`, em INFO — invisível no formatter padrão, serializado por completo por qualquer handler estruturado (JSON/DataDog) |

O checklist é explícito: nenhum log em produção contém PII. Não existe helper de redaction,
allowlist nem `Debug` mascarado em lugar nenhum.

**Recomendação.** A correção estrutural é adotar `tracing` com um único ponto de
formatação, e aí a redaction vira uma camada. Enquanto isso, dois itens baratos e de alto
retorno: remover o `extra={"decision": entry}` de `base.py:72` (o log já diz qual agente
registrou) e truncar/hashear o `raw_text` de `_json.py`.

**Evidência.**
```sh
grep -rn 'tracing' crates/*/Cargo.toml    # vazio
grep -rn 'eprintln!\|println!' --include=*.rs crates/ | wc -l   # 148 em a3e14f4
sed -n '30,42p' python/packages/btv-squad/src/btv_squad/_json.py
sed -n '68,74p' python/packages/btv-squad/src/btv_squad/agents/base.py
```

---

### Risco-016 · PII duplicada em seis armazenamentos independentes

| Campo | Conteúdo |
|---|---|
| **Localização** | `.btv/events.db` (`event.data`); `.btv/btv.db` (`ledger.body`); `.btv/cache.db` (`prompt_cache.response`); `.btv/prompt_library.db` (`rendered`); `.btv/squad-memory/agent_memories.jsonl`; `.btv/tool-outputs/*.txt` |
| **Categoria** | Retenção / direitos do titular |
| **Base legal violada** | Art. 18, VI |
| **Princípio PbD** | (3) Privacidade no Design |
| **Impacto** | **Médio** — cenários A e B |

**Problema.** O mesmo turno do usuário pode existir simultaneamente em seis lugares, cada
um com regra de escrita própria e nenhum com regra de exclusão. Dois merecem destaque:
`prompt_cache.response` guarda a **completion inteira** do modelo (a chave é hash do
request, então o prompt não é armazenado — mas a resposta é), e
`orchestrator.py:281-290` serializa o **dict de tarefa inteiro** no JSONL a cada decisão.

Isto não é um risco novo — é o multiplicador dos Riscos 005, 006 e 007. Qualquer
procedimento de eliminação que enderece só um armazenamento dá falsa garantia.

**Recomendação.** Antes de escrever qualquer job de expurgo, produzir o mapa "titular →
onde ele existe". O [RIPD §2](../RIPD.md) é esse mapa; mantê-lo atualizado é o que evita
que o expurgo futuro seja teatro.

**Evidência.** Ver o inventário completo em [RIPD §2](../RIPD.md) e
[modelo de dados (11)](../diagramas/11-modelo-de-dados.md).

---

### Risco-017 · Falha de append no ledger não derruba a requisição

| Campo | Conteúdo |
|---|---|
| **Localização** | `crates/btv-cli/src/btv_agent.rs:656,662,668,673,681,768,1038,1286,1400` |
| **Categoria** | Auditoria |
| **Base legal violada** | Art. 37 (registro das operações de tratamento) |
| **Princípio PbD** | (5) Transparência |
| **Impacto** | **Médio** — cenários A e B |

**Problema.** Nove call sites seguem o padrão:

```rust
if let Err(e) = LedgerRepository::append(&mut *ledger, &ctx, &evento) {
    eprintln!("btv: falha ao registrar remoção de perfil no ledger: {e}");
}
StatusCode::OK.into_response()
```

A operação é reportada como bem-sucedida ao cliente enquanto a trilha de auditoria falhou
em silêncio, num `eprintln!` que ninguém coleta (Risco-015). Uma trilha com furos não
comprova conformidade — que é a única razão de ela existir.

Nota justa: o código melhorou aqui. O comentário em `btv_agent.rs:1396-1399` registra que o
`let _` que engolia o erro foi removido. O passo que falta é decidir o que fazer com o erro
agora que ele é visível.

**Recomendação.** Decidir explicitamente por operação: ou o append é parte da transação (e
a falha derruba a requisição), ou a falha vai para uma fila de reconciliação. "Loga e
segue" não é nenhuma das duas.

**Evidência.**
```sh
grep -n 'falha ao registrar' crates/btv-cli/src/btv_agent.rs
sed -n '1390,1404p' crates/btv-cli/src/btv_agent.rs
```

---

### Risco-028 · Não existe rota de correção de dado cadastral

| Campo | Conteúdo |
|---|---|
| **Localização** | `crates/btv-cli/src/btv_agent.rs:1544-1557` (router) |
| **Categoria** | Direitos do titular |
| **Base legal violada** | **Art. 18, III** (correção); Art. 6º, V (qualidade dos dados) |
| **Princípio PbD** | (7) Centrado no Usuário |
| **Impacto** | **Médio** — cenários A e B |

**Problema.** O router expõe, para `/api/btv/users/{id}`, **apenas `delete`**:

```rust
// crates/btv-cli/src/btv_agent.rs:1548
.route("/api/btv/users/{id}", axum::routing::delete(delete_user_handler))
.route("/api/btv/users/{id}/ativo",      post(set_user_ativo_handler))
.route("/api/btv/users/{id}/pin",        post(set_user_pin_handler))
.route("/api/btv/users/{id}/verify-pin", post(verify_user_pin_handler))
```

Há rota para mudar o estado ativo e o PIN, mas **nenhuma para corrigir `nome` ou
`email`**. O único caminho é apagar e recriar — o que gera um `id` novo, quebra a
atribuição histórica no ledger (`btv.user_removed{user_id}` aponta para um id que não
existe mais) e não corrige o dado nas trilhas já gravadas.

O Art. 6º, V exige que os dados sejam exatos e atualizados *conforme a necessidade*. Um
sistema onde corrigir um e-mail digitado errado exige destruir e recriar a identidade não
atende a isso.

**Recomendação.** `PUT /api/btv/users/{id}` com allowlist de campos corrigíveis, registrado
no ledger como fato de correção (que é o uso legítimo do "override é nova entrada marcada"
de `ledger.rs:5-7`). Diferente do Risco-005, a correção **não** exige apagar nada — é
exatamente o caso que o ledger append-only atende bem.

**Evidência.**
```sh
sed -n '1544,1558p' crates/btv-cli/src/btv_agent.rs
grep -n 'put(.*user' crates/btv-cli/src/btv_agent.rs      # vazio
```

---

### Risco-029 · `prompt_cache` e `telemetry_event` não têm `tenant_id`

| Campo | Conteúdo |
|---|---|
| **Localização** | `crates/btv-store/src/prompt_cache.rs:24-28`; `crates/btv-store/src/telemetry.rs:60-66` |
| **Categoria** | Segregação / minimização |
| **Base legal violada** | Art. 6º, III; Art. 46 |
| **Princípio PbD** | (2) Privacy by Default |
| **Impacto** | **Médio** — cenário B |

**Problema.** Duas tabelas ficaram fora da migração multitenant do ADR 0026:

```sql
CREATE TABLE IF NOT EXISTS prompt_cache (
    hash TEXT PRIMARY KEY, response TEXT NOT NULL, created_at TEXT NOT NULL);

CREATE TABLE IF NOT EXISTS telemetry_event (
    id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL,
    session_id TEXT NOT NULL, props TEXT NOT NULL, ts TEXT NOT NULL);
```

Nenhuma tem `tenant_id`. No cenário A isso é irrelevante — há um titular. No cenário B,
combinado com o Risco-027 (todos no mesmo tenant), significa:

- o **cache de respostas do modelo é compartilhado** entre titulares. A chave é o hash
  canônico do request, então um acerto de cache só ocorre com prompts idênticos — o que
  torna a colisão improvável, não impossível. Registro como **risco condicional**: a
  probabilidade é baixa, o impacto (resposta gerada para o conteúdo de A entregue a B) é
  alto, e a barreira que hoje impede isso é a improbabilidade da colisão, não um controle;
- a **telemetria de todos os titulares vive no mesmo balde**, exposta por `GET /api/events`
  (que não tem autenticação — Risco-001).

**Recomendação.** Se o cenário B se consolidar, incluir as duas tabelas na chave de
tenant. Para o cache, a alternativa mais simples e mais segura é desabilitá-lo em modo
multiusuário — o ganho de latência não compensa a superfície.

**Evidência.**
```sh
sed -n '22,30p' crates/btv-store/src/prompt_cache.rs
sed -n '58,68p' crates/btv-store/src/telemetry.rs
```

---

### Risco-030 · Sem encarregado, sem canal do titular, sem plano de incidente

| Campo | Conteúdo |
|---|---|
| **Localização** | ausência em todo o repositório |
| **Categoria** | Governança |
| **Base legal violada** | **Art. 41** (encarregado); **Art. 18 §1º** (canal de atendimento); **Art. 48** (comunicação de incidente); Art. 37 |
| **Princípio PbD** | (1) Proativo e Preventivo, (5) Transparência |
| **Impacto** | **Médio** — cenários A e B |

**Problema.** Três ausências organizacionais, não técnicas — e por isso fáceis de não ver
numa revisão de código:

1. **Nenhum encarregado (DPO) nomeado.** Um `grep` por `encarregado|dpo|anpd|privacidade@`
   em `docs/`, `crates/`, `btv-web/src` e `web/src` retorna zero (fora este relatório).
2. **Nenhum canal de atendimento ao titular.** Sem endereço, sem formulário, sem endpoint.
   Sem canal, os direitos do Risco-007 não têm nem por onde ser exercidos.
3. **Nenhum plano de resposta a incidente de segurança.** O
   [mapa de failure modes (03)](03-failure-modes.md) é excelente para falha **técnica** —
   sidecar que morre, cadeia corrompida, provedor fora do ar — mas não cobre **vazamento
   de dado pessoal**: quem é acionado, em quanto tempo, o que se preserva como cadeia de
   evidências, quando e como se comunica a ANPD e o titular.

O checklist §13 pede que cada alteração no sistema inclua análise de cenário de incidente
como parte da review. Hoje não há o plano contra o qual fazer essa análise.

**Recomendação.** Os três são baratos em código e caros em decisão — precisam de dono, não
de PR. Ordem sugerida: (a) nomear encarregado e publicar o canal (uma linha no README e
uma tela); (b) escrever o runbook de incidente reaproveitando a estrutura do `03-failure-
modes.md`, que já é o formato certo; (c) definir o SLA de comunicação.

**Evidência.**
```sh
grep -riE "encarregado|\bdpo\b|anpd|privacidade@" docs/ crates/ btv-web/src web/src \
  | grep -v '09-auditoria-lgpd\|RIPD'      # vazio = confirmação
grep -riE "incidente|vazamento|breach" docs/documentacao/mapeamentos/03-failure-modes.md
```

---

## 9.4 P2 — endurecimento, transparência e prevenção de regressão

| ID | Risco | Localização | Artigo | PbD | Cenário | Correção |
|---|---|---|---|---|---|---|
| **Risco-018** | Nome da entrega interpolado sem escape em `content-disposition` — aspas/CR/LF quebram o valor quoted (injeção de header / spoofing de nome de arquivo). `nome` vem de dado influenciado pelo usuário. | `crates/btv-cli/src/btv_agent.rs:867-870,894-897` | Art. 46 | (6) | B | Sanitizar, ou usar `filename*` (RFC 5987). |
| **Risco-019** | `ProviderConfig` deriva `Debug` com `api_key: String`. Nenhum call site formata hoje — um `{:?}` ou `dbg!` futuro imprime a chave. Ponto mais frágil do (ótimo) manejo de segredo. | `crates/btv-llm/src/gateway.rs:16-21` | Art. 46 | (1) | A e B | `impl Debug` manual mascarando o campo. |
| **Risco-020** | Sem política de privacidade, sem termos, sem aviso de tratamento em nenhuma tela. O gear drawer tem só cor de marca e atalhos. | ausente; `btv-web/src/components/shell/GearDrawer.tsx` | Art. 9º | (5),(7) | A e B | Página de transparência listando o que é coletado, onde fica e para onde vai (Riscos 003/004). |
| **Risco-021** | "Anexar arquivos" no wizard captura **só o nome**, nunca os bytes (`{ t: 'arquivo', label: f.name }`) — mas a UI ("arraste arquivos aqui ou clique para anexar") sugere upload. E nome de arquivo é dado pessoal (`contrato-joao-silva.pdf`). | `btv-web/src/components/wizard/Wizard.tsx:225,242` | Art. 6º, VI | (5) | A e B | Corrigir a cópia da UI. O comportamento (não subir bytes) está certo. |
| **Risco-022** | `props` da telemetria é `serde_json::Value` irrestrito. Hoje só `model` + tokens (ver §9.5), mas nada impede um caller futuro de gravar prompt — e `GET /api/events` devolve o blob verbatim. Convenção, não fronteira. | `crates/btv-store/src/telemetry.rs:60-66` | Art. 46 | (3) | A e B | Tipar `props` com um enum fechado por evento. |
| **Risco-023** | Consoles devolvem diagnóstico interno em corpo 200: `/api/doctor` expõe linhas ofensoras do scan (`tabela[linha].coluna = valor`), `/api/mcp` o `command` completo de cada servidor, `/api/lsp` idem. | `crates/btv-server/src/doctor_console.rs:152-166`; `crates/btv-cli/src/mcp_console.rs:94-111` | Art. 46 | (6) | B | Resumir no corpo; detalhe só no log. |
| **Risco-024** | Submodule `vendor/bpmn` de repositório GitHub externo entra no bundle do SPA; não auditado (checkout vazio nesta análise). Pinado por commit + lockfile — o controle correto. | `.gitmodules:1-3`; `btv-web/scripts/ensure-bpmn.mjs:14-38` | Art. 39 | (1) | A e B | Revisão obrigatória a cada movimento do pin. |
| **Risco-025** | Sem CI de privacidade: nenhum lint de PII em URL/query, nenhum gate de catálogo de dados. O CI já tem gitleaks (bloqueante), cargo-deny e `arch-lint.sh` — que falha o build por fronteira arquitetural e é o ponto de extensão natural. | `.github/workflows/ci.yml`; `scripts/arch-lint.sh` | Art. 46; Art. 50 | (1) | A e B | Regra nova no `arch-lint.sh`: barrar `?cpf=`/`?email=`/`?phone=` em rota, e campo novo de PII sem entrada no catálogo. |
| **Risco-026** | **A documentação contradiz o código.** `crates/btv-server/src/lib.rs:5` afirma *"Nada sai da máquina do usuário — o servidor escuta só em `127.0.0.1`"*; `mapeamentos/08-*.md` §8.4 afirma *"Não há endpoint na internet pública"*. O `docker-compose.prod.yml:42` (bind `0.0.0.0`) e o Google Fonts (Risco-004) dizem o contrário. | `crates/btv-server/src/lib.rs:5`; `mapeamentos/08-*.md:70-73` | Art. 6º, VI | (5) | A e B | Corrigir os dois textos. Transparência começa pelo que o sistema afirma sobre si. |

---

## 9.5 CONFORME — controles verificados que devem ser creditados

Uma auditoria que só lista falhas mente por omissão e destrói a confiança no próprio
relatório. Estes controles foram verificados e **passam**:

| # | Controle | Evidência |
|---|---|---|
| 1 | **API keys nunca tocam disco.** Só env var → memória do processo Rust → header HTTPS. Nunca vão ao Python (ADR 0001), nunca ao ledger, nunca à telemetria, nunca à UI. Não há `.env`, `dotenv`, `python-dotenv` nem `env_file:` no compose. É a parte mais forte do código. | `crates/btv-llm/src/gateway.rs:60-95,116,124` |
| 2 | **Telemetria sem conteúdo.** Só `model` + contagem de tokens. Sem prompt, sem completion, sem caminho, sem usuário, sem IP, sem hostname. | `crates/btv-cli/src/rate_limit_gen.rs:47-56`; `crates/btv-cli/src/cache.rs:68-85` |
| 3 | **`localStorage` só com preferência de UI** — `btv_theme` e `btv_accent` (enum de 5 cores). Exatamente o que o checklist §4 exige. Sem `sessionStorage`, sem `IndexedDB`, sem `document.cookie`, **sem sessão client-side alguma**. | `web/src/state/AppContext.tsx:24-63`; `btv-web/src/state/AppContext.tsx:31-76` |
| 4 | **Zero analytics, pixels, tag managers ou SDK de erro.** Ambos os frontends têm exatamente duas dependências de runtime: `react` e `react-dom`. | `web/package.json:16-19`; `btv-web/package.json:20-23` |
| 5 | **Todo tráfego de aplicação é same-origin relativo para loopback.** Nenhuma URL absoluta, nenhum CORS, nenhum beacon. (A exceção é o Google Fonts — Risco-004 — disparado pelo navegador, não pelo código da app.) | `web/src/api/client.ts:24-51`; `btv-web/src/api/client.ts:20-43` |
| 6 | **404, nunca 403, para recurso de outro tenant** — item explícito do checklist §3, implementado. *Ressalva honesta: sem autorização (Risco-001), a indistinguibilidade protege pouco.* | `crates/btv-cli/src/btv_agent.rs:19-23` |
| 7 | **Guard de CSRF/DNS-rebinding real e fail-closed** em rota mutável com `Origin` presente — dentro do que se propõe a ser, e o ADR é explícito que não substitui autenticação. | `crates/btv-server/src/guard.rs:22-36`; ADR 0015 |
| 8 | **Permissão de ferramenta fail-closed**: timeout → Deny, e a matriz de permissão é auditada. | ADR 0017; ADR 0018 |
| 9 | **`user.turn` no ledger grava só a contagem de caracteres**, não o texto — minimização deliberada. Idem `llm.turn` (só provedor + tokens). Prova que a disciplina do Risco-005 é alcançável. | `crates/btv-cli/src/main.rs:837`; `crates/btv-cli/src/session.rs:65-66` |
| 10 | **Prompt de persona vai ao ledger como `sha256`**, não em claro — procedência sem conteúdo. | `crates/btv-domain/src/ports.rs:208-213`; `crates/btv-cli/src/btv_agent.rs:1033` |
| 11 | **Token de sessão SaaS**: 256 bits CSPRNG, prefixo grepável `btvs_`, só o hash persistido, deadline absoluto 30d + idle 24h. Padrão correto. | `crates/btv-store/src/pg.rs:1054-1070`; `migrations_pg/0002_sessions.sql:21-33` |
| 12 | **`X-Tenant-Id` vetado**; tenant só de sessão autenticada server-side, fail-closed sem fallback para LOCAL. Decisão certa — ainda que o login não exista (Risco-001). | ADR 0029 |
| 13 | **Mensagem de PIN incorreto constante**, sem revelar a causa. | `btv-web/src/components/screens/admin/Usuarios.tsx:70,73` |
| 14 | **RLS por tenant no Postgres + `WHERE tenant_id`** em profundidade; hash-chain com o tenant dentro do hash (anti-transplante). | ADR 0026; ADR 0027; `migrations_pg/0001_schema_multitenant.sql:86-111` |
| 15 | **`recall.py` (TF-IDF) não persiste nada** — índice derivado, reconstruído em memória, offline. Não é cópia adicional de dado pessoal, e é rotulado honestamente como léxico, não semântico. **`btv_promptforge` é stateless** — nenhuma escrita em disco, nenhum conteúdo logado. | `python/packages/btv-squad/src/btv_squad/recall.py:19-22`; `btv_promptforge/server.py:34-87`; ADR 0013 |

---

## 9.6 NÃO APLICÁVEL — e a razão

"Não se aplica" só é resultado válido de auditoria quando vem com a razão. Estes itens do
checklist não incidem sobre este sistema:

| Item do checklist | Por que não se aplica |
|---|---|
| **Consentimento granular, toggles default-OFF, "Recusar tudo"** | Não há tratamento baseado em consentimento hoje, nem script de terceiro a ser gateado. O único item que *precisaria* de gate é o Google Fonts — e a correção certa ali é **remover** a transferência (Risco-004), não pedir consentimento para uma coisa evitável. |
| **Decisão automatizada e direito à revisão (Art. 20)** | O sistema não decide nada *sobre pessoas*: gera texto e código a pedido. Não há score, triagem, elegibilidade nem classificação de indivíduo. Sem gatilho de Art. 20, não há canal de revisão a exigir. |
| **Dado pessoal sensível (Art. 11)** | Não há biometria, saúde, origem racial, convicção religiosa, opinião política, filiação sindical ou dado genético em nenhum campo do schema. **Este resultado depende do descope de §9.1**: se conteúdo de terceiros entrar no escopo (um briefing de RH, um caso clínico), a premissa cai e o Risco-003 sobe de patamar — passa a exigir modelo on-premise ou contrato específico. Registrado como pressuposto no [RIPD §2.4](../RIPD.md). |
| **Governança de treino de ML: dataset pseudonimizado, teste de disparidade, features proxy, AIA, membership inference, sanitização pós-inferência** | O projeto **não treina nem faz fine-tune** de modelo algum, e não há dataset de treino a pseudonimizar. O `recall.py` é TF-IDF léxico local. *Permanece do checklist §10 apenas o Risco-003 (opt-out de treino do lado do fornecedor) — que é gestão de fornecedor, não governança de ML própria.* |
| **PII em URL / linter de `?cpf=`, `?email=`** | Nenhuma rota recebe CPF, e-mail ou telefone em path ou query; `email` só entra no corpo do POST. O problema com ids é anti-enumeração (Risco-002), não PII em URL. O lint preventivo continua recomendado (Risco-025). |
| **k-anonimato (k ≥ 5) em dashboards** | A agregação do dashboard é do próprio operador sobre a própria telemetria da máquina, não sobre uma população de titulares. Não há célula a suprimir. |
| **Contratos de dados com parceiro, filtragem de payload por allowlist, chave por parceiro, revogação por webhook em 24h** | Não há parceiro recebendo dados. Os únicos terceiros são os provedores de LLM (Risco-003) e o Google Fonts (Risco-004) — tratados como transferência internacional, não como integração de parceiro. |

---

## 9.7 Como reproduzir esta auditoria

Todo achado deste documento é verificável por leitura estática. Os comandos abaixo
reproduzem as evidências principais; **ausência se prova com o grep que retorna vazio.**

```sh
# ── P0 ────────────────────────────────────────────────────────────────────
# Risco-001 — o guard só olha não-GET, e Origin ausente passa; tenant é no-op local
sed -n '14,40p'   crates/btv-server/src/guard.rs
sed -n '158,205p' crates/btv-cli/src/tenant_extractor.rs

# Risco-002 — id sequencial de squad e replay do snapshot no subscribe
sed -n '140,150p' crates/btv-cli/src/squad_agent.rs
sed -n '204,216p' crates/btv-cli/src/squad_agent.rs

# Risco-003 — destinos da transferência internacional
sed -n '58,95p' crates/btv-llm/src/gateway.rs
grep -rn "DEFAULT_BASE_URL\|OPENAI_BASE_URL\|DEEPSEEK_BASE_URL" crates/btv-llm/src/

# Risco-004 — a única requisição externa dos frontends
grep -rn "https://" btv-web/index.html web/index.html

# Risco-005 — o hash cobre o payload, e há texto livre no payload
sed -n '56,80p'   crates/btv-schemas/src/ledger.rs
sed -n '162,200p' crates/btv-domain/src/ports.rs

# Risco-006 — ausência de retenção (só sai documentação e deadlines de sessão)
grep -rniE "VACUUM|retention|retencao_ate|\bttl\b|expire|purge|prune|older_than" \
  --include=*.rs --include=*.py crates/ python/

# Risco-027 — BTV_MODE ausente no compose (vazio = confirmação) e o ator fixo
grep -n "BTV_MODE" infra/docker/docker-compose.prod.yml
sed -n '28,32p' crates/btv-cli/src/tenant_extractor.rs

# ── P1 ────────────────────────────────────────────────────────────────────
# Risco-008 — sem cifra e sem permissão restritiva (ambos devem sair vazios)
grep -rniE "sqlcipher|encrypt|cipher|\baes\b|argon2|bcrypt|pbkdf2" \
  --include=*.rs --include=*.py crates/ python/
grep -rn "set_permissions\|PermissionsExt\|0o700" --include=*.rs crates/

# Risco-009 — o hash do PIN e o salt derivável
sed -n '1245,1256p' crates/btv-store/src/btv.rs

# Risco-010 — entidade de domínio direto no wire
sed -n '24,36p'     crates/btv-domain/src/user.rs
sed -n '1326,1340p' crates/btv-cli/src/btv_agent.rs

# Risco-012 — sem rate limit / body limit / timeout (deve sair vazio)
grep -rn "RateLimitLayer\|governor\|TimeoutLayer\|DefaultBodyLimit" --include=*.rs crates/

# Risco-013 — sem CSP nem header de segurança (deve sair vazio)
grep -rniE "content-security-policy|referrer-policy|x-content-type-options|integrity=" \
  crates/ web/ btv-web/ infra/

# Risco-014 — contagem dos erros crus
grep -c 'store_error' crates/btv-cli/src/btv_agent.rs

# Risco-015 — sem framework de log
grep -rn 'tracing' crates/*/Cargo.toml

# Risco-028 — só DELETE em /api/btv/users/{id}, sem rota de correção
sed -n '1544,1558p' crates/btv-cli/src/btv_agent.rs

# Risco-029 — DDL de cache e telemetria sem tenant_id
sed -n '22,30p' crates/btv-store/src/prompt_cache.rs
sed -n '58,68p' crates/btv-store/src/telemetry.rs

# Risco-030 — sem encarregado, canal ou plano de incidente (vazio = confirmação)
grep -riE "encarregado|\bdpo\b|anpd|privacidade@" docs/ crates/ btv-web/src web/src \
  | grep -v '09-auditoria-lgpd\|RIPD'

# ── Cenário B ─────────────────────────────────────────────────────────────
grep -n "0.0.0.0\|BTV_TRUSTED_ORIGINS" infra/docker/docker-compose.prod.yml

# ── CONFORME ──────────────────────────────────────────────────────────────
grep -rn "localStorage\|sessionStorage\|document.cookie" web/src btv-web/src
sed -n '47,56p' crates/btv-cli/src/rate_limit_gen.rs
```

---

## 9.8 Ordem de ataque sugerida

A ordem não é a da severidade — é a das dependências. Vários P0 não têm correção possível
antes de outro.

1. **Risco-004 (Google Fonts)**, **Risco-026 (documentação)** e **Risco-027 (documentar o
   tenant/ator único do compose)** — baratos, independentes, e o primeiro destrava o
   Risco-013.
2. **Risco-030 (encarregado, canal, runbook de incidente)** — não é código, é dono. Pode
   começar hoje, em paralelo a tudo.
3. **Risco-008 (`0o700`)** — uma linha por call site, fecha o vetor local mais provável.
4. **Risco-003 (fornecedor de LLM)** — decisão de governança, não de código: qual
   provedor, sob qual base do Art. 33, com qual documento.
5. **Risco-001 (identidade)** — é pré-requisito do Risco-002, do Risco-007, do Risco-027 e
   de qualquer rate limit por ator. O desenho já está aceito no ADR 0029.
6. **Risco-006 (catálogo → retenção)**, nesta ordem: catálogo primeiro, job depois.
7. **Risco-005 (eliminação vs. ledger)** — o mais caro e o que merece ADR próprio.

Os P1 de higiene (011, 012, 013, 014, 015, 028, 029) podem correr em paralelo a qualquer
momento; nenhum depende dos anteriores. O Risco-028 (rota de correção) é o mais barato
deles e fecha um inciso inteiro do Art. 18.
