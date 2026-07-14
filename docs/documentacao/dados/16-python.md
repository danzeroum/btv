# Dicionário de dados — pacotes Python (`python/packages/`)

Mapa de fluxo de dados exaustivo dos 4 pacotes Python do repositório:
**btv-promptforge**, **btv-proto-py**, **btv-review**, **btv-squad**. Cada seção `##`
cobre um arquivo `.py` (os `*_pb2.py`/`*_pb2_grpc.py` gerados só têm os tipos de
mensagem registrados, a partir do proto-fonte). Direções: `entrada`, `saída`,
`intermediário`, `estado`, `config`, `wire`.

---

# Pacote btv-promptforge

## python/packages/btv-promptforge/src/btv_promptforge/__init__.py

Fachada de re-export do pacote de prompts (geradores, hashing, lint).

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
| --- | --- | --- | --- | --- |
| `GeneratorField` (=`Field`) | classe pydantic | saída | `generators` → API pública | re-export renomeado |
| `Generator`, `GENERATORS` | classe / dict | saída | `generators` → API pública | catálogo de geradores |
| `CacheKeyError`, `canonical_json`, `request_hash`, `sha256_hex`, `validate_cache_key` | símbolos | saída | `hashing` → API pública | contrato `prompt-cache-key.v1` |
| `lint_prompt` | função | saída | `lint` → API pública | quality linter |
| `__all__` | `list[str]` | config | módulo | superfície pública explícita |

Fluxo: só agrega/reexporta os submódulos; nenhum dado transita aqui.

## python/packages/btv-promptforge/src/btv_promptforge/hashing.py

Hash canônico de cache de prompt (`prompt-cache-key.v1`) — o "gêmeo" Python do Rust `btv_schemas::canonical`.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
| --- | --- | --- | --- | --- |
| `CacheKeyError(ValueError)` | classe exceção | saída | módulo → caller | erro de contrato v1 (número que divergiria JS×Rust/Python) |
| `value` (canonical_json) | `Any` | entrada | param → `json.dumps` | serializado com `sort_keys=True`, `separators=(",",":")`, `ensure_ascii=False` = JSON canônico (chaves ordenadas todos os níveis, sem espaços, UTF-8 cru) |
| retorno `canonical_json` | `str` | saída | `json.dumps` → caller | string JSON canônica |
| `text` (sha256_hex) | `str` | entrada | param → `hashlib.sha256` | codificado UTF-8, digest hex minúsculo |
| retorno `sha256_hex` | `str` | saída | hashlib → caller | sha256 hex |
| `value`, `path` (_reject_forbidden_numbers) | `Any`,`str` | entrada | recursão | varre dict/list/tuple; `path` acumula trilha (`$.k`, `$[i]`) para msg de erro |
| regra `bool` | intermediário | — | isinstance | `bool` passa (subclasse de int, não float) |
| regra `float` não-finito | intermediário | — | `math.isfinite` | NaN/Inf → `CacheKeyError` |
| regra `float.is_integer()` | intermediário | — | validação | 1.0 proibido (JS serializa como "1") → `CacheKeyError` sugerindo `int(value)` |
| `messages`, `temperature` (validate_cache_key) | `Any` | entrada | params → `_reject_forbidden_numbers` | valida ambos com paths `$.messages` / `$.temperature` |
| `messages`, `temperature` (request_hash) | `Any` | entrada | params → hash | **passo a passo:** 1) `validate_cache_key` (rejeita floats proibidos); 2) monta `{"messages":..., "temperature":...}`; 3) `canonical_json` (ordena+compacta); 4) `sha256_hex` → digest |
| retorno `request_hash` | `str` | saída | sha256 → caller | hash idêntico ao `btv_schemas::request_hash` (Rust); paridade garantida por fixtures |

Fluxo: `messages`+`temperature` (entrada) → guard de números proibidos → dict `{messages,temperature}` (intermediário) → JSON canônico → sha256 hex (saída). Twin determinístico do Rust; ADR 0032 enforça restrição de floats com fração zero.

## python/packages/btv-promptforge/src/btv_promptforge/generators.py

Geradores declarativos de prompt (origem: prompte `generators.js`).

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
| --- | --- | --- | --- | --- |
| `Field.name` | `str` | wire | pydantic BaseModel | nome do campo de entrada |
| `Field.label` | `str` | wire | pydantic | rótulo de exibição |
| `Field.required` | `bool`=True | wire | pydantic | obrigatoriedade |
| `Field.placeholder` | `str`="" | wire | pydantic | dica |
| `Generator.name` | `str` | wire | pydantic | id do gerador |
| `Generator.category` | `str` | wire | pydantic | categoria (ex.: "codigo") |
| `Generator.fields` | `list[Field]` | wire/estado | pydantic | campos do template |
| `Generator.build` | `Callable[[dict[str,str]],str]` | estado | pydantic | função de montagem (não serializável) |
| `data` (render) | `dict[str,str]` | entrada | param → build | valores dos campos |
| `missing` (render) | `list[str]` | intermediário | comprehension | campos obrigatórios ausentes → `ValueError` se houver |
| retorno `render` | `str` | saída | `build(data)` → caller | prompt montado |
| `_build_code_review` in `data` | `dict[str,str]` | entrada | usa `language`/`context`/`code` | interpola template de code review PT |
| `_build_bug_fix` in `data` | `dict[str,str]` | entrada | usa `symptom`/`expected`/`code` | interpola template de bug fix PT |
| `GENERATORS` | `dict[str,Generator]` | config/saída | comprehension `{g.name: g}` | catálogo: `code-review` (fields language/context/code), `bug-fix` (fields symptom/expected/code) |

Fluxo: `data` (entrada) → validação de required (`missing` intermediário) → `build(data)` interpola → prompt string (saída). `GENERATORS` é o registro estático consultado pelo server.

## python/packages/btv-promptforge/src/btv_promptforge/lint.py

Quality linter de prompts ("ESLint para prompts", origem: prompte `promptQuality.js`).

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
| --- | --- | --- | --- | --- |
| `VAGUE_TERMS` | `list[str]` | config | const | 8 termos vagos PT (melhor, bom, rápido, simples, otimizado, adequado, apropriado, etc) |
| `MIN_CONTEXT_LENGTH` | `int`=40 | config | const | comprimento mínimo do prompt |
| `LintIssue.rule` | `str` | wire | pydantic | id da regra |
| `LintIssue.message` | `str` | wire | pydantic | mensagem |
| `LintReport.score` | `float` 0–1 | wire | pydantic | pontuação |
| `LintReport.issues` | `list[LintIssue]` | wire | pydantic | achados |
| `LintReport.grade` | `str` | saída | `@property` | A≥0.9, B≥0.7, C≥0.5, senão D |
| `prompt` (lint_prompt) | `str` | entrada | param | texto a lintar |
| `issues` | `list[LintIssue]` | intermediário/saída | acumulador | achados coletados |
| `lowered` | `str` | intermediário | `prompt.lower()` | busca case-insensitive |
| `found_vague` | `list[str]` | intermediário | comprehension | termos vagos com match `f" {term}"` → 1 issue `vague-term` cada |
| guard missing-context | intermediário | — | `len(prompt.strip())<40` | issue `missing-context` |
| guard missing-input | intermediário | — | sem "```" e sem "entrada:/input:/exemplo:" | issue `missing-input` |
| `penalty` | `float` | intermediário | `0.2 * len(issues)` | penalidade linear |
| retorno | `LintReport` | saída | módulo → caller | `score=max(0.0, 1.0-penalty)` |

Fluxo: `prompt` (entrada) → busca de termos vagos/contexto/entrada (`issues` acumulador) → `penalty` (intermediário) → `LintReport{score, issues}` com `grade` derivado (saída).

## python/packages/btv-promptforge/src/btv_promptforge/server.py

Servidor gRPC `PromptForgeService` sobre UDS (Fase 3) — expõe lint/render/geradores; nunca chama LLM.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
| --- | --- | --- | --- | --- |
| `VERSION` | `str` | config/estado | `importlib.metadata.version` | fallback "0.1.0" |
| `logger` | Logger | estado | módulo | logging |
| `request.prompt` (Lint) | `str` (wire) | entrada | LintRequest → `lint_prompt` | prompt a lintar |
| `report` (Lint) | `LintReport` | intermediário | `lint_prompt(...)` | resultado do linter |
| LintReport de saída | proto (wire) | saída | servicer → cliente Rust | `score`/`grade`/`issues[{rule,message}]` mapeados campo a campo |
| `request.generator` (Render) | `str` (wire) | entrada | RenderRequest → `GENERATORS.get` | nome do gerador |
| `request.fields` (Render) | `map<string,string>` (wire) | entrada | → `dict(request.fields)` → `render` | valores; gerador desconhecido → `abort(NOT_FOUND)` |
| `prompt` (Render) | `str` | intermediário/saída | `generator.render(...)` | `ValueError` (campo faltando) → `abort(INVALID_ARGUMENT)` |
| RenderResponse | proto (wire) | saída | servicer → cliente | `prompt=...` |
| `infos` (ListGenerators) | `list[GeneratorInfo]` | intermediário/saída | comprehension sobre `GENERATORS.values()` | cada `GeneratorInfo{name,category,fields[{name,label,required,placeholder}]}` |
| HealthResponse | proto (wire) | saída | servicer | `ready=True, version=VERSION` |
| `socket_path` (serve) | `str` | config/entrada | arg CLI | remove socket existente; `add_insecure_port("unix://...")` |
| `server` | grpc.aio.server | estado | serve | servidor assíncrono |
| `--socket` | arg CLI | config | argparse (required) | caminho do UDS |
| `BTV_LOG_LEVEL` | env | config | `os.environ.get` | nível de log (default INFO) |

Fluxo: requests gRPC (entrada wire) → módulos puros `lint_prompt`/`GENERATORS.render` (intermediário) → mensagens proto de resposta (saída wire). Degrada com aborts gRPC em erro; nunca gera texto de LLM.

---

# Pacote btv-proto-py

Stubs gRPC gerados (`*_pb2.py`/`*_pb2_grpc.py`) — só os TIPOS de mensagem, extraídos dos protos-fonte em `schemas/proto/`. Nunca editados à mão.

## python/packages/btv-proto-py/src/btv_proto/__init__.py

Placeholder/docstring do pacote de stubs.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
| --- | --- | --- | --- | --- |
| (nenhum) | — | — | — | Docstring: stubs gerados por `just gen-proto` a partir de `schemas/proto/{core,squad,llm}.proto`. Sem símbolos. |

## python/packages/btv-proto-py/src/btv_proto/core_pb2.py + core_pb2_grpc.py

