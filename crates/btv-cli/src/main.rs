//! `btv` — CLI da plataforma unificada (BuildToValue + opencode + prompte).
//!
//! Fase 1: `run` executa o loop de agente real e `chat` abre o REPL
//! multi-turno — gateway LLM com streaming e cache por hash, ferramentas
//! sob permissão interativa e ledger em `.btv/btv.db`.
//! `squad` ativa o sidecar Python na Fase 4; `verify` completa na Fase 5.

mod btv_agent;
#[cfg(test)]
mod btv_agent_golden;
mod cache;
mod convert;
mod doctor_console;
mod lsp_console;
mod mcp_console;
mod memory_console;
mod prompt_render;
mod rate_limit_gen;
mod sandbox_console;
mod session;
mod sidecar;
mod skills;
mod squad;
mod squad_agent;
// E1s.2 constrói o extractor e o prova por testes unitários; a E1s.3 o
// A E1s.3 wirou a borda: o extractor `Tenant` entra nos seis consumidores e o
// `guarda_tenant` vira o layer de cobertura universal do `merged_router` — o
// módulo está todo em uso, o `#[allow(dead_code)]` do scaffolding saiu. Se
// algum item ficar sem uso, o lint reacende (era essa a promessa do allow).
mod tenant_extractor;
#[cfg(test)]
mod test_support;
mod tui_app;
mod web_agent;

use anyhow::{bail, Context, Result};
use btv_core::{
    AgentLoop, CompactionPolicy, DurableSession, LoopEvent, PermissionResolver, BUILD, PLAN,
};
use btv_llm::chat::ChatMessage;
use btv_llm::{tier_from_id, Gateway, Generator, ModelTier, RateLimiter};
use btv_schemas::experiment::{ExperimentReport, VariantStats};
use btv_store::{EventStore, PromptCache, Telemetry};
use btv_tools::ToolRegistry;
use cache::CachedGenerator;
use clap::{Parser, Subcommand};
use rate_limit_gen::RateLimitedGenerator;
use serde_json::Value;
use session::now_rfc3339;
use std::io::{BufRead, Write};
use std::path::PathBuf;

type CliGenerator = CachedGenerator<RateLimitedGenerator<Gateway>>;

#[derive(Parser)]
#[command(
    name = "btv",
    version,
    about = "Coding agent unificado (Rust + Python)"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(clap::Args, Clone)]
struct RunOpts {
    /// Modelo a usar (define o ModelTier).
    #[arg(long, default_value = "claude-sonnet-5")]
    model: String,
    /// Perfil de agente: build (edita) ou plan (somente leitura).
    #[arg(long, default_value = "build")]
    agent: String,
    /// Aprova automaticamente pedidos de permissão (use com cautela).
    #[arg(long)]
    yes: bool,
    /// Desliga o cache de prompts por hash.
    #[arg(long)]
    no_cache: bool,
    /// Retoma (ou nomeia) uma sessão durável; sem valor, cria uma nova.
    #[arg(long)]
    session: Option<String>,
    /// Janela de contexto do modelo, em tokens (para a compaction).
    #[arg(long, default_value_t = 200_000)]
    context_window: usize,
}

#[derive(Subcommand)]
enum Commands {
    /// Executa uma tarefa única com o agente ativo.
    Run {
        /// Descrição da tarefa.
        task: String,
        #[command(flatten)]
        opts: RunOpts,
    },
    /// Abre o REPL de conversa multi-turno.
    Chat {
        #[command(flatten)]
        opts: RunOpts,
    },
    /// Abre a interface de terminal (ratatui).
    Tui {
        #[command(flatten)]
        opts: RunOpts,
    },
    /// Roda o pipeline de verificação determinística (typecheck/test/lint/
    /// SAST) e grava a evidência `verification-evidence.v1`. Não confundir
    /// com a integridade do ledger — aquilo é `session.verify()` (a cadeia
    /// de hash, checada ao fim de `run`/`chat`); isto verifica código.
    Verify {
        /// Caminho do btv.toml (default: ./btv.toml; se ausente, roda o
        /// pipeline default espelhando o job `rust` do CI).
        #[arg(long)]
        config: Option<PathBuf>,
        /// Onde gravar a evidência (default: .btv/evidence/<run_id>.json).
        #[arg(long)]
        out: Option<PathBuf>,
        /// Formato do resumo impresso no stdout.
        #[arg(long, value_enum, default_value = "human")]
        format: VerifyFormat,
    },
    /// Delega a tarefa ao squad multi-agente (sidecar Python + gateway
    /// Rust). Degrada para agente-único → safe-mode se o squad falhar.
    Squad {
        /// Descrição da tarefa.
        task: String,
        #[command(flatten)]
        opts: RunOpts,
    },
    /// Sobe o dashboard de telemetria (`.btv/telemetry.db`) em localhost.
    Dashboard {
        /// Porta local do dashboard.
        #[arg(long, default_value_t = 7878)]
        port: u16,
        /// Endereço de bind. Default `127.0.0.1` (loopback, modo local-first).
        /// Use `0.0.0.0` SÓ atrás de um proxy/ingress COM autenticação e com
        /// `BTV_TRUSTED_ORIGINS` setado — o dashboard executa código e guarda
        /// API keys; expô-lo direto na internet é inseguro por design.
        #[arg(long, default_value = "127.0.0.1")]
        host: std::net::IpAddr,
        /// Fase 7 Onda 15 (fecho): as rotas do agente web (sessão/permissão
        /// via SSE, squad ao vivo, designer) vêm HABILITADAS por padrão — o
        /// navegador é a forma primária de uso desta fase, não mais opt-in
        /// (Onda 1). `--no-web-agent` volta ao dashboard só-leitura de
        /// antes, por trás da mesma guarda de Origin/Host.
        #[arg(long, default_value_t = false)]
        no_web_agent: bool,
    },
    /// Gera o relatório de A/B testing de um experimento a partir da telemetria
    /// local: compara a taxa de sucesso das duas variantes com teste de
    /// significância. Sem diferença real → "sem significância", nunca um
    /// vencedor fabricado (a régua Nada Fake aplicada a estatística).
    Experiment {
        /// Nome do experimento (`props.experiment` na telemetria).
        experiment: String,
        /// Banco de telemetria (default: `.btv/telemetry.db`).
        #[arg(long)]
        db: Option<PathBuf>,
        /// Formato da saída.
        #[arg(long, value_enum, default_value = "human")]
        format: VerifyFormat,
    },
    /// (saas) Emite uma sessão para um usuário de um tenant e imprime o
    /// token UMA vez. Ferramenta de OPERADOR — a "chave da porta" do modo
    /// saas (ADR 0029, E1s.1): não é login de usuário, é o admin que cria a
    /// sessão inicial. O banco guarda só o hash; o token não persiste.
    #[cfg(feature = "pg")]
    Session {
        #[command(subcommand)]
        cmd: SessionCmd,
    },
}

