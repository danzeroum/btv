# Dicionário de Dados — `crates/btv-tools` e `crates/btv-verify`

Mapa exaustivo de fluxo de dados (entrada / saída / intermediário / estado / config / wire) dos dois crates Rust de ferramentas determinísticas e do pipeline de verificação. Cada seção cobre um arquivo-fonte, com o papel em 1 linha e a tabela de dados, encerrada por uma linha `Fluxo:`.

Taxonomia de Direção:
- **entrada** = parâmetro de função / leitura de disco-rede-arg
- **saída** = retorno / escrita / evento emitido
- **intermediário** = local / buffer (mesmo que descartado)
- **estado** = campo de struct/enum
- **config** = const / env var / TOML
- **wire** = tipo proto / JSON / DB / serde na fronteira

Contrato central compartilhado (`btv_domain::tool`, re-exportado por `btv-tools`): trait `Tool` com `name()/description()/input_schema()/scope(args)/run(args)`; `ToolOutput { content, truncated, overflow_path, diff }`; `ToolError::{InvalidArgs, Execution}`; `DiffLine::{Context, Removed, Added}`.

---

## crates/btv-domain/src/tool.rs

Papel: define o contrato `Tool` (D1t) e seus tipos de dado — mora no domínio; as implementações ficam em `btv-tools`.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
| --- | --- | --- | --- | --- |
| `ToolError::InvalidArgs(String)` | enum variant (thiserror) | wire/saída | tool `run` → loop de agente | mensagem "argumentos inválidos: {0}" |
| `ToolError::Execution(String)` | enum variant (thiserror) | wire/saída | tool `run` → loop | mensagem "falha de execução: {0}" |
| `DiffLine::Context/Removed/Added(String)` | enum serde (tuple-variant) | wire | `edit`/`diff` → TUI/web | representação serde default preservada byte-idêntica ao histórico de `btv-tools::diff` |
| `ToolOutput.content` | `String` | estado/saída | tool → loop → modelo | texto inline devolvido ao contexto |
| `ToolOutput.truncated` | `bool` | estado/saída | tool → loop | true quando output excedeu o limite |
| `ToolOutput.overflow_path` | `Option<String>` | estado/saída/wire | `bound_output_managed` → loop | caminho relativo ao workspace do Managed Tool Output File |
| `ToolOutput.diff` | `Option<Vec<DiffLine>>` | estado/saída | `edit` → TUI/web | diff estruturado quando alterou arquivo texto |
| `Tool::name/description` | `&str` | saída | impl → registry/modelo | `&str` (lifetime de `&self`) para permitir identidade dinâmica (SkillTool com `String`) |
| `Tool::input_schema` | `serde_json::Value` | saída/wire | impl → modelo | JSON Schema dos args |
| `Tool::scope(args)` | `String` | saída | impl → motor de permissões | caminho/comando avaliado |
| `Tool::run(args)` | `Result<ToolOutput, ToolError>` | saída | impl → loop | execução com args JSON |

Fluxo: `args JSON → Tool::{scope→permissão, run} → ToolOutput|ToolError` — os tipos moram no domínio, o cálculo vive em `btv-tools`.

---

## crates/btv-tools/src/lib.rs