`CoreService` — serviços que o núcleo Rust expõe ao sidecar Python (keys/permissões/ledger só no Rust). Gerado.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
| --- | --- | --- | --- | --- |
| **serviço** `CoreService` | gRPC | wire | Rust serve, Python chama | RPCs abaixo |
| `Generate(LlmRequest)→stream LlmChunk` | rpc | wire | Python→Rust | geração LLM streaming |
| `RunTool(ToolCall)→ToolResult` | rpc | wire | Python→Rust | execução de ferramenta sob permissões |
| `AppendLedger(LedgerAppend)→LedgerAck` | rpc | wire | Python→Rust | append-only |
| `Recall(RecallRequest)→RecallResponse` | rpc | wire | (Unimplemented — direção errada) | memória; não usado |
| `Remember(RememberRequest)→RememberAck` | rpc | wire | (Unimplemented) | não usado |
| `RequestPermission(PermissionRequest)→PermissionDecision` | rpc | wire | Python→Rust | HITL |
| `ToolCall{tool,args_json,scope}` | msg | wire | Python→Rust | `scope` informativo — Rust recalcula escopo real de `args_json` |
| `ToolResult{content,truncated,exit_code}` | msg | wire | Rust→Python | exit_code: 0 ok, 1 erro, -1 negado |
| `LedgerAppend{kind,actor,payload_json,fake_marker?}` | msg | wire | Python→Rust | entrada de ledger |
| `LedgerAck{seq,entry_hash}` | msg | wire | Rust→Python | confirmação hash-chain |
| `RecallRequest{agent,query,limit}` | msg | wire | — | não usado |
| `RecallResponse{memories_json[]}` | msg | wire | — | não usado |
| `RememberRequest{agent,memory_json}` / `RememberAck{stored}` | msg | wire | — | não usado |
| `PermissionRequest{tool,scope,reason,confidence}` | msg | wire | Python→Rust | `confidence` gatilho HITL |
| `PermissionDecision{decision(ALLOW/DENY/UNSPECIFIED),operator_note?}` | msg | wire | Rust→Python | UNSPECIFIED(0) = fail-closed |

## python/packages/btv-proto-py/src/btv_proto/llm_pb2.py + llm_pb2_grpc.py

Tipos do gateway LLM compartilhados entre CoreService/SquadService. Gerado (sem serviço próprio).

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
| --- | --- | --- | --- | --- |
| `LlmRequest{model,messages_json,temperature?,max_tokens?,requester}` | msg | wire | Python→Rust | `messages_json` = JSON canônico (prompt-cache-key.v1); `requester` p/ telemetria+rate-limit |
| `LlmChunk{oneof: text_delta\|usage\|error}` | msg | wire | Rust→Python (stream) | chunk de streaming |
| `Usage{input_tokens,output_tokens,cache_hit,provider}` | msg | wire | Rust→Python | métricas do chunk final |

## python/packages/btv-proto-py/src/btv_proto/squad_pb2.py + squad_pb2_grpc.py

`SquadService` — serviço que o Python expõe ao Rust. Gerado.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
| --- | --- | --- | --- | --- |
| **serviço** `SquadService` | gRPC | wire | Python serve, Rust chama | `ExecuteTask(SquadTask)→stream SquadEvent`; `Health` |
| `SquadTask{task_id,description,decision_type,verification_evidence,model,roster[],tenant_id,actor}` | msg | wire | Rust→Python | tag 4 `max_autonomy_level` REMOVIDA (ADR 0033; era ignorada, ADR 0021); `verification_evidence` tipada (D3t) |
| `PersonaSpec{papel,prompt,funcao,ordem,custom}` | msg | wire | Rust→Python | roster de personas U7 |
| `SquadEvent{task_id,ts,tenant_id,actor, oneof: proposal\|consensus\|handoff\|hitl\|step\|error\|chat}` | msg | wire | Python→Rust (stream) | tenant/actor ecoados VERBATIM |
| `ChatMessage{author,author_role,text,in_reply_to}` | msg | wire | Python→Rust | author_role AGENT/HUMAN/SYSTEM |
| `Proposal{agent,confidence,content_json}` | msg | wire | Python→Rust | proposta de agente |
| `Consensus{decision_maker,strength,decision_json,requires_human}` | msg | wire | Python→Rust | `requires_human` setado à mão (proto3) |
| `Handoff{phase(START/ACK/COMPLETE/ERROR),from_agent,to_agent,contract,payload_digest}` | msg | wire | Python→Rust | transição de agente |
| `HitlEscalation{reason,confidence}` | msg | wire | Python→Rust | escalonamento humano |
| `StepResult{step_id,success,summary}` | msg | wire | Python→Rust | resultado de passo |
| `VerificationEvidence{run_id,git_sha,steps[],verdict,produced_at}` | msg | wire | Rust→Python | `verification-evidence.v1` tipada |
| `VerificationStep{name,tool,exit_code,duration_ms,findings[]}` | msg | wire | Rust→Python | passo do /verify |
| `VerificationFinding{tool,severity,message,file?,line?}` | msg | wire | Rust→Python | `optional` preserva presença |
| `Verdict` enum {UNSPECIFIED,PASS,FAIL,SKIPPED} | enum | wire | — | UNSPECIFIED = fail-closed |
| `HealthRequest{}` / `HealthResponse{ready,version}` | msg | wire | — | health check |

## python/packages/btv-proto-py/src/btv_proto/memory_pb2.py + memory_pb2_grpc.py

`MemoryService` — direção OPOSTA: Python serve (dono do corpus), Rust chama (ADR 0022). Gerado.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
| --- | --- | --- | --- | --- |
| **serviço** `MemoryService` | gRPC | wire | Python serve, Rust chama | `Health`; `Recall`; `List` |
| `RecallRequest{query,k}` | msg | wire | Rust→Python | busca léxica TF-IDF |
| `MemoryMatch{id,agent,decision_json,timestamp,score}` | msg | wire | Python→Rust | match recuperado |
| `RecallResponse{matches[]}` | msg | wire | Python→Rust | resultados |
| `ListRequest{agent?,limit}` | msg | wire | Rust→Python | mapa de memória |
| `MemorySummary{agent,count,latest_decision_json,latest_timestamp,top_confidence}` | msg | wire | Python→Rust | resumo REAL (sem tendência de esquecimento) |
| `ListResponse{agents[]}` | msg | wire | Python→Rust | resumos por agente |
| `HealthRequest{}`/`HealthResponse{ready,version}` | msg | wire | — | health |

## python/packages/btv-proto-py/src/btv_proto/promptforge_pb2.py + promptforge_pb2_grpc.py

`PromptForgeService` — Python serve geradores/linter ao Rust (Fase 3). Gerado.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
| --- | --- | --- | --- | --- |
| **serviço** `PromptForgeService` | gRPC | wire | Python serve, Rust chama | `Health`; `Lint`; `Render`; `ListGenerators` |
| `LintRequest{prompt}` | msg | wire | Rust→Python | prompt a lintar |
| `LintIssue{rule,message}` | msg | wire | Python→Rust | achado |
| `LintReport{score,grade,issues[]}` | msg | wire | Python→Rust | relatório |
| `RenderRequest{generator,fields:map<string,string>}` | msg | wire | Rust→Python | render de gerador |
| `RenderResponse{prompt}` | msg | wire | Python→Rust | prompt montado |
| `GeneratorField{name,label,required,placeholder}` | msg | wire | Python→Rust | campo |
| `GeneratorInfo{name,category,fields[]}` | msg | wire | Python→Rust | metadados de gerador |
| `ListGeneratorsResponse{generators[]}` | msg | wire | Python→Rust | catálogo |
| `HealthRequest{}`/`HealthResponse{ready,version}` | msg | wire | — | health |

---

# Pacote btv-review

## python/packages/btv-review/src/btv_review/__init__.py

Fachada do review orientado a valor (4 reviewers ponderados + gates duros + certificação).

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
| --- | --- | --- | --- | --- |
| `Certification`, `certify`, `evidence_hash` | símbolos | saída | `certification` → API | artefato certificável |
| `SECURITY_FLOOR`, `ReviewVerdict`, `evaluate` | símbolos | saída | `gates` → API | gates duros |
| `APPROVAL_THRESHOLD`, `ReviewScores`, `value_score` | símbolos | saída | `score` → API | média ponderada |
| `__all__` | `list[str]` | config | módulo | superfície pública |

Fluxo: reexporta os submódulos; nenhum dado transita.

## python/packages/btv-review/src/btv_review/score.py

Cálculo do `value_score` ponderado (contrato do BuildToValue review system).

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
| --- | --- | --- | --- | --- |
| `APPROVAL_THRESHOLD` | `float`=0.7 | config | const | limiar de aprovação por média |
| `WEIGHTS` | `dict[str,float]` | config | const | technical 0.25, performance 0.20, security 0.30, value 0.25 |
| `ReviewScores.technical/performance/security/value` | `float` 0–1 | wire | pydantic Field(ge=0,le=1) | as 4 dimensões |
| `scores` (value_score) | `ReviewScores` | entrada | param → soma ponderada | `Σ(dim*weight)` |
| retorno `value_score` | `float` | saída | módulo → caller | média ponderada |
| retorno `is_approved` | `bool` | saída | `value_score > 0.7` | aprovação simples |

Fluxo: `ReviewScores` (entrada) → `Σ dim*WEIGHTS[dim]` (intermediário) → score float (saída). Base que os gates SOBREPÕEM.

## python/packages/btv-review/src/btv_review/reviewers.py

Reviewers determinísticos que derivam scores da evidência REAL do `/verify` (código novo, não portado — nota de proveniência honesta).

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
| --- | --- | --- | --- | --- |
| `_SEVERITY_PENALTY` | `dict[str,float]` | config | const | critical 0.4, error 0.4, warning 0.1; não-listadas usam piso 0.05 |
| `evidence` (technical_score) | `Optional[dict]` | entrada | param | `verification-evidence.v1` |
| retorno `technical_score` | `float` | saída | módulo | fração de passos com `exit_code==0`; sem evidência/passos → 0.5 (neutro) |
| `steps`, `passed` (technical) | `list`,`int` | intermediário | `evidence["steps"]`, `sum(exit_code==0)` | contagem |
| `evidence` (security_score) | `Optional[dict]` | entrada | param | idem |
| `score` (security) | `float` | intermediário | acumulador de 1.0 | subtrai `_SEVERITY_PENALTY[severity]` por finding, piso 0.0 |
| retorno `security_score` | `float` | saída | módulo | 1.0 − penalidades; sem evidência → 0.5 |

Fluxo: `evidence` (entrada) → varredura de `steps[].exit_code` / `steps[].findings[].severity` (intermediário) → score float (saída). `performance`/`value` NÃO têm sinal determinístico — ficam a cargo do caller (nota anti-fabricação).

## python/packages/btv-review/src/btv_review/gates.py