/// Subcomandos de `btv session` (só sob a feature `pg`).
#[cfg(feature = "pg")]
#[derive(clap::Subcommand)]
enum SessionCmd {
    /// Emite uma sessão e imprime o token opaco UMA vez no stdout.
    Issue {
        /// UUID do tenant dono da sessão.
        #[arg(long)]
        tenant: String,
        /// Id do usuário (vira `actor = user:<id>` na trilha de auditoria).
        #[arg(long)]
        user: String,
        /// URL do Postgres (default: env `BTV_PG_URL`).
        #[arg(long)]
        db_url: Option<String>,
    },
}

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
enum VerifyFormat {
    /// Resumo legível por passo + veredito + caminho do artefato.
    Human,
    /// A própria evidência JSON no stdout (além do arquivo gravado).
    Json,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Run { task, opts } => {
            let (generator, root) = prepare(&opts)?;
            run_once(&generator, &opts, &root, task).await
        }
        Commands::Chat { opts } => {
            let (generator, root) = prepare(&opts)?;
            chat_repl(&generator, &opts, &root).await
        }
        Commands::Tui { opts } => {
            let (generator, root) = prepare(&opts)?;
            tui_app::run_tui(std::sync::Arc::new(generator), opts, root).await
        }
        Commands::Verify {
            config,
            out,
            format,
        } => run_verify(config, out, format),
        Commands::Squad { task, opts } => {
            let (generator, root) = prepare(&opts)?;
            squad::run_squad(generator, &opts, &root, task).await
        }
        Commands::Dashboard {
            port,
            host,
            no_web_agent,
        } => run_dashboard(host, port, !no_web_agent).await,
        #[cfg(feature = "pg")]
        Commands::Session { cmd } => run_session(cmd).await,
        Commands::Experiment {
            experiment,
            db,
            format,
        } => run_experiment(experiment, db, format),
    }
}

/// Gera o relatório de A/B de um experimento a partir da telemetria local
/// (`.btv/telemetry.db`). Exige exatamente 2 variantes (o A/B é entre duas);
/// o veredito de significância é derivado dos dados — nunca inventa vencedor.
fn run_experiment(experiment: String, db: Option<PathBuf>, format: VerifyFormat) -> Result<()> {
    let root = std::env::current_dir().context("diretório atual")?;
    let db_path = db.unwrap_or_else(|| root.join(".btv").join("telemetry.db"));
    let telemetry = Telemetry::open(db_path.to_str().unwrap_or(".btv/telemetry.db"))?;

    let variants = telemetry.experiment_variants(&experiment);
    if variants.len() < 2 {
        bail!(
            "um experimento comparável exige >=2 variantes; '{experiment}' tem {} \
             (procuro eventos com props.experiment='{experiment}' e props.variant na telemetria)",
            variants.len()
        );
    }
    let stats: Vec<VariantStats> = variants
        .into_iter()
        .map(|(v, n, s)| VariantStats::new(v, n, s))
        .collect();
    let report = ExperimentReport::from_variants(experiment, "success_rate", stats, now_rfc3339());

    match format {
        VerifyFormat::Json => println!("{}", serde_json::to_string_pretty(&report)?),
        VerifyFormat::Human => print_experiment_human(&report),
    }
    Ok(())
}

