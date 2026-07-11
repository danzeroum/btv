# 10 — Mapa de dados: domínio e contratos Rust (btv-domain, btv-schemas)

Escopo: dicionário de dados exaustivo dos crates `crates/btv-domain` (núcleo
DDD sem infraestrutura — tenant, agregado Run, eventos, ports) e
`crates/btv-schemas` (contratos serializados/auditáveis — ledger, verification,
experiment, persona, plan, review, squad_template, telemetry, workflow, handoff,
e o hash canônico de cache de prompt). Cada arquivo `.rs` das duas fatias foi
lido linha a linha. Nenhum arquivo de código foi modificado.

Legenda da taxonomia de Direção (rótulo exato usado na coluna "Direção"):

- `entrada` — dado que entra do exterior: parâmetro de fn pública/privada, arg,
  leitura de disco/fixture, mensagem recebida.
- `saída` — dado que sai: valor de retorno, escrita, evento/mensagem emitido.
- `intermediário` — variável local, buffer, acumulador ou valor calculado
  (mesmo que descartado dentro da própria função).
- `estado` — dado retido em campo de struct entre chamadas.
- `config` — constante, env var ou fonte de configuração/catálogo embutido.
- `wire` — dado que cruza fronteira serializada: campo serde/schemars, coluna de
  banco, campo de proto/JSON schema, representação textual congelada por golden.

Observação transversal: em `btv-domain`, os tipos do produto (`Run`,
`Deliverable`, `User`, `PersonaOverride`, `CustomPersona`) carregam um campo
`tenant: TenantId` marcado `#[serde(skip_serializing)]` — é `estado` no domínio
mas NÃO é `wire` nesta fase (o adapter o preenche do `TenantContext`; goldens T1
congelam a ausência no JSON).

---

# Parte A — `crates/btv-domain`

## `crates/btv-domain/src/lib.rs`
Raiz do crate de domínio: declara os módulos e re-exporta os tipos públicos.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| módulos `chat`,`event`,`ledger_kind`,`persona`,`ports`,`run`,`tenant`,`tool`,`user` | `pub mod` | config | árvore de módulos → consumidores | fronteira arquitetural verificada por `scripts/arch-lint.sh` (sem rusqlite/axum/tonic/reqwest, direta ou transitiva) |
| re-export `LedgerKind, UnknownKind` | tipos | saída | `ledger_kind` → API pública | vocabulário fechado do ledger |
| re-export `CustomPersona, PersonaOverride` | tipos | saída | `persona` → API pública | personas U7 |
| re-export `BriefingResposta, Deliverable, InvalidTaskId, Run, TaskId` | tipos | saída | `run` → API pública | agregado e value objects do produto |
| re-export `ActorId, TenantContext, TenantError, TenantId` | tipos | saída | `tenant` → API pública | multitenancy fail-closed |
| re-export `PinCheck, User` | tipos | saída | `user` → API pública | perfil local |

Fluxo: entrada = nenhuma (só declaração); processamento = agrega submódulos;
saída = superfície pública do crate re-exportada para os adapters e o loop de agente.

## `crates/btv-domain/src/chat.rs`
Representação de conversa/tool-use neutra de provider (D1t): tipos que o loop de
agente consome via `LlmPort`, sem conhecer HTTP/provider.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `Role` = `User`\|`Assistant` | enum | wire | serde `snake_case` | papel da mensagem |
| `ContentBlock::Text.text` | `String` | wire | serde tag `type=text` | texto puro |
| `ContentBlock::ToolUse{id,name,input}` | `String,String,Value` | wire | serde tag `type=tool_use` | pedido do modelo p/ executar ferramenta; `input` = args JSON opacos |
| `ContentBlock::ToolResult{tool_use_id,content,is_error}` | `String,String,bool` | wire | serde tag `type=tool_result` | resultado devolvido ao modelo |
| `ChatMessage.role` / `.content` | `Role` / `Vec<ContentBlock>` | wire/estado | serde | uma mensagem completa |
| `ChatMessage::user_text(text)` param `text` | `impl Into<String>` | entrada | arg → `ContentBlock::Text` | fábrica: monta msg `User` com um bloco de texto |
| `ChatMessage::user_text` retorno | `Self` | saída | → chamador | mensagem de usuário pronta |
| `ToolSpec.name`/`.description`/`.input_schema` | `String,String,Value` | wire | serde | especificação anunciada ao modelo; `input_schema` = JSON Schema dos args |
| `StopReason` = `EndTurn`\|`ToolUse`\|`MaxTokens`\|`Other` | enum | wire | serde `snake_case` | razão de parada do turno |
| `Usage.input_tokens`/`.output_tokens` | `u64,u64` | wire | serde (`Default`) | contagem de tokens |
| `AssistantTurn.content` | `Vec<ContentBlock>` | wire/estado | agregado do stream | blocos do turno |
| `AssistantTurn.stop_reason` | `StopReason` | wire | serde | parada |
| `AssistantTurn.usage` | `Usage` | wire | serde | uso de tokens |
| `AssistantTurn.provider` | `String` | wire | serde | provider que atendeu (telemetria/ledger) |
| `AssistantTurn::tool_uses()` retorno | `Vec<(&str,&str,&Value)>` | saída | filtro sobre `content` → chamador | extrai `(id,name,input)` só dos blocos `ToolUse` |
| `AssistantTurn::text()` retorno | `String` | saída | concat sobre `content` → chamador | junta o texto de todos os blocos `Text` (join `""`) |
| `GenerateRequest.model` | `String` | entrada | loop de agente → gateway | id do modelo |
| `GenerateRequest.system` | `String` | entrada | idem | prompt de sistema |
| `GenerateRequest.messages` | `Vec<ChatMessage>` | entrada | idem | histórico |
| `GenerateRequest.tools` | `Vec<ToolSpec>` | entrada | idem | ferramentas anunciadas |
| `GenerateRequest.max_tokens` | `u32` | entrada | idem | teto de saída |
| `GenerateRequest.temperature` | `Option<f64>` | entrada | idem | temperatura (opcional) |
| `ModelTier` = `Small`\|`Medium`\|`Large` | enum | wire (Serialize-only) | serde `snake_case` | classe de capacidade; classificação de id→tier fica em `btv-llm` |
| `ModelTier::compaction_threshold()` retorno | `f64` | saída | `self` → chamador | `Small`→0.75, demais→0.90 (fração da janela p/ disparar compaction) |

Fluxo: entrada = `GenerateRequest` (model/system/messages/tools/limites) montado
pelo loop; processamento = neutro de provider (só tipos); saída = `AssistantTurn`
(content/stop_reason/usage/provider), do qual `tool_uses()`/`text()` derivam
projeções para o loop.

## `crates/btv-domain/src/tool.rs`
Contrato de ferramenta e tipos de dado de saída (D1t); o cálculo de diff mora em
`btv-tools`, aqui só o tipo repassado aos observadores.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `ToolError::InvalidArgs(String)` | enum var | saída | erro → chamador | argumentos inválidos |
| `ToolError::Execution(String)` | enum var | saída | erro → chamador | falha de execução |
| `DiffLine` = `Context`\|`Removed`\|`Added` (cada `String`) | enum | wire | serde default de tuple-variant | linha de diff colorida (TUI/web) |
| `ToolOutput.content` | `String` | saída/wire | ferramenta → observador | saída textual da ferramenta |
| `ToolOutput.truncated` | `bool` | saída/wire | serde | se o output foi truncado |
| `ToolOutput.overflow_path` | `Option<String>` | saída/wire | serde | caminho do output completo persistido (Managed Tool Output File) |
| `ToolOutput.diff` | `Option<Vec<DiffLine>>` | saída/wire | serde | diff quando a ferramenta alterou arquivo texto (hoje `edit`) |
| `Tool::name()` retorno | `&str` | saída | impl → registry/modelo | identidade estável |
| `Tool::description()` retorno | `&str` | saída | idem | descrição p/ o modelo |
| `Tool::input_schema()` retorno | `Value` | saída | idem | JSON Schema dos args anunciado ao modelo |
| `Tool::scope(args)` param `args` / retorno | `&Value` / `String` | entrada / saída | args JSON → motor de permissões | escopo avaliado (caminho, comando…) |
| `Tool::run(args)` param `args` / retorno | `&Value` / `Result<ToolOutput,ToolError>` | entrada / saída | args JSON → execução | executa a ferramenta |

