# BuildToValue no Docker — imagem de TESTE / homologação

> **Enquadramento honesto (leia primeiro).** O BuildToValue é **local-first**: o produto
> é um CLI/TUI de desenvolvedor, e o `btv dashboard` amarra em `127.0.0.1` por
> **decisão de arquitetura** (`crates/btv-server/src/lib.rs:6`,
> `crates/btv-cli/src/main.rs:247`), não config. Não existe caminho de deploy
> hospedado (`infra/README.md`). Esta imagem **não** transforma o BuildToValue num
> serviço web multiusuário — ela empacota o CLI + o sidecar Python para você
> **rodar o BuildToValue como ferramenta, dentro de um container, na sua VPS via SSH**.
> É homologação legítima do produto no modo em que ele foi projetado — só não é
> "subir um serviço exposto na internet".

## Build & uso

A partir da **raiz do repositório** (o contexto precisa da árvore toda):

```sh
docker build -f infra/docker/Dockerfile -t btv:test .

# shell interativo com o `btv` no PATH e o seu projeto montado em /work:
docker run --rm -it -e ANTHROPIC_API_KEY=sk-ant-... -v "$PWD":/work btv:test
```

Ou via compose:

```sh
export ANTHROPIC_API_KEY=sk-ant-...
docker compose -f infra/docker/docker-compose.yml run --rm btv
```

Dentro do container, na ordem recomendada de teste:

```sh
btv verify                 # 1. self-teste (mesmo comando do CI) — valida o ambiente
btv run "descreva este repo"   # 2. tarefa única (caminho mais simples)
btv chat                   # 3. sessão interativa
btv squad "tarefa multi-agente"# 4. exercita o sidecar Python (squad)
```

## Usando a API da DeepSeek (em vez da Anthropic)

O gateway (`crates/btv-llm/src/gateway.rs`) já reconhece DeepSeek nativamente —
mesma cadeia de fallback, mesmo protocolo (compatível com OpenAI):

```sh
docker run --rm -it -e DEEPSEEK_API_KEY=sk-... -v "$PWD":/work btv:test
```

**⚠️ Passo obrigatório: sempre passe `--model deepseek-chat`.** O gateway **não**
escolhe o provider pelo nome do modelo — ele manda o texto de `--model` **como
está** no corpo da requisição, para qualquer provider configurado
(`openai.rs:22: "model": req.model`). O default do CLI é `claude-sonnet-5`
(`btv-cli/src/main.rs`); sem sobrescrever, o BuildToValue mandaria `"claude-sonnet-5"`
para a API da DeepSeek, que rejeita (400 — modelo desconhecido). Então:

```sh
btv run --model deepseek-chat "descreva este repo"
btv chat --model deepseek-chat
btv squad --model deepseek-chat "tarefa multi-agente"
```

**Só pelo CLI.** `btv squad --model` (acima) spawna um processo Python novo
a cada chamada e passa `--model` de verdade (`squad.rs::run_squad` →
`SquadSupervisor::spawn(..., &opts.model)`) — funciona como os outros dois.

**O botão "Squad" do dashboard (navegador) é diferente.** Ali os 5 agentes
Python rodam num pool de longa duração com capacidade 1, criado **uma vez**
quando o `btv dashboard` sobe (não a cada tarefa) — não existe hoje um
caminho por-tarefa pra escolher modelo (nem a tela "Modelo" do frontend, nem
nenhum campo do request chegam até o squad; ver `pendencias.md`). Configure
**antes** de subir o dashboard, via env var:

```sh
docker run --rm --network host -e DEEPSEEK_API_KEY=sk-... -e BTV_SQUAD_MODEL=deepseek-chat \
  -v "$PWD":/work btv:test btv dashboard --port 7878
```

Sem `BTV_SQUAD_MODEL`, o squad do dashboard continua mandando
`claude-sonnet-5` pro provider configurado — que a DeepSeek rejeita (400),
mesmo com a key certa.