fn print_experiment_human(report: &ExperimentReport) {
    use btv_schemas::experiment::{ExperimentVerdict, MIN_SAMPLES};
    println!(
        "Experimento: {}  (métrica: {})",
        report.experiment, report.metric
    );
    for v in &report.variants {
        println!(
            "  {}: {}/{} sucessos = {:.1}%",
            v.variant,
            v.successes,
            v.n,
            v.rate * 100.0
        );
    }
    match report.verdict {
        ExperimentVerdict::Significant => println!(
            "Veredito: VENCEDOR {} (p = {:.4} < {ALPHA}) — diferença significativa",
            report.winner.as_deref().unwrap_or("?"),
            report.p_value,
            ALPHA = btv_schemas::experiment::ALPHA,
        ),
        ExperimentVerdict::Inconclusive => println!(
            "Veredito: SEM SIGNIFICÂNCIA (p = {:.4}) — sem vencedor",
            report.p_value
        ),
        ExperimentVerdict::InsufficientData => {
            println!("Veredito: DADOS INSUFICIENTES — mínimo de {MIN_SAMPLES} eventos por variante")
        }
    }
}

/// Sobe o dashboard de telemetria lendo `.btv/telemetry.db` do diretório
/// atual (criado, se ausente, por `run`/`chat`).
/// (saas) `btv session issue` — emite uma sessão e imprime o token UMA vez.
/// A conexão PG e a gravação (só o hash) ficam no `PgStore::issue_session`;
/// aqui é só a ergonomia de operador: valida o tenant, chama a porta,
/// imprime o token com o aviso de que ele não será mostrado de novo.
#[cfg(feature = "pg")]
async fn run_session(cmd: SessionCmd) -> Result<()> {
    match cmd {
        SessionCmd::Issue {
            tenant,
            user,
            db_url,
        } => {
            let tenant = btv_domain::TenantId::parse(&tenant)
                .map_err(|e| anyhow::anyhow!("tenant inválido: {e}"))?;
            let url = db_url
                .or_else(|| std::env::var("BTV_PG_URL").ok())
                .context("URL do Postgres ausente (--db-url ou env BTV_PG_URL)")?;
            // block_on interno do PgStore não convive com o runtime tokio do
            // main — a emissão é síncrona e curta, então roda em spawn_blocking.
            let emitido = tokio::task::spawn_blocking(move || {
                let store = btv_store::pg::PgStore::connect(&url)
                    .map_err(|e| anyhow::anyhow!("conexão Postgres: {e}"))?;
                store
                    .issue_session(&tenant, &user)
                    .map_err(|e| anyhow::anyhow!("emissão da sessão: {e}"))
            })
            .await
            .context("task de emissão")??;
            println!("{}", emitido.token);
            eprintln!(
                "sessão emitida — o token acima NÃO será mostrado de novo \
                 (o banco guarda só o hash). Entregue-o pelo canal seguro."
            );
            Ok(())
        }
    }
}

async fn run_dashboard(host: std::net::IpAddr, port: u16, web_agent: bool) -> Result<()> {
    let root = std::env::current_dir().context("diretório atual")?;
    let btv_dir = root.join(".btv");
    std::fs::create_dir_all(&btv_dir)?;
    let telemetry = Telemetry::open(
        btv_dir
            .join("telemetry.db")
            .to_str()
            .unwrap_or(".btv/telemetry.db"),
    )?;
    // Mesmo arquivo (`.btv/prompt_library.db`) que `/prompt save|library|...`
    // do chat REPL já usa — não uma segunda biblioteca de prompts. Aberta uma
    // vez aqui (não por requisição) e compartilhada via Arc<Mutex<_>>, mesmo
    // motivo de `Telemetry` já ser um handle compartilhável (Fase 7 Onda 5).
    let prompt_library =
        std::sync::Arc::new(std::sync::Mutex::new(btv_store::PromptLibrary::open(
            btv_dir
                .join("prompt_library.db")
                .to_str()
                .unwrap_or(".btv/prompt_library.db"),
        )?));
    // Mesmo ledger (`.btv/btv.db`) que `session.rs`/`squad_agent.rs` já
    // gravam — a tela só lê o que a CLI/squad já registraram, não uma
    // segunda cadeia (Fase 7 Onda 6).
    let ledger = std::sync::Arc::new(std::sync::Mutex::new(btv_store::LedgerStore::open(
        btv_dir.join("btv.db").to_str().unwrap_or(".btv/btv.db"),
    )?));
    let addr = std::net::SocketAddr::new(host, port);
    let web_dir = btv_server::default_web_dir();
    if web_agent {
        eprintln!(
            "btv dashboard — http://{addr} (assets: {}; console dev em /dev; sessão/permissão/squad ao vivo)",
            web_dir.display()
        );
        let hub = web_agent::default_hub();
        let squad_hub = squad_agent::default_hub();
        let squad_pool = squad_agent::default_squad_pool(&root);
        // BuildToValue: mesma hub/pool do squad (uma ativação pela galeria e
        // um `POST /api/squad/run` são o MESMO motor), ledger compartilhado e
        // store próprio (`.btv/btv.db`).
        let btv_store = std::sync::Arc::new(std::sync::Mutex::new(btv_store::BtvStore::open(
            btv_dir.join("btv.db").to_str().unwrap_or(".btv/btv.db"),
        )?));
        // O contador de `task_id` da squad é por-processo e reinicia a cada
        // restart; o volume das runs sobrevive. Semeia o contador acima do maior
        // `sq{n}` já persistido para a primeira ativação após um redeploy não
        // colidir (`UNIQUE constraint failed: runs.task_id`).
        {
            let store = btv_store.lock().unwrap_or_else(|e| e.into_inner());
            squad_hub.seed_task_seq(store.max_run_task_seq());
            // Runs que ficaram `ativa` no volume são zumbis (o processo que as
            // rodava morreu — o estado vivo da squad é só em memória). No
            // arranque, reconcilia para `encerrada` — senão ficam "ativa" para
            // sempre na tela, sem nunca concluir.
            match store.reconcile_stale_runs(&now_rfc3339()) {
                Ok(n) if n > 0 => {
                    eprintln!("btv: {n} run(s) 'ativa' órfã(s) → 'encerrada' no arranque")
                }
                _ => {}
            }
        }
        let doctor_router = doctor_console::router(doctor_console::DoctorStores {
            ledger: ledger.clone(),
            btv: btv_store.clone(),
        });
        let btv_router = btv_agent::router(
            squad_hub.clone(),
            squad_pool.clone(),
            ledger.clone(),
            btv_store,
        );
        let squad_router = squad_agent::router(squad_hub, squad_pool);
        let sidecar_service = prompt_render::default_sidecar_service(&root);
        let prompt_router = prompt_render::router(sidecar_service);
        let mcp_router = mcp_console::router(root.clone());
        let memory_service = memory_console::default_memory_service(&root);
        let memory_router = memory_console::router(memory_service);
        let sandbox_router = sandbox_console::router();
        let lsp_router = lsp_console::router(root.clone());
        let extra_router = squad_router
            .merge(btv_router)
            .merge(prompt_router)
            .merge(mcp_router)
            .merge(memory_router)
            .merge(sandbox_router)
            .merge(lsp_router)
            .merge(doctor_router);
        web_agent::serve_with_agent(
            telemetry,
            prompt_library,
            ledger,
            &root,
            addr,
            web_dir,
            hub,
            extra_router,
            // Modo local (o dashboard): sem resolver — a borda universal é
            // no-op e o extractor devolve TenantContext::local. A onda saas
            // injeta aqui `Some(Arc::new(PgStore))`, a MESMA fonte do resolver
            // do BtvAgentState (a borda gateia toda rota, o extractor produz o
            // contexto dos seis) — troca a fonte sem tocar a borda.
            tenant_extractor::TenantResolucao::default(),
        )
        .await?;
    } else {
        eprintln!(
            "btv dashboard --no-web-agent (somente leitura) — http://{addr} (assets: {})",
            web_dir.display()
        );
        btv_server::serve(telemetry, prompt_library, ledger, &root, addr, web_dir).await?;
    }
    Ok(())
}