Fluxo: entrada = `args: &Value` (JSON do modelo); processamento = `scope`/`run`
na implementação (em btv-tools); saída = `ToolOutput` (content/truncated/
overflow_path/diff) ou `ToolError`.

## `crates/btv-domain/src/tenant.rs`
Tenant como tipo, fail-closed por construção (D1/ADR 0025). Modo local É um
tenant (`TenantId::LOCAL`), não a ausência de um.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `TenantError::InvalidTenantId(String)` | enum var | saída | erro → chamador | id textual inválido |
| `TenantError::EmptyActor` | enum var | saída | erro → chamador | actor vazio (auditoria sem autor proibida) |
| `TenantId(Uuid)` | newtype opaco | estado/wire | serde = UUID textual | sem `From<String>`/`From<Uuid>`, sem `Default` — construção só por `parse`/`LOCAL` |
| `TenantId::LOCAL` | `const TenantId` | config | `Uuid::from_u128(1)` | `00000000-0000-0000-0000-000000000001`, backfill determinístico |
| `TenantId::parse(s)` param `s` / retorno | `&str` / `Result<Self,TenantError>` | entrada / saída | UUID textual → TenantId | borda de auth SaaS; `Uuid::parse_str` valida |
| `TenantId::local()` retorno | `Self` | saída | → serde `default` | default EXPLÍCITO (não `impl Default`), retorna `LOCAL` |
| `Display for TenantId` | `String` | saída | `self.0` → fmt | UUID textual |
| `ActorId(String)` | newtype | estado/wire | serde | não-vazio por construção; convenção de prefixo `web:`, `btv-cli:` |
| `ActorId::new(actor)` param `actor` / retorno | `impl Into<String>` / `Result<Self,TenantError>` | entrada / saída | string → ActorId | `trim().is_empty()` → `EmptyActor` |
| `ActorId::as_str()` retorno | `&str` | saída | `self.0` → chamador | acesso ao string interno |
| `Display for ActorId` | `String` | saída | `self.0` → fmt | escreve o string |
| `TenantContext.tenant` | `TenantId` | estado | campo | tenant obrigatório de toda operação de repo |
| `TenantContext.actor` | `ActorId` | estado | campo | autoria que viaja p/ o ledger |
| `TenantContext::new(tenant,actor)` params/retorno | `TenantId,ActorId` / `Self` | entrada / saída | args → contexto | sem `Default` — decisão explícita do chamador |
| `TenantContext::local(actor)` param `actor` / retorno | `ActorId` / `Self` | entrada / saída | actor → contexto LOCAL | tenant fixo `LOCAL` + actor explícito |

Fluxo: entrada = string de tenant/actor da borda; processamento = validação
(UUID válido / actor não-vazio) na construção; saída = `TenantContext` que
atravessa toda a Trilha B como `&TenantContext`. Wire: `TenantId` serializa como
UUID textual (exercido só quando exposto na Trilha E).

## `crates/btv-domain/src/user.rs`
Perfil local (A6) — embrião do Contexto de Identidade; `BtvUser` movido de
`btv-store::btv` com `tenant` desde já.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `PinCheck` = `NoPin`\|`Ok`\|`Wrong` | enum | saída | `verify_pin` (adapter) → domínio | veredito da verificação de PIN; o hash nunca sai do adapter |
| `User.id` | `i64` | wire | serde | id do perfil |
| `User.nome` | `String` | wire | serde | nome (campo pt congelado) |
| `User.email` | `String` | wire | serde | email |
| `User.papel` | `String` | wire | serde | papel do perfil |
| `User.ativo` | `bool` | wire | serde | perfil ativo |
| `User.has_pin` | `bool` | wire | serde | se exige PIN; expõe presença, nunca o hash |
| `User.tenant` | `TenantId` | estado | `#[serde(skip_serializing)]` | dono; fora do wire nesta fase |
| teste `tenant_fica_fora_do_wire` | `User` dummy → `Value` | intermediário | `to_value` | prova que `tenant` some e o JSON tem 6 campos |

Fluxo: entrada = campos vindos do adapter (leitura do store); processamento =
nenhum comportamento (só tipagem); saída = JSON de 6 campos (sem tenant) e
`PinCheck` derivado pela porta.

## `crates/btv-domain/src/persona.rs`
Personas U7 — override de prompt por template+papel e personas próprias
(movidos de `btv-store::btv`, só tipagem).

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `PersonaOverride.template_id` | `String` | wire | serde | template alvo |
| `PersonaOverride.papel` | `String` | wire | serde | papel sobreposto (campo pt) |
| `PersonaOverride.prompt` | `String` | wire | serde | prompt efetivo (auditado como `btv.persona_updated`) |
| `PersonaOverride.tenant` | `TenantId` | estado | `skip_serializing` | dono; fora do wire |
| `CustomPersona.id` | `i64` | wire | serde | id da persona própria |
| `CustomPersona.template_id` | `String` | wire | serde | template |
| `CustomPersona.nome` | `String` | wire | serde | nome |
| `CustomPersona.prompt` | `String` | wire | serde | prompt |
| `CustomPersona.tenant` | `TenantId` | estado | `skip_serializing` | dono; fora do wire |
| teste `tenant_fica_fora_do_wire` | `CustomPersona` dummy → `Value` | intermediário | `to_value` | prova 4 campos serializados, sem tenant |

Fluxo: entrada = dados do adapter; processamento = só tipagem (Serialize-only);
saída = JSON sem tenant (override 3 campos; custom 4 campos).

## `crates/btv-domain/src/run.rs`
`Run` (squad ativada) e `Deliverable` (entrega da Biblioteca), `TaskId` e
`BriefingResposta` — value objects do produto BTV (A2/A3).

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `Run.id` | `i64` | estado/wire | serde | id; `0` = ainda não persistido |
| `Run.task_id` | `TaskId` | estado/wire | serde `sq{hex}` | id da tarefa de squad |
| `Run.template_id` | `String` | estado/wire | serde | template ativado |
| `Run.template_versao` | `String` | estado/wire | serde | versão (campo pt) |
| `Run.nome` | `String` | estado/wire | serde | nome da ativação |
| `Run.briefing_json` | `String` | estado/wire | serde (JSON `[{label,resposta}]` embutido) | respostas do briefing serializadas |
| `Run.papeis_json` | `String` | estado/wire | serde (JSON `["papel",…]`) | papéis ativos; DERIVADO de `roles` em `activate` |
| `Run.status` | `RunStatus` | estado/wire | serde = `as_str` | máquina de transições; muta só pelo agregado |
| `Run.gates_aprovados` | `i64` | estado/wire | serde | contador de gates HITL |
| `Run.created_ts`/`.updated_ts` | `String,String` | estado/wire | serde | RFC3339 do chamador |
| `Run.tenant` | `TenantId` | estado | `skip_serializing` | dono; fora do wire (11 campos no wire) |
| `Deliverable.id`/`.run_id` | `i64,i64` | estado/wire | serde | id da entrega e do run pai |
| `Deliverable.task_id` | `TaskId` | estado/wire | serde | tarefa |
| `Deliverable.template_id` | `String` | estado/wire | serde | template |
| `Deliverable.nome`/`.path`/`.formato`/`.versao`/`.trilha` | `String`×5 | estado/wire | serde | nome, caminho do arquivo REAL, formato, versão, trilha de procedência (campos pt) |
| `Deliverable.created_ts` | `String` | estado/wire | serde | RFC3339 |
| `Deliverable.tenant` | `TenantId` | estado | `skip_serializing` | dono; fora do wire (10 campos no wire) |
| `TaskId(u64)` | newtype | estado/wire | serde via `collect_str`/`parse` | única repr textual = `sq{hex}` |
| `InvalidTaskId(String)` | erro | saída | `parse` → chamador | string fora de `sq{hex}` |
| `TaskId::new(seq)` param/retorno | `u64` / `Self` | entrada / saída | seq → TaskId | wrapper |
| `TaskId::seq()` retorno | `u64` | saída | `self.0` → chamador | expõe o seq |
| `TaskId::parse(s)` param `s` / retorno | `&str` / `Result<Self,InvalidTaskId>` | entrada / saída | `sq{hex}` → TaskId | `strip_prefix("sq")` + `from_str_radix(16)`; leniente c/ hex maiúsculo; fail-closed em prefixo/hex/overflow |
| `Display for TaskId` | `String` | saída/wire | `write!("sq{:x}")` | fonte da repr textual |
| `Serialize/Deserialize for TaskId` | via `collect_str`/`String::deserialize`+`parse` | wire | serde | round-trip byte-a-byte (proptest) |
| `BriefingResposta.label`/`.resposta` | `String,String` | wire | serde (campos pt) | item do `briefing_json`; mesmo shape do corpo de `POST /api/btv/squads` |
| teste `tenant_fica_fora_do_wire` | Run/Deliverable dummy → `Value` | intermediário | `to_value` | 11 e 10 campos; `status="ativa"`, `task_id="sq1"` |
| teste `task_id_roundtrip_display_parse` (proptest) | `seq: u64` | intermediário | any u64 → display→parse | `parse(display)==id` e JSON `"sq{seq:x}"` |
| teste `briefing_roundtrip_byte_a_byte` | JSON real → `Vec<BriefingResposta>` | intermediário | fixture → parse → reserialize | round-trip byte-idêntico; rejeita shape errado e não-JSON |