Modelos disponíveis na DeepSeek hoje: `deepseek-chat` (V3, uso geral) e
`deepseek-reasoner` (R1, raciocínio — mais lento). Confirme na doc oficial da
DeepSeek o nome exato vigente e a janela de contexto real do modelo escolhido;
`deepseek-chat` sem sufixo classifica como tier **Medium** no BuildToValue
(`model_tier.rs`, compaction a 90% da janela). Se a janela real do seu modelo for
menor que os 200k-tokens padrão do CLI, passe `--context-window` com o valor
correto — senão a compaction dispara tarde demais e uma conversa longa pode
estourar o limite real da API antes do BuildToValue perceber.

**Fallback em cadeia:** se você setar `ANTHROPIC_API_KEY` **e**
`DEEPSEEK_API_KEY` juntas, a ordem é Anthropic → DeepSeek → OpenAI
(`gateway.rs:44-45`) — o BuildToValue tenta Anthropic primeiro e só cai para DeepSeek se
a chamada falhar. Para testar **só** a DeepSeek, não defina `ANTHROPIC_API_KEY`
no ambiente do container.

## Acessando via web (o dashboard de telemetria)

**Antes de tudo: "acessar via web" hoje = o dashboard de telemetria, não o
agente.** Não há como rodar `btv run/chat/squad` pelo navegador — isso só
existe via CLI/TUI. O dashboard mostra telemetria e status de vetting de skills
(dados reais); a maioria das outras telas do frontend (sessão, squad, prompts...)
são vitrines com dado mock, documentadas em `docs/LEVANTAMENTO-UI-DESIGNER.md`.

A imagem já builda a SPA (`web/dist`) e aponta `BTV_WEB_DIR` — falta só a rede:
o `btv dashboard` amarra em `127.0.0.1`, e um `-p 7878:7878` normal **não
alcança** isso (o publish do Docker mapeia pro IP do container, não pro
`127.0.0.1` interno). Use `--network host` pra o loopback do container virar o
loopback da própria VPS:

```sh
# na VPS:
docker run --rm --network host -v "$PWD":/work btv:test btv dashboard --port 7878
# (ou: docker compose -f infra/docker/docker-compose.yml run --rm dashboard)

# na SUA máquina local (não na VPS) — túnel SSH:
ssh -L 7878:127.0.0.1:7878 usuario@ip-da-vps

# abra no navegador LOCAL:
http://127.0.0.1:7878
```

**Nunca** publique a porta direto (`-p 7878:7878` + firewall aberto) — o
dashboard não foi projetado nem testado para acesso público na internet.

## `btv verify` dentro do container

A imagem é construída **inteira em cima de `rust:1-bookworm`** (não builda o
binário numa imagem e roda noutra) — de propósito: `btv verify` roda
`cargo test/clippy/fmt` de verdade contra o que estiver montado em `/work`, e
isso exige o mesmo `cc`/`libc`/linker que compilou o binário. Uma versão
anterior tentava copiar só o toolchain Rust (`/usr/local/cargo`,
`/usr/local/rustup`) de uma imagem `rust:1-bookworm` para um runtime
`debian:bookworm-slim` separado — e isso quebrou com `linking with cc failed`
(o `gcc` reinstalado numa base diferente não bate 100% com o que o Rust daquela
imagem espera). Uma imagem só elimina essa classe de bug por construção; o
custo é uma imagem maior — aceitável para teste, não para produção.

**Primeira execução é mais lenta**: como `/work` é o seu projeto montado (bind
mount), o `target/` que o `cargo test` gera fica no **host**, não só no
container — então a primeira `btv verify` compila do zero (alguns minutos,
dependendo da VPS), mas as próximas reaproveitam esse cache e são bem mais
rápidas, mesmo depois de `--rm` no container.

## As 4 pegadinhas de container (o que muda vs. rodar na máquina)