/// Carrega `btv.toml` (`root/btv.toml` se `config` for `None`) ou cai no
/// default que espelha o job `rust` do CI, e roda o pipeline determinístico.
/// Compartilhado entre `btv verify` e `btv squad` (Fase 5 Onda 3: o squad
/// roda o mesmo `/verify` antes de disparar a tarefa, anexando a evidência
/// ao `SquadTask`) — evita duplicar a lógica de carregar config + rodar.
pub(crate) fn run_verify_pipeline(
    root: &std::path::Path,
    config: Option<&std::path::Path>,
) -> Result<btv_schemas::verification::VerificationEvidence> {
    let config_path = config
        .map(std::path::Path::to_path_buf)
        .unwrap_or_else(|| root.join("btv.toml"));
    let steps = match btv_verify::config::load_config(&config_path)
        .with_context(|| format!("lendo {}", config_path.display()))?
    {
        Some(cfg) => cfg.to_step_specs(),
        None => btv_verify::config::default_steps(),
    };

    let run_id = format!("run-{:x}", nanos_now() & 0xffff_ffff_ffff);
    let sha = git_sha().unwrap_or_else(|| "unknown".to_string());
    let produced_at = now_rfc3339();
    Ok(btv_verify::run_pipeline(
        &run_id,
        &sha,
        &produced_at,
        &steps,
    ))
}

/// Roda `/verify`: carrega `btv.toml` (ou cai no default, que espelha o
/// job `rust` do CI) na raiz do diretório atual, executa o pipeline
/// determinístico e grava `verification-evidence.v1` em disco.
///
/// Sai com código ≠ 0 quando o veredito é `Fail` — é o gate que a Onda 6
/// (CI) vai cobrar para o self-hosting. Isso é resultado legítimo do
/// verify, não um crash: por isso usa `process::exit` **depois** de gravar
/// o artefato e imprimir o resumo, em vez de `anyhow::bail!` (que
/// imprimiria como se fosse erro inesperado, com o prefixo "Error:").
///
/// Não recebe `root` via `prepare()` como `run`/`chat`/`squad` — verify é
/// determinístico e offline (sem provider de LLM), então resolve
/// `current_dir()` por conta própria, igual a `run_dashboard`.
fn run_verify(config: Option<PathBuf>, out: Option<PathBuf>, format: VerifyFormat) -> Result<()> {
    let root = std::env::current_dir().context("diretório atual")?;
    let btv_dir = root.join(".btv");
    std::fs::create_dir_all(&btv_dir)?;

    let evidence = run_verify_pipeline(&root, config.as_deref())?;
    let run_id = evidence.run_id.clone();

    let out_path = out.unwrap_or_else(|| btv_dir.join("evidence").join(format!("{run_id}.json")));
    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(&evidence).context("serializando evidência")?;
    std::fs::write(&out_path, &json).with_context(|| format!("gravando {}", out_path.display()))?;

    match format {
        VerifyFormat::Json => println!("{json}"),
        VerifyFormat::Human => {
            println!("btv verify — run {run_id} ({})", evidence.git_sha);
            for step in &evidence.steps {
                let mark = if step.exit_code == 0 { "✓" } else { "✗" };
                println!(
                    "  {mark} {} ({}ms) — {} finding(s)",
                    step.name,
                    step.duration_ms,
                    step.findings.len()
                );
                for finding in &step.findings {
                    let loc = match (&finding.file, finding.line) {
                        (Some(f), Some(l)) => format!(" [{f}:{l}]"),
                        (Some(f), None) => format!(" [{f}]"),
                        _ => String::new(),
                    };
                    println!("      {} {}{loc}", finding.severity, finding.message);
                }
            }
            println!("veredito: {:?}", evidence.verdict);
            println!("evidência: {}", out_path.display());
        }
    }

    if matches!(evidence.verdict, btv_schemas::verification::Verdict::Fail) {
        std::process::exit(1);
    }
    Ok(())
}