Fluxo: entrada = campos do adapter / `roles` na factory; processamento = deriva
`papeis_json`, valida `TaskId`/`status`; saída = JSON de wire congelado (sem
tenant) e representações textuais estáveis (`sq{hex}`, strings de status).

## `crates/btv-domain/src/event.rs`
Tipos do event store (D1t) — nasceram em `btv-store::events`, byte-idênticos
para o replay ler JSON antigo.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `EventInput.kind` | `String` | wire | serde `rename="type"` | tipo+versão embutida (ex.: `message.1`) |
| `EventInput.data` | `Value` | wire | serde | corpo livre |
| `EventInput::new(kind,data)` params/retorno | `impl Into<String>,Value` / `Self` | entrada / saída | args → EventInput | wrapper; `id`/`seq` atribuídos pelo store |
| `StoredEvent.id` | `String` | wire | serde | id do evento persistido |
| `StoredEvent.aggregate_id` | `String` | wire | serde | agregado; `(aggregate_id,seq)` único |
| `StoredEvent.seq` | `i64` | wire | serde | sequência |
| `StoredEvent.kind` | `String` | wire | serde `rename="type"` | tipo+versão |
| `StoredEvent.data` | `Value` | wire | serde | corpo |

Fluxo: entrada = `EventInput` (kind/data) do chamador; processamento = store
atribui `id`/`seq`; saída = `StoredEvent` para replay (a sessão durável de
`btv-core` reconstrói estado sem conhecer o driver).

## `crates/btv-domain/src/ledger_kind.rs`
`LedgerKind` — vocabulário FECHADO dos 21 kinds reais de produção (A3), provado
contra a fixture canônica.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `LedgerKind` (21 variantes) | enum | wire (vocabulário) | — | 8 fatos de domínio `Btv*` + 13 de instrumentação operacional |
| `LedgerKind::ALL` | `[LedgerKind;21]` | config | const → testes | base do round-trip e cobertura vs. fixture |
| `LedgerKind::as_str()` retorno | `&'static str` | saída/wire | match exaustivo → string do banco | ex.: `BtvGateApproved`→`"btv.gate_approved"`, `LlmTurn`→`"llm.turn"` |
| `LedgerKind::parse(s)` param `s` / retorno | `&str` / `Result<Self,UnknownKind>` | entrada / saída | string do banco → variante | busca em `ALL`; fail-closed = `UnknownKind` |
| `Display for LedgerKind` | `String` | saída | `as_str` → fmt | escreve a string do banco |
| `UnknownKind(String)` | erro | saída | `parse` → chamador | kind fora do vocabulário |
| teste `kinds_da_fixture()` | fixture `wire-strings.v1.json` → `BTreeSet<String>` | entrada→intermediário | leitura de disco | extrai `ledger_kinds` da fixture |
| testes de vocabulário/round-trip/exclusão | conjuntos de string | intermediário | enum × fixture | igualdade de conjuntos; `certification` NÃO parseia (exclusão consciente) |

Fluxo: entrada = string de kind (call site / banco); processamento = parse
fail-closed contra vocabulário fechado; saída = variante tipada ou `UnknownKind`.
O enum é dado (vocabulário), não porta.

## `crates/btv-domain/src/ports.rs`
Coração do domínio: `RunStatus` (máquina), agregado `Run`
(`activate`/`activation_event`/`approve_gate`/`transition_to`), `DomainEvent` +
`DomainEventKind`, erros, e as traits de repositório e do runtime de agente.

RunStatus e serde:

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `RunStatus` = `Ativa`\|`Concluida`\|`Encerrada`\|`Erro` | enum | wire | serde manual = `as_str` | 4 estados reais do banco (T3) |
| `RunStatus::ALL` | `[RunStatus;4]` | config | const → testes | cobertura exaustiva |
| `RunStatus::as_str()` retorno | `&'static str` | saída/wire | match → coluna do banco | `ativa`/`concluida`/`encerrada`/`erro` byte-a-byte |
| `RunStatus::parse(s)` param `s` / retorno | `&str` / `Result<Self,RunError>` | entrada / saída | string → status | fail-closed = `InvalidStatus` |
| `RunStatus::can_transition_to(target)` params/retorno | `self,RunStatus` / `bool` | entrada / saída | par (self,target) → bool | só `Ativa`→terminal; terminal e auto-transição = false |
| `Serialize/Deserialize for RunStatus` | via `serialize_str(as_str)` / `parse` | wire | serde | troca `String`→`RunStatus` não move byte |

DomainEvent / DomainEventKind / fatos:

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `DomainEvent.tenant` | `TenantId` | estado/saída | envelope | dono obrigatório estruturalmente |
| `DomainEvent.actor` | `ActorId` | estado/saída | envelope | autoria obrigatória |
| `DomainEvent.ts` | `String` | entrada→saída | chamador → evento | RFC3339; domínio não lê relógio (determinismo) |
| `DomainEvent.kind` | `DomainEventKind` | saída | evento → ledger | fato consumado |
| `ActivationFacts.custom_personas` | `Vec<String>` | entrada | insumo → `activation_event` | nomes de personas próprias (U7) |
| `ActivationFacts.prompt_hashes` | `Vec<PromptHash>` | entrada | insumo | procedência dos prompts efetivos |
| `ActivationFacts.refs` | `Vec<String>` | entrada | insumo | referências do briefing |
| `PromptHash.role`/`.prompt_sha256`/`.custom` | `String,String,bool` | wire (fechado) | procedência | hash do prompt efetivo; `custom` distingue persona própria; adapter omite chave quando `false` |
| `DomainEventKind::SquadActivated{task_id,run_id,template_id,template_version,name,roles,custom_personas,prompt_hashes,refs}` | struct-variant | saída/wire | agregado → ledger `btv.squad_activated` | payload real de nascimento da ativação |
| `DomainEventKind::GateApproved{task_id,stage,gates_approved}` | struct-variant | saída/wire | agregado → `btv.gate_approved` | contador PÓS-incremento |
| `DomainEventKind::AdjustRequested{task_id,stage,instruction,gate_released}` | struct-variant | saída/wire | → `btv.adjust_requested` | ajuste HITL |
| `DomainEventKind::DeliverableProduced{task_id,deliverable_id,name,format,trail}` | struct-variant | saída/wire | → `btv.export_generated` | entrega + trilha de procedência |
| `DomainEventKind::PersonaUpdated{template_id,role,prompt_sha256}` | struct-variant | saída/wire | → `btv.persona_updated` | hash, nunca prompt em claro |
| `DomainEventKind::TemplatePublished{template_id,published}` | struct-variant | saída/wire | → `btv.template_published` | publicação |
| `DomainEventKind::FlowSaved{name,blocks,diagram_sha256,semantic_version,snapshot_hash,audit_head,audit_len}` | struct-variant | saída/wire | → `btv.flow_saved` | fluxo do Designer salvo+auditado |
| `DomainEventKind::UserRemoved{user_id}` | struct-variant | saída/wire | → `btv.user_removed` | remoção de perfil |
| `DomainEventKind::wire_kind()` retorno | `&'static str` | saída/wire | match → kind do wire | mapeamento declarativo 1:1 variante→kind `btv.*` |