1. **`BTV_PYTHON_DIR` é obrigatório** — e a imagem já o define
   (`/src/python`). Sem essa env var, o sidecar procura um caminho de
   compile-time inexistente na imagem e o **squad/promptforge degrada em
   silêncio** para agente-único (`crates/btv-cli/src/sidecar.rs:20`). Se você
   montar/rebuild de outro jeito, mantenha `BTV_PYTHON_DIR` apontando para o
   `python/` com `uv sync` já rodado.

2. **A key só no ambiente** (`-e ANTHROPIC_API_KEY=...`), nunca num arquivo
   commitado. A arquitetura garante que a key só existe no processo Rust
   (ADR 0001) — o lado Python nunca a vê.

3. **Sandbox de skills de terceiro = Docker-in-Docker.** O sandbox conecta ao
   daemon local (`bollard::Docker::connect_with_local_defaults()`,
   `crates/btv-tools/src/sandbox.rs:99`). Rodando *dentro* de um container, você
   precisa expor o socket do host: `-v /var/run/docker.sock:/var/run/docker.sock`.
   Sem isso, skills de terceiro **fail-close** (recusam rodar — é o design, não
   um bug). **Caveat de path:** com o socket montado, os contêineres do sandbox
   sobem no daemon do *host*, mas os caminhos que o btv passa (o mount da skill)
   são internos ao *container* — então o sandbox de terceiro via DinD é a parte
   mais frágil; deixe-a por último no seu teste.

4. **Dashboard em `127.0.0.1` por padrão = loopback do container.** Para uso
   pessoal, ver "Acessando via web" acima (`--network host` + túnel SSH). Para
   hospedar atrás de um ingress, use `--host 0.0.0.0` + `BTV_TRUSTED_ORIGINS`
   (seção abaixo) — **nunca** publicar a porta direto na internet.

## Hospedagem atrás de um ingress (opt-in, COM autenticação)

O caminho existe, mas é **opt-in e default-seguro** — sem os dois passos abaixo,
o comportamento é idêntico ao local-first (loopback + só-localhost):

1. **Bind:** `btv dashboard --host 0.0.0.0` amarra em todas as interfaces *do
   container*. Rode-o **na rede bridge `btv-prod-net`, SEM `ports:` publicado**
   — assim só o gateway nginx (mesma rede) alcança por nome de container; a
   porta nunca aparece no host/internet.
2. **Guarda de Origin (ADR 0015):** por padrão toda requisição mutável de uma
   origin ≠ localhost recebe 403. `BTV_TRUSTED_ORIGINS=squad.exemplo.cloud`
   (lista separada por vírgula) libera a(s) origin(s) do seu domínio. Isso
   **afrouxa a proteção de CSRF** — por isso só é aceitável **com autenticação
   na borda** (basic auth no nginx; o gateway já monta um `.htpasswd`).

> O dashboard **executa código e usa API keys**. Sem basic auth no ingress, não
> suba: qualquer um na internet teria um shell de agente no seu servidor.

Pronto para usar: `infra/docker/docker-compose.prod.yml` (bridge, sem porta
publicada, `BTV_TRUSTED_ORIGINS` por env) + um `server {}` no nginx do ingress
com `auth_basic` apontando para `proxy_pass http://btv-squad-dashboard:7878`.

## Nota

Esta imagem foi escrita a partir de passos **verificados individualmente fora do
Docker** (`cargo build --release -p btv-cli`, `uv sync`, e `pnpm install &&
pnpm build` de `web/` **e** de `btv-web/` — este com `git submodule update
--init` do `vendor/bpmn` — rodam limpos no repo), mas o **Dockerfile em si não
foi buildado num daemon Docker** durante a autoria (o ambiente de dev não tinha
um). Builde-o na sua VPS **após** `git submodule update --init --recursive` (o
estágio `btvweb-build` precisa do `vendor/bpmn` no contexto); se algo faltar no
runtime slim, o ajuste típico é uma lib de sistema a mais no `apt-get install`.