/// `git rev-parse HEAD` best-effort — git ausente/repo fora de um worktree
/// não deve abortar o verify, só perder a rastreabilidade do sha.
fn git_sha() -> Option<String> {
    let output = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout)
        .ok()
        .map(|s| s.trim().to_string())
}

fn nanos_now() -> u128 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0)
}

/// Monta o gerador concreto (gateway + rate limit + cache, salvo
/// --no-cache) e valida que há providers configurados. Telemetria
/// (`.btv/telemetry.db`) registra `llm.call`/`cache.hit`/`cache.miss`
/// sem nunca derrubar o caminho principal.
fn prepare(opts: &RunOpts) -> Result<(CliGenerator, PathBuf)> {
    let gateway = Gateway::from_env();
    let available = gateway.available();
    if available.is_empty() {
        bail!(
            "nenhum provider configurado — defina ANTHROPIC_API_KEY, DEEPSEEK_API_KEY ou OPENAI_API_KEY"
        );
    }
    let root = std::env::current_dir().context("diretório atual")?;
    let tier = tier_from_id(&opts.model);
    eprintln!(
        "btv — modelo {} ({}) · agente {} · providers: {} · cache: {}",
        opts.model,
        tier_name(tier),
        opts.agent,
        available.join(", "),
        if opts.no_cache { "off" } else { "on" }
    );

    let btv_dir = root.join(".btv");
    std::fs::create_dir_all(&btv_dir)?;
    let cache = if opts.no_cache {
        // Cache em memória: satisfaz o tipo sem persistir nada.
        PromptCache::open_in_memory()?
    } else {
        PromptCache::open(btv_dir.join("cache.db").to_str().unwrap_or(".btv/cache.db"))?
    };
    let telemetry = Telemetry::open(
        btv_dir
            .join("telemetry.db")
            .to_str()
            .unwrap_or(".btv/telemetry.db"),
    )
    .ok();
    let limited =
        RateLimitedGenerator::new(gateway, RateLimiter::for_tier(tier), telemetry.clone());
    Ok((CachedGenerator::new(limited, cache, telemetry), root))
}

fn build_loop<'a, G: Generator>(
    generator: &'a G,
    opts: &RunOpts,
    tools: &'a ToolRegistry,
) -> Result<AgentLoop<'a, G>> {
    let profile = match opts.agent.as_str() {
        "build" => &BUILD,
        "plan" => &PLAN,
        other => bail!("agente desconhecido: {other} (use build ou plan)"),
    };
    let tier = tier_from_id(&opts.model);
    Ok(AgentLoop {
        generator,
        tools,
        permissions: (profile.permissions)(),
        model: opts.model.clone(),
        system: system_prompt(tier),
        max_steps: 20,
        max_tokens: 4096,
    })
}

/// Abre a sessão durável (nova ou retomada) em `.btv/sessions.db`.
fn open_durable(
    root: &std::path::Path,
    opts: &RunOpts,
    task_hint: &str,
) -> Result<DurableSession<EventStore>> {
    let store = EventStore::open(
        root.join(".btv")
            .join("sessions.db")
            .to_str()
            .unwrap_or(".btv/sessions.db"),
    )?;
    let session_id = opts.session.clone().unwrap_or_else(|| {
        use std::time::{SystemTime, UNIX_EPOCH};
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        format!("s{:x}", nanos & 0xffff_ffff_ffff)
    });
    // D1t: a sessão declara em nome de quem opera — o CLI local É o
    // tenant LOCAL (mesma decisão de porta legada do B2).
    let ctx = btv_domain::TenantContext::local(
        btv_domain::ActorId::new("cli:run").expect("actor fixo válido"),
    );
    let durable = DurableSession::open(store, ctx, &session_id, task_hint, &opts.model)?;
    if durable.resumed_messages() > 0 {
        eprintln!(
            "sessão {session_id} retomada — {} mensagem(ns) no histórico",
            durable.resumed_messages()
        );
    } else {
        eprintln!("sessão {session_id} (retome com --session {session_id})");
    }
    Ok(durable)
}