Quality gates reais: regras duras que SOBREPÕEM a média ponderada (Fase 5 Onda 4).

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
| --- | --- | --- | --- | --- |
| `SECURITY_FLOOR` | `float`=0.5 | config | const | piso duro da dimensão security (piso de não-desqualificação, < 0.7 de propósito) |
| `ReviewVerdict.approved` | `bool` | wire | pydantic | veredito final |
| `ReviewVerdict.value_score` | `float` | wire | pydantic | média calculada |
| `ReviewVerdict.reason` | `str` | wire | pydantic | justificativa |
| `ReviewVerdict.gate_triggered` | `Optional[str]` | wire | pydantic | qual gate reprovou (None=média) |
| `evidence` (_has_critical_finding) | `Optional[dict]` | entrada | param | busca `severity=="critical"` em steps[].findings[] → bool |
| `scores` (evaluate) | `ReviewScores` | entrada | param → `value_score` | 4 dimensões |
| `evidence` (evaluate) | `Optional[dict]` | entrada | param | evidência do /verify |
| `vs` | `float` | intermediário | `value_score(scores)` | média ponderada |
| gate 1 critical_finding | intermediário | — | `_has_critical_finding` | reprova ANTES da média |
| gate 2 verify_fail | intermediário | — | `evidence["verdict"]=="fail"` | reprova |
| gate 3 security_floor | intermediário | — | `scores.security < 0.5` | reprova |
| gate 4 média | intermediário | — | `vs > APPROVAL_THRESHOLD` | só se nenhum gate disparou |
| retorno `evaluate` | `ReviewVerdict` | saída | módulo → caller | veredito com gate_triggered |

Fluxo: `scores`+`evidence` (entrada) → `vs` (intermediário) → cascata de 3 gates duros na ordem (critical→fail→floor) → senão média decide → `ReviewVerdict` (saída wire). Uma dimensão ruim não é "salva" pela média.

## python/packages/btv-review/src/btv_review/certification.py

Certificação: artefato do que foi verificado + hash da evidência — registrável no ledger Rust (`kind:"certification"`).

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
| --- | --- | --- | --- | --- |
| `Certification.run_id/git_sha` | `str` | wire | pydantic | identidade da run |
| `Certification.verdict` | `ReviewVerdict` | wire | pydantic | veredito embutido |
| `Certification.evidence_hash` | `str` | wire | pydantic | sha256 do JSON canônico da evidência (link imutável) |
| `Certification.steps_summary` | `list[str]` | wire | pydantic | ex.: "typecheck: ok" |
| `Certification.produced_at` | `str` | wire | pydantic | RFC3339 |
| `evidence` (evidence_hash) | `dict` | entrada | param → `canonical_json`→`sha256_hex` | reusa `btv_promptforge.hashing` (mesmo esquema do prompt-cache-key.v1) |
| retorno `evidence_hash` | `str` | saída | módulo | sha256 hex |
| `run_id,git_sha,verdict,evidence,produced_at` (certify) | mixed | entrada | params | monta a certificação |
| `steps_summary` (certify) | `list[str]` | intermediário | comprehension | `f"{name}: {'ok' if exit_code==0 else 'fail'}"` por step |
| retorno `certify` | `Certification` | saída | módulo → caller Rust | artefato completo |

Fluxo: `evidence`+metadados (entrada) → `steps_summary` + `evidence_hash` via hash canônico (intermediário) → `Certification` pydantic (saída wire). Python PRODUZ, Rust REGISTRA no ledger.

---

# Pacote btv-squad

## python/packages/btv-squad/src/btv_squad/__init__.py

Fachada do squad multi-agente (sidecar Python). Regra de ouro: nunca chama LLM direto — só via gateway Rust.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
| --- | --- | --- | --- | --- |
| Agentes/subsistemas re-exportados | classes | saída | submódulos → API | Architect/Auditor/Base/Designer/Developer/Ops, ConsensusResult, orquestrador, etc |
| `__all__` | `list[str]` | config | módulo | 30+ símbolos públicos |

Fluxo: agrega toda a API do pacote; nenhum dado transita.

## python/packages/btv-squad/src/btv_squad/config.py

Configuração 12-Factor do sidecar.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
| --- | --- | --- | --- | --- |
| `DEFAULT_MODEL` | `str` | config | `os.getenv("BTV_SQUAD_MODEL", "claude-sonnet-5")` | modelo padrão dos agentes; overridável por env |

Fluxo: env `BTV_SQUAD_MODEL` (entrada config) → const `DEFAULT_MODEL` consumida por todos os agentes/planner/servers.

## python/packages/btv-squad/src/btv_squad/_json.py

Extração robusta de um único objeto JSON de respostas de modelo (DRY do parse dos agentes).

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
| --- | --- | --- | --- | --- |
| `_DECODER` | `json.JSONDecoder` | estado | módulo | decoder reutilizado |
| `raw_text` | `str` | entrada | param → `find("{")` + `raw_decode` | resposta bruta do modelo |
| `context` | `str`="" | config/entrada | kwarg | rótulo p/ log ("ReAct" etc) |
| `rotulo` | `str` | intermediário | `f" ({context})"` | anexado ao warning |
| `inicio` | `int` | intermediário | `raw_text.find("{")` | posição do 1º `{`; -1 → `{}` |
| `candidate` | `Any` | intermediário | `_DECODER.raw_decode(raw_text[inicio:])` | acha fim REAL do objeto (corrige regex gulosa); JSONDecodeError → `{}` |
| retorno | `dict[str,Any]` | saída | módulo → agentes | `candidate` se dict, senão `{}` (nunca derruba o agente) |

Fluxo: `raw_text` (entrada) → localiza `{` → `raw_decode` (intermediário) → dict ou `{}` defensivo (saída). Usado por architect/auditor/developer/ops/designer/planning.

## python/packages/btv-squad/src/btv_squad/gateway.py

Contrato do gateway LLM consumido pelos agentes (Protocol desacoplado do gRPC, ADR 0005).

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
| --- | --- | --- | --- | --- |
| `LlmRequest.model` | `str` | wire | pydantic | espelha `btv.llm.v1.LlmRequest` |
| `LlmRequest.messages` | `list[dict]` | wire | pydantic (default []) | mensagens role/content |
| `LlmRequest.temperature` | `float\|None` | wire | pydantic | opcional |
| `LlmRequest.max_tokens` | `int\|None` | wire | pydantic | opcional |
| `LlmRequest.requester` | `str` | wire | pydantic | nome do agente (telemetria/rate-limit) |
| `LlmResponse.text` | `str` | wire | pydantic | agregado dos text_delta |
| `LlmResponse.input_tokens/output_tokens` | `int` | wire | pydantic | de Usage |
| `LlmResponse.cache_hit` | `bool` | wire | pydantic | de Usage |
| `LlmResponse.provider` | `str` | wire | pydantic | de Usage |
| `GatewayClient.generate` | Protocol | — | contrato | `LlmRequest→LlmResponse` |
| `ScriptedGatewayClient._responses` | `list[LlmResponse]` | estado | ctor | respostas roteirizadas (testes) |
| `ScriptedGatewayClient.requests` | `list[LlmRequest]` | estado/saída | acumulador | histórico; esgotou → AssertionError |
| `request` (generate scripted) | `LlmRequest` | entrada | param → `requests.append` + `pop(0)` | FIFO determinístico |

Fluxo: `LlmRequest` (entrada) → impl real (gRPC) ou Scripted (FIFO) → `LlmResponse` (saída). Fronteira que desacopla agentes do transporte.

## python/packages/btv-squad/src/btv_squad/permission.py

Contrato do client de permissão/HITL consumido pelo autonomia manager (Protocol, ADR 0005).

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
| --- | --- | --- | --- | --- |
| `PermissionRequest.tool` | `str` | wire | pydantic | espelha `btv.core.v1.PermissionRequest` |
| `PermissionRequest.scope` | `str` | wire | pydantic | escopo (agente) |
| `PermissionRequest.reason` | `str` | wire | pydantic | motivo |
| `PermissionRequest.confidence` | `float` | wire | pydantic | gatilho HITL |
| `PermissionDecision.approved` | `bool` | wire | pydantic | decisão |
| `PermissionDecision.operator_note` | `str\|None` | wire | pydantic | nota do operador |
| `PermissionClient.request_permission` | Protocol | — | contrato | `PermissionRequest→PermissionDecision` |
| `ScriptedPermissionClient._decisions` | `list[PermissionDecision]` | estado | ctor | decisões roteirizadas |
| `ScriptedPermissionClient.requests` | `list[PermissionRequest]` | estado | acumulador | histórico; esgotou → AssertionError |

Fluxo: `PermissionRequest` (entrada) → impl real (gRPC RequestPermission) ou Scripted → `PermissionDecision` (saída).

## python/packages/btv-squad/src/btv_squad/tool_client.py

Contrato do client de ferramentas consumido pelo DeveloperAgent (Protocol, "tool execution architecture").

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
| --- | --- | --- | --- | --- |
| `ToolCallRequest.tool` | `str` | wire | pydantic | espelha `btv.core.v1.ToolCall` |
| `ToolCallRequest.args_json` | `str` | wire | pydantic | args serializados |
| `ToolCallRequest.scope` | `str`="" | wire | pydantic | informativo |
| `ToolCallResult.content` | `str` | wire | pydantic | saída da ferramenta |
| `ToolCallResult.truncated` | `bool`=False | wire | pydantic | truncamento |
| `ToolCallResult.exit_code` | `int`=0 | wire | pydantic | 0 ok, 1 erro, -1 negado |
| `ToolClient.run_tool` | Protocol | — | contrato | `ToolCallRequest→ToolCallResult` |
| `ScriptedToolClient._results` | `list[ToolCallResult]` | estado | ctor | resultados roteirizados |
| `ScriptedToolClient.requests` | `list[ToolCallRequest]` | estado | acumulador | histórico; esgotou → AssertionError |

Fluxo: `ToolCallRequest` (entrada) → impl real (gRPC RunTool, executa no Rust) ou Scripted → `ToolCallResult` (saída). Python nunca toca disco.

## python/packages/btv-squad/src/btv_squad/grpc_clients.py

Implementações gRPC reais dos Protocols (Gateway/Permission/Tool), falando `CoreService` do Rust (ADR 0005).

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
| --- | --- | --- | --- | --- |
| `channel` / `_stub` | grpc canal/stub | estado | ctor | `CoreServiceStub(channel)` |
| **GrpcGatewayClient.generate** | | | | |
| `request` (LlmRequest pydantic) | entrada | entrada | param | do agente |
| `proto_req` (llm_pb2.LlmRequest) | intermediário | wire (saída) | mapeado | `model`, `messages_json=json.dumps(messages)`, `requester`; `temperature`/`max_tokens` só se não-None |
| stream `chunk` (LlmChunk) | wire (entrada) | entrada | `Generate(proto_req)` | consumido via `WhichOneof("payload")` |
| `text_parts` | `list[str]` | intermediário | acumulador | `text_delta` concatenados |
| `input_tokens/output_tokens/cache_hit/provider` | mixed | intermediário | de `chunk.usage` | métricas |
| ramo `error` | — | — | `chunk.error` | → `RuntimeError("gateway error: ...")` |
| retorno | `LlmResponse` | saída | módulo → agente | `text="".join(text_parts)` + métricas |
| **GrpcPermissionClient.request_permission** | | | | |
| `request` (PermissionRequest) | entrada | entrada | param | do autonomia manager |
| `proto_req` (core_pb2.PermissionRequest) | intermediário | wire (saída) | mapeado | tool/scope/reason/confidence |
| `decision` (proto) | wire (entrada) | entrada | `RequestPermission(proto_req)` | resposta |
| `approved` | `bool` | intermediário | `decision.decision == ALLOW` | UNSPECIFIED/DENY → False (fail-closed) |
| `note` | `str\|None` | intermediário | `operator_note if HasField else None` | presença explícita |
| retorno | `PermissionDecision` | saída | módulo → manager | approved+note |
| **GrpcToolClient.run_tool** | | | | |
| `request` (ToolCallRequest) | entrada | entrada | param | do developer |
| `proto_req` (core_pb2.ToolCall) | intermediário | wire (saída) | mapeado | tool/args_json/scope |
| `result` (proto ToolResult) | wire (entrada) | entrada | `RunTool(proto_req)` | resposta |
| retorno | `ToolCallResult` | saída | módulo → developer | content/truncated/exit_code |

