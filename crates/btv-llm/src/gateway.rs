//! Gateway LLM: transporte HTTP com streaming, keys só neste processo,
//! cadeia de fallback entre providers (princípio do proxy do prompte).

use crate::anthropic;
use crate::chat::{AssistantTurn, GenerateRequest};
use crate::openai;
use crate::provider::ProviderId;
use crate::sse::SseParser;
use futures_util::StreamExt;

// D1t: o contrato de geração e seu erro moram em `btv-domain::ports`
// (`LlmPort`/`LlmError`) — o loop de agente os consome de lá; este crate
// re-exporta sob os nomes históricos e implementa a porta no `Gateway`.
pub use btv_domain::ports::{LlmError as GatewayError, LlmPort as Generator};

#[derive(Debug, Clone)]
struct ProviderConfig {
    id: ProviderId,
    api_key: String,
    base_url: String,
}

pub struct Gateway {
    client: reqwest::Client,
    providers: Vec<ProviderConfig>,
}

/// Cliente HTTP dos providers COM timeouts. Sem eles (`reqwest::Client::new()`),
/// uma chamada a um provider que trava (rede caída, rate-limit sem resposta,
/// servidor pendurado) fica ESPERANDO PARA SEMPRE e congela o agente/squad
/// inteiro — visto em produção: uma squad ao vivo parou na Revisão e a run
/// ficou `ativa` zumbi sem nunca concluir nem errar.
///
/// `read_timeout` é de OCIOSIDADE (dispara se NENHUM byte chega na janela),
/// então não corta um stream longo porém ativo — só o que emperrou de vez;
/// `connect_timeout` cobre o aperto de mão. Ambos configuráveis por env.
fn build_http_client() -> reqwest::Client {
    let secs = |var: &str, default: u64| -> u64 {
        std::env::var(var)
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(default)
    };
    reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(secs(
            "BTV_LLM_CONNECT_TIMEOUT_SECS",
            30,
        )))
        .read_timeout(std::time::Duration::from_secs(secs(
            "BTV_LLM_READ_TIMEOUT_SECS",
            120,
        )))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
}

impl Gateway {
    /// Detecta providers pelas variáveis de ambiente, na ordem da cadeia de
    /// fallback padrão: Anthropic → DeepSeek → OpenAI.
    pub fn from_env() -> Self {
        let candidates = [
            (
                ProviderId::Anthropic,
                "ANTHROPIC_API_KEY",
                anthropic::DEFAULT_BASE_URL,
            ),
            (
                ProviderId::Deepseek,
                "DEEPSEEK_API_KEY",
                openai::DEEPSEEK_BASE_URL,
            ),
            (
                ProviderId::Openai,
                "OPENAI_API_KEY",
                openai::OPENAI_BASE_URL,
            ),
        ];
        let providers = candidates
            .into_iter()
            .filter_map(|(id, env, base)| {
                std::env::var(env)
                    .ok()
                    .filter(|k| !k.is_empty())
                    .map(|api_key| ProviderConfig {
                        id,
                        api_key,
                        base_url: base.to_string(),
                    })
            })
            .collect();
        Self {
            client: build_http_client(),
            providers,
        }
    }

    /// Nomes dos providers disponíveis (para o CLI reportar).
    pub fn available(&self) -> Vec<String> {
        self.providers
            .iter()
            .map(|p| provider_name(&p.id).to_string())
            .collect()
    }

    async fn call_provider(
        &self,
        cfg: &ProviderConfig,
        req: &GenerateRequest,
        on_delta: &mut (dyn FnMut(&str) + Send),
    ) -> Result<AssistantTurn, String> {
        let (url, request) = match cfg.id {
            ProviderId::Anthropic => (
                format!("{}/v1/messages", cfg.base_url),
                self.client
                    .post(format!("{}/v1/messages", cfg.base_url))
                    .header("x-api-key", &cfg.api_key)
                    .header("anthropic-version", anthropic::API_VERSION)
                    .json(&anthropic::build_request_body(req)),
            ),
            _ => (
                format!("{}/v1/chat/completions", cfg.base_url),
                self.client
                    .post(format!("{}/v1/chat/completions", cfg.base_url))
                    .bearer_auth(&cfg.api_key)
                    .json(&openai::build_request_body(req)),
            ),
        };

        let resp = request.send().await.map_err(|e| format!("{url}: {e}"))?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!(
                "{url}: HTTP {status}: {}",
                body.chars().take(300).collect::<String>()
            ));
        }

        let mut parser = SseParser::new();
        let mut stream = resp.bytes_stream();
        match cfg.id {
            ProviderId::Anthropic => {
                let mut agg = anthropic::TurnAggregator::new();
                while let Some(chunk) = stream.next().await {
                    let chunk = chunk.map_err(|e| format!("stream: {e}"))?;
                    for event in parser.push(&chunk) {
                        if let Some(delta) = agg.handle(&event.data) {
                            on_delta(&delta);
                        }
                    }
                }
                Ok(agg.finish())
            }
            _ => {
                let mut agg = openai::TurnAggregator::new(provider_name(&cfg.id));
                while let Some(chunk) = stream.next().await {
                    let chunk = chunk.map_err(|e| format!("stream: {e}"))?;
                    for event in parser.push(&chunk) {
                        if let Some(delta) = agg.handle(&event.data) {
                            on_delta(&delta);
                        }
                    }
                }
                Ok(agg.finish())
            }
        }
    }
}