/// Compacta a sessão se a política mandar e a fronteira for segura.
/// Retorna true se uma nova época começou.
async fn maybe_compact<G: Generator>(
    generator: &G,
    opts: &RunOpts,
    durable: &mut DurableSession<EventStore>,
    session: &mut session::Session,
    force: bool,
) -> Result<bool> {
    let policy = CompactionPolicy::for_tier(tier_from_id(&opts.model), opts.context_window);
    if !(force || policy.needs_compaction(&durable.messages)) {
        return Ok(false);
    }
    if !CompactionPolicy::is_safe_boundary(&durable.messages) {
        if force {
            eprintln!("  compaction adiada: fronteira insegura (turno incompleto)");
        }
        return Ok(false);
    }
    let summary = policy
        .summarize(generator, &opts.model, &durable.messages)
        .await
        .map_err(|e| anyhow::anyhow!("compaction: {e}"))?;
    durable.compact(&summary)?;
    session.note(
        "compaction.applied",
        serde_json::json!({"epoch": durable.epoch(), "summary_chars": summary.len()}),
    );
    eprintln!(
        "  ⟲ contexto compactado — época {} ({} chars de resumo)",
        durable.epoch(),
        summary.len()
    );
    Ok(true)
}

/// Registra no ledger o veredito do vetter para cada skill (built-in +
/// terceiro), como auditoria append-only (Fase 6 Onda 3). Recebe as decisões
/// que o carregamento do registry JÁ tomou (`build_registry_with_vetting`) —
/// não re-veta (fechamento do "double-vet" da pendência: os passos
/// `[[verify]]` de uma skill de terceiro rodavam 2×).
fn record_skill_vetting(
    statuses: &[btv_verify::vetter::SkillStatus],
    session: &mut session::Session,
) {
    for s in statuses {
        session.note(
            "skill.vetting",
            serde_json::json!({"id": s.id, "status": s.status, "detail": s.detail}),
        );
    }
}

async fn run_once<G: Generator>(
    generator: &G,
    opts: &RunOpts,
    root: &std::path::Path,
    task: String,
) -> Result<()> {
    let (tools, skill_vetting) = crate::skills::build_registry_with_vetting(root);
    let agent_loop = build_loop(generator, opts, &tools)?;
    let mut session = session::Session::open(root, &task, &opts.model)?;
    // Fase 6 Onda 3: audita no ledger (append-only) o veredito do vetter para
    // cada skill carregada. A execução de skill já entra no ledger pelos
    // LoopEvents; isto registra a decisão de vetting em si.
    record_skill_vetting(&skill_vetting, &mut session);
    let mut durable = open_durable(root, opts, &task)?;
    let mut resolver = CliResolver { auto_yes: opts.yes };

    // Sidecar opcional (Fase 3): lint consultivo, nunca bloqueante.
    if let Some((_supervisor, mut client)) = sidecar::try_start().await {
        if let Ok(report) = client.lint(&task).await {
            if let Some(notice) = sidecar::advisory(&report) {
                eprintln!("{notice}");
            }
        }
    }

    maybe_compact(generator, opts, &mut durable, &mut session, false).await?;
    durable.messages.push(ChatMessage::user_text(&task));
    let result = {
        let mut on_event = |event: LoopEvent| {
            print_event(&event);
            session.record(&event);
        };
        agent_loop
            .continue_run(&mut durable.messages, &mut resolver, &mut on_event)
            .await
    };
    let persisted = durable.persist_new().unwrap_or_else(|e| {
        eprintln!("  [sessão] falha ao persistir: {e}");
        0
    });
    match result {
        Ok(summary) => {
            session.finish(true, summary.steps)?;
            eprintln!(
                "\nconcluído em {} passo(s) · {} mensagem(ns) persistida(s) · ledger íntegro: {} entrada(s)",
                summary.steps,
                persisted,
                session.verify()?
            );
            Ok(())
        }
        Err(e) => {
            session.finish(false, 0)?;
            bail!("{e}");
        }
    }
}

