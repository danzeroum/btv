//! Guarda de `Origin`/`Host` do dashboard (ADR 0015) — movida de `lib.rs` na
//! C2, código intacto. `origin_allowed`/`trusted_origin_hosts` continuam
//! `pub` no MESMO caminho (`btv_server::origin_allowed`, re-export em
//! `lib.rs`): o agente web (`btv-cli::web_agent`) aplica exatamente a mesma
//! regra na cópia dele da guarda.

use axum::extract::Request;
use axum::http::{header, Method, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Json, Response};

use crate::ErrorBody;

/// Guarda de CSRF/DNS-rebinding local (Fase 7 Onda 1, ADR 0015): qualquer
/// requisição ≠ GET com um `Origin` que não seja localhost recebe 403. Sem
/// `Origin` (curl/CLI) passa — o cabeçalho só existe em requisições de
/// navegador.
///
/// Exceção opt-in para hospedagem atrás de proxy/ingress: hosts extras podem
/// ser liberados por `BTV_TRUSTED_ORIGINS` (ver `trusted_origin_hosts`). VAZIO
/// por padrão → o comportamento é idêntico ao anterior (só localhost).
pub(crate) async fn require_local_origin(req: Request, next: Next) -> Response {
    if req.method() != Method::GET {
        if let Some(origin) = req.headers().get(header::ORIGIN) {
            let origin_str = origin.to_str().unwrap_or("");
            if !origin_allowed(origin_str, &trusted_origin_hosts()) {
                return (
                    StatusCode::FORBIDDEN,
                    Json(ErrorBody::new("forbidden_origin", "origin não permitida")),
                )
                    .into_response();
            }
        }
    }
    next.run(req).await
}

/// Extrai o host de uma `Origin`/URL ou de um host puro:
/// "https://squad.exemplo.cloud:443/x" → "squad.exemplo.cloud". `None` só para
/// string vazia.
fn origin_host(origin: &str) -> Option<&str> {
    let rest = origin
        .strip_prefix("http://")
        .or_else(|| origin.strip_prefix("https://"))
        .unwrap_or(origin);
    let host_port = rest.split('/').next().unwrap_or("");
    let host = host_port
        .rsplit_once(':')
        .map(|(h, _)| h)
        .unwrap_or(host_port);
    (!host.is_empty()).then_some(host)
}

/// `true` para as origins de loopback de navegador (exige esquema http(s)://
/// explícito — um host puro não é `Origin` válida). Comportamento original.
fn is_local_origin(origin: &str) -> bool {
    let has_scheme = origin.starts_with("http://") || origin.starts_with("https://");
    has_scheme
        && matches!(
            origin_host(origin),
            Some("127.0.0.1" | "localhost" | "::1" | "[::1]")
        )
}

/// Núcleo testável da guarda: a `Origin` é aceita se for loopback OU se seu
/// host estiver na lista de confiança `extra`. `pub` para que o agente web
/// (`btv-cli::web_agent`), que tem a sua própria cópia da guarda nas rotas que
/// aprovam execução, aplique EXATAMENTE a mesma regra.
pub fn origin_allowed(origin: &str, extra: &[String]) -> bool {
    if is_local_origin(origin) {
        return true;
    }
    match origin_host(origin) {
        Some(h) => {
            let h = h.to_ascii_lowercase();
            extra.contains(&h)
        }
        None => false,
    }
}

/// Hosts extras confiáveis para requisições mutáveis, lidos de
/// `BTV_TRUSTED_ORIGINS` (lista separada por vírgula; cada item pode ser um
/// host puro `squad.exemplo.cloud` ou uma origin `https://squad.exemplo.cloud`).
/// VAZIO por padrão → só localhost, como antes.
///
/// **Segurança:** afrouxar esta guarda reabre o vetor de CSRF que o ADR 0015
/// fecha. Só use ao hospedar o dashboard atrás de um proxy/ingress **com
/// autenticação na borda** — o dashboard executa código e guarda API keys.
pub fn trusted_origin_hosts() -> Vec<String> {
    std::env::var("BTV_TRUSTED_ORIGINS")
        .ok()
        .into_iter()
        .flat_map(|v| {
            v.split(',')
                .filter_map(|e| origin_host(e.trim()).map(str::to_ascii_lowercase))
                .collect::<Vec<_>>()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn origin_host_extrai_host_de_varias_formas() {
        assert_eq!(
            origin_host("https://squad.exemplo.cloud"),
            Some("squad.exemplo.cloud")
        );
        assert_eq!(
            origin_host("https://squad.exemplo.cloud:443/x"),
            Some("squad.exemplo.cloud")
        );
        assert_eq!(
            origin_host("squad.exemplo.cloud"),
            Some("squad.exemplo.cloud")
        );
        assert_eq!(origin_host(""), None);
    }

    #[test]
    fn origin_allowed_so_localhost_por_padrao() {
        // Sem allowlist: mantém exatamente o comportamento do ADR 0015.
        let vazio: &[String] = &[];
        assert!(origin_allowed("http://localhost:5173", vazio));
        assert!(origin_allowed("http://127.0.0.1:7878", vazio));
        assert!(!origin_allowed("https://squad.exemplo.cloud", vazio));
        assert!(!origin_allowed("https://evil.example", vazio));
    }

    #[test]
    fn origin_allowed_libera_hosts_da_allowlist() {
        let extra = vec!["squad.exemplo.cloud".to_string()];
        assert!(origin_allowed("https://squad.exemplo.cloud", &extra));
        // case-insensitive no host
        assert!(origin_allowed("https://SQUAD.exemplo.CLOUD", &extra));
        // localhost continua valendo mesmo com allowlist
        assert!(origin_allowed("http://localhost", &extra));
        // host fora da lista continua bloqueado
        assert!(!origin_allowed("https://outro.exemplo.cloud", &extra));
    }
}