Erros de domínio:

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `RunError::InvalidTransition{from,to}` | erro | saída | agregado → chamador | transição proibida pela máquina |
| `RunError::InvalidStatus(String)` | erro | saída | `parse` → chamador | status fora do vocabulário |
| `RunError::InvalidState(String)` | erro | saída | factory/derivação → chamador | `roles` vazio, `id==0`, `papeis_json` corrompido |
| `RepositoryError::NotFound` | erro | saída | adapter → chamador | registro inexistente |
| `RepositoryError::ConcurrencyConflict{expected,found}` | erro | saída | append otimista → chamador | corrida perdida no read-modify-write |
| `RepositoryError::Storage(String)` | erro | saída | adapter → chamador | erro de driver traduzido (sem tipo de driver na fronteira) |

Agregado `Run` (impl):

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `Run::activate` params `ctx,task_id,template_id,template_versao,nome,briefing_json,roles,ts` | `&TenantContext,TaskId,String×4,&[String],String` | entrada | serviço de aplicação → factory | valida `roles` não-vazio |
| `activate` local `papeis_json` | `String` | intermediário | `serde_json::to_string(roles)` | DERIVA papéis do vetor (fonte única); erro → `InvalidState` |
| `activate` retorno `Run` | `Result<Run,RunError>` | saída | → chamador | run nascente `id=0`, `status=Ativa`, `gates_aprovados=0`, `created_ts=updated_ts=ts`, `tenant=ctx.tenant` |
| `Run::activation_event` params `ctx,facts,ts` | `&TenantContext,ActivationFacts,String` | entrada | serviço → derivação | evento de nascimento pós-save |
| `activation_event` guarda `self.id==0` | `bool` | intermediário | check → `InvalidState` fail-closed | sem save não há `run_id` a auditar |
| `activation_event` local `roles` | `Vec<String>` | intermediário | `serde_json::from_str(papeis_json)` | volta da fonte única; corrompido → `InvalidState` |
| `activation_event` retorno | `Result<DomainEvent,RunError>` | saída | → chamador | `SquadActivated` com tenant/actor do ctx + facts |
| `Run::approve_gate` params `ctx,stage,ts` | `&TenantContext,Option<String>,String` | entrada | serviço → agregado | única porta p/ incrementar gates |
| `approve_gate` guarda `status != Ativa` | check | intermediário | → `InvalidTransition{from,to:Ativa}` | estado NÃO muda em erro |
| `approve_gate` mutações `gates_aprovados+=1`, `updated_ts=ts` | estado | estado | incremento | contador deixa de ser i64 solto |
| `approve_gate` retorno | `Result<DomainEvent,RunError>` | saída | → chamador | `GateApproved` c/ contador pós-incremento |
| `Run::transition_to` params `_ctx,target,ts` | `&TenantContext,RunStatus,String` | entrada | serviço → agregado | `_ctx` não usado (transição não é fato auditado) |
| `transition_to` guarda `!can_transition_to` | check | intermediário | → `InvalidTransition` | estado intacto em erro |
| `transition_to` mutações `status=target`, `updated_ts=ts` | estado | estado | transição válida | retorna `Result<(),_>`, NÃO evento (Nada Fake) |

Traits de repositório (todas recebem `&TenantContext`):

| Trait / método | params (entrada) | retorno (saída) | observação |
|---|---|---|---|
| `RunRepository::get` | `ctx,task_id:&str` | `Result<Option<Run>,RepositoryError>` | `None`=inexistente NESTE tenant (isolamento fail-closed) |
| `RunRepository::list` | `ctx` | `Result<Vec<Run>,_>` | mais recente primeiro |
| `RunRepository::save` | `ctx,run:&Run` | `Result<(),_>` | upsert por `task_id` no tenant |
| `RunRepository::save_with_deliverables` | `ctx,run:&Run,novas:&[Deliverable]` | `Result<(),_>` | unidade transacional run+entregas (critério 4) |
| `RunRepository::list_deliverables` | `ctx` | `Result<Vec<Deliverable>,_>` | Biblioteca U4 |
| `RunRepository::get_deliverable` | `ctx,id:i64` | `Result<Option<Deliverable>,_>` | por id |
| `RunRepository::max_task_seq` | `ctx` | `Result<u64,_>` | semeia o contador do hub no arranque |
| `PersonaRepository` (8 métodos) | `ctx`+`template_id/papel/prompt/nome/id` | `Vec<PersonaOverride>`/`Vec<CustomPersona>`/`i64`/`()` | list/set/delete/clear overrides + list/insert/update/delete custom |
| `TemplatePublicationRepository::set_published` | `ctx,template_id:&str,published:bool` | `Result<(),_>` | upsert de publicação |
| `TemplatePublicationRepository::list_published` | `ctx` | `Result<Vec<(String,bool)>,_>` | overrides de publicação; leitura fail-closed |
| `UserRepository::list` | `ctx` | `Result<Vec<User>,_>` | perfis do tenant |
| `UserRepository::create` | `ctx,nome,email,papel:&str,pin:Option<&str>` | `Result<i64,_>` | `created_ts` é do adapter |
| `UserRepository::remove` | `ctx,id:i64` | `Result<(),_>` | remove |
| `UserRepository::set_active` | `ctx,id:i64,ativo:bool` | `Result<(),_>` | ativa/desativa |
| `UserRepository::set_pin` | `ctx,id:i64,pin:Option<&str>` | `Result<(),_>` | define/limpa PIN |
| `UserRepository::verify_pin` | `ctx,id:i64,pin:&str` | `Result<PinCheck,_>` | compara DENTRO do adapter; hash nunca sai |
| `LedgerRepository` (assoc. `type Entry`)::append | `ctx,event:&DomainEvent` | `Result<u64,_>` (seq) | consome `DomainEvent`, não string; ts/actor do evento são a verdade |
| `LedgerRepository::recent` | `ctx,limit:u32,actor:Option<&ActorId>` | `Result<Vec<Entry>,_>` | recentes da cadeia do tenant |
| `LedgerRepository::verify_chain` | `ctx` | `Result<u64,_>` | verifica a cadeia do tenant |
| `LedgerRepository::export` | `ctx` | `Result<Vec<Entry>,_>` | cadeia completa verificável isoladamente |
| `EventStorePort` (assoc. `NewEvent`,`StoredEvent`)::append | `ctx,aggregate_id:&str,expected_head:i64,events:Vec<NewEvent>` | `Result<i64,_>` | append otimista; head errado → `ConcurrencyConflict` |
| `EventStorePort::read` | `ctx,aggregate_id:&str,from_seq:i64` | `Result<Vec<StoredEvent>,_>` | leitura desde seq |
| `EventStorePort::head_seq` | `ctx,aggregate_id:&str` | `Result<i64,_>` | head atual |

Portas do runtime de agente (D1t):

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `LlmError::NoProvider` / `AllFailed(String)` / `RateLimited(String)` | erro | saída | adapter LLM → loop | 3 variantes do `GatewayError` histórico; sem tipo de driver |
| `LlmPort::generate` params `req,on_delta` | `GenerateRequest`, `&mut dyn FnMut(&str)+Send` | entrada | loop → gateway | streaming como callback de deltas de texto |
| `LlmPort::generate` retorno | `impl Future<Output=Result<AssistantTurn,LlmError>>+Send` | saída | gateway → loop | turno agregado do stream |
| `ToolsPort::specs()` retorno | `Vec<ToolSpec>` | saída | registry → modelo | anúncio das ferramentas |
| `ToolsPort::get(name)` param/retorno | `&str` / `Option<&dyn Tool>` | entrada / saída | resolução por nome | `Send+Sync` supertrait (cruza `.await` em spawn) |

