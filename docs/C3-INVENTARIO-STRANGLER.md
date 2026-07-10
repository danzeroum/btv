# C3 — inventário do strangler: endpoints → serviços de aplicação

Preparação do C3 (escolha **(a)** do dono — enriquecer variantes, wire
intacto): o mapa dos endpoints dos três módulos-roteadores de `btv-cli`
na **ordem proposta de estrangulamento**, com o serviço de aplicação de
destino de cada um. Este documento é a descrição dos futuros PRs do C3 —
cada onda vira um PR que troca os emissores/SQL do handler pela porta de
domínio correspondente (Trilhas A/B, já na main), com os goldens T1/T3
como juízes de que o wire não moveu.

Regra do mapa: **onda = um agregado/porta**, não "um arquivo" — o
estrangulamento segue as costuras do domínio, e cada PR fecha um conjunto
que pode ser julgado pela mesma suíte. Endpoints de infraestrutura de
sessão/streaming (SSE, permissões ao vivo) ficam por último: dependem da
E1s para o `TenantContext` da borda chegar neles com sessão real.

## Onda C3.1 — Runs + gates + entregas (`RunRepository` + `LedgerRepository`)

O coração do produto; é onde a decisão (a) morde (variantes enriquecem
até o payload real dos emissores; `tenant` entra no corpo das entradas
novas com regravação justificada — ADR 0027).

| Endpoint | Handler | Destino |
|---|---|---|
| `POST /api/btv/squads` | `ativar_squad_handler` | serviço de aplicação `ativar_squad` → `RunRepository::save` + `LedgerRepository::append(SquadActivated)` |
| `GET /api/btv/squads` | `list_runs_handler` | `RunRepository::list` |
| `POST /api/btv/squads/{task_id}/gate` | `aprovar_gate_handler` | `Run::approve_gate` (agregado) + `save` + `append(GateApproved)` |
| `POST /api/btv/squads/{task_id}/ajuste` | `pedir_ajuste_handler` | `Run::transition_to` + `append(AdjustRequested)` |
| `GET /api/btv/deliverables` | `list_deliverables_handler` | `RunRepository::list_deliverables` |
| `GET /api/btv/deliverables/{id}/download` | `download_deliverable_handler` | `RunRepository::get_deliverable` (+ leitura de arquivo, fora da porta) |

## Onda C3.2 — Personas (`PersonaRepository` + `LedgerRepository`)

| Endpoint | Handler | Destino |
|---|---|---|
| `GET/DELETE /api/btv/personas/{template_id}` | `list_personas_handler` / `clear_overrides_handler` | `PersonaRepository::{list_overrides+list_custom, clear_overrides}` |
| `PUT/DELETE /api/btv/personas/{template_id}/{papel}` | `set_override_handler` / `delete_override_handler` | `PersonaRepository::{set_override, delete_override}` + `append(PersonaUpdated)` |
| `POST /api/btv/personas/{template_id}/custom` | `create_custom_handler` | `PersonaRepository::insert_custom` |
| `PUT/DELETE /api/btv/personas/{template_id}/custom/{id}` | `update_custom_handler` / `delete_custom_handler` | `PersonaRepository::{update_custom, delete_custom}` |

## Onda C3.3 — Templates + Designer (`LedgerRepository`; publicação ainda sem porta)

| Endpoint | Handler | Destino |
|---|---|---|
| `POST /api/btv/designer/flows` | `salvar_fluxo_handler` | `append(FlowSaved)` (validação `squad.workflow.v1` intacta) |
| `GET /api/btv/templates/publicacao` | `list_publicacao_handler` | **lacuna declarada**: `template_pub` não tem porta (fora da 1ª leva do ADR 0026) — segue no `BtvStore` legado até a trilha correspondente |
| `POST /api/btv/templates/{id}/publicacao` | `set_publicacao_handler` | idem + `append(TemplatePublished)` |

## Onda C3.4 — Users (porta a nascer com a E1s)

`GET/POST /api/btv/users`, `DELETE /{id}`, `POST /{id}/ativo|pin|verify-pin`
(`list/create/delete/set_ativo/set_pin/verify_pin`): o `User` do domínio
existe (A2), mas o repositório de users é decisão da **E1s** (identidade) —
estrangular antes dela criaria uma porta que a E1s redesenharia. Ficam por
último DELIBERADAMENTE; o `append(UserRemoved)` migra junto.

## Fora do estrangulamento do C3 (declarado, não esquecido)

- **`web_agent.rs`** (`/api/session/*`, `/api/permissions/*`): sessão de
  código ao vivo + matriz de permissões — infraestrutura da Fase 7, sem
  porta de domínio correspondente nesta fase; o `TenantContext` chega
  neles pela E1s (extractor), não pelo strangler.
- **`squad_agent.rs`** (`/api/squad/*`): já fala com o orquestrador real
  via gRPC (D2t propaga tenant/actor); o que há para estrangular é o
  ledger operacional que ele grava — entra na unificação de portas
  (`OperationalEvent`, pendência do G1), não no C3.
- **C4** (mover os módulos-roteadores para `btv-server`) continua
  DEPOIS do C3 avançado, na janela dele — este inventário não o adianta.

## Pré-requisito comum (o PR "C3.0")

A decisão (a): enriquecer as variantes de `DomainEventKind` até o payload
real dos emissores (`trilha` no `ExportGenerated`; `template_versao`/
`nome`/`papeis`/`personas_proprias` no `SquadActivated`; `refs`/
`prompt_hashes` com tipo pela regra da cerca — struct tipada ou vetor de
strings, razão exposta no PR) + goldens intactos. Nenhuma onda anda antes
disso.