impl Generator for Gateway {
    async fn generate(
        &self,
        req: GenerateRequest,
        on_delta: &mut (dyn FnMut(&str) + Send),
    ) -> Result<AssistantTurn, GatewayError> {
        if self.providers.is_empty() {
            return Err(GatewayError::NoProvider);
        }
        let mut failures = Vec::new();
        for cfg in &self.providers {
            match self.call_provider(cfg, &req, on_delta).await {
                Ok(turn) => return Ok(turn),
                Err(e) => failures.push(format!("{}: {e}", provider_name(&cfg.id))),
            }
        }
        Err(GatewayError::AllFailed(failures.join(" | ")))
    }
}

fn provider_name(id: &ProviderId) -> &'static str {
    match id {
        ProviderId::Anthropic => "anthropic",
        ProviderId::Deepseek => "deepseek",
        ProviderId::Openai => "openai",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sem_keys_nao_ha_providers() {
        // from_env depende do ambiente; aqui garantimos só a construção
        // vazia → NoProvider (o CI não tem keys definidas).
        let gw = Gateway {
            client: reqwest::Client::new(),
            providers: vec![],
        };
        let err = futures_util::future::FutureExt::now_or_never(gw.generate(
            GenerateRequest {
                model: "x".into(),
                system: String::new(),
                messages: vec![],
                tools: vec![],
                max_tokens: 16,
                temperature: None,
            },
            &mut |_| {},
        ))
        .expect("resolve imediatamente")
        .unwrap_err();
        assert!(matches!(err, GatewayError::NoProvider));
    }

    fn req_simples() -> GenerateRequest {
        GenerateRequest {
            model: "x".into(),
            system: String::new(),
            messages: vec![],
            tools: vec![],
            max_tokens: 16,
            temperature: None,
        }
    }

    /// T3 — falha REAL de transporte (conexão recusada), não mock: um provider
    /// apontado a uma porta fechada vira `AllFailed` (a cadeia de fallback tenta
    /// e reporta), nunca pendura nem entra em pânico.
    #[tokio::test]
    async fn conexao_recusada_vira_all_failed() {
        // Porta livre obtida e liberada na hora → conexão recusada determinística.
        let porta = {
            let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            l.local_addr().unwrap().port()
        };
        let gw = Gateway {
            client: reqwest::Client::new(),
            providers: vec![ProviderConfig {
                id: ProviderId::Anthropic,
                api_key: "k".into(),
                base_url: format!("http://127.0.0.1:{porta}"),
            }],
        };
        let err = gw.generate(req_simples(), &mut |_| {}).await.unwrap_err();
        let GatewayError::AllFailed(msg) = err else {
            panic!("esperava AllFailed em conexão recusada");
        };
        assert!(
            msg.contains("anthropic"),
            "mensagem deve nomear o provider: {msg}"
        );
    }

    /// T3 — provider que ACEITA a conexão mas nunca responde: o timeout do
    /// cliente HTTP corta em vez de esperar para sempre (o bug de produção que
    /// os timeouts do gateway resolveram — squad "ativa" zumbi). Vira `AllFailed`
    /// rápido, sem pendurar.
    #[tokio::test]
    async fn servidor_que_pendura_estoura_timeout_sem_pendurar() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let porta = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            // Aceita e segura o socket sem escrever resposta HTTP.
            if let Ok((sock, _)) = listener.accept().await {
                tokio::time::sleep(std::time::Duration::from_secs(30)).await;
                drop(sock);
            }
        });
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_millis(400))
            .build()
            .unwrap();
        let gw = Gateway {
            client,
            providers: vec![ProviderConfig {
                id: ProviderId::Anthropic,
                api_key: "k".into(),
                base_url: format!("http://127.0.0.1:{porta}"),
            }],
        };
        let inicio = std::time::Instant::now();
        let err = gw.generate(req_simples(), &mut |_| {}).await.unwrap_err();
        assert!(
            inicio.elapsed() < std::time::Duration::from_secs(5),
            "deveria cortar no timeout curto, não esperar o servidor"
        );
        assert!(matches!(err, GatewayError::AllFailed(_)));
    }
}