Testes deste módulo (dados intermediários notáveis):

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `uma_de_cada_variante()` | `Vec<DomainEventKind>` | intermediário | dummies → cobertura | uma instância de cada variante; força match exaustivo em `wire_kind` |
| `run_status_roundtrip...` fixture | `wire-strings.v1.json["run_status"]` → `BTreeSet<String>` | entrada→intermediário | disco → comparação | igualdade enum×fixture + round-trip + fail-closed |
| `variantes_cobrem...` fixture | `["ledger_kinds"]` filtrado `btv.*` × `wire_kind()` | intermediário | disco × enum | conjuntos idênticos (impede kind órfão) |
| `run_ativo()`/`ctx()` | `Run`/`TenantContext` dummy | intermediário | fábrica de teste | base dos testes de agregado |

Fluxo: entrada = `TenantContext` + insumos (task_id/ts/roles/facts) do serviço
de aplicação; processamento = validação de máquina de estados e derivação de
eventos DENTRO do agregado (única porta de mutação); saída = `Run` mutado +
`DomainEvent` tipado (que o chamador persiste via `RunRepository` e audita via
`LedgerRepository`). As traits definem a fronteira com adapters sem vazar
infraestrutura; as portas `LlmPort`/`ToolsPort` fecham o loop de agente sobre
tipos de domínio.

---

# Parte B — `crates/btv-schemas`

## `crates/btv-schemas/src/lib.rs`
Raiz dos contratos serializados/auditáveis; re-exporta o hash canônico.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| módulos `canonical,experiment,handoff,ledger,persona,plan,review,squad_template,telemetry,verification,workflow` | `pub mod` | config | árvore → consumidores | contratos compatíveis com `schemas/json/*.v1.schema.json` |
| re-export `canonical_json, request_hash, sha256_hex, validate_cache_key, CacheKeyError` | fns/tipo | saída | `canonical` → API pública | superfície do hash de cache |

Fluxo: entrada = nenhuma; processamento = agrega submódulos; saída = superfície
pública dos contratos + funções de hash.

## `crates/btv-schemas/src/canonical.rs`
Canonicalização JSON + hash de cache de prompt (`prompt-cache-key.v1`),
implementação-espelho do Python. **Algoritmo de hash como fluxo de dados.**

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `CacheKeyError::NumeroProibido{path,motivo}` | erro | saída | validação → chamador | número que divergiria entre produtores JS×Rust/Python (ADR 0032) |
| `reject_forbidden_numbers(value,path)` param `value` | `&Value` | entrada | recursão | percorre todo o valor |
| `reject_forbidden_numbers` param `path` | `&str` | entrada/intermediário | acumulador de caminho | `$.messages`, `$.temperature`, `.chave`, `[i]` |
| `reject_forbidden_numbers` — float não-finito | `f64` | intermediário | check `!is_finite()` | rejeita NaN/Inf (defensivo em Rust; serde nunca constrói) |
| `reject_forbidden_numbers` — float fração-zero | `f64` | intermediário | check `fract()==0.0` | rejeita `1.0`/`0.0` (JS emitiria `1`/`0`); sugere o inteiro |
| `reject_forbidden_numbers` retorno | `Result<(),CacheKeyError>` | saída | → validador | recursão por Object/Array via `try_for_each` |
| `validate_cache_key(messages,temperature)` params | `&Value,&Value` | entrada | chamador → guard | público p/ validar antes de montar request |
| `validate_cache_key` retorno | `Result<(),CacheKeyError>` | saída | → chamador | aplica `reject_forbidden_numbers` em `$.messages` e `$.temperature` |
| `canonical_json(value)` param | `&Value` | entrada | chamador → serializador | valor a canonicalizar |
| `canonical_json` local `out` | `String` (buffer) | intermediário | acumulador | recebe o texto canônico |
| `canonical_json` retorno | `String` | saída | → chamador | JSON com chaves ordenadas, sem espaços |
| `write_canonical(value,out)` — Object: `keys` | `Vec<&String>` | intermediário | `map.keys().collect()` + `sort()` | ordena chaves em todos os níveis; escreve `{"k":v,...}` |
| `write_canonical` — Array | itera itens sem espaço | intermediário | `[i0,i1,...]` | preserva ordem |
| `write_canonical` — scalar | `serde_json::to_string(scalar)` | intermediário | escreve compacto | ex.: `0.7`, `null`, `"olá"` |
| `sha256_hex(text)` param `text` | `&str` | entrada | chamador → hasher | texto a hashear |
| `sha256_hex` local `hasher` | `Sha256` | intermediário | `update(bytes)`+`finalize` | digest binário |
| `sha256_hex` retorno | `String` | saída | `hex::encode` → chamador | hex minúsculo |
| `request_hash(messages,temperature)` params | `&Value,&Value` | entrada | caminho quente (cada chamada LLM) | insumos do hash de cache |
| `request_hash` — passo 1 validação | `validate_cache_key(...)?` | intermediário | guard | rejeita números proibidos ANTES de hashear |
| `request_hash` — passo 2 envelope | `Value` `{"messages":...,"temperature":...}` | intermediário | `serde_json::json!` | monta o envelope |
| `request_hash` — passo 3 canonicalização | `String` | intermediário | `canonical_json(&envelope)` | chaves ordenadas (`messages` antes de `temperature`), sem espaços |
| `request_hash` — passo 4 hash | `String` | saída | `sha256_hex(...)` | sha256 hex do texto canônico |
| `request_hash` retorno | `Result<String,CacheKeyError>` | saída | → gateway/cache | chave idêntica nos dois lados da fronteira |
| testes `ordena_chaves.../escalares.../sha256_conhecido/request_hash_*` | Values/strings dummy | intermediário | asserções | `sha256("abc")`=`ba7816bf…`; rejeita `1.0`/`0.0`; `Number::from_f64(Inf/NaN)`=None |

Fluxo (algoritmo de hash, passo a passo): entrada `messages` + `temperature`
(`&Value`) → (1) `validate_cache_key` rejeita floats fração-zero e não-finitos
recursivamente → (2) monta envelope `{"messages","temperature"}` → (3)
`canonical_json` serializa com chaves ordenadas em todos os níveis e sem espaços
→ (4) `sha256_hex` produz o digest hex minúsculo → saída = a chave de cache
`prompt-cache-key.v1`, byte-idêntica ao lado Python.

## `crates/btv-schemas/src/ledger.rs`
Entrada do ledger append-only (`ledger-entry.v1`) com hash-chain e anti-transplante por tenant.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `OverrideMark.marked` | `bool` | wire | serde/schemars | override marcado |
| `OverrideMark.reason` | `Option<String>` | wire | `skip_serializing_if=None` | motivo do override |
| `LedgerEntry.seq` | `u64` | wire | serde | sequência monotônica POR tenant (ADR 0027) |
| `LedgerEntry.prev_hash` | `String` | wire | serde | hash da entrada anterior (`""` = primeira da cadeia do tenant) |
| `LedgerEntry.entry_hash` | `String` | wire | serde | sha256 de `prev_hash + hash_body` (calc. pelo storage) |
| `LedgerEntry.kind` | `String` | wire | serde | tipo do evento (`session.start`, `tool.run`, …) |
| `LedgerEntry.actor` | `String` | wire | serde | quem produziu |
| `LedgerEntry.payload` | `Value` | wire | serde | corpo livre |
| `LedgerEntry.override` (`r#override`) | `Option<OverrideMark>` | wire | `skip_serializing_if=None` | marca de override |
| `LedgerEntry.fake_marker` | `Option<String>` | wire | `skip_serializing_if=None` | "Nada Fake": presente quando payload é simulado |
| `LedgerEntry.ts` | `String` | wire | serde | RFC3339 |
| `LedgerEntry.tenant` | `Option<btv_domain::TenantId>` | wire (aditivo) | `skip_serializing_if=None`, `schemars(with="Option<String>")` | ADR 0027 item 2; entra no corpo hasheado quando presente |
| `LedgerEntry::hash_body()` retorno | `String` | saída/intermediário | campos → JSON canônico | exclui `seq`/`prev_hash`/`entry_hash`; inclui `tenant` SÓ quando `Some` (evita invalidar hashes legados) |
| `hash_body` local `body` | `Value` | intermediário | `json!{kind,actor,payload,override,fake_marker,ts}` + tenant condicional | corpo canonicalizado por `canonical_json` |
| `LedgerEntry::chain_hash(prev_hash)` param/retorno | `&str` / `String` | entrada / saída | `sha256_hex(prev_hash + hash_body)` | hash encadeado |
| teste `entry()` | `LedgerEntry` dummy | intermediário | fábrica | base dos testes |
| teste hashes congelados pré-B3 | strings esperadas | intermediário | valores de commit externo | `1f4285b3…` / `5aef8583…`; corpo canônico byte-idêntico sem tenant |
| teste anti-transplante | tenant no corpo | intermediário | trocar tenant → hash muda | detecta reatribuição (coluna muda, corpo também) |