Fluxo: modelos pydantic (entrada) → mensagens proto campo a campo (intermediário/wire) → RPC ao Rust → resposta proto → modelos pydantic (saída). Defesa contra default-zero do proto3 mapeando explicitamente; `WwhichOneof` discrimina o stream de LlmChunk.

## python/packages/btv-squad/src/btv_squad/server.py

Servidor gRPC `SquadService.ExecuteTask` — roda o orquestrador e streama SquadEvent; bidirecional (Python serve, chama CoreService de volta).

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
| --- | --- | --- | --- | --- |
| `VERSION` | `str` | config/estado | metadata | fallback "0.1.0" |
| `_PHASE` | `dict[str,enum]` | config | const | start/ack/complete/error → Handoff.Phase |
| **_to_squad_event(task_id, event, tenant_id, actor)** | | | | tradutor dict→proto |
| `task_id,tenant_id,actor` | `str` | entrada | params | ecoados no SquadEvent (VERBATIM) |
| `event` | `dict[str,Any]` | entrada | param (do orquestrador) | discriminado por `event["kind"]` |
| `ev` (SquadEvent) | proto | intermediário/saída | construído | `ts=datetime.now(utc).isoformat()` |
| kind=proposal | wire (saída) | — | `Proposal(agent,confidence,content_json=json.dumps(content))` | content→JSON |
| kind=consensus | wire (saída) | — | `Consensus(decision_maker or "", strength, decision_json, requires_human=bool)` | requires_human setado à mão |
| kind=hitl | wire (saída) | — | `HitlEscalation(reason,confidence)` | |
| kind=handoff | wire (saída) | — | `Handoff(phase=_PHASE[phase],from_agent,to_agent)` | |
| kind=step | wire (saída) | — | `StepResult(step_id,success,summary)` | |
| kind=chat | wire (saída) | — | `ChatMessage(author,author_role,text,in_reply_to)` | in_reply_to default "" |
| kind=error | wire (saída) | — | `ev.error = message` | |
| kind desconhecido | — | — | `ev.error = "evento desconhecido: ..."` | guarda defensiva |
| **_verification_evidence_from_request(request)** | | | | |
| `request.verification_evidence` | proto (entrada wire) | entrada | `HasField` + `VerificationEvidence.from_proto` | ausente → `(None, True)` fail-closed; inválida → `(None, True)` |
| retorno | `tuple[Optional[dict],bool]` | saída | módulo → ExecuteTask | `(evidencia_canonica via to_wire_dict, ausente_ou_invalida)` |
| **SquadServicer** | | | | |
| `self.core_socket/model/memory_dir` | mixed | estado | ctor | config do sidecar |
| **ExecuteTask(request, context)** — stream | | | | |
| `ctx` (TenantContext) | intermediário | — | `TenantContext.from_wire(tenant_id, actor)` | inválido → 1 SquadEvent de erro + return |
| `tenant_id,actor` | `str` | intermediário | de `ctx` ou "" | eco |
| `evidence,evidence_missing` | tuple | intermediário | `_verification_evidence_from_request` | |
| `roster` | `list[dict]` | intermediário | comprehension sobre `request.roster` | {papel,prompt,funcao,ordem,custom} |
| `task` | `dict` | intermediário | montado | {task_id,description,decision_type("architecture" default),verification_evidence,verification_evidence_missing,roster} |
| `queue` | `asyncio.Queue` | estado/intermediário | eventos do orquestrador | ponte sink→stream |
| `sink(event)` | callback async | intermediário | `queue.put(event)` | event_sink do orquestrador |
| `channel` | grpc canal | estado | `insecure_channel("unix://core_socket", grpc.default_authority=localhost)` | fix de interop UDS↔tonic |
| `gateway/permission/tool_client` | Grpc*Client | intermediário | sobre `channel` | de volta ao Rust |
| `memory` | AgentMemorySystem | intermediário | opcional `memory_dir` | |
| `task_model` | `str` | intermediário | `request.model or self.model` | modelo por tarefa sobrepõe default |
| `orchestrator` | UnifiedOrchestrator | intermediário | construído | gateway+permission+model+memory+tool |
| `run()` | task async | intermediário | `execute_complex_task(task, sink)` | exceção→`{"kind":"error"}`; finally→sentinela None |
| `runner` | asyncio.Task | estado | `create_task(run())` | |
| loop yield | SquadEvent (saída wire) | saída | `_to_squad_event` por item da queue | até sentinela None; finally: await runner + close channel |
| **serve(socket_path, core_socket, model)** | | | | |
| `socket_path/core_socket/model` | `str` | config/entrada | args | remove socket; add servicer; port unix:// |
| `--socket/--core-socket/--model` | args CLI | config | argparse | required/required/default DEFAULT_MODEL |
| `BTV_LOG_LEVEL` | env | config | logging | nível |

Fluxo: `SquadTask` (entrada wire) → parse tenant/evidence/roster → `task` dict + orquestrador → `execute_complex_task` emite event dicts na `queue` (intermediário) → `_to_squad_event` traduz cada um → stream de `SquadEvent` (saída wire). Bidirecional: durante o stream, os Grpc*Client chamam CoreService de volta ao Rust.

## python/packages/btv-squad/src/btv_squad/orchestrator.py

Orquestrador unificado — coordenação determinística que compõe agentes/consenso/planner/memória/autonomia; emite os event dicts (ADR 0004/0005/0006).

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
| --- | --- | --- | --- | --- |
| `_consensus_dict(consensus)` | `dict` | intermediário | `{**model_dump(), requires_human}` | preserva `@property` requires_human (não sai no model_dump) |
| `_AGENT_DISPLAY` | `dict[str,str]` | config | const | architect→Arquiteto, developer→Desenvolvedor, etc |
| `_summarize_proposal_content(content,confidence)` | `str` | intermediário/saída | busca keys recommendation/final_output/strategy/pattern/notes | resumo humano p/ chat; `pct=round(confidence*100)%`; fallback genérico |
| **__init__** (estado de instância) | | | | |
| `planner` (AdaptivePlanner) | estado | — | `attach_gateway(gateway)` | |
| `router/parallel/memory/evaluator/consensus/autonomy/sandbox/chain_manager` | estado | — | subsistemas | memory default se None |
| `permission_client` | estado | — | `autonomy.attach_permission_client` se dado | |
| `_event_sink` | `Optional[callback]` | estado | por execução | None=silencioso |
| `agents` | `dict[str,Agent]` | estado | 5 agentes | cada um `attach_memory`+`attach_gateway`; só developer recebe `attach_tool_client` |
| **_apply_persona_roster(roster)** | | | | |
| `roster` | `list[dict]` | entrada | param | personas U7 |
| `funcao_para_agente` | `dict` | config | mapa | plan→architect, produce→developer, review/validate→auditor |
| `acumulado` | `dict[str,list[str]]` | intermediário | acumulador | blocos `[Persona: papel]\n{prompt}` por agente |
| `agent.persona_prompt` | estado (agente) | saída | `"\n\n".join(blocos)` | combina VÁRIAS personas na mesma função |
| **execute_complex_task(task, event_sink)** | | | | pipeline principal |
| `task` | `dict` | entrada | param | {task_id,description,decision_type,verification_evidence,roster,...} |
| `event_sink` | callback | entrada/config | param → `_event_sink` | |
| `task_id` | `str` | intermediário | `task.get("task_id") or uuid4` | |
| `start` | datetime | intermediário | now(utc) | p/ duração |
| `relevant_context` | `dict` | intermediário | `memory.recall_similar(description, k=5)` | recall léxico → alimenta planejamento |
| `plan` | `dict` | intermediário | `planner.create_adaptive_plan(task, relevant_context)` | plano com steps |
| `proposals` | `dict[str,Proposal]` | intermediário | `_get_squad_proposals(plan)` | de architect/developer/auditor |
| `consensus` | ConsensusResult | intermediário | `consensus.reach_consensus(proposals, "architecture")` | |
| evento **consensus** | dict (saída) | saída | `_emit` | {kind,decision_maker,strength,requires_human,decision} |
| `_pct` | `int` | intermediário | `round(strength*100)` | p/ chat |
| evento **chat** (consenso) | dict (saída) | saída | `_emit_chat("Squad","SYSTEM",...)` | fraco→pede orientação; forte→narra líder |
| ramo HITL | — | — | se `requires_human`: emit hitl + `autonomy.execute_with_autonomy("orchestrator", {action:approve_plan,plan,critical:True})` | rejeitado→return `{success:False,reason:"Plan rejected"}` |
| `execution_results` | `list[dict]` | intermediário | `_execute_plan_steps(plan, task)` | resultados reais dos passos |
| `is_squad_de_produto` | `bool` | intermediário | `bool(task.get("roster"))` | produto não fail-closa por evidência ausente |
| `final_validation` | `dict` | intermediário/saída | fail-closed `{approved:False,...}` OU `auditor.validate_results(results, evidence)` | veredito |
| `overall_success` | `bool` | intermediário | `final_validation["approved"]` | |
| evento **step final_validation** | dict (saída) | saída | `_emit` | step_id="final_validation", summary=JSON{approved,confidence,issues} |
| `autonomy.record_action` | efeito | saída | ADR 0006 | registra resultado REAL |
| `memory.remember_decision("orchestrator", {...})` | efeito/wire | saída | grava JSONL | {task_id,task,plan_id,validation,context_recall_count,duration_seconds,confidence} |
| `_update_learning(task, execution_results)` | efeito | saída | router | atualiza rotas |
| retorno | `dict` | saída | módulo → server (descartado) | {success,task_id,plan,consensus,results,validation,confidence} |
| **_get_squad_proposals(plan)** | | | | |
| `goal` | `str` | intermediário | `plan.get("goal")` | |
| `architect_result/developer_result/audit_result` | `dict` | intermediário | `agents[x].execute({description,plan[,metrics]})` | audit recebe metrics{complexity=len(steps),coverage=85} |
| `proposals[x]` | Proposal | intermediário | `Proposal(confidence,content=result)` | confidence de `result.get("confidence",0.5)` |
| `_emit_proposal` | | saída | emit proposal + chat | |
| **_emit_proposal(agent, proposal)** | evento proposal + chat | saída | `_emit` | {kind:proposal,agent,confidence,content} + chat via `_summarize_proposal_content` |
| **_emit_chat(author,author_role,text)** | evento chat | saída | `_emit` | {kind:chat,author,author_role,text} |
| **_execute_plan_steps(plan, task)** | | | | |
| `results` | `list[dict]` | intermediário/saída | acumulador | resultados dos passos |
| `agent_name` | `str` | intermediário | `_select_agent_for_step(step)` | |
| `step_id` | `str` | intermediário | `str(step.get("step","?"))` | |
| evento **handoff start** | dict (saída) | saída | `_emit` | orchestrator→agent |
| ramo paralelo | — | — | `_can_parallelize` → `_extract_parallel_tasks` → `parallel.execute_parallel_with_limits` | evaluate cada; `_emit_step` all success |
| `step_task` | `dict` | intermediário | {description,action,step,prior_results=list(results)} | prior_results = passos concluídos |
| `result` | `dict` | intermediário | `agents[agent_name].execute(step_task)` | |
| `quality` | `dict` | intermediário | `evaluator.evaluate_agent_performance(...)` | technical_score |
| ramo replan | — | — | se `technical_score<0.6`: `planner.replan_from_point(plan, step, reflection{reason:low_quality,score})` | |
| **_emit_step(step_id, success, agent_name)** | handoff complete/error + step | saída | `_emit` | |
| **_select_agent_for_step(step)** | `str` | intermediário | mapa analyze→architect/design→designer/implement→developer/validate→auditor/deploy→ops | default developer |
| **_can_parallelize(step, plan)** | `bool` | intermediário | False se dependencies ou action∈{validate,deploy} | |
| **_extract_parallel_tasks(step, results)** | `list[callable]` | intermediário | developer_task + designer_task | ambos recebem {description,action,prior_results} |
| **_update_learning(task, results)** | efeito | saída | por result: `router.update_route_performance(task, route, success, latency)` | route/success/duration de cada result |
| **_attempt_recovery(task, error)** | `Optional[dict]` | intermediário/saída | `execute_complex_task(simplified_task)` | simplified={description,priority:low,simplified:True} |