async fn chat_repl<G: Generator>(
    generator: &G,
    opts: &RunOpts,
    root: &std::path::Path,
) -> Result<()> {
    let (tools, skill_vetting) = crate::skills::build_registry_with_vetting(root);
    let agent_loop = build_loop(generator, opts, &tools)?;
    let mut session = session::Session::open(root, "<chat>", &opts.model)?;
    // Mesma auditoria de vetting do `run_once` — antes só ele registrava
    // (lacuna anotada na própria pendência da Fase 6 Onda 3).
    record_skill_vetting(&skill_vetting, &mut session);
    let mut resolver = CliResolver { auto_yes: opts.yes };

    let mut durable = open_durable(root, opts, "<chat>")?;
    // Sidecar opcional (Fase 3): mantido vivo durante todo o chat para
    // lint consultivo e o comando /prompt; None se indisponível (degrada).
    let sidecar_session = sidecar::try_start().await;
    if sidecar_session.is_none() {
        eprintln!("  (sidecar PromptForge indisponível — render de geradores fica desativado; biblioteca continua ativa)");
    }
    let library = btv_store::PromptLibrary::open(
        root.join(".btv")
            .join("prompt_library.db")
            .to_str()
            .unwrap_or(".btv/prompt_library.db"),
    )?;
    eprintln!("btv chat — digite a mensagem (vazio, \"sair\" ou Ctrl-D encerra; /compact força nova época; /prompt lista geradores; /prompt save|library|use|fav|rm gerencia a biblioteca)\n");
    let stdin = std::io::stdin();
    let mut turns = 0usize;

    loop {
        eprint!("> ");
        let _ = std::io::stderr().flush();
        let mut line = String::new();
        if stdin.lock().read_line(&mut line)? == 0 {
            break; // EOF
        }
        let input = line.trim();
        if input.is_empty() || matches!(input, "sair" | "exit" | "quit") {
            break;
        }
        if input == "/compact" {
            if !maybe_compact(generator, opts, &mut durable, &mut session, true).await? {
                eprintln!("  nada a compactar");
            }
            continue;
        }
        if let Some(rest) = input.strip_prefix("/prompt") {
            let sidecar_client = sidecar_session.as_ref().map(|(_, c)| c.clone());
            handle_prompt_command(sidecar_client, &library, rest.trim()).await;
            continue;
        }
        maybe_compact(generator, opts, &mut durable, &mut session, false).await?;

        if let Some((_, client)) = &sidecar_session {
            if let Ok(report) = client.clone().lint(input).await {
                if let Some(notice) = sidecar::advisory(&report) {
                    eprintln!("{notice}");
                }
            }
        }

        session.note("user.turn", serde_json::json!({"chars": input.len()}));
        durable.messages.push(ChatMessage::user_text(input));
        let result = {
            let mut on_event = |event: LoopEvent| {
                print_event(&event);
                session.record(&event);
            };
            agent_loop
                .continue_run(&mut durable.messages, &mut resolver, &mut on_event)
                .await
        };
        if let Err(e) = durable.persist_new() {
            eprintln!("  [sessão] falha ao persistir: {e}");
        }
        match result {
            Ok(_) => turns += 1,
            Err(e) => {
                eprintln!("\nerro: {e}");
                break;
            }
        }
        println!();
    }

    session.finish(true, turns)?;
    eprintln!(
        "\nchat encerrado após {turns} turno(s) · ledger íntegro: {} entrada(s)",
        session.verify()?
    );
    Ok(())
}