Fluxo: entrada = campos do evento (kind/actor/payload/ts, tenant opcional);
processamento = `hash_body` canonicaliza o corpo (sem campos derivados) e
`chain_hash` encadeia com `prev_hash`; saída = `entry_hash` gravado pelo storage,
formando a cadeia append-only por tenant.

## `crates/btv-schemas/src/verification.rs`
Evidência de verificação determinística (`verification-evidence.v1`) consumida pelo Auditor.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `Verdict` = `Pass`\|`Fail`\|`Skipped` | enum | wire | serde `snake_case` | veredito |
| `Finding.tool`/`.severity`/`.message` | `String`×3 | wire | serde | achado (ferramenta, severidade, msg) |
| `Finding.file`/`.line` | `Option<String>`/`Option<u64>` | wire | `skip_serializing_if=None` | localização opcional |
| `VerificationStep.name`/`.tool` | `String,String` | wire | serde | passo (typecheck/test/lint/sast) e ferramenta |
| `VerificationStep.exit_code` | `i32` | wire | serde | código de saída |
| `VerificationStep.duration_ms` | `u64` | wire | serde | duração |
| `VerificationStep.findings` | `Vec<Finding>` | wire | `serde(default)` | achados do passo |
| `VerificationEvidence.run_id`/`.git_sha` | `String,String` | wire | serde | identidade da execução |
| `VerificationEvidence.steps` | `Vec<VerificationStep>` | wire | serde | passos |
| `VerificationEvidence.verdict` | `Verdict` | wire | serde | veredito geral |
| `VerificationEvidence.produced_at` | `String` | wire | serde | timestamp |
| `derive_verdict(steps)` param/retorno | `&[VerificationStep]` / `Verdict` | entrada / saída | passos → veredito | `Fail` se qualquer `exit_code != 0`, senão `Pass` |

Fluxo: entrada = passos executados (nome/tool/exit_code/findings) por ferramentas
determinísticas; processamento = `derive_verdict` reduz ao veredito honesto;
saída = `VerificationEvidence` JSON consumida pelo Auditor e pelo `review.rs`.

## `crates/btv-schemas/src/review.rs`
Review por valor DERIVADO da evidência real do `/verify` (só dimensões determinísticas + gates duros).

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `SECURITY_FLOOR` | `const f64 = 0.5` | config | piso duro | mesma semântica do `btv_review.gates` |
| `GateTriggered` = `CriticalFinding`\|`VerifyFail`\|`SecurityFloor` | enum | wire | serde `snake_case` | qual gate duro reprovou |
| `ValueReview.technical` | `f64` | wire | serde | fração de passos com `exit_code==0` |
| `ValueReview.security` | `f64` | wire | serde | `1.0 - penalidade`, piso `0.0` |
| `ValueReview.gates_passed` | `bool` | wire | serde | gates duros passaram? (NÃO é certificação plena) |
| `ValueReview.gate_triggered` | `Option<GateTriggered>` | wire | `skip_serializing_if=None` | gate que reprovou |
| `ValueReview.reason` | `String` | wire | serde | explicação do veredito |
| `severity_penalty(sev)` param/retorno | `&str` / `f64` | entrada / saída | severidade → penalidade | `critical`/`error`→0.4, `warning`→0.1, outro→0.05 |
| `from_evidence(evidence)` param | `&VerificationEvidence` | entrada | Auditor → review | insumo real |
| `from_evidence` local `technical` | `f64` | intermediário | passos ok / total | `0.5` se sem passos |
| `from_evidence` local `security` | `f64` | intermediário | `1.0 - soma(penalidades)`, `.max(0.0)` | `0.5` se sem passos |
| `from_evidence` local `has_critical` | `bool` | intermediário | algum finding `severity=="critical"` | gatilho do gate |
| `from_evidence` locals `(gates_passed,gate_triggered,reason)` | tupla | intermediário | ordem: crítico → verify fail → piso segurança → passou | mesma ordem do `btv_review.gates.evaluate` |
| `from_evidence` retorno | `ValueReview` | saída | → chamador | só dimensões determinísticas (performance/value ficam de fora de propósito) |

Fluxo: entrada = `VerificationEvidence` (passos/findings/veredito); processamento
= deriva `technical`/`security` e aplica gates duros em ordem fixa; saída =
`ValueReview` honesto (nenhuma média alta "salva" um gate duro).

## `crates/btv-schemas/src/experiment.rs`
Relatório de A/B (`experiment.v1`): teste z de duas proporções + Bonferroni, veredito derivado dos dados.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `ALPHA` | `const f64 = 0.05` | config | nível de significância | 5% |
| `MIN_SAMPLES` | `const u64 = 20` | config | amostra mínima por variante | abaixo disso → `InsufficientData` |
| `ExperimentVerdict` = `Significant`\|`Inconclusive`\|`InsufficientData` | enum | wire | serde `snake_case` | veredito honesto |
| `VariantStats.variant` | `String` | wire | serde | nome da variante |
| `VariantStats.n` | `u64` | wire | serde | tamanho da amostra |
| `VariantStats.successes` | `u64` | wire | serde | sucessos |
| `VariantStats.rate` | `f64` | wire | serde | `successes/n` (0 quando n=0) |
| `VariantStats::new(variant,n,successes)` params/retorno | `impl Into<String>,u64,u64` / `Self` | entrada / saída | args → stats | calcula `rate` |
| `ExperimentReport.experiment`/`.metric` | `String,String` | wire | serde | id e métrica (`success_rate`) |
| `ExperimentReport.variants` | `Vec<VariantStats>` | wire | serde | variantes (ordenadas por taxa desc) |
| `ExperimentReport.verdict` | `ExperimentVerdict` | wire | serde | veredito derivado |
| `ExperimentReport.winner` | `Option<String>` | wire | `skip_serializing_if=None` | vencedor SÓ quando `Significant` |
| `ExperimentReport.p_value` | `f64` | wire | serde | p-valor decisivo (maior p entre vencedor e demais) |
| `ExperimentReport.comparisons` | `u64` | wire | serde | `m*(m-1)/2` (Bonferroni) |
| `ExperimentReport.produced_at` | `String` | wire | serde | timestamp |
| `from_two_variants(...)` params | `experiment,metric,a,b,produced_at` | entrada | açúcar → `from_variants` | caso de 2 variantes |
| `from_variants(...)` param `variants` | `Vec<VariantStats>` (mut) | entrada | → ordenação | ordenado por `rate` desc, desempate por nome |
| `from_variants` local `m`/`comparisons` | `usize`/`u64` | intermediário | `variants.len()` / `m*(m-1)/2` | nº de comparações par-a-par |
| `from_variants` local `insufficient` | `bool` | intermediário | `m<2` \|\| algum `n<MIN_SAMPLES` | gatilho de `InsufficientData` |
| `from_variants` local `best`/`others` | `&VariantStats`/`&[...]` | intermediário | `variants[0]`/`[1..]` | maior taxa vs. demais |
| `from_variants` local `worst_p` | `f64` | intermediário | `max` dos p-valores vencedor vs cada outra | comparação decisiva (mais apertada) |
| `from_variants` local `corrected_alpha` | `f64` | intermediário | `ALPHA / comparisons.max(1)` | correção de Bonferroni |
| `from_variants` local `strictly_best` | `bool` | intermediário | `best.rate > todas` | vencedor tem que bater todas |
| `from_variants` locals `(verdict,winner,p_value)` | tupla | intermediário | decisão | `Significant`+winner só se `strictly_best && worst_p<corrected_alpha` |
| `from_variants` retorno | `ExperimentReport` | saída | → chamador | nunca vencedor sem significância |
| `two_proportion_p_value(x1,n1,x2,n2)` params/retorno | `u64`×4 / `f64` | entrada / saída | contagens → p-valor bicaudal | pooled variance; `1.0` se n=0 ou variância nula |
| `two_proportion_p_value` locals `p1,p2,p_pool,se,z` | `f64` | intermediário | proporções, erro-padrão pooled, z-score | `(2*(1-normal_cdf(|z|))).clamp(0,1)` |
| `normal_cdf(z)` param/retorno | `f64` / `f64` | entrada / saída | z → CDF | `0.5*(1+erf(z/√2))` |
| `erf(x)` param/retorno | `f64` / `f64` | entrada / saída | Abramowitz-Stegun 7.1.26 | \|erro\| ≤ 1.5e-7; hand-rolled (sem crate de stats) |
| `erf` locals `sign,t,y` | `f64` | intermediário | aproximação polinomial | |