Fluxo: `task` (entrada) → recall→plano→propostas→consenso (intermediários) → emite eventos consensus/hitl/chat → executa passos (handoff/step/proposal/chat) → auditoria final (fail-closed ou gateway) → step final_validation → record_action + remember_decision + update_learning → dict de retorno (descartado pelo server; os SquadEvent são o produto observável).

## python/packages/btv-squad/src/btv_squad/agents/__init__.py

Fachada dos 5 agentes + ReviewSystem.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
| --- | --- | --- | --- | --- |
| Architect/Auditor/Base/Designer/Developer/Ops/ReviewSystem | classes | saída | submódulos → API | 5 agentes reais instanciados pelo orquestrador |
| `__all__` | `list[str]` | config | módulo | superfície pública |

Fluxo: reexporta; nenhum dado transita.

## python/packages/btv-squad/src/btv_squad/agents/base.py

Classe base abstrata dos agentes (@abstractmethod execute; injeção de memory/gateway/persona).

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
| --- | --- | --- | --- | --- |
| `agent_type` | `str` | estado | ctor param | tipo do agente |
| `agent_id` | `str` | estado | `uuid4()` | id único |
| `created_at` | datetime | estado | now(utc) | |
| `confidence_threshold` | `float`=0.7 | config/estado | ctor | limiar |
| `memory` | `Any` | estado | `attach_memory` (lazy) | backend de memória |
| `gateway` | `Optional[GatewayClient]` | estado | `attach_gateway` (lazy, ADR 0005) | |
| `tools` | `list[str]` | estado | ctor | ferramentas declaradas |
| `persona_prompt` | `Optional[str]` | estado | injetado pelo roster (U7) | system prompt da persona |
| **system_with_persona(base)** | `str` | intermediário/saída | `f"{persona}\n\n{base}"` se persona | prepend da persona ao prompt operacional |
| **execute(task)** | abstract | — | subclasse | @abstractmethod |
| **validate_input(task)** | `bool` | intermediário | `"description" in task and bool(...)` | checagem básica |
| **log_decision(decision)** | `dict` | intermediário/saída | monta `entry{timestamp,agent,agent_id,decision}` → `memory.remember_decision` | persiste (defensivo: exceção só loga) |
| **attach_memory/attach_gateway** | efeito | — | setters de injeção | |
| **validate_confidence(confidence)** | `bool` | intermediário | `confidence >= threshold` | None→False |

Fluxo: `task` (entrada) → subclasses implementam execute → decisões viram `entry` (intermediário) → `memory.remember_decision` (saída/wire). Persona prepended ao system prompt.

## python/packages/btv-squad/src/btv_squad/agents/architect.py

Agente arquiteto com Chain-of-Thought real via gateway.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
| --- | --- | --- | --- | --- |
| `_SYSTEM_PROMPT` | `str` | config | const | pede JSON {problem_analysis,constraints,applicable_patterns,trade_offs,recommendation,architecture,components,risks,mitigations,estimated_effort,confidence} |
| `model/reasoning_history/tools` | estado | ctor | | tools=[analyze_architecture,generate_adr] |
| **execute(task)** | `dict` | entrada→saída | valida input | |
| `description` | `str` | intermediário | `task.get("description")` | |
| `reasoning` | `dict` | intermediário | `reason_with_cot(description)` | CoT do modelo |
| `plan` | `dict` | intermediário | `create_plan(task, reasoning)` | |
| `adr` | `str` | intermediário | `create_adr({title,problem_analysis,recommendation,trade_offs})` | |
| `decision` | `dict` | intermediário/saída | `log_decision` | {task,reasoning,plan,adr,confidence} |
| retorno | `dict` | saída | orquestrador | {success,agent,reasoning,plan,adr,confidence} |
| **reason_with_cot(problem)** | | | gateway None→RuntimeError | |
| `request` (LlmRequest) | wire (saída) | — | system+user, requester=architect | |
| `raw` (LlmResponse) | wire (entrada) | — | `gateway.generate` | |
| `response` | `dict` | intermediário | `_parse_reasoning(problem, raw.text)` | + timestamp |
| **_parse_reasoning** | `dict` | intermediário | `extract_json_object` | fallbacks defensivos p/ cada campo; confidence=float |
| **create_plan(task, reasoning)** | `dict` | intermediário/saída | {goal,architecture,components,patterns,risks,mitigations,estimated_effort} | tudo do reasoning (nada fixo) |
| **create_adr(decision)** | `str` | saída | template markdown ADR | Status/Context/Decision/Consequences |

Fluxo: `task.description` (entrada) → LlmRequest→gateway→`raw.text` → `extract_json_object` → `reasoning` (intermediário) → `plan`+`adr` → dict de retorno (saída) + decisão na memória.

## python/packages/btv-squad/src/btv_squad/agents/auditor.py