Papel: raiz do crate — re-exports do contrato, helpers de truncamento/persistência de overflow e utilitários de args.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
| --- | --- | --- | --- | --- |
| `DEFAULT_OUTPUT_LIMIT` | `const usize = 32*1024` | config | usado por read/grep/bash/skill/mcp/lsp | limite de bytes devolvidos inline |
| `bound_output(content, limit)` param `content` | `String` | entrada | chamador → helper | texto bruto a truncar |
| `bound_output` `limit` | `usize` | entrada/config | chamador → helper | fronteira de bytes |
| `bound_output` local `cut` | `usize` | intermediário | loop de fronteira UTF-8 | recua até `is_char_boundary` |
| `bound_output` retorno | `ToolOutput` | saída | helper → tool | `overflow_path: None` sempre (não persiste) |
| `bound_output_managed` `root` | `&Path` | entrada | tool → helper | raiz do workspace |
| `bound_output_managed` local `rel_dir` | `PathBuf` = `.btv/tool-outputs` | intermediário | helper | dir relativo do overflow |
| `bound_output_managed` local `nanos` | `u128` (SystemTime→UNIX_EPOCH) | intermediário | relógio → filename | `unwrap_or(0)` em falha de relógio |
| `bound_output_managed` local `filename` | `String` `{nanos:024x}.txt` | intermediário/wire | helper → disco | nome único do arquivo de overflow |
| escrita de overflow | arquivo `<root>/.btv/tool-outputs/<nanos>.txt` | saída/wire | helper → disco | conteúdo COMPLETO persistido |
| `bound_output_managed` retorno `overflow_path` | `Option<String>` (barra normalizada `\`→`/`) | saída/wire | helper → ToolOutput | caminho relativo do arquivo |
| erro de dir/write | `ToolError::Execution("overflow dir/write: {e}")` | saída | fs → helper | |
| `required_str(args, field)` | `Result<&str, ToolError>` | entrada→saída | tool → helper | extrai campo string obrigatório; erro `InvalidArgs("campo '{field}' obrigatório")` |

Fluxo: `content grande → bound_output_managed → trunca inline (fronteira UTF-8) + grava restante em .btv/tool-outputs/<nanos>.txt → overflow_path`.

---

## crates/btv-tools/src/read.rs

Papel: ferramenta `read` — lê arquivo texto do workspace com números de linha, com offset/limit.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
| --- | --- | --- | --- | --- |
| `ReadTool.root` | `PathBuf` | estado/config | registry → tool | raiz do workspace |
| `DEFAULT_LINE_LIMIT` | `const usize = 2000` | config | `run` | máximo de linhas default |
| input_schema | JSON `{path*, offset, limit}` | saída/wire | tool → modelo | `path` obrigatório |
| `scope(args)` | `String` = `args["path"]` | saída | tool → permissão | vazio se ausente |
| arg `path` | `&str` (required) | entrada/wire | modelo → run | caminho relativo |
| local `full` | `PathBuf` = `root.join(path)` | intermediário | run | caminho absoluto |
| `content` | `String` | intermediário | `fs::read_to_string` | erro `Execution("{full}: {e}")` se ausente |
| arg `offset` | `u64→usize` (`unwrap_or(1).max(1)`) | entrada | modelo → run | linha inicial 1-based |
| arg `limit` | `u64→usize` (`unwrap_or(2000)`) | entrada | modelo → run | máximo de linhas |
| `out` | `String` numerada `"{i+1}\t{line}\n"` | intermediário | lines→enumerate→skip→take→map | numeração 1-based após offset |
| retorno | `ToolOutput` via `bound_output_managed` | saída | run → loop | truncado se > 32 KiB |

Fluxo: `path/offset/limit → fs::read_to_string → lines enumeradas 1-based (skip offset-1, take limit) → bound_output_managed`.

---

## crates/btv-tools/src/grep.rs

Papel: ferramenta `grep` — busca regex nos arquivos do workspace via libs do ripgrep (`grep`+`ignore`), respeitando .gitignore.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
| --- | --- | --- | --- | --- |
| `GrepTool.root` | `PathBuf` | estado/config | registry → tool | raiz do workspace |
| `MAX_MATCHES` | `const usize = 200` | config | `run` | cap de ocorrências |
| input_schema | JSON `{pattern*, path}` | saída/wire | tool → modelo | descrição diz coluna 1-based (subtrair 1 p/ tools `lsp__*`) |
| `scope(args)` | `String` = `args["path"]` (`unwrap_or(".")` | saída | tool → permissão | |
| arg `pattern` | `&str` (required) | entrada/wire | modelo → run | expressão regular |
| `matcher` | `RegexMatcher` | intermediário | `RegexMatcher::new` | erro `InvalidArgs("regex: {e}")` se inválida |
| local `base` | `PathBuf` | intermediário | `root.join(path)` ou `root` | raiz da busca |
| `matches` | `Vec<String>` | intermediário/saída | sink UTF8 → run | linhas `rel:line:column:conteúdo` |
| `walker` | `ignore::WalkBuilder` (`hidden(true)`, `require_git(false)`) | intermediário | run | respeita .gitignore mesmo fora de repo git |
| local `rel` | `PathBuf` = `path.strip_prefix(root)` | intermediário | por entrada | caminho relativo exibido |
| `column` | `usize` = `matcher.find().start()+1` (`unwrap_or(1)`) | intermediário | por linha | coluna 1-based do 1º match (offset de byte, convenção `rg --column`) |
| flag `full` | `bool` | intermediário/estado | por arquivo | true ao atingir MAX_MATCHES → `Ok(false)` para o arquivo |
| marcador de limite | `String` "... (limite de 200 ocorrências atingido)" | intermediário | run | `break 'outer` |
| output vazio | `ToolOutput{content:"nenhuma ocorrência"}` | saída | run → loop | truncated:false |
| retorno | `ToolOutput` via `bound_output_managed` (matches.join("\n")) | saída | run → loop | erros por arquivo (binário/não-UTF8) são pulados, não abortam |

Fluxo: `pattern → RegexMatcher + WalkBuilder(.gitignore) → Searcher UTF8 por arquivo → matches "rel:line:col:conteúdo" (cap 200) → bound_output_managed`.

---

## crates/btv-tools/src/edit.rs

Papel: ferramenta `edit` — substituição exata e única (ou `replace_all`) de trecho num arquivo, com diff estruturado.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
| --- | --- | --- | --- | --- |
| `EditTool.root` | `PathBuf` | estado/config | registry → tool | raiz do workspace |
| input_schema | JSON `{path*, old_string*, new_string*, replace_all=false}` | saída/wire | tool → modelo | 3 campos obrigatórios |
| `scope(args)` | `String` = `args["path"]` | saída | tool → permissão | |
| arg `path/old/new` | `&str` (required) | entrada/wire | modelo → run | trecho exato |
| guarda `old==new` | erro `InvalidArgs("old_string e new_string são iguais")` | saída | run | |
| arg `replace_all` | `bool` (`unwrap_or(false)`) | entrada | modelo → run | |
| `content` | `String` | intermediário | `fs::read_to_string(full)` | erro `Execution` se ausente |
| `occurrences` | `usize` = `content.matches(old).count()` | intermediário | run | decide o ramo |
| ramo 0 | erro `Execution("old_string não encontrada em {path}")` | saída | run | |
| ramo 1 | `content.replacen(old,new,1)` → escrita | saída/wire | run → disco | grava `updated` |
| ramo n+replace_all | `content.replace(old,new)` → escrita | saída/wire | run → disco | mensagem "({n} ocorrências)" |
| ramo n (sem replace_all) | erro `Execution("old_string aparece {n} vezes... forneça trecho único ou replace_all")` | saída | run | edit ambíguo rejeitado |
| `updated` | `String` | intermediário | run → disco/diff | novo conteúdo |
| `diff` | `Vec<DiffLine>` = `line_diff(content, updated)` | saída | run → ToolOutput.diff | diff estruturado |
| retorno `content` | `String` "editado: {path}\n{format_diff}" | saída | run → loop | truncated:false, overflow_path:None |

Fluxo: `path/old_string/new_string → fs::read → count(old) (0=erro, 1=replacen, n+all=replace, n=ambíguo) → fs::write(updated) → line_diff → ToolOutput{content,diff}`.

---

## crates/btv-tools/src/diff.rs

Papel: cálculo de diff de linhas (prefixo-comum / sufixo-comum) entre antes/depois de um `edit`, com contexto limitado; formatação unificada.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
| --- | --- | --- | --- | --- |
| `CONTEXT_LINES` | `const usize = 2` | config | `line_diff` | linhas de contexto ao redor da região alterada |
| `line_diff` `before/after` | `&str` | entrada | edit → diff | conteúdo antes/depois |
| `before_lines/after_lines` | `Vec<&str>` | intermediário | `.lines().collect()` | |
| `prefix` | `usize` | intermediário | loop maior-prefixo-comum | |
| `suffix` | `usize` | intermediário | loop maior-sufixo-comum | |
| `removed/added` | `&[&str]` (slices) | intermediário | fatias entre prefix e suffix | vazios ⇒ `Vec::new()` |
| `ctx_before_start / after_region_end / ctx_after_end` | `usize` | intermediário | janela de contexto | `saturating_sub`/`.min()` |
| marcadores de elipse | `DiffLine::Context("… (N linhas antes/depois)")` | saída | diff → texto | quando há linhas fora da janela |
| `line_diff` retorno | `Vec<DiffLine>` | saída | diff → edit/TUI | Context/Removed/Added |
| `format_diff(lines)` retorno | `String` | saída | diff → modelo | prefixos `  `/`- `/`+ ` |

Fluxo: `before,after → prefixo/sufixo comum → região removida+adicionada + contexto (±2 linhas, elipses) → Vec<DiffLine> → format_diff (unificado)`. Observação: exato para edições localizadas; com `replace_all` distante o "meio" pode incluir trechos inalterados (diff informativo, não patch aplicável).

---

## crates/btv-tools/src/bash.rs

Papel: ferramenta `bash` — executa `sh -c <command>` na raiz do workspace com timeout e drenagem concorrente de pipes.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
| --- | --- | --- | --- | --- |
| `BashTool.root` | `PathBuf` | estado/config | registry → tool | cwd do subprocesso |
| `DEFAULT_TIMEOUT_MS` | `const u64 = 120_000` | config | `run` | timeout padrão 120s |
| `MAX_TIMEOUT_MS` | `const u64 = 600_000` | config | `run` | teto do timeout |
| `drena(fonte)` | `Option<JoinHandle<String>>` | intermediário/saída | pipe → thread | lê stdout/stderr em thread dedicada até EOF; evita bloqueio quando saída > ~64KB do buffer do pipe |
| input_schema | JSON `{command*, timeout_ms}` | saída/wire | tool → modelo | |
| `scope(args)` | `String` = `args["command"]` | saída | tool → permissão | |
| arg `command` | `&str` (required) | entrada/wire | modelo → run | |
| arg `timeout_ms` | `u64` (`unwrap_or(120000).min(600000)`) | entrada/config | modelo → run | vira `Duration` |
| `child` | `std::process::Child` (`sh -c`, stdin null, stdout/stderr piped) | intermediário/estado | `Command::spawn` | erro `Execution("spawn: {e}")` |
| `out_thread/err_thread` | `Option<JoinHandle<String>>` | intermediário | drena(stdout/stderr) | concorrente com o wait |
| loop de espera | `try_wait` + `sleep(25ms)` | intermediário | run | mata (`kill`+`wait`) no timeout → erro `Execution("timeout de {ms}ms excedido")` |
| `status` | `ExitStatus` | intermediário | try_wait | |
| `output` | `String` | intermediário/saída | join(out)+join(err) | stdout depois stderr, na ordem |
| sufixo de falha | `"\n[exit code: {code}]"` (`code().unwrap_or(-1)`) | saída | run → output | apenas se `!status.success()` |
| retorno | `ToolOutput` via `bound_output_managed` | saída | run → loop | trunca+persiste se > 32 KiB |

Fluxo: `command/timeout_ms → sh -c em cwd=root, pipes drenados por threads concorrentes → try_wait loop (kill no timeout) → stdout+stderr+[exit code:N] → bound_output_managed`.

---

## crates/btv-tools/src/skill.rs

Papel: `SkillTool` — expõe uma skill (dir + entrypoint do manifest) como `dyn Tool`; built-in roda direto (grupo de processos, kill de grupo no timeout), terceiro roda confinado no Sandbox (fail-closed).

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
| --- | --- | --- | --- | --- |
| `DEFAULT_SKILL_TIMEOUT_MS` | `const u64 = 30_000` | config | `new` | timeout padrão de skill |
| `SkillTool.name/description` | `String` | estado | manifest → tool | identidade dinâmica (por isso `Tool` devolve `&str`) |
| `SkillTool.entrypoint` | `String` | estado/config | `skill.toml` → tool | corpo shell rodado via `sh -c`, `input` como `$1` |
| `SkillTool.dir` | `PathBuf` | estado/config | loader → tool | cwd do subprocesso |
| `SkillTool.timeout` | `Duration` | estado/config | `with_timeout` | |
| `SkillTool.sandboxed` | `bool` | estado | `.sandboxed()` | terceiro (untrusted) → sandbox; built-in → direto |
| input_schema | JSON `{input*: string}` | saída/wire | tool → modelo | schema genérico (Onda 1) |
| `scope(args)` | `String` "skill:{name} {preview}" | saída | tool → permissão | preview = 60 chars de `input` |
| arg `input` | `&str` (`unwrap_or("")`) | entrada/wire | modelo → run | passado como `$1` |
| `run_entrypoint` `child` | `Child` (`sh -c <entrypoint> btv-skill <input>`, `process_group(0)`) | intermediário | spawn | `$0`=btv-skill, `$1`=input |
| `pid` | `u32` | intermediário | child.id | alvo do kill de grupo |
| loop de espera | `try_wait` + `sleep(20ms)` | intermediário | run | timeout → `kill_process_group(pid)` → erro "skill excedeu o timeout de {ms}ms" |
| `out` | `String` | intermediário/saída | stdout+stderr | ordem stdout depois stderr |
| sufixo de falha | `"\n[skill exit code: {code}]"` | saída | run | se `!success()` |
| `kill_process_group(pid)` | `libc::kill(-pid, SIGKILL)` (unix) | saída/efeito | timeout → SO | mata grupo inteiro (netos inclusive) |
| `run_in_sandbox` `cmd` | `Vec<String>` `["sh","-c",entrypoint,"btv-skill",input]` | intermediário/wire | skill → Sandbox | mesmo contrato, cwd `/work` |
| sandbox `SandboxOutput` | `{stdout, exit_code, timed_out}` | entrada | Sandbox → skill | `timed_out`→"[skill: timeout no sandbox]"; exit≠0→"[skill exit code:N]" |
| `SandboxError::DaemonUnavailable` | erro | saída | Sandbox → run | fail-closed: "sandbox Docker indisponível — skill de terceiro não roda" |
| `run_sandbox_blocking` | ponte sync→async (thread + runtime `current_thread`) | intermediário | Tool::run sync → Sandbox async | `block_on` em thread dedicada (não aninha em runtime do loop) |
| retorno | `ToolOutput` via `bound_output` | saída | run → loop | (não persiste overflow) |

Fluxo: `input → (built-in) sh -c em grupo próprio, kill de grupo no timeout | (sandboxed) Sandbox.run em thread+runtime dedicados, fail-closed sem daemon → stdout+[exit/timeout] → bound_output`.

---

## crates/btv-tools/src/sandbox.rs

Papel: sandbox Docker real (via `bollard`) — roda cmd+env em contêiner confinado (rootfs read-only, único mount `/work`, rede off, limites mem/cpu/tempo, `cap-drop ALL`, `no-new-privileges`); fail-closed sem daemon.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
| --- | --- | --- | --- | --- |
| `Sandbox.image` | `String` (default `python:3.11-slim`) | estado/config | `new`/`with_image` | imagem base |
| `Sandbox.mount` | `PathBuf` | estado/config | `new` | dir do host montado gravável em `/work` |
| `Sandbox.network_disabled` | `bool` (default true) | estado/config | `new`/`with_network` | |
| `Sandbox.mem_limit_mb` | `u64` (default 512) | estado/config | `new`/`with_mem_limit_mb` | |
| `Sandbox.cpu_quota` | `f64` (default 0.5) | estado/config | `new` | |
| `Sandbox.timeout` | `Duration` (default 30s) | estado/config | `new`/`with_timeout` | timeout do CONTÊINER (bollard) |
| `SandboxOutput.stdout` | `String` | saída/wire | contêiner → chamador | stdout+stderr colhidos via `logs` |
| `SandboxOutput.exit_code` | `i64` | saída/wire | `wait_container` | exit != 0 vem como erro bollard com código dentro; timeout→-1 |
| `SandboxOutput.timed_out` | `bool` | saída/wire | timeout do wait | |
| `SandboxError::DaemonUnavailable(String)` | erro | saída | daemon → chamador | fail-closed — a Onda 3 sabe que NÃO rodou |
| `SandboxError::Execution(String)` | erro | saída | create/start/wait/pull → chamador | |
| `ping()` / `ping_with(docker)` | `async → bool` | saída | daemon → chamador | reachability sem criar contêiner; qualquer erro → false, nunca panic |
| `run(cmd, env)` | entrada `cmd:&[String]`, `env:&[(String,String)]` | entrada/wire | chamador → daemon | conecta `connect_with_local_defaults` → `run_with` |
| `ensure_image` `from_image/tag` | `(String, String)` | intermediário | `rsplit_once(':')` | pull só se `inspect_image` falhar; tag default "latest" |
| `env_vec` | `Vec<String>` `"{k}={v}"` | intermediário/wire | env → HostConfig | |
| `HostConfig.memory` | `i64` = `mem_limit_mb*1024*1024` | intermediário/wire | run_with → Docker | |
| `HostConfig.nano_cpus` | `i64` = `cpu_quota*1e9` | intermediário/wire | run_with → Docker | |
| `HostConfig.network_mode` | `"none"`\|`"bridge"` | intermediário/wire | run_with → Docker | conforme `network_disabled` |
| `HostConfig.binds` | `["{mount}:/work"]` | intermediário/wire | run_with → Docker | único ponto gravável |
| `HostConfig.cap_drop/security_opt/readonly_rootfs` | `["ALL"]`/`["no-new-privileges"]`/`true` | intermediário/wire | run_with → Docker | superfície de contenção |
| `Config.user` | `Option<String>` = `mount_user(mount)` `uid:gid` | intermediário/wire | metadata do mount → Docker | não-root; escreve em /work sem CAP_DAC_OVERRIDE |
| `created.id` | `String` | intermediário/estado | `create_container` | id do contêiner |
| `exit_code` (wait) | `i64` via `tokio::time::timeout(self.timeout, wait_container)` | intermediário | wait | timeout → `kill_container` + `timed_out=true` |
| `stdout` (logs) | `String` (`from_utf8_lossy`) | intermediário/saída | `logs(stdout+stderr, tail=all)` | colhido antes de remover |
| `remove_quiet(docker, id)` | efeito (`remove_container force`) | saída/efeito | run_with → Docker | limpeza sempre |

Fluxo: `cmd+env → connect+ping (fail-closed) → ensure_image → create_container (HostConfig: mem/cpu/rede/binds/cap-drop/ro-rootfs, user uid:gid) → start → wait com timeout (kill se estourar) → logs → SandboxOutput{stdout,exit_code,timed_out} → remove`.

---

## crates/btv-tools/src/mcp.rs

Papel: cliente MCP (via `rmcp`, transporte child-process/stdio) — lê `.btv/mcp.toml`, conecta servidores externos (sessão persistente por servidor, comandos por mpsc, timeouts) e expõe as tools como `dyn Tool` namespaced (`mcp__<server>__<tool>`).

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
| --- | --- | --- | --- | --- |
| `CONNECT_TIMEOUT` | `const Duration = 10s` | config | `ensure_client` | prazo de spawn+handshake |
| `OP_TIMEOUT` | `const Duration = 30s` | config | list/call | prazo de uma operação |
| `McpClient` | alias `rmcp::RunningService<RoleClient,()>` | estado | rmcp | cliente conectado |
| **`.btv/mcp.toml`** | arquivo TOML (`[[server]] id/command/args`) | entrada/config/wire | disco → `read_server_configs` | ausente/inválido → `Vec` vazio (fail-soft); erro impresso em stderr |
| `McpServerConfig.id/command/args` | `String`/`String`/`Vec<String>` (serde Serialize) | estado/config/wire | toml → registry-builder/console | servidor declarado |
| `McpToolMeta.name/description/input_schema` | `String`/`String`/`Value` (Serialize) | wire/estado | servidor MCP → registry | metadados anunciados |
| `McpCommand::ListTools{reply}` | enum + `mpsc::Sender<Result<Vec<McpToolMeta>,String>>` | wire/intermediário | McpSession → session_loop | comando de listagem |
| `McpCommand::Call{tool,args,reply}` | enum + `mpsc::Sender<Result<String,String>>` | wire/intermediário | McpTool.run → session_loop | comando de chamada |
| `McpSession.tx` | `mpsc::Sender<McpCommand>` | estado | tool → thread | canal de comandos |
| `McpSession._thread` | `JoinHandle<()>` | estado | connect | thread encerra ao cair a última `Arc` (tx fecha → `recv` Err) |
| `session_loop` `rt` | `tokio Runtime current_thread` | intermediário | thread própria | não aninha `block_on` no worker do loop |
| `session_loop` `client` | `Option<McpClient>` | estado/intermediário | conexão preguiçosa reusada | timeout → `client=None` (reconecta no próximo cmd) |
| list_all_tools result | `Vec<McpToolMeta>` | saída/wire | rmcp → reply | `input_schema` = `Value::Object((*t.input_schema).clone())` |
| sentinela `"__timeout__"` | `String` | intermediário | timeout interno | vira msg "timeout (30s) em {what} MCP" via `timeout_msg` |
| `ensure_client` | `Result<&McpClient,String>` | intermediário | conexão bounded | timeout de conexão → erro; falha deixa `client=None` |
| `reply_error(cmd, msg)` | efeito | saída | runtime ausente → reply | responde Err a cada comando |
| `McpTool.full_name` | `String` `mcp__{id}__{name}` | estado/wire | register → registry | namespaced (guarda de colisão) |
| `McpTool.{description,input_schema}` | `String`/`Value` | estado | meta → tool | |
| `McpTool.session` | `Arc<McpSession>` | estado | compartilhada por servidor | uma conexão reusada |
| `McpTool.{server_id,tool}` | `String`/`String` | estado | config/meta | usados no scope e na call |
| `scope(args)` | `String` "mcp:{server_id}/{tool} {preview}" | saída | tool → permissão | preview = 60 chars de `args.to_string()` |
| `run(args)` | `ToolOutput` via `bound_output` | saída | session.call → loop | erro → `ToolError::Execution` |
| `list_tools_blocking(config)` | `Result<Vec<McpToolMeta>,String>` | saída | console MCP | sessão efêmera (Arc única cai → thread encerra) |
| `connect(config)` | `async Result<McpClient,String>` | intermediário | `TokioChildProcess::new(cmd)` + `().serve` | erro "spawn MCP"/"handshake MCP" |
| `render_content(result)` | `String` | intermediário/saída | CallToolResult → texto | serializa cada bloco, puxa campo `text`, ignora não-texto, `join("\n")` |
| `register_mcp_server(registry, config)` | `Result<usize,String>` | saída | loader → registry | registra `McpTool` por tool; colisão pulada (stderr); `0` ⇒ Arc cai e conexão encerra |

Fluxo: `.btv/mcp.toml → McpServerConfig → McpSession (thread + runtime + conexão preguiçosa) ← McpCommand via mpsc (bounded por CONNECT/OP_TIMEOUT) → rmcp list/call → McpToolMeta/render_content → McpTool namespaced no registry`.

---

## crates/btv-tools/src/lsp.rs

Papel: cliente LSP hand-rolled (JSON-RPC com framing `Content-Length` sobre stdio, zero-dep além de serde_json) — lê `.btv/lsp.toml`, sobe o language server (sessão preguiçosa, reader de fundo com `Condvar`, diagnostics stash) e expõe definição/referências/diagnósticos/símbolo como `dyn Tool`.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
| --- | --- | --- | --- | --- |
| `READY_TIMEOUT` | `const Duration = 60s` | config | `retry_until_ready` | orçamento de indexação |
| `DIAG_BUDGET` | `const Duration = 12s` | config | `diagnostics` | espera diagnósticos assentarem |
| `REQUEST_TIMEOUT` | `const Duration = 30s` | config | `request` | prazo de um round-trip |
| `LSP_POLL_INTERVAL` | `const Duration = 300ms` | config | retry | intervalo de polling |
| **`.btv/lsp.toml`** | TOML (`[[server]] id/command/args`) | entrada/config/wire | disco → `read_server_configs` | ausente/inválido → vazio (fail-soft) |
| `LspServerConfig.id/command/args/root` | `String`/`String`/`Vec<String>`/`PathBuf` (Serialize) | estado/config/wire | toml → registry/console | `root` = raiz do workspace |
| `LspQuery::{Definition,References,Diagnostics,Symbol}` | enum Copy | estado | fixas (LSP tem conjunto conhecido) | `as_str()` → "definition"/... |
| `Shared.inner` | `Mutex<SharedInner>` | estado | reader ↔ consulta | protegido |
| `Shared.cv` | `Condvar` | estado/sincronização | reader → consulta | sinaliza resposta nova ou server morto |
| `Shared.diagnostics` | `Mutex<HashMap<String,Value>>` | estado/wire | reader → diagnostics | stash por URI (`publishDiagnostics`) |
| `SharedInner.responses` | `HashMap<i64, Result<Value,String>>` | estado | reader → request | respostas por id |
| `SharedInner.dead` | `Option<String>` | estado | reader → consultas | EOF/erro do server; desperta consultas presas |
| `LspProc.child` | `Child` (stdin/stdout piped, stderr null) | estado | spawn | morto no `Drop` (kill+wait, sem shutdown handshake) |
| `LspProc.stdin` | `Arc<Mutex<ChildStdin>>` | estado | compartilhado c/ reader | reader responde requests do server no handshake |
| `LspProc.next_id` | `i64` | estado | `request` | id incremental do JSON-RPC |
| `LspProc.opened` | `HashSet<String>` | estado | ensure_open | `didOpen` uma vez por URI (reabrir viola protocolo) |
| `LspProc.reader` | `Option<JoinHandle<()>>` | estado | ensure_started | join no Drop (nada de thread pendurada) |
| `reader_loop` roteamento | resposta→`responses`, request-do-server→`respond_server_request`, notificação→stash/descarta | intermediário/wire | stdout do server → Shared | drenagem contínua (nada entope o pipe) |
| `write_msg(w, v)` | `Content-Length: {len}\r\n\r\n{body}` | saída/wire | cliente → server | framing LSP |
| `read_msg(r)` | `Result<Value,String>` | entrada/wire | server → cliente | parseia header Content-Length + body; EOF → erro |
| `respond_server_request` | `Value::Null` ou `Array[Null;n]` p/ `workspace/configuration` | saída/wire | reader → server | evita travar handshake |
| `uri_for(file)` | `String` `file://{abs canonicalizado}` | intermediário/wire | consulta | bate com o que o server devolve |
| `read_file(file)` | `String` | intermediário | disco → didOpen | texto do documento |
| `ensure_started` `root_uri` | `String` `file://{root}` | intermediário/wire | spawn + `initialize`/`initialized` | handshake |
| `ensure_open` didOpen params | `{uri, languageId, version:1, text}` | saída/wire | consulta → server | `language_id` por extensão (.rs→rust, .py→python, senão plaintext) |
| `definition/references` params | `{textDocument.uri, position{line,character}}` (0-indexed) | entrada/wire | tool → server | references adiciona `context.includeDeclaration:true` |
| `symbol(name)` params | `{query: name}` (`workspace/symbol`) | entrada/wire | tool → server | acha por NOME, resolve posição |
| `request(method, params, timeout)` | `Result<Value,String>` | saída | write_msg + espera no `cv.wait_timeout` | timeout → "timeout LSP em '{method}'"; server morto → erro |
| `retry_until_ready(request)` | `Result<Value,String>` | intermediário | position/symbol | re-tenta enquanto resultado vazio ou erro retryable até READY_TIMEOUT |
| `is_retryable_lsp_error(e)` | `bool` | intermediário | erro → retry | contém "-32801" (ContentModified) ou "-32802" (ServerCancelled) |
| `diagnostics(file)` loop | polling do stash até assentar/`DIAG_BUDGET` | intermediário/saída | Shared.diagnostics → tool | `dead`→erro; URI vazia dá 3s de graça; senão `json!([])` |
| `LspTool.full_name/kind/server_id/session` | `String`/`LspQuery`/`String`/`Arc<LspSession>` | estado | register | 4 tools compartilham a sessão |
| `input_schema` | JSON por kind | saída/wire | tool → modelo | Diagnostics `{file}`, Symbol `{name}`, outros `{file,line,character}` |
| `scope(args)` | `String` "lsp:{server_id}/{kind} {file}" | saída | tool → permissão | |
| `run(args)` | `ToolOutput` via `bound_output` | saída | session → loop | Symbol tratado antes de exigir `file`; erros `InvalidArgs` por campo |
| `render_locations(v)` | `String` linhas `caminho:line:char` (0-indexed) | saída | Location/LocationLink → texto | robusto ao shape (uri/targetUri, range/targetSelectionRange/targetRange) |
| `render_symbols(v)` | `String` linhas `nome:caminho:linha:coluna` | saída | SymbolInformation/WorkspaceSymbol → texto | |
| `render_diagnostics(file, v)` | `String` linhas `file:line:ch: sev: msg` | saída | diagnostics → texto | severity 1=error/2=warning/3=info/4=hint |
| `uri_to_path(uri)` | `String` | intermediário | `strip_prefix("file://")` | exibição |
| `register_lsp_server(registry, config)` | `usize` | saída | loader → registry | registra 4 tools (`lsp__<id>__<kind>`); NÃO sobe o server (preguiçoso); colisão pulada |

Fluxo: `.btv/lsp.toml → LspServerConfig → LspSession preguiçosa; primeiro uso sobe Child + reader_loop (drena stdout, roteia por Condvar) + handshake initialize/initialized → position/symbol/diagnostics queries (framing Content-Length, retry na indexação, diagnostics via stash) → render_* → bound_output; Child morto no Drop`.

---

## crates/btv-tools/src/registry.rs

Papel: `ToolRegistry` — conjunto de ferramentas anunciado ao modelo; conjunto padrão (read/grep/edit/bash) + `register` para skills/MCP/LSP; implementa `ToolsPort`.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
| --- | --- | --- | --- | --- |
| `ToolRegistry.tools` | `Vec<Box<dyn Tool>>` | estado | default_set/register | conjunto vivo |
| `default_set(root)` | `Self` | saída | init → loop | ReadTool/GrepTool/EditTool/BashTool com `root` clonado |
| `register(tool)` | efeito (`push`) | entrada/saída | skill/MCP/LSP loader → registry | ponto de extensão da Fase 6 |
| `get(name)` | `Option<&dyn Tool>` | saída | loop → tool | resolução por nome |
| `iter()` | `impl Iterator<Item=&dyn Tool>` | saída | registry → specs | |
| `specs()` (ToolsPort) | `Vec<ToolSpec{name,description,input_schema}>` | saída/wire | registry → modelo | anúncio ao modelo (projeção de `iter()`) |
| `ToolsPort::get(name)` | `Option<&dyn Tool>` | saída | loop → registry | delega a `ToolRegistry::get` |

Fluxo: `root → default_set (4 tools) + register (N skills/MCP/LSP) → specs() anuncia ao modelo, get(name) resolve para execução`.

---

## crates/btv-tools/src/bin/btv_mcp_fixture.rs

Papel: servidor MCP fixture (via `rmcp`) — expõe uma tool `echo` via stdio para o teste cross-process do cliente MCP.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
| --- | --- | --- | --- | --- |
| `Fixture` | struct (`ServerHandler`) | estado | servidor | sem estado próprio |
| `get_info` | `ServerInfo` (capabilities: tools) | saída/wire | servidor → cliente | |
| `list_tools` retorno | `ListToolsResult{tools:[echo], ...}` | saída/wire | servidor → cliente | schema `{input:string}` |
| `call_tool` arg `input` | `&str` (de `req.arguments["input"]`, `unwrap_or("")`) | entrada/wire | cliente → servidor | |
| `call_tool` retorno | `CallToolResult::success([text "ECHO:{input}"])` | saída/wire | servidor → cliente | |
| `main` | serve `(stdin, stdout)` até `waiting()` | efeito | processo | transporte stdio |

Fluxo: `stdin JSON-RPC → list_tools(echo) / call_tool(input) → "ECHO:{input}" → stdout`.

---

## crates/btv-tools/src/bin/btv_lsp_fixture.rs

Papel: servidor LSP fixture — fala o mínimo do protocolo (framing, `initialize`, `textDocument/definition`) devolvendo uma Location fixa (linha 3, char 7) para o teste hermético do cliente LSP.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
| --- | --- | --- | --- | --- |
| `write_msg/read_msg` | framing `Content-Length` | saída/entrada/wire | stdio | mesmo framing do cliente |
| `method` | `&str` (de `m["method"]`) | entrada/wire | cliente → fixture | dispatch |
| `id` | `Value` (de `m["id"]`) | entrada/wire | cliente → resposta | eco do id |
| `initialize` result | `{capabilities:{definitionProvider,referencesProvider}}` | saída/wire | fixture → cliente | |
| `textDocument/definition` `uri` | `Value` (de params.textDocument.uri) | entrada/wire | cliente → fixture | ecoado no resultado |
| definition result | `[{uri, range: start{line:3,char:7} end{line:3,char:11}}]` | saída/wire | fixture → cliente | Location conhecida e fixa |
| `shutdown`/`exit` | result null / break | saída/efeito | cliente → fixture | encerra loop |
| requests desconhecidos com id | result null | saída/wire | fixture → cliente | notificações ignoradas |

Fluxo: `stdin (framing) → initialize / definition (Location fixa 3:7) / shutdown|exit → stdout`.

---

## crates/btv-schemas/src/verification.rs

Papel: contrato `verification-evidence.v1` — tipos serde/JsonSchema da evidência de verificação e a derivação do veredito (referenciado por btv-verify).

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
| --- | --- | --- | --- | --- |
| `Verdict::{Pass,Fail,Skipped}` | enum serde (`snake_case`) | wire | evidência | serializa "pass"/"fail"/"skipped" |
| `Finding.tool/severity/message` | `String` | wire | parsers/vetter → evidência | |
| `Finding.file` | `Option<String>` (`skip_serializing_if None`) | wire | parsers | |
| `Finding.line` | `Option<u64>` (`skip_serializing_if None`) | wire | parsers | |
| `VerificationStep.name/tool` | `String` | wire | run_step | |
| `VerificationStep.exit_code` | `i32` | wire | run_step | -1 falha exec, 124 timeout |
| `VerificationStep.duration_ms` | `u64` | wire | exec | |
| `VerificationStep.findings` | `Vec<Finding>` (`default`) | wire | parser | |
| `VerificationEvidence.{run_id,git_sha,produced_at}` | `String` | wire | chamador | metadados |
| `VerificationEvidence.steps` | `Vec<VerificationStep>` | wire | run_pipeline | |
| `VerificationEvidence.verdict` | `Verdict` | wire/saída | `derive_verdict` | |
| `derive_verdict(steps)` | `Verdict` | saída | steps → veredito | `Fail` se qualquer `exit_code != 0`, senão `Pass` |

Fluxo: `steps (exit_code) → derive_verdict (Fail se algum ≠0) → VerificationEvidence serializável (JsonSchema)`.

---

## crates/btv-verify/src/lib.rs

Papel: pipeline de verificação determinística (`/verify`) — roda `StepSpec`s como subprocessos com timeout e monta `VerificationEvidence`; sentinela de timeout, findings por parser.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
| --- | --- | --- | --- | --- |
| `git_sha()` | `Option<String>` | saída | `git rev-parse HEAD` | `None` fora de repo/sem git; trim do stdout |
| `Parser::{CargoTest,ClippyJson,RuffJson}` | enum Copy | config/estado | StepSpec | `apply(stdout)` → `Vec<Finding>` delega a `parsers::*` |
| `StepSpec.name/program/args` | `String`/`String`/`Vec<String>` | estado/config | config/default → pipeline | comando do passo |
| `StepSpec.timeout` | `Option<Duration>` | estado/config | `with_timeout` | `None` = sem limite |
| `StepSpec.parser` | `Option<Parser>` | estado/config | `with_parser` | opcional |
| `run_pipeline(run_id,git_sha,produced_at,steps)` | `VerificationEvidence` | saída | chamador → evidência | callback vazio |
| `run_pipeline_with_progress` `on_step` | `FnMut(usize,usize,&VerificationStep)` | entrada/saída | pipeline → dashboard | reporta `(passo_concluído, total, &step)` por passo |
| local `executed` | `Vec<VerificationStep>` | intermediário | loop | |
| `TIMEOUT_EXIT_CODE` | `const i32 = 124` | config | run_step | sentinela do coreutils `timeout` |
| `run_step(spec)` `result` | `exec::StepResult` | intermediário | exec → run_step | `{output, duration_ms, timed_out}` |
| `run_step` `tool` | `String` `"{program} {args}"` (trim) | intermediário/wire | run_step → step | rótulo da ferramenta |
| `run_step` `stdout` | `Cow<str>` (`from_utf8_lossy`) | intermediário | output → parser | |
| `run_step` `findings` | `Vec<Finding>` | intermediário/saída | parser(+timeout finding) | timeout adiciona finding "error" |
| `run_step` `exit_code` | `i32` | saída | 124 se timeout, senão `code().unwrap_or(-1)` | |
| erro de exec | `VerificationStep{exit_code:-1, findings:[falha ao executar]}` | saída | exec Err → step | programa inexistente não panica |
| retorno | `VerificationEvidence` | saída | pipeline → CLI/vetter/dashboard | verdict via `derive_verdict` |

Fluxo: `StepSpec[] → run_step (exec::run_with_timeout → StepResult → parser findings + timeout/-1 sentinela) → VerificationStep[] → derive_verdict → VerificationEvidence`.

---

## crates/btv-verify/src/config.rs

Papel: config de passos por projeto (`btv.toml`) — parse TOML `[[step]]` → `StepSpec`; default sensato quando ausente.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
| --- | --- | --- | --- | --- |
| **`btv.toml`** (raiz do projeto) | TOML (`[[step]]`) | entrada/config/wire | disco → `load_config` | ausente → `Ok(None)`; inválido → `Err` (falha alto) |
| `VerifyConfig.steps` | `Vec<StepConfig>` (`rename "step"`, `default`) | estado/wire | toml | |
| `to_step_specs()` | `Vec<StepSpec>` | saída | config → pipeline | mapeia `StepSpec::from` |
| `StepConfig.name/program` | `String` | config/wire | toml | |
| `StepConfig.args` | `Vec<String>` (`default`) | config/wire | toml | |
| `StepConfig.timeout_ms` | `Option<u64>` | config/wire | toml | vira `Duration::from_millis` |
| `StepConfig.parser` | `Option<String>` | config/wire | toml | "cargo_test"/"clippy_json"/"ruff_json"; outro/ausente → sem parser |
| `StepConfig::parser()` | `Option<Parser>` | intermediário | string → enum | match dos 3 nomes |
| `From<&StepConfig> for StepSpec` | `StepSpec` | saída | config → spec | aplica timeout+parser condicionalmente |
| `ConfigError::{Io,Parse}` | enum (thiserror) | saída | load_config | Io(std::io::Error), Parse(toml) |
| `load_config(path)` | `Result<Option<VerifyConfig>,ConfigError>` | saída | disco → CLI | `None` se ausente |
| `default_steps()` | `Vec<StepSpec>` | saída/config | fallback | espelha job `rust` do CI |
| default `test` | StepSpec `cargo test --workspace`, timeout 900s | config | | |
| default `lint` | StepSpec `cargo clippy --workspace --message-format=json -- -D warnings`, timeout 180s, parser ClippyJson | config | | `-D warnings` |
| default `fmt` | StepSpec `cargo fmt --all --check`, timeout 30s | config | | |

Fluxo: `btv.toml → VerifyConfig[StepConfig] → to_step_specs()/StepSpec::from (timeout_ms→Duration, parser string→enum) → StepSpec[]`; ausente → `default_steps()` (test/lint/fmt do CI).

---

## crates/btv-verify/src/exec.rs

Papel: execução de um passo com timeout que mata o GRUPO de processos (não só o pid direto) — lição do órfão da Fase 4d.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
| --- | --- | --- | --- | --- |
| `StepResult.output` | `io::Result<Output>` | saída | exec → run_step | stdout/stderr/status capturados |
| `StepResult.duration_ms` | `u64` | saída | `start.elapsed()` | |
| `StepResult.timed_out` | `bool` | saída | exec → run_step | true se matou por timeout |
| `run_with_timeout(program,args,timeout)` params | `&str`/`&[String]`/`Option<Duration>` | entrada/config | run_step → exec | |
| `command` | `Command` (stdout/stderr piped, `process_group(0)` unix) | intermediário | spawn | pgid do filho = pid (grupo isolado) |
| erro de spawn | `StepResult{output:Err(e), timed_out:false}` | saída | spawn falhou | não panica |
| `pid` | `u32` | intermediário | child.id | alvo do kill de grupo |
| `handle` | `JoinHandle<io::Result<Output>>` (`wait_with_output`) | intermediário | thread | drena e espera |
| ramo sem timeout | `join_output(handle)`, `timed_out:false` | saída | sem limite | roda até o fim |
| `deadline` | `Instant` | intermediário | now + timeout | |
| loop de espera | `is_finished` + `sleep(20ms)` | intermediário | exec | `Instant::now()>=deadline` → `kill_process_group(pid)` + `timed_out=true` |
| `join_output(handle)` | `io::Result<Output>` | intermediário/saída | thread → exec | pânico da thread → `io::Error::other` |
| `kill_process_group(pid)` | `libc::kill(-pid, SIGKILL)` (unix) | saída/efeito | timeout → SO | mata grupo inteiro (netos inclusive) |

Fluxo: `program+args → spawn (grupo isolado via process_group(0)) → thread wait_with_output; timeout → kill(-pid) grupo inteiro → StepResult{output,duration_ms,timed_out}`.

---

## crates/btv-verify/src/parsers.rs

Papel: parsers de findings por ferramenta — funções puras best-effort (campo ausente/formato inesperado descarta só aquele item, nunca panica).

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
| --- | --- | --- | --- | --- |
| `parse_cargo_test(stdout)` | `Vec<Finding>` | entrada→saída | stdout → findings | captura linhas `test <nome> ... FAILED`; tool "cargo test", severity "error" |
| `parse_clippy_json(stdout)` | `Vec<Finding>` | entrada→saída | stdout (1 JSON/linha) → findings | filtra `reason=="compiler-message"` + level warning/error |
| clippy `primary_span` | span com `is_primary==true` | intermediário/wire | message.spans → file/line | `file_name` → `Finding.file`; `line_start` → `Finding.line` |
| clippy `Finding` | tool "clippy", severity = level | saída/wire | | |
| `parse_ruff_json(stdout)` | `Vec<Finding>` | entrada→saída | array JSON raiz → findings | JSON inválido → `[]` |
| ruff campos | `message`/`severity`(unwrap "error")/`filename`/`location.row`/`code`(unwrap "ruff") | intermediário/wire | item → Finding | tool `"ruff({code})"` |

Fluxo: `stdout da ferramenta → parse específico (texto FAILED | JSON por linha | array JSON) → Vec<Finding>{tool,severity,message,file?,line?}`, robusto a formato inesperado.

---

## crates/btv-verify/src/vetter.rs

Papel: skill-vetter — reusa `run_pipeline` apontado ao dir da skill + checagens estáticas (padrão perigoso, permissão incoerente) e decide `Vet`/`Block` de forma dura e fail-closed.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
| --- | --- | --- | --- | --- |
| **`skill.toml`** (raiz do dir da skill) | TOML | entrada/config/wire | disco → `read_manifest` | ausente/inválido → Block (fail-closed) |
| `SkillManifest.name/description` | `String` | config/wire | toml | |
| `SkillManifest.entrypoint` | `Option<String>` (`default`) | config/wire | toml | |
| `SkillManifest.permissions` | `Vec<String>` (`default`) | config/wire | toml | ex.: read/bash/webfetch |
| `SkillManifest.verify_steps` | `Vec<StepConfig>` (`rename "verify"`, `default`) | config/wire | toml | passos próprios da skill |
| `ManifestError::{Io,Parse}` | enum (thiserror) | saída | read_manifest | |
| `Decision::{Vet,Block}` | enum Copy | saída/estado | vet_skill | |
| `decision_to_skill_status(d)` | `&'static str` "aprovado"/"bloqueado" | saída/wire | vetter → frontend | vocabulário de `SkillEntry.status` |
| `VettingResult.decision/evidence` | `Decision`/`VerificationEvidence` | saída | vet_skill → chamador | |
| `DANGEROUS_PATTERNS` | `const &[(&str,&str)]` | config | scan | `("rm -rf /", ...)`, `(":(){ :|:& };:", "fork bomb")` |
| `BASH_SIGNATURES` | `const &[&str]` | config | scan_permission_mismatch | `Command::new`, `subprocess.`, `os.system(`, `child_process` |
| `NETWORK_SIGNATURES` | `const &[&str]` | config | scan_permission_mismatch | `reqwest::`, `requests.`, `fetch(`, `urllib.`, `http.client` |
| `walk_files(dir,out)` | recursivo → `Vec<PathBuf>` | intermediário | dir → files | |
| `skill_files(dir)` | `Vec<PathBuf>` | intermediário | walk (exclui `skill.toml`) | |
| `relative(dir,path)` | `String` | intermediário | strip_prefix | caminho relativo p/ finding |
| `scan_dangerous_patterns` | `Vec<Finding>` (severity "critical") | saída | files → findings | padrão perigoso; também curl/wget + `| sh`/`| bash` (pipe-to-shell) |
| `scan_permission_mismatch` | `Vec<Finding>` (severity "critical") | saída | files+manifest → findings | bash/net usado sem permissão declarada; flag uma vez cada |
| `has_critical_finding(evidence)` | `bool` | intermediário | steps → decisão | |
| `vet_skill` `checks_exit_code` | `i32` (1 se crítico, senão 0) | intermediário | checagens → step | |
| `vet_skill` `steps` | `Vec<VerificationStep>` (manifest + checks + verify_steps) | intermediário | vetter → evidência | |
| `vet_skill` decisão | `Decision` | saída | `has_critical_finding \|\| verdict==Fail` → Block | nunca aprova por default |
| `SkillStatus.id/status/detail/source` | `String` (Serialize) | saída/wire | vetter → `/api/skills` | `status` no vocabulário do frontend |
| `list_skill_statuses(dir, source)` | `Vec<SkillStatus>` | saída | veta subdirs → tela admin | fail-closed: subdir sem manifesto → bloqueado |

Fluxo: `skill.toml + arquivos → read_manifest (ausente/inválido=Block) → scan_dangerous_patterns + scan_permission_mismatch (Findings critical) + run_pipeline(verify_steps) → VerificationEvidence → Vet|Block (crítico ou Fail = Block)`.

---

## crates/btv-verify/src/prompt_integrity.rs

Papel: validador de integridade de contrato de prompt (JSON) — campos obrigatórios, ética, piso de qualidade, padrão perigoso (severidade por modo/tier); fail-closed.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
| --- | --- | --- | --- | --- |
| `PromptMode::{Vitrine,Enterprise}` | enum serde (`snake_case`) | entrada/config/wire | chamador → validate | decide severidade do padrão perigoso |
| `Severity::{Warning,Error}` | enum serde | wire | issue | |
| `IntegrityIssue.code/message/severity` | `String`/`String`/`Severity` (Serialize) | saída/wire | validate → relatório | code estável (ex.: "missing_field") |
| `IntegrityReport.valid` | `bool` | saída/wire | validate → chamador | true só sem nenhum `Error` |
| `IntegrityReport.score` | `f64` | saída/wire | validate | `(1.0 − 0.1·issues.len()).max(0.0)` |
| `IntegrityReport.issues` | `Vec<IntegrityIssue>` | saída/wire | validate | |
| `REQUIRED_FIELDS` | `const [&str;4]` | config | validate | name/version/ethics_check/quality_gates |
| `DANGEROUS_PATTERNS` | `const [&str;7]` | config | validate | rm -rf/drop table/eval(/exec(/__import__/os.system/subprocess |
| `MIN_QUALITY_FLOOR` | `const f64 = 0.7` | config | validate | piso de qualidade |
| `validate_contract(contract, mode)` `contract` | `&Value` | entrada/wire | chamador → validate | contrato JSON |
| local `issues` | `Vec<IntegrityIssue>` | intermediário | validate | acumulador |
| checagem 1 (campos) | issue `missing_field` (Error) | saída | contract.get(field) None | |
| checagem 2 (ética) | issues `ethics_disabled`/`ethics_rule_missing` (Warning) | saída | ethics_check.enabled/rules | espera regras no_pii/no_bias |
| checagem 3 (qualidade) | issue `quality_floor` (Warning) | saída | quality_gates.min_score < 0.7 | |
| checagem 4 (perigo) | issue `dangerous_pattern` (severidade por modo) | saída | `contract.to_string().to_lowercase()` contém pattern | Enterprise→Error, Vitrine→Warning |
| `haystack` | `String` (contrato serializado minúsculo) | intermediário | validate | busca de substring |
| `errors` | `usize` | intermediário | filtro Error | decide `valid` |

Fluxo: `contract JSON + mode → 4 checagens (campos obrigatórios, ética, piso de qualidade, padrão perigoso c/ severidade por tier) → IntegrityReport{valid (0 errors), score (1−0.1·N), issues}`.

---

## crates/btv-tools/tests/loop_com_ferramentas_reais.rs

Papel: teste de integração — o fio completo do loop de agente com ferramentas REAIS (edit/bash/read/skill executando em tempdir); registry → permissão → subprocesso → output.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
| --- | --- | --- | --- | --- |
| `Scripted.turns` | `Mutex<Vec<AssistantTurn>>` | estado/entrada | teste → loop | turnos pré-definidos (LlmPort mock) |
| `Counting.seen` | `Mutex<Vec<usize>>` | estado/saída | loop → asserção | conta mensagens por chamada |
| `AllowAll`/`DenyAll` (resolver) | `PermissionResolver` | entrada | teste → loop | resolve tudo true / nega |
| tool_use `edit` | JSON `{path,old_string,new_string}` | wire | modelo scriptado → EditTool | grava `valor=2` real no tempdir |
| tool_result managed | `ToolOutput` truncado + `.btv/tool-outputs/` | saída/wire | bash (40000 bytes) → loop | valida Managed Tool Output File |
| negação `rm -rf /` | `LoopEvent::ToolDenied` + `ToolResult{is_error:true}` | saída | DenyAll → loop | modelo continua |
| skill registrada | `SkillTool` `printf 'OLA:%s'` | entrada | teste → registry | stdout real "OLA:mundo" volta como tool_result |
| skill negada | entrypoint `touch EXECUTOU` NÃO roda | saída | DenyAll → skill | prova que permissão barra antes de executar |
| `LoopError::MaxSteps(5)` | erro | saída | loop 6 turnos → erro | limite de passos |

Fluxo: `turnos scriptados (tool_use) → registry real → PermissionEngine (Allow/Deny) → run em subprocesso/fs real → ToolResult (incl. managed output, denial) → asserções sobre disco e eventos`.

---

## crates/btv-tools/tests/mcp_integration.rs

Papel: teste cross-process do cliente MCP — sobe `btv_mcp_fixture` como processo separado, registra tools namespaced e faz chamada real ida-e-volta.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
| --- | --- | --- | --- | --- |
| `fixture_config()` | `McpServerConfig` (`CARGO_BIN_EXE_btv_mcp_fixture`) | entrada/config | teste → register | env var do cargo |
| `register_mcp_server` retorno `n` | `usize` | saída | register → asserção | ≥1 tool do fixture |
| `mcp__fixture__echo` | `&dyn Tool` | intermediário | registry.get | namespaced, não sombreia `bash` |
| chamada `run({input:mundo})` | `ToolOutput` "ECHO:mundo" | saída/wire | processo MCP → teste | ida-e-volta real |
| 2º registro | `n2 == 0` | saída | colisão pulada | não duplica |
| sessão persistente | 3 chamadas reusam conexão | saída | tool → session | prova conexão viva |
| servidor inexistente | `register_mcp_server` → `Err` | saída | comando morto | bounded, não pendura (fim do thread leak) |

Fluxo: `CARGO_BIN_EXE_btv_mcp_fixture → register_mcp_server → mcp__fixture__echo.run → "ECHO:mundo"; colisão/inexistente provam guarda e bounded-fail`.

---

## crates/btv-tools/tests/lsp_integration.rs

Papel: teste de integração LSP em 2 camadas — hermético (fixture, sempre roda) e real (rust-analyzer, `#[ignore]`, CI).

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
| --- | --- | --- | --- | --- |
| `fixture_server_config(root)` | `LspServerConfig` (`CARGO_BIN_EXE_btv_lsp_fixture`) | entrada/config | teste → register | |
| `register_lsp_server` retorno | `usize == 4` | saída | register → asserção | definition/references/diagnostics/symbol |
| chamada definition | `ToolOutput` contém `:3:7` + `alvo.txt` | saída/wire | fixture → teste | Location fixa do fixture |
| 2º registro | `n2 == 0` | saída | colisão | não duplica |
| `rust_analyzer_disponivel()` | `bool` | intermediário | probe `--version` | guarda de honestidade |
| fixture cargo real | `src/lib.rs` (`alvo` def linha 0:7, uso linha 5), `src/ruim.rs` (erro sintaxe) | entrada | teste → rust-analyzer | |
| definition real | `lib.rs:0:7` | saída/wire | rust-analyzer → teste | definição semântica derivada do código real |
| references real | contém `lib.rs:5:` | saída/wire | rust-analyzer → teste | |
| diagnostics real | contém "error" | saída/wire | rust-analyzer → teste | erro de sintaxe |
| symbol real | `alvo` resolvido em `lib.rs:0:` | saída/wire | workspace/symbol → teste | busca por NOME |

Fluxo: `fixture (Location fixa 3:7, sempre) + rust-analyzer real (definição/refs/diag/symbol por igualdade com posições conhecidas, #[ignore]/CI)`.

---

## crates/btv-verify/tests/schema_golden.rs

Papel: teste golden — evidência real de `run_pipeline` (com findings preenchidos) valida contra `schemas/json/verification-evidence.v1.schema.json`; fecha risco de drift.

| Dado | Tipo | Direção | Origem → Destino | Transformação / observação |
| --- | --- | --- | --- | --- |
| `schema()` | `Value` (de `verification-evidence.v1.schema.json`) | entrada/wire | disco → validator | via `CARGO_MANIFEST_DIR` |
| evidência com findings | `VerificationEvidence` (`false` + printf clippy JSON) | intermediário/wire | run_pipeline → instance | findings reais (não `[]`) |
| validação | `errors: Vec` vazio | saída | jsonschema | evidência bate o schema |
| evidência vazia | `run_pipeline(...&[])` | intermediário | | também valida |
| documento quebrado | JSON sem `verdict` | entrada | negativo | `is_valid == false` (prova que não é "sempre passa") |

Fluxo: `run_pipeline → serde_json::to_value → jsonschema validator_for(verification-evidence.v1) → is_valid; caso quebrado (sem verdict) reprova`.