Fluxo: entrada = `VariantStats` por variante (n/successes derivados da
telemetria); processamento = ordena por taxa, checa amostra mínima, roda teste z
de duas proporções par-a-par com correção de Bonferroni; saída =
`ExperimentReport` com veredito honesto (`Significant`+winner, `Inconclusive` ou
`InsufficientData`) e p-valor decisivo.

## `crates/btv-schemas/src/persona.rs`
Persona de squad como **conteúdo** (`persona.v1`) — item de galeria publicável.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `AutonomyLabel` = `L1..L5` | enum | wire | serde/schemars | rótulo DESCRITIVO (não dispara loop; ADR 0021) |
| `MentalModel.reference`/`.apply_when` | `String,String` | wire | serde | referência canônica e quando aplicar |
| `PrincipleSeverity` = `Low`\|`Medium`\|`High`\|`Critical` | enum | wire | serde `snake_case` | severidade de princípio violado |
| `CorePrinciple.id`/`.description`/`.validation`/`.severity` | `String×3,PrincipleSeverity` | wire | serde | princípio + como validar |
| `Autonomy.level` | `AutonomyLabel` | wire | serde | nível |
| `Autonomy.can_decide_alone`/`.requires_approval` | `Vec<String>` | wire | `serde(default)` | escopos de decisão |
| `Autonomy.can_veto` | `bool` | wire | serde | pode vetar entrega (Auditor/Segurança) |
| `ActivationTriggers.semantic_patterns`/`.context_keywords` | `Vec<String>` | wire | `serde(default)` | gatilhos |
| `ActivationTriggers.confidence_threshold` | `f64` | wire | serde | limiar [0,1] p/ acender |
| `Communication.receives_from`/`.delivers_to` | `Vec<String>` | wire | `serde(default)` | handoff |
| `Communication.handoff_contract` | `String` | wire | serde | contrato de entrega |
| `Persona.id`/`.display_name`/`.domain` | `String`×3 | wire | serde | identidade e domínio |
| `Persona.mental_models`/`.core_principles` | `Vec<...>` | wire | `serde(default)` | modelos mentais e princípios |
| `Persona.autonomy`/`.activation_triggers`/`.communication` | structs | wire | serde | blocos compostos |
| `Persona.delivery_formats` | `Vec<String>` | wire | `serde(default)` | formatos exportáveis |
| `Persona::validate()` retorno | `Result<(),String>` | saída | `self` → chamador | checa `confidence_threshold` em [0,1]; erro claro com id |
| teste `persona_min()` | `Persona` dummy | intermediário | fábrica | base do teste de validação |

Fluxo: entrada = JSON `persona.v1` (galeria); processamento = `validate` além do
schema (limiar em [0,1]); saída = `Persona` tipada / erro de galeria.

## `crates/btv-schemas/src/plan.rs`
Manifesto de plano/entrega da esteira (`plan.v1`) — ticket declarativo.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `Prerequisites.contracts`/`.approvals`/`.dependencies` | `Vec<String>` | wire | `serde(default)`, `Default` | pré-requisitos |
| `PlanPhase.order` | `u32` | wire | serde | ordem 1-based única/sequencial |
| `PlanPhase.primary_role` | `String` | wire | serde | responsável |
| `PlanPhase.support_roles` | `Vec<String>` | wire | `serde(default)` | apoio |
| `PlanPhase.deliverables` | `Vec<String>` | wire | `serde(default)` | artefatos exportáveis |
| `PlanPhase.approval_required` | `bool` | wire | serde | gate humano |
| `PlanPhase.estimated_confidence` | `f64` | wire | serde | confiança [0,1] |
| `PlanPhase.quality_gates` | `Vec<String>` | wire | `serde(default)` | gates de qualidade |
| `SuccessCriteria.functional`/`.non_functional` | `Vec<String>` | wire | `serde(default)`, `Default` | critérios |
| `Budget.estimated_cost` | `f64` | wire | serde | custo estimado |
| `Budget.max_llm_calls` | `u32` | wire | serde | teto de chamadas |
| `RollbackStrategy.kill_switch` | `bool` | wire | `serde(default)`, `Default` | aborta a esteira |
| `Plan.prerequisites`/`.execution_sequence`/`.success_criteria`/`.budget`/`.rollback_strategy` | campos | wire | serde | manifesto completo |
| `Plan::validate()` retorno | `Result<(),String>` | saída | `self` → chamador | sequência não-vazia; `order` = 1..=N única/sequencial; cada `estimated_confidence` em [0,1] |
| `validate` local `orders` | `Vec<u32>` | intermediário | `map(order)` + `sort_unstable` | detecta buracos/duplicatas |
| testes `phase`/`plan_of` | fábricas | intermediário | dummies | aceita `[1,2,3]`, rejeita `[]`/`[1,3]`/`[1,1]` e confiança fora de faixa |

Fluxo: entrada = JSON `plan.v1`; processamento = `validate` (esteira sem
buracos, confiança válida); saída = `Plan` tipado / erro 422.

## `crates/btv-schemas/src/squad_template.rs`
Modelo de squad (`squad-template.v1`) — fonte única dos 12 modelos da galeria + catálogo embutido.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `CategoriaSquad` = `Conteudo`\|`Analise`\|`Criativa`\|`Operacoes` | enum | wire | serde `snake_case` | categoria |
| `FormatoEntrega.nome` | `String` | wire | serde | nome do formato |
| `FormatoEntrega.binario` | `bool` | wire | serde | export direto indisponível até haver conversor (honestidade) |
| `PerguntaBriefing.label`/`.placeholder` | `String,String` | wire | serde | pergunta do wizard |
| `SquadTemplate.id`/`.nome` | `String,String` | wire | serde | identidade |
| `SquadTemplate.categoria` | `CategoriaSquad` | wire | serde | categoria |
| `SquadTemplate.cor` | `String` | wire | serde | hex `#rrggbb` (handoff §4) |
| `SquadTemplate.onda` | `u8` | wire | serde | maturidade 1–3 |
| `SquadTemplate.versao` | `String` | wire | serde | `vMAJOR.MINOR` |
| `SquadTemplate.publicado` | `bool` | wire | serde | publicação (A5) |
| `SquadTemplate.descricao` | `String` | wire | serde | descrição do card |
| `SquadTemplate.papeis` | `Vec<String>` | wire | serde | equipe |
| `SquadTemplate.formatos` | `Vec<FormatoEntrega>` | wire | serde | formatos de entrega |
| `SquadTemplate.perguntas` | `Vec<PerguntaBriefing>` | wire | serde | briefing |
| `SquadTemplate.gates` | `Vec<String>` | wire | serde | pontos de parada HITL |
| `SquadTemplate::validate()` retorno | `Result<(),String>` | saída | `self` → chamador | `papeis` não-vazio, `onda` em 1..=3, `formatos` não-vazio |
| `TEMPLATE_SOURCES` | `[&str;12]` | config | `include_str!` dos 12 JSON | catálogo embutido no binário em compile-time |
| `builtin_templates()` retorno | `&'static [SquadTemplate]` | saída | parse único cacheado (`OnceLock`) | 12 modelos; `expect` seguro (coberto por teste) |
| testes catálogo | 12 templates | intermediário | `builtin_templates()` | valida os 12; confere os 12 hex de cor exatos e marcação de binário (DOCX binário, MD não) |