/// Trata `/prompt` no chat. Sem argumentos (ou `list`) lista os geradores
/// do sidecar; `<gerador> chave=valor ...` renderiza e imprime o prompt;
/// `save <nome> [tags=a,b] <gerador> chave=valor ...` renderiza e grava na
/// biblioteca (origem: prompte `library.js`); `library [tag]` lista os
/// prompts salvos; `use <id>` reimprime um prompt salvo; `fav <id>`
/// inverte o favorito; `rm <id>` remove. A biblioteca funciona mesmo sem
/// sidecar — só `save` e o render bruto exigem o gerador Python ativo.
async fn handle_prompt_command(
    mut sidecar: Option<btv_sidecar::SidecarClient>,
    library: &btv_store::PromptLibrary,
    rest: &str,
) {
    if rest.is_empty() || rest == "list" {
        let Some(client) = sidecar.as_mut() else {
            eprintln!("  sidecar indisponível — geradores desativados");
            return;
        };
        match client.list_generators().await {
            Ok(generators) => {
                eprintln!("  geradores disponíveis:");
                for g in generators {
                    let fields: Vec<String> = g.fields.iter().map(|f| f.name.clone()).collect();
                    eprintln!(
                        "    {} [{}] — campos: {}",
                        g.name,
                        g.category,
                        fields.join(", ")
                    );
                }
            }
            Err(e) => eprintln!("  falha ao listar geradores: {e}"),
        }
        return;
    }

    let first_token = rest.split_whitespace().next().unwrap_or("");
    let command_arg = rest[first_token.len()..].trim();

    if first_token == "library" {
        let tag = if command_arg.is_empty() {
            None
        } else {
            Some(command_arg)
        };
        match library.list(tag) {
            Ok(prompts) if prompts.is_empty() => eprintln!("  biblioteca vazia"),
            Ok(prompts) => {
                eprintln!("  prompts salvos:");
                for p in prompts {
                    eprintln!(
                        "    #{} {}{} [{}] — tags: {}",
                        p.id,
                        p.name,
                        if p.favorite { " ★" } else { "" },
                        p.generator,
                        p.tags.join(", ")
                    );
                }
            }
            Err(e) => eprintln!("  falha ao listar biblioteca: {e}"),
        }
        return;
    }

    if first_token == "use" {
        let Ok(id) = command_arg.parse::<i64>() else {
            eprintln!("  uso: /prompt use <id>");
            return;
        };
        match library.get(id) {
            Ok(Some(p)) => eprintln!(
                "  --- {} ({}) ---\n{}\n  ---------------------",
                p.name, p.generator, p.rendered
            ),
            Ok(None) => eprintln!("  prompt #{id} não encontrado"),
            Err(e) => eprintln!("  falha ao buscar prompt #{id}: {e}"),
        }
        return;
    }

    if first_token == "fav" {
        let Ok(id) = command_arg.parse::<i64>() else {
            eprintln!("  uso: /prompt fav <id>");
            return;
        };
        match library.toggle_favorite(id) {
            Ok(Some(state)) => eprintln!("  prompt #{id} favorito: {state}"),
            Ok(None) => eprintln!("  prompt #{id} não encontrado"),
            Err(e) => eprintln!("  falha ao favoritar #{id}: {e}"),
        }
        return;
    }

    if first_token == "rm" {
        let Ok(id) = command_arg.parse::<i64>() else {
            eprintln!("  uso: /prompt rm <id>");
            return;
        };
        match library.delete(id) {
            Ok(true) => eprintln!("  prompt #{id} removido"),
            Ok(false) => eprintln!("  prompt #{id} não encontrado"),
            Err(e) => eprintln!("  falha ao remover #{id}: {e}"),
        }
        return;
    }

    if first_token == "save" {
        let Some(client) = sidecar.as_mut() else {
            eprintln!("  sidecar indisponível — save exige o gerador Python ativo");
            return;
        };
        let mut parts = command_arg.split_whitespace();
        let Some(prompt_name) = parts.next() else {
            eprintln!("  uso: /prompt save <nome> [tags=a,b] <gerador> chave=valor ...");
            return;
        };
        let mut tags = Vec::new();
        let mut generator_name = None;
        let mut fields = std::collections::HashMap::new();
        for token in parts {
            if let Some(list) = token.strip_prefix("tags=") {
                tags = list
                    .split(',')
                    .map(str::to_string)
                    .filter(|s| !s.is_empty())
                    .collect();
            } else if generator_name.is_none() {
                generator_name = Some(token.to_string());
            } else if let Some((k, v)) = token.split_once('=') {
                fields.insert(k.to_string(), v.to_string());
            }
        }
        let Some(generator_name) = generator_name else {
            eprintln!("  uso: /prompt save <nome> [tags=a,b] <gerador> chave=valor ...");
            return;
        };
        let fields_json: Value = fields
            .iter()
            .map(|(k, v)| (k.clone(), Value::String(v.clone())))
            .collect::<serde_json::Map<_, _>>()
            .into();
        match client.render(&generator_name, fields).await {
            Ok(rendered) => {
                match library.save(
                    prompt_name,
                    &generator_name,
                    &fields_json,
                    &rendered,
                    &tags,
                    &now_rfc3339(),
                ) {
                    Ok(id) => eprintln!("  prompt #{id} salvo na biblioteca"),
                    Err(e) => eprintln!("  falha ao salvar na biblioteca: {e}"),
                }
            }
            Err(e) => eprintln!("  falha ao renderizar {generator_name}: {e}"),
        }
        return;
    }

    let Some(client) = sidecar.as_mut() else {
        eprintln!("  sidecar indisponível — render de geradores desativado");
        return;
    };
    let mut parts = rest.split_whitespace();
    let Some(name) = parts.next() else { return };
    let mut fields = std::collections::HashMap::new();
    for pair in parts {
        if let Some((k, v)) = pair.split_once('=') {
            fields.insert(k.to_string(), v.to_string());
        }
    }
    match client.render(name, fields).await {
        Ok(prompt) => eprintln!("  --- prompt gerado ---\n{prompt}\n  ---------------------"),
        Err(e) => eprintln!("  falha ao renderizar {name}: {e}"),
    }
}

fn print_event(event: &LoopEvent) {
    match event {
        LoopEvent::TextDelta(d) => {
            print!("{d}");
            let _ = std::io::stdout().flush();
        }
        LoopEvent::TurnCompleted { .. } => println!(),
        LoopEvent::ToolStarted { name, scope } => eprintln!("  ⚒ {name} {scope:?}"),
        LoopEvent::ToolFinished {
            name, ok, summary, ..
        } => {
            eprintln!("  {} {name}: {summary}", if *ok { "✓" } else { "✗" })
        }
        LoopEvent::ToolDenied { name, scope } => eprintln!("  ⛔ {name} {scope:?} negado"),
    }
}

/// Pergunta ao usuário no terminal quando a política devolve `Ask`.
struct CliResolver {
    auto_yes: bool,
}

impl PermissionResolver for CliResolver {
    fn resolve(&mut self, tool: &str, scope: &str) -> bool {
        if self.auto_yes {
            return true;
        }
        eprint!("\n  permitir {tool} em {scope:?}? [s/N] ");
        let _ = std::io::stderr().flush();
        let mut answer = String::new();
        if std::io::stdin().read_line(&mut answer).is_err() {
            return false;
        }
        matches!(
            answer.trim().to_lowercase().as_str(),
            "s" | "sim" | "y" | "yes"
        )
    }
}

fn system_prompt(tier: ModelTier) -> String {
    let base = "Você é o btv, um coding agent de terminal. Trabalhe no diretório atual \
usando as ferramentas disponíveis (read, grep, edit, bash). Leia antes de editar; edits \
exigem old_string única. Verifique seu trabalho com as ferramentas (testes, build) antes \
de concluir. Seja direto e objetivo nas respostas.";
    match tier {
        // Disciplina de passos para modelos small (fork do opencode).
        ModelTier::Small => format!("{base} Faça UMA ação por vez e reavalie após cada resultado."),
        _ => base.to_string(),
    }
}

fn tier_name(tier: ModelTier) -> &'static str {
    match tier {
        ModelTier::Small => "small",
        ModelTier::Medium => "medium",
        ModelTier::Large => "large",
    }
}