Agente auditor — checagens determinísticas como evidência + veredito real via gateway; gate duro de escrita.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
| --- | --- | --- | --- | --- |
| `_AUDIT_SYSTEM_PROMPT` | `str` | config | const | JSON {passed,confidence,notes,additional_checks}; nunca afirma arquivo criado |
| `_VALIDATE_RESULTS_SYSTEM_PROMPT` | `str` | config | const | JSON {approved,confidence,issues,agent_scores}; pesa verification_evidence |
| `_DANGEROUS_PATTERNS` | `list[tuple]` | config | const | eval(/exec(/__import__/os.system/subprocess |
| `_MUTATING_TOOLS` | `set` | config | const | {edit,bash} — única evidência mecânica de escrita |
| `_GATE_ISSUE` | `str` | config | const | mensagem do gate duro |
| **_claims_completion_without_write_evidence(results)** | `bool` | intermediário | varre results | True se developer "completed" com "tool_calls" mas sem `edit/bash exit_code==0`; ignora results sem "tool_calls" (caminho single-call) |
| `model/validation_history/tools` | estado | ctor | | |
| **execute(task)** | `dict` | entrada→saída | `audit(task)` | {success,agent,assessment,confidence} |
| **audit(task)** | `dict` | | gateway None→RuntimeError | |
| `prior_results` | `list` | entrada/intermediário | `task.get("prior_results")` | gate duro antes do gateway |
| gate curto-circuito | `dict` (saída) | — | se sem write evidence → {passed:False,confidence:0.0,notes:_GATE_ISSUE,...} | reprova ANTES do modelo |
| `issues` | `list[dict]` | intermediário | `check_security(task.code)` | achados de padrão perigoso |
| `warnings` | `list[dict]` | intermediário | `check_quality(task.metrics)` | achados de métrica |
| `payload` | `dict` | intermediário/wire | {task_description,security_issues,quality_warnings[,prior_agent_results]} | contexto p/ gateway |
| `request/raw` | wire | — | gateway.generate | requester=auditor |
| `judgment` | `dict` | intermediário | `_parse_judgment(raw.text)` | |
| retorno audit | `dict` | saída | {issues,warnings,**judgment} | |
| **validate_results(results, evidence)** | `dict` | entrada→saída | usado pelo orquestrador ao fim do plano | |
| gate curto-circuito | `dict` (saída) | — | sem write evidence → {approved:False,issues:[_GATE_ISSUE],...} | |
| `payload` | `dict` | intermediário/wire | {results[,verification_evidence]} | evidence entra como contexto (nunca decide sozinha) |
| retorno | `dict` | saída | `_parse_validation` | {approved,confidence,issues,agent_scores} |
| **check_security(code)** | `list[dict]` | intermediário/saída | busca `_DANGEROUS_PATTERNS` | {type:security,severity:critical,pattern,description} |
| **check_quality(metrics)** | `list[dict]` | intermediário/saída | complexity>10, coverage<80 | {type:quality,severity:warning,metric,value,threshold} |
| **_parse_judgment/_parse_validation** | `dict` | intermediário | `extract_json_object` | coerção bool/float, fallbacks |

Fluxo: `task`/`results`+`evidence` (entrada) → gate duro de write evidence (curto-circuito antes do gateway) → checagens determinísticas `issues`/`warnings` (intermediário) → `payload` JSON → gateway → veredito parseado (saída). Regra dura sobrepõe o LLM (mesma filosofia de gates.py).

## python/packages/btv-squad/src/btv_squad/agents/developer.py

Agente desenvolvedor — loop ReAct real (tool_call/final_answer) + caminho single-call para proposta/avaliação.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
| --- | --- | --- | --- | --- |
| `_SYSTEM_PROMPT` | `str` | config | const | JSON {final_output,status,confidence,notes} single-call |
| `_REACT_SYSTEM_PROMPT` | `str` | config | const | tool_call{tool:read/grep/edit/bash,args,reasoning} ou final_answer; edit só arquivo existente, bash cria |
| `_MAX_REACT_STEPS` | `int`=12 | config | const | teto de passos |
| `_REACT_TIMEOUT_SECONDS` | `int`=600 | config | const | teto de tempo |
| `ReviewSystem` | Protocol | — | contrato | review_code(code,metadata) |
| `model/review_system/history/tools/tool_client` | estado | ctor | | tools=[write_code,generate_tests,refactor,debug,analyze_requirements] |
| **attach_tool_client** | efeito | — | setter | |
| **execute(task)** | `dict` | entrada→saída | | |
| `description` | `str` | intermediário | `task.get("description")` | |
| `use_tools` | `bool` | intermediário | `bool(task.get("action")) and tool_client is not None` | sinal de ativação do ReAct |
| `result` | `dict` | intermediário | `implement_task(description, use_tools)` | |
| `decision` | `dict` | intermediário/saída | log_decision | {task,result,confidence} |
| retorno | `dict` | saída | {success,agent,**result} | |
| **create_code(task)** | `str` | saída | `implement_task(...).final_output` | |
| **generate_code(task)** | `str` | saída | code + review_system opcional | metadata{task_id,task_description,estimated_value,priority,filename} |
| **auto_fix_issues(code, reviews)** | `str` | intermediário/saída | correções mecânicas | remove vulnerabilities, prefixo se Degraded, TODO se coverage<30 |
| **implement_task(task, use_tools)** | `dict` | | gateway None→RuntimeError | |
| ramo ReAct | — | — | `_implement_with_tools(task)` + history.append | |
| ramo single-call | | | | |
| `request` (LlmRequest) | wire | — | system(persona)+user, requester=developer | |
| `raw` | wire | — | gateway.generate | |
| `result` | `dict` | intermediário | `_parse_result(raw.text)` | |
| **_implement_with_tools(task)** — loop ReAct | | | | |
| `messages` | `list[dict]` | estado/intermediário | acumulador de conversa | system(REACT)+user, cresce com assistant/user |
| `tool_calls` | `list[dict]` | intermediário/saída | acumulador | {tool,args,exit_code,content} por chamada |
| loop `_MAX_REACT_STEPS` | — | — | gateway.generate → `_parse_react_action` | |
| `action` | `dict` | intermediário | `_parse_react_action(raw.text)` | discrimina final_answer/tool_call/parse_error |
| final_answer | `dict` (saída) | — | {final_output,status,confidence,notes,tool_calls} | |
| tool_call | — | — | `messages.append(assistant)` + `tool_client.run_tool(ToolCallRequest(tool,args_json=json.dumps(args)))` | |
| `result` (ToolCallResult) | wire (entrada) | — | do Rust | |
| `observation` | `dict` | intermediário/wire | {tool,content,truncated,exit_code} → `messages.append(user, json)` | vira turno da conversa |
| parse_error | — | — | pede retry (assistant + user com instrução) | |
| esgotou passos/timeout | `dict` (saída) | — | {final_output:"",status:incomplete,confidence:0.0,notes,tool_calls} | nunca fabrica sucesso |
| **_parse_react_action(raw)** | `dict` | intermediário | `extract_json_object(context="ReAct")` | action∉{tool_call,final_answer}→{action:parse_error} |
| **_parse_result(raw)** | `dict` | intermediário | `extract_json_object` | {final_output,status,confidence,notes} |

Fluxo: `task` (entrada) → `use_tools` decide ReAct×single-call → ReAct: `messages` cresce com tool_call/observation (intermediário) executando `run_tool` no Rust, acumula `tool_calls`; para em final_answer ou teto → dict de resultado (saída) com tool_calls que o auditor inspeciona. `history` guarda estado entre chamadas.

## python/packages/btv-squad/src/btv_squad/agents/designer.py

Agente designer — design real via gateway + guarda de domínio de padrão.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
| --- | --- | --- | --- | --- |
| `_SYSTEM_PROMPT` | `str` | config | const | JSON {pattern,components,colors,typography,responsive,accessibility,confidence,notes} |
| `model/design_patterns/tools` | estado | ctor | | design_patterns=[material,fluent,carbon] |
| **execute(task)** | `dict` | entrada→saída | `create_design(task)` | {success,agent,**design} |
| `design` | `dict` | intermediário | `create_design` | |
| `decision` | `dict` | intermediário/saída | log_decision | {task,design,confidence} |
| **create_design(task)** | `dict` | | gateway None→RuntimeError | |
| `request/raw` | wire | — | system+user(json.dumps(task)), requester=designer | |
| `design` | `dict` | intermediário | `_parse_design(raw.text)` | |
| guarda de domínio | — | — | `if pattern not in design_patterns: pattern="material"` | validação de escolha externa |
| **_parse_design(raw)** | `dict` | intermediário/saída | `extract_json_object` | fallbacks: pattern=material, responsive=bool, confidence=float |

Fluxo: `task` (entrada) → LlmRequest(json task)→gateway→`raw.text` → `_parse_design` (intermediário) → guarda de pattern → dict de design (saída) + decisão na memória.

## python/packages/btv-squad/src/btv_squad/agents/ops.py

Agente de operações — plano de deploy/monitoramento real via gateway + guarda de estratégia.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
| --- | --- | --- | --- | --- |
| `_SYSTEM_PROMPT` | `str` | config | const | JSON {strategy,stages,rollback_plan,health_checks,scaling,monitoring{metrics,alerts,dashboards,logging},confidence,notes} |
| `model/deployment_strategies/tools` | estado | ctor | | deployment_strategies=[blue-green,canary,rolling] |
| **execute(task)** | `dict` | entrada→saída | `plan_deployment(task)` | {success,agent,**plan} |
| `plan` | `dict` | intermediário | `plan_deployment` | |
| `decision` | `dict` | intermediário/saída | log_decision | {task,plan,confidence} |
| **plan_deployment(task)** | `dict` | | gateway None→RuntimeError | |
| `request/raw` | wire | — | system+user(json.dumps(task)), requester=ops | |
| `plan` | `dict` | intermediário | `_parse_plan(raw.text)` | |
| guarda de domínio | — | — | `if strategy not in deployment_strategies: strategy="blue-green"` | |
| **_parse_plan(raw)** | `dict` | intermediário/saída | `extract_json_object` | fallbacks: strategy=blue-green, rollback_plan=bool, confidence=float |

Fluxo: `task` (entrada) → LlmRequest(json task)→gateway→`raw.text` → `_parse_plan` (intermediário) → guarda de strategy → dict de plano (saída) + decisão na memória.

## python/packages/btv-squad/src/btv_squad/consensus.py

Consenso ponderado por expertise (pydantic) — Proposal/Dissent/ConsensusResult.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
| --- | --- | --- | --- | --- |
| `DEFAULT_AGENT_WEIGHTS` | `dict[str,dict[str,float]]` | config | const | pesos por agente×domínio (architect architecture 0.9, developer implementation 0.95, auditor security 0.95, etc) |
| `HITL_ESCALATION_THRESHOLD` | `float`=0.7 | config | const | abaixo → escala p/ humano |
| `Proposal.confidence` | `float` 0–1 | wire | pydantic Field(default 0.5) | convicção |
| `Proposal.content` | `dict` | wire | pydantic (default {}) | conteúdo da proposta |
| `Dissent.agent/score` | `str/float` | wire | pydantic | voto divergente |
| `ConsensusResult.decision` | `Proposal\|None` | wire | pydantic | proposta vencedora |
| `ConsensusResult.consensus_strength` | `float` | wire | pydantic | força |
| `ConsensusResult.decision_maker` | `str\|None` | wire | pydantic | agente líder |
| `ConsensusResult.dissenting_opinions` | `list[Dissent]` | wire | pydantic | divergentes |
| `ConsensusResult.requires_human` | `bool` | saída | `@property` | `consensus_strength < 0.7` (não sai no model_dump) |
| `WeightedConsensusEngine.agent_weights` | `dict` | estado | pydantic Field | cópia dos defaults |
| **reach_consensus(proposals, decision_type)** | | | | |
| `proposals` | `dict[str,Proposal]` | entrada | param | do orquestrador |
| `decision_type` | `str` | entrada | param | ex.: "architecture" |
| `weighted` | `dict[str,float]` | intermediário | `weight * proposal.confidence` por agente | weight=`agent_weights[agent][decision_type]` default 0.5 |
| vazio | ConsensusResult | saída | — | strength 0.0, decision None |
| `winner` | `str` | intermediário | `max(weighted, key=...)` | |
| `total` | `float` | intermediário | `sum(weighted.values()) or 1.0` | |
| retorno | ConsensusResult | saída | orquestrador | strength=`weighted[winner]/total`, dissent=demais |

Fluxo: `proposals` (entrada) → voto ponderado `weight*confidence` (`weighted` intermediário) → `winner`/`total` → ConsensusResult com `requires_human` derivado (saída). Consenso <0.7 aciona HITL.

## python/packages/btv-squad/src/btv_squad/planning.py

Planejamento adaptativo — decomposição real via gateway; replan + classificação de falha determinística.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
| --- | --- | --- | --- | --- |
| **_format_recall(context)** | `str\|None` | intermediário | resume `context["documents"][:5]` | trunca a 400 chars; bloco p/ prompt; None se vazio |
| **_format_roster_domain(roster)** | `str\|None` | intermediário | avisa p/ não gerar deploy | squad de PRODUTO (papeis) → nota anti-deploy; None p/ engenharia |
| `_DECOMPOSE_SYSTEM_PROMPT` | `str` | config | const | JSON {steps[{step,action,description,estimated_time,dependencies,can_fail}],estimated_duration,confidence}; action∈analyze/design/implement/validate/deploy |
| `_REPLAN_SYSTEM_PROMPT` | `str` | config | const | JSON {recovery_steps[],confidence_penalty} |
| `model/gateway/plan_history/failure_patterns` | estado | ctor | | gateway lazy |
| **create_adaptive_plan(task, relevant_context)** | `dict` | entrada→saída | gateway None→RuntimeError | |
| `messages` | `list[dict]` | intermediário | system(DECOMPOSE) + domain_note? + context_note? + user(json task) | recall entra como contexto |
| `domain_note/context_note` | `str\|None` | intermediário | `_format_roster_domain`/`_format_recall` | |
| `request/raw` | wire | — | gateway, requester=planner | |
| `decomposition` | `dict` | intermediário | `_parse_decomposition(raw.text)` | {steps,estimated_duration,confidence} |
| `plan` | `dict` | intermediário/saída | montado | {plan_id=uuid4,task_id,goal,steps,estimated_duration,confidence,created_at,adaptive:True} → plan_history.append |
| **replan_from_point(original_plan, failed_step, reflection)** | `dict` | entrada→saída | | |
| `failure_key` | `str` | intermediário/estado | `f"{action}_{reason}"` → `failure_patterns[key]+=1` | contador de padrões de falha |
| `request/raw` | wire | — | gateway (json {failed_step,reflection}) | |
| `recovery` | `dict` | intermediário | `_parse_recovery(raw.text)` | {recovery_steps,confidence_penalty} |
| `new_plan` | `dict` | intermediário/saída | dict(original) + {plan_id novo,replanned,replanning_reason,parent_plan} | |
| `completed/remaining` | `list` | intermediário | split por `step["step"]` vs failed_step | |
| `reordered` | `list[dict]` | intermediário/saída | completed + recovery_steps renumerados + remaining | renumera step+dependencies |
| `new_plan.confidence` | `float` | saída | `max(0.0, original.confidence - confidence_penalty)` | |
| **analyze_failure(error, plan)** | `dict` | intermediário/saída | classificação por substring | timeout→increase_timeout, memory→optimise_memory; {error_type,error_message,failed_at,plan_id,reason,suggestion} |
| **_parse_decomposition/_parse_recovery** | `dict` | intermediário | `extract_json_object` | coerção int/float |

Fluxo: `task`+`relevant_context` (entrada) → `messages` com notas de domínio/recall (intermediário) → gateway → `decomposition` → `plan` com uuid (saída) + plan_history. Replan reordena passos concluídos+recovery+restantes; `failure_patterns` acumula estado.

## python/packages/btv-squad/src/btv_squad/hitl.py

Autonomia progressiva / HITL — só o portão de aprovação (não executa); record_action separado (ADR 0006).

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
| --- | --- | --- | --- | --- |
| `_AUTONOMY_THRESHOLDS` | `tuple`=(0.4,0.6,0.8) | config | const | score→nível 0..3 |
| `_TRUST_REWARD` | `float`=0.02 | config | const | sobe devagar |
| `_TRUST_PENALTY` | `float`=0.1 | config | const | cai rápido (assimétrico) |
| `autonomy_levels` | `dict[int,str]` | estado | @dataclass field | 0 full_human_control..3 full_autonomy |
| `agent_trust_scores` | `dict[str,float]` | estado | field | score por agente (default 0.5) |
| `action_history` | `list[dict]` | estado/saída | field | histórico de ações reais |
| `permission_client` | `Optional[PermissionClient]` | estado | attach | |
| **_get_autonomy_level(agent)** | `int` | intermediário | `sum(score>=limiar)` | 0..3 |
| **execute_with_autonomy(agent, action)** | `dict` | entrada→saída | portão | |
| `critical` | `bool` | intermediário | `action.get("critical")` | |
| não precisa aprovação | `{executed:True}` | saída | `_needs_human_approval` False | |
| sem permission_client | — | — | RuntimeError | |
| `request` (PermissionRequest) | wire (saída) | — | {tool=action,scope=agent,reason=str(plan),confidence=trust_score} | |
| `decision` | wire (entrada) | — | `permission_client.request_permission` | |
| rejeitado | `dict` | saída | `record_action(success=False)` + `{executed:False,reason:"Rejected by human",feedback:operator_note}` | |
| aprovado | `{executed:True}` | saída | | |
| **record_action(agent, action, success)** | efeito | saída | ADR 0006 — chamado após execução REAL | |
| `current_level` | `int` | intermediário | `_get_autonomy_level` | |
| `_update_score(agent, success)` | efeito/estado | — | reward/penalty | |
| entrada history | `dict` | saída/estado | {timestamp,agent,action,success,trust_score,autonomy_level} | append |
| **_update_score(agent, success)** | `float` | intermediário/estado | min(1.0,+0.02) ou max(0.0,-0.1) | atualiza `agent_trust_scores` |
| **_needs_human_approval(agent, critical)** | `bool` | intermediário | level 3→False, 2→critical, senão True | |

Fluxo: `action` (entrada) → nível de autonomia por `trust_score` → portão: aprovação automática ou `PermissionRequest`→Rust (wire) → `{executed}` (saída). `record_action` (separado) atualiza `trust_score`/`action_history` com resultado REAL.

## python/packages/btv-squad/src/btv_squad/memory.py

Memória persistente — corpus episódico JSONL (fonte da verdade) + cache + recall_similar (delega a recall.py).

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
| --- | --- | --- | --- | --- |
| `short_term` | `dict` | estado | ctor | memória curta (não usada no fluxo mostrado) |
| `storage_dir` | Path | config/estado | ctor | default `.btv/squad-memory` (mkdir) |
| `episodic_path` | Path | estado | `storage_dir/agent_memories.jsonl` | corpus JSONL |
| `_corpus_cache` | `list[dict]` | estado | cache | corpus parseado |
| `_corpus_stamp` | `tuple[int,int]\|None` | estado | `(mtime_ns, size)` | invalida em append de qualquer processo |
| **_corpus_fingerprint()** | `tuple\|None` | intermediário | `episodic_path.stat()` | (mtime_ns,size); FileNotFound→None |
| **remember_decision(agent, decision)** | efeito/wire | entrada→saída | grava JSONL | |
| `memory` (linha JSONL) | `dict` (wire) | saída | append | **campos:** `timestamp`(now utc iso), `agent`, `decision`(dict livre), `confidence`(float de decision.confidence) |
| **_load_corpus()** | `list[dict]` | intermediário | lê JSONL | cache até stamp mudar; linhas malformadas puladas; só dicts com "decision" |
| `records` | `list[dict]` | intermediário | acumulador | parse linha a linha |
| **list_memories(agent, limit)** | `list[dict]` | entrada→saída | `_load_corpus` + filtro agent + `reversed`[:limit] | mais recente primeiro |
| **recall_similar(query, k, embedder)** | `dict` | entrada→saída | recall léxico ou semântico | |
| `corpus` | `list[dict]` | intermediário | `_load_corpus()` | |
| `docs` | `list[str]` | intermediário | `json.dumps(rec["decision"])` por registro | corpus de busca |
| `ranked` | `list[tuple[int,float]]` | intermediário | `recall.rank(query,docs,k)` (léxico) ou `recall.semantic_rank(...,embedder,k)` | |
| retorno | `dict` | saída | listas paralelas | **campos:** `ids`(f"{agent}_{timestamp}"), `documents`, `metadatas`({agent,timestamp}), `scores`, `query`([query]), `n_results`(k) |

Fluxo: `remember_decision` grava linha JSONL {timestamp,agent,decision,confidence} (saída wire) → `_load_corpus` (cache por fingerprint) → `recall_similar` monta `docs` e delega a recall.rank → listas paralelas ids/documents/metadatas/scores (saída). Corpus é a fonte da verdade cross-sessão.

## python/packages/btv-squad/src/btv_squad/recall.py

Recuperação local por similaridade (Fase 6) — índice TF-IDF esparso + cosseno; embedder semântico plugável.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
| --- | --- | --- | --- | --- |
| `_STOPWORDS` | `set` | config | const | stopwords PT+EN (descontadas do TF-IDF) |
| `_TOKEN` | regex | config | const | `\w+` unicode |
| **_tokenize(text)** | `list[str]` | intermediário | casefold + filtro | remove stopwords e tokens <2 chars |
| **_idf(docs_tokens)** | `dict[str,float]` | intermediário | `log((1+N)/(1+df))+1` | IDF suavizado ≥1 (memória única recuperável) |
| `df` | `Counter` | intermediário | contagem documental | |
| **_vector(tokens, idf)** | `dict[str,float]` | intermediário | TF×IDF normalizado L2 | norm 0→{} |
| **_cosine(a, b)** | `float` | intermediário | produto interno de normalizados | otimiza iterando o menor |
| **rank(query, docs, k)** | `list[tuple[int,float]]` | entrada→saída | ranking léxico | |
| `docs_tokens` | `list[list[str]]` | intermediário | tokeniza cada doc | |
| `idf` | `dict` | intermediário | `_idf` | |
| `qvec` | `dict` | intermediário | `_vector(_tokenize(query), idf)` | vazio→[] |
| `scored` | `list[tuple]` | intermediário | cosseno por doc, filtra `>1e-9` | ordenado desc, top-k |
| retorno rank | `list[(idx,score)]` | saída | memory.py | score positivo = termos distintivos em comum; sem match→[] |
| `Embedder` | Protocol | — | contrato | `embed(texts)→list[list[float]]` |
| **_dense_cosine(a, b)** | `float` | intermediário | cosseno de vetores densos | |
| **semantic_rank(query, docs, embedder, k)** | `list[tuple]` | entrada→saída | ranking semântico | |
| `vectors` | `list[list[float]]` | intermediário | `embedder.embed([query,*docs])` | |
| `qvec/dvecs` | vetores | intermediário | split | |
| `scored` | `list[tuple]` | intermediário/saída | `_dense_cosine` filtra `>1e-9`, top-k | sinônimo/paráfrase casam |

Fluxo: `query`+`docs` (entrada) → tokenize → IDF → vetores TF-IDF normalizados (intermediário) → cosseno → top-k índices+scores positivos (saída). `semantic_rank` é o caminho neural plugável (default é léxico, ADR 0013).

## python/packages/btv-squad/src/btv_squad/routing.py

Router adaptativo que aprende com desempenho histórico.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
| --- | --- | --- | --- | --- |
| `route_performance` | `dict[str,dict[str,float]]` | estado | @dataclass field | stats por rota+hash |
| **smart_route(request)** | `str` | entrada→saída | `request.get("preferred_route") or "default"` | roteamento simples |
| **update_route_performance(request, route, success, latency)** | efeito/estado | entrada | acumula stats | |
| `key` | `str` | intermediário | `f"{route}_{_hash_request(request)}"` | |
| `stats` | `dict` | intermediário/estado | setdefault {attempts,successes,total_latency} | incrementa; deriva `success_rate`, `avg_latency` |
| **_hash_request(request)** | `str` | intermediário | `sha1(repr(sorted(items)))` | usedforsecurity=False |

Fluxo: `request,route,success,latency` (entrada) → `key` por hash (intermediário) → `stats` acumulado com success_rate/avg_latency (estado). `smart_route` devolve rota preferida ou default.

## python/packages/btv-squad/src/btv_squad/evaluation.py

Avaliação contínua da qualidade dos agentes — deriva de campos REAIS (confidence+success), sem default fabricado.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
| --- | --- | --- | --- | --- |
| `metrics` | `dict[str,list[float]]` | estado | @dataclass field | task_success_rate, average_confidence, technical_score (rolantes) |
| **evaluate_agent_performance(agent_name, task_result)** | `dict` | entrada→saída | | |
| `task_result` | `dict` | entrada | param | resultado real do agente |
| `technical` | `float` | intermediário | `evaluate_technical_quality` | |
| `improvement` | `float` | intermediário | `compare_with_baseline(technical)` | |
| `_record(...)` | efeito/estado | — | append em metrics | success→1.0/0.0, confidence, technical |
| retorno | `dict` | saída | orquestrador | {agent,technical_score,improvement} |
| **evaluate_technical_quality(result)** | `float` | intermediário/saída | `0.0 se not success else confidence` | nenhum default fabricado |
| **compare_with_baseline(technical)** | `float` | intermediário/saída | `technical - média(history)` | 0.0 na 1ª avaliação |
| **_record(metric, value)** | efeito/estado | — | `metrics[metric].append` | |

Fluxo: `task_result` (entrada) → `technical`(=confidence se success) + `improvement`(delta vs baseline real) (intermediário) → grava métricas rolantes (estado) → dict {technical_score,improvement} (saída). Portão de replan do orquestrador usa technical_score<0.6.

## python/packages/btv-squad/src/btv_squad/parallel.py

Execução paralela com limite de recurso (semáforo + gather) — plumbing determinístico.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
| --- | --- | --- | --- | --- |
| `TaskLike` | type alias | config | — | `Awaitable\|Callable[[],Awaitable]` |
| `limits` | `dict[str,float]` | config/estado | @dataclass field | default {max_concurrent:5} |
| **execute_parallel_with_limits(tasks)** | `list[Any]` | entrada→saída | | |
| `tasks` | Iterable[TaskLike] | entrada | param | callables ou awaitables |
| `semaphore` | asyncio.Semaphore | intermediário | `int(limits["max_concurrent"])` | |
| `_ensure/_run` | closures | intermediário | await task sob semáforo | |
| retorno | `list` | saída | `asyncio.gather(...)` | resultados na ordem |

Fluxo: `tasks` (entrada) → semáforo limita concorrência (intermediário) → `gather` → lista de resultados (saída). Sem chamada de LLM (decisão mecânica).

## python/packages/btv-squad/src/btv_squad/chains.py

Cadeia de prompts resiliente com retry simples.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
| --- | --- | --- | --- | --- |
| `ChainStep.name` | `str` | estado/config | @dataclass | nome do passo |
| `ChainStep.execute` | `Callable[[Any],Any]` | estado | @dataclass | função do passo |
| `ResilientPromptChain.steps` | Iterable[ChainStep] | estado | @dataclass | passos |
| `ResilientPromptChain.max_retries` | `int`=3 | config | @dataclass | tentativas |
| **execute_with_checkpoints(initial_input)** | `Any` | entrada→saída | | |
| `current` | `Any` | intermediário/estado | encadeia saída→entrada | acumulador entre passos |
| `result` | `Any` | intermediário | `step.execute(current)` | await se coroutine |
| falha final | — | — | `RuntimeError(f"Falha na etapa {name}: ...")` | após max_retries; `sleep(0)` entre tentativas |
| retorno | `Any` | saída | caller | valor final da cadeia |

Fluxo: `initial_input` (entrada) → cada `step.execute` transforma `current` (intermediário, encadeado) com retry → valor final (saída). Instanciado vazio no orquestrador (`ResilientPromptChain([])`).

## python/packages/btv-squad/src/btv_squad/verification.py

Espelho pydantic do `verification-evidence.v1` (D3t) — from_proto (parse) + to_wire_dict (canônico).

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
| --- | --- | --- | --- | --- |
| `_VERDICT_FROM_PROTO` | `dict[int,str\|None]` | config | const | 0→None(fail-closed),1→pass,2→fail,3→skipped |
| `Finding.tool/severity/message` | `str` | wire | pydantic | achado |
| `Finding.file/line` | `Optional[str/int]` | wire | pydantic | presença explícita |
| **Finding.from_proto(msg)** | Finding | entrada→intermediário | `HasField` p/ file/line | ausência ≠ vazio |
| **Finding.to_wire_dict()** | `dict` | saída/wire | omite file/line se None | espelha `skip_serializing_if` do Rust |
| `VerificationStep.name/tool/exit_code/duration_ms/findings` | mixed | wire | pydantic | passo |
| **VerificationStep.from_proto/to_wire_dict** | | entrada/saída | mapeia findings | findings sempre presente (não skip) |
| `VerificationEvidence` (frozen) | model | wire | pydantic | imutável |
| `VerificationEvidence.run_id/git_sha/steps/verdict/produced_at` | mixed | wire | pydantic | verdict pass/fail/skipped |
| **VerificationEvidence.from_proto(msg)** | VerificationEvidence | entrada→saída | parse-don't-validate | verdict UNSPECIFIED→`ValueError` (fail-closed) |
| `verdict` | `str\|None` | intermediário | `_VERDICT_FROM_PROTO.get(msg.verdict)` | None→raise |
| **VerificationEvidence.to_wire_dict()** | `dict` | saída/wire | forma canônica do schema | idêntica ao Rust — prompt do auditor byte-idêntico |

Fluxo: mensagem proto `VerificationEvidence` (entrada wire) → `from_proto` valida verdict/presença (intermediário) → objeto frozen pydantic → `to_wire_dict()` reproduz JSON canônico (saída) que desce ao orquestrador/auditor. Usado por server.py `_verification_evidence_from_request`.

## python/packages/btv-squad/src/btv_squad/tenant.py

Espelho pydantic mínimo do `TenantContext` do Rust (D4t) — validação canônica de tenant/actor.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
| --- | --- | --- | --- | --- |
| `LOCAL_TENANT_ID` | `str` | config | const | UUID fixo do modo local (`00...01`, ADR 0025) |
| `TenantContext` (frozen) | model | wire | pydantic | imutável |
| `TenantContext.tenant_id` | `str` | wire | pydantic | UUID canônico |
| `TenantContext.actor` | `str` | wire | pydantic | operador |
| **_tenant_canonico(v)** `@field_validator` | `str` | intermediário | `uuid.UUID(v)` + `str(parsed)==v` | não-UUID ou não-canônico→`ValueError` (eco VERBATIM do D2t exige forma canônica) |
| **_actor_nao_vazio(v)** `@field_validator` | `str` | intermediário | `v.strip()` | vazio→`ValueError` |
| **from_wire(tenant_id, actor)** | `Optional[TenantContext]` | entrada→saída | par vazio→None (pré-D2t); parcial/inválido→`ValueError` (fail-closed) | |

Fluxo: par `(tenant_id, actor)` do proto (entrada wire) → `from_wire` → par vazio vira None, senão valida forma canônica (intermediário) → `TenantContext` frozen (saída) cujo tenant/actor são ecoados VERBATIM nos SquadEvent.

## python/packages/btv-squad/src/btv_squad/security.py

Config de segurança (defesa em profundidade Python; autoridade final é o Rust).

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
| --- | --- | --- | --- | --- |
| `MAX_EXECUTION_TIME_SECONDS` | `int`=30 | config | const | limite |
| `MAX_MEMORY_PER_TOOL_MB` | `int`=512 | config | const | limite |
| `MAX_CONCURRENT_TOOLS` | `int`=5 | config | const | limite |
| `FORBIDDEN_PATTERNS` | `list[str]` | config | const | regex perigosos (rm -rf /, fork bomb, dd, DROP DATABASE, <script, eval(, __import__, os.system, subprocess., socket.) |
| `ALLOWED_DOMAINS` | `set` | config | const | api.buildtoflip.com, localhost, 127.0.0.1 |
| `HIGH_RISK_TOOLS` | `set` | config | const | database_write, send_email, make_payment, delete_resource, modify_production_config |
| **canonical_params(params)** | `str` | intermediário | `json.dumps(sort_keys,default=str)` | serialização ÚNICA (fecha bypass repr×json) |
| **validate_tool_call(tool_name, params)** | `tuple[bool,str]` | entrada→saída | | |
| high-risk | `(False, "requires human approval")` | saída | — | |
| `payload` | `str` | intermediário | `canonical_params(params)` | |
| padrão proibido | `(False, "Forbidden pattern...")` | saída | `re.search` IGNORECASE | |
| ok | `(True, "OK")` | saída | | |

Fluxo: `tool_name,params` (entrada) → checa HIGH_RISK → `canonical_params` (intermediário) → varre FORBIDDEN_PATTERNS → `(bool, reason)` (saída). Camada complementar, nunca único portão.

## python/packages/btv-squad/src/btv_squad/sandbox.py

Sandbox de execução de ferramentas — DockerSandbox (stub) + SecureToolSandbox (degrada sem Docker).

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
| --- | --- | --- | --- | --- |
| `docker` | módulo\|None | estado | import opcional | ImportError→None |
| `SecurityError(RuntimeError)` | exceção | saída | módulo | violação de guardrail |
| `DockerSandbox.image` | `str` | config | @dataclass | python:3.11-slim |
| `DockerSandbox.network_disabled` | `bool`=True | config | @dataclass | isolamento de rede |
| **DockerSandbox.run(command, environment, timeout)** | `str` | entrada→saída | container run | docker None→RuntimeError |
| `container/result/logs` | mixed | intermediário | docker SDK | StatusCode≠0→RuntimeError(logs); logs decode UTF-8 |
| `SecureToolSandbox.docker_sandbox/execution_timeout/memory_limit_mb/cpu_quota` | mixed | config/estado | @dataclass | 30s/512MB/0.5 |
| **execute_tool_sandboxed(tool_name, params)** | `dict` | entrada→saída | | |
| `_validate_security` | efeito | — | valida antes | |
| sem docker | `dict` | saída | resposta simulada {tool,params,sandboxed:False,message} | degradação graciosa |
| `command` | `list` | intermediário | `["python","-m",tool_name]` | |
| `output` | `str` | intermediário | `docker_sandbox.run(command, {PARAMS:json.dumps(params)}, timeout)` | |
| retorno com docker | `dict` | saída | {tool,output,sandboxed:True} | |
| **_validate_security(tool_name, params)** | efeito | — | `SecurityConfig.validate_tool_call` + `_validate_params_safety` | is_safe False→SecurityError; params dangerous→ValueError |
| **_validate_params_safety(params)** | `bool` | intermediário | `canonical_params` + FORBIDDEN_PATTERNS | mesma serialização de validate_tool_call (não pode divergir) |

Fluxo: `tool_name,params` (entrada) → `_validate_security` (SecurityConfig + patterns) → sem Docker: dict simulado (saída); com Docker: `command`+PARAMS env → container → output (saída). Stub desta onda; contêineres reais são Fase 6.

## python/packages/btv-squad/src/btv_squad/memory_server.py

Servidor gRPC `MemoryService` (ADR 0022) — Python serve leitura de memória ao Rust; nunca grava.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
| --- | --- | --- | --- | --- |
| `VERSION` | `str` | config/estado | metadata | fallback "0.1.0" |
| **_summarize_by_agent(records)** | `list[MemorySummary]` | intermediário/saída | agrupa por agente | |
| `records` | `list[dict]` | entrada | param (de list_memories, ordenado) | |
| `by_agent` | `dict[str,list]` | intermediário | agrupamento | |
| `latest/top` | `dict` | intermediário | `recs[0]` (mais recente), `max(confidence)` | |
| `MemorySummary` (wire) | proto | saída | {agent,count,latest_decision_json,latest_timestamp,top_confidence} | sem tendência de esquecimento (forgetting.py removido) |
| `MemoryServicer.memory` | AgentMemorySystem | estado | ctor | opcional memory_dir |
| **Recall(request, context)** | RecallResponse | entrada→saída | | |
| `request.query/request.k` | wire | entrada | `recall_similar(query, k or 5)` | |
| `result` | `dict` | intermediário | listas paralelas | |
| `matches` | `list[MemoryMatch]` | saída/wire | zip(ids,documents,metadatas,scores) | {id,agent,decision_json=doc,timestamp,score} |
| **List(request, context)** | ListResponse | entrada→saída | | |
| `agent` | `str\|None` | intermediário | `request.agent if HasField else None` | |
| `limit` | `int` | intermediário | `request.limit or 50` | |
| `records` | `list[dict]` | intermediário | `list_memories(agent, limit)` | |
| retorno List | ListResponse (wire) | saída | `_summarize_by_agent(records)` | |
| **Health** | HealthResponse | saída | ready=True,version | |
| **serve(socket_path, memory_dir)** | | config | args | remove socket; port unix:// |
| `--socket/--memory-dir` | args CLI | config | argparse | required / default .btv/squad-memory |
| `BTV_LOG_LEVEL` | env | config | logging | nível |

Fluxo: `RecallRequest`/`ListRequest` (entrada wire) → `AgentMemorySystem.recall_similar`/`list_memories` (intermediário) → `MemoryMatch[]` / `MemorySummary[]` (saída wire). Só leitura — quem grava é o orquestrador em processo via `remember_decision`.