Fluxo: entrada = 12 JSON embutidos (`include_str!`) / JSON de rota; processamento
= parse cacheado + `validate` semântico; saída = `&'static [SquadTemplate]`
servido em `GET /api/btv/templates` e consumido pelo wizard/ativação.

## `crates/btv-schemas/src/telemetry.rs`
Evento de telemetria offline-first (`telemetry-event.v1`).

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `TelemetryEvent.name` | `String` | wire | serde | nome (`llm.call`, `cache.hit`, `rate.limited`) |
| `TelemetryEvent.session_id` | `String` | wire | serde | sessão |
| `TelemetryEvent.props` | `Value` | wire | `serde(default)` | propriedades livres (inclui `experiment`/`variant`/`success` lidos pelo `experiment.rs`) |
| `TelemetryEvent.ts` | `String` | wire | serde | RFC3339 |

Fluxo: entrada = evento enfileirado localmente (SQLite); processamento = nenhum
(só DTO); saída = lote descarregado e agregado pelo dashboard/experimento.

## `crates/btv-schemas/src/workflow.rs`
Grafo do Squad Designer (`squad.workflow.v1`) — salvar valida forma+arestas e grava no ledger (não aplica ao orquestrador).

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `WorkflowNodeKind` = `Card`\|`Pill` | enum | wire | serde `snake_case` | tipo de nó |
| `WorkflowNodeParam.k`/`.v` | `String,String` | wire | serde | par chave/valor |
| `WorkflowNode.id` | `String` | wire | serde | id do nó |
| `WorkflowNode.x`/`.y` | `f64,f64` | wire | serde | posição no canvas |
| `WorkflowNode.kind` | `WorkflowNodeKind` | wire | serde | Card/Pill |
| `WorkflowNode.name`/`.role`/`.color`/`.icon`/`.sub` | `String`×5 | wire | serde | metadados visuais |
| `WorkflowNode.params` | `Vec<WorkflowNodeParam>` | wire | serde | parâmetros |
| `WorkflowNode.removable` | `bool` | wire | serde | pode remover |
| `WorkflowEdge.from`/`.to` | `String,String` | wire | serde | aresta |
| `WorkflowEdge.label` | `Option<String>` | wire | `skip_serializing_if=None` | rótulo opcional |
| `SquadWorkflow.nodes`/`.edges` | `Vec<...>` | wire | serde | grafo completo |
| `SquadWorkflow::validate_edges()` retorno | `Result<(),String>` | saída | `self` → chamador | toda aresta referencia nó existente |
| `validate_edges` local `ids` | `HashSet<&str>` | intermediário | `nodes.map(id)` | conjunto de ids; aresta órfã → erro citando lado (`from`/`to`) e id |
| testes `node`/grafo | dummies | intermediário | fábricas | válido passa; aresta p/ `fantasma` reprovada com erro que cita o id |

Fluxo: entrada = JSON `squad.workflow.v1` do Designer; processamento =
`validate_edges` (integridade referencial não expressável em JSON Schema);
saída = grafo validado gravado no ledger ("salvo e validado", nunca aplicado ao
orquestrador real).

## `crates/btv-schemas/src/handoff.rs`
Evento de handoff entre agentes (`handoff-event.v1`).

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `HandoffPhase` = `Start`\|`Ack`\|`Complete`\|`Error` | enum | wire | serde `snake_case` | fase do handoff |
| `HandoffEvent.event` | `HandoffPhase` | wire | serde | fase |
| `HandoffEvent.task_id` | `String` | wire | serde | tarefa |
| `HandoffEvent.from_agent`/`.to_agent` | `String,String` | wire | serde | origem/destino |
| `HandoffEvent.contract` | `String` | wire | serde | contrato trafegado (`plan.v1`, `proposal.v1`) |
| `HandoffEvent.payload_digest` | `String` | wire | serde | sha256 do payload (o payload em si vai no ledger) |
| `HandoffEvent.ts` | `String` | wire | serde | RFC3339 |
| `HandoffEvent.error` | `Option<String>` | wire | `skip_serializing_if=None` | erro (fase `Error`) |
| teste `fases_serializam_em_snake_case` | `HandoffPhase::Complete`→`"complete"` | intermediário | asserção | wire snake_case |

Fluxo: entrada = handoff emitido pela matriz de orquestração; processamento =
nenhum (só DTO); saída = evento `start`/`ack`/`complete`/`error` com telemetria e
digest do payload.

---

# Parte C — testes e benches (fatia btv-schemas)

## `crates/btv-schemas/benches/canonical.rs`
Bench criterion do caminho quente `request_hash`.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `messages` | `Value` (histórico realista) | entrada/config | fixture in-code → `request_hash` | 3 turnos user/assistant |
| `temperature` | `Value = json!(0.7)` | entrada/config | idem | float não inteiro (válido) |
| `bench_request_hash(c)` | `&mut Criterion` | entrada | criterion → laço | `b.iter(request_hash(black_box(...)))` |
| baseline de tempo | medição | saída | criterion → relatório | regressões de canonicalização/sha256 aparecem aqui |

Fluxo: entrada = messages+temperature fixos; processamento = mede `request_hash`
em laço; saída = baseline de performance.

## `crates/btv-schemas/tests/parity.rs`
Contrato cross-language do `prompt-cache-key.v1` (Rust × Python).

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `path`/`raw`/`doc` | caminho→String→`Value` | entrada→intermediário | `schemas/fixtures/prompt-cache-key.v1.json` | leitura de disco |
| `cases` | `&[Value]` (≥5) | intermediário | `doc["cases"]` | casos válidos |
| por caso: `name`,`expected`,`got` | `&str,&str,String` | intermediário | `request_hash(messages,temperature)` | `got == expected` (sha256 esperado da fixture) |
| `reject` | `&[Value]` | intermediário | `doc["reject_cases"]` | casos proibidos (ADR 0032) |
| por reject: `is_err()` | `bool` | saída | asserção | ambos os lados RECUSAM |

Fluxo: entrada = fixtures compartilhadas; processamento = recalcula hash e checa
igualdade / rejeição; saída = garantia de paridade Rust×Python do cache.

## `crates/btv-schemas/tests/schema_fixtures.rs`
Golden round-trip dos contratos schemars contra os JSON Schema `*.v1`.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
|---|---|---|---|---|
| `schema(name)` retorno | `Value` | entrada | `schemas/json/{name}.v1.schema.json` | carrega schema |
| `fixture(name)` retorno | `Value` | entrada | `schemas/fixtures/{name}.v1.json` | carrega fixture (`valid`/`invalid_*`) |
| `validator` | `jsonschema::Validator` | intermediário | `validator_for(schema)` | valida documentos |
| `parsed` (por teste) | `HandoffEvent`/`LedgerEntry`/`TelemetryEvent`/`ExperimentReport`/`SquadWorkflow`/`SquadTemplate`/`Persona`/`Plan` | intermediário/saída | `serde_json::from_value(doc["valid"])` | prova schema↔struct; asserções de campo (ex.: `task_id="task-1"`, `winner=Some("A")`, `tenant=None`/`Some(LOCAL)`) |
| casos `invalid_*` | `Value` | intermediário | `!validator.is_valid(...)` | prova que o negativo reprova (schema não é permissivo demais) |
| galeria de personas | `Vec<PathBuf>` | entrada→intermediário | `schemas/personas/**/*.json` recursivo | cada arquivo bate `persona.v1` + `validate()` |
| `prompt-template` | `Value` | entrada | fixture sem tipo Rust/Python | só protege o schema contra drift de sintaxe |

Fluxo: entrada = schemas + fixtures de disco; processamento = valida forma
(JSON Schema) e desserializa nos tipos schemars, roda `validate()` semântico;
saída = garantia de que os DTOs deste crate permanecem compatíveis com os
contratos canônicos `*.v1`.
