//! Extractor de `TenantContext` na borda (E1s.2, ADR 0029 aceito).
//!
//! PEÇA ÚNICA: a resolução por `BTV_MODE` vive SÓ aqui. Nenhum handler lê
//! `BTV_MODE` nem monta contexto por conta própria — "seis ifs de borda são
//! o SQL-em-handler da autenticação", e a cura é a mesma do T4-B: peça única
//! + juiz mecânico (o lint **T4-D** varre `BTV_MODE` fora deste módulo).
//!
//! - **local** (default): TODO request vira `TenantContext::local` — o
//!   `actor` é `web:btv`, o MESMO valor que os seis consumidores da C3.1
//!   já gravam hoje, para a E1s.3 trocar a FONTE do contexto sem mudar um
//!   byte do wire (goldens verdes). O `web:dashboard` do texto do ADR era
//!   ilustrativo; o contrato é "modo local byte-idêntico".
//! - **saas**: o token de sessão (cookie `btv_session` HttpOnly ou header
//!   `Authorization: Bearer`) resolve, via `SessionResolver` (o `PgStore`
//!   da E1s.1, plugado na E1s.3), para `(tenant, user_id)` →
//!   `TenantContext` com `actor = user:{user_id}` (item 6 do ADR — a trilha
//!   por pessoa dentro do tenant). Sem sessão / inválida = recusa
//!   fail-closed (401), NUNCA fallback para LOCAL.

use axum::extract::{FromRef, FromRequestParts};
use axum::http::request::Parts;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use btv_domain::{ActorId, TenantContext, TenantId};
use std::sync::Arc;

/// Actor do modo local — casado com o valor que os seis handlers da C3.1
/// gravam hoje (`web:btv`), para a troca da fonte na E1s.3 ser byte-idêntica.
const ACTOR_LOCAL: &str = "web:btv";

/// Modo de operação (ADR 0026 item 6). Resolvido de `BTV_MODE` uma vez.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Local,
    Saas,
}

/// Lê `BTV_MODE` — default `local` (o modo do produto local-first). Só
/// `saas` (case-insensitive) liga o modo SaaS; qualquer outro valor é local.
pub fn current_mode() -> Mode {
    match std::env::var("BTV_MODE") {
        Ok(v) if v.eq_ignore_ascii_case("saas") => Mode::Saas,
        _ => Mode::Local,
    }
}

/// Resolve um token de sessão em `(tenant, user_id)`. Implementado pelo
/// `PgStore` da E1s.1 (saas-only) e plugado na borda pela E1s.3; um mock
/// exercita a resolução aqui. `None` = token ausente da tabela, expirado ou
/// revogado (o `resolve_session` já é fail-closed).
pub trait SessionResolver: Send + Sync {
    fn resolve(&self, token: &str) -> Option<(TenantId, String)>;
}

/// Estado que a borda injeta: o resolver de sessões (Some no saas, None no
/// local). `FromRef` para o extractor puxá-lo de qualquer `AppState`.
#[derive(Clone, Default)]
pub struct TenantResolucao(pub Option<Arc<dyn SessionResolver>>);

/// Motivos de recusa da borda — cada um com o seu status HTTP.
#[derive(Debug, PartialEq, Eq)]
pub enum Recusa {
    /// saas sem sessão autenticada → 401 (NUNCA fallback para LOCAL).
    SemSessao,
    /// saas com token que não resolve (ausente/expirado/revogado) → 401.
    SessaoInvalida,
    /// saas sem resolver configurado — erro de deploy (build sem `pg` ou
    /// estado mal montado), não do cliente → 500.
    SaasSemResolver,
}

impl IntoResponse for Recusa {
    fn into_response(self) -> Response {
        match self {
            Recusa::SemSessao => (StatusCode::UNAUTHORIZED, "sessão ausente").into_response(),
            Recusa::SessaoInvalida => (StatusCode::UNAUTHORIZED, "sessão inválida").into_response(),
            Recusa::SaasSemResolver => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "modo saas sem resolver de sessões",
            )
                .into_response(),
        }
    }
}

/// Extrai o token da requisição: cookie `btv_session` (HttpOnly, o caminho
/// de navegador) OU `Authorization: Bearer <token>` (o caminho de API).
/// `None` se nenhum dos dois traz um token não-vazio.
pub(crate) fn extrair_token(headers: &HeaderMap) -> Option<String> {
    if let Some(auth) = headers.get("authorization").and_then(|v| v.to_str().ok()) {
        if let Some(tok) = auth.strip_prefix("Bearer ").map(str::trim) {
            if !tok.is_empty() {
                return Some(tok.to_string());
            }
        }
    }
    let cookies = headers.get("cookie").and_then(|v| v.to_str().ok())?;
    for par in cookies.split(';') {
        let par = par.trim();
        if let Some(tok) = par.strip_prefix("btv_session=") {
            if !tok.is_empty() {
                return Some(tok.to_string());
            }
        }
    }
    None
}

/// O coração da borda (a função que o T4-D protege e os testes exercitam):
/// modo + headers + resolver → `TenantContext` ou `Recusa`. Local nunca
/// recusa (LOCAL implícito); saas nunca faz fallback (fail-closed).
pub(crate) fn resolver_contexto(
    mode: Mode,
    headers: &HeaderMap,
    resolver: Option<&dyn SessionResolver>,
) -> Result<TenantContext, Recusa> {
    match mode {
        Mode::Local => Ok(TenantContext::local(
            ActorId::new(ACTOR_LOCAL).expect("actor local fixo válido"),
        )),
        Mode::Saas => {
            let resolver = resolver.ok_or(Recusa::SaasSemResolver)?;
            let token = extrair_token(headers).ok_or(Recusa::SemSessao)?;
            let (tenant, user_id) = resolver.resolve(&token).ok_or(Recusa::SessaoInvalida)?;
            let actor =
                ActorId::new(format!("user:{user_id}")).map_err(|_| Recusa::SessaoInvalida)?;
            Ok(TenantContext::new(tenant, actor))
        }
    }
}

/// O extractor axum: `Tenant(ctx)` num handler resolve o contexto ANTES do
/// corpo rodar. A E1s.3 troca o `TenantContext::local(...)` fixo dos seis
/// consumidores por este `Tenant`.
pub struct Tenant(pub TenantContext);

impl<S> FromRequestParts<S> for Tenant
where
    TenantResolucao: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let TenantResolucao(resolver) = TenantResolucao::from_ref(state);
        resolver_contexto(current_mode(), &parts.headers, resolver.as_deref())
            .map(Tenant)
            .map_err(IntoResponse::into_response)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Resolver mock: conhece um único token válido.
    struct MockResolver {
        token_ok: String,
        tenant: TenantId,
        user_id: String,
    }
    impl SessionResolver for MockResolver {
        fn resolve(&self, token: &str) -> Option<(TenantId, String)> {
            (token == self.token_ok).then(|| (self.tenant, self.user_id.clone()))
        }
    }

    fn headers_com(k: &'static str, v: &str) -> HeaderMap {
        let mut h = HeaderMap::new();
        h.insert(k, v.parse::<axum::http::HeaderValue>().unwrap());
        h
    }

    #[test]
    fn local_sempre_resolve_no_tenant_local_com_actor_web_btv() {
        let ctx = resolver_contexto(Mode::Local, &HeaderMap::new(), None).unwrap();
        assert_eq!(ctx.tenant, TenantId::LOCAL);
        assert_eq!(
            ctx.actor.as_str(),
            "web:btv",
            "byte-idêntico aos 6 handlers"
        );
        // Local ignora token/resolver — nunca recusa.
        let ctx2 = resolver_contexto(Mode::Local, &headers_com("authorization", "Bearer x"), None)
            .unwrap();
        assert_eq!(ctx2.tenant, TenantId::LOCAL);
    }

    fn resolver_de(tenant: &str, user: &str, token: &str) -> MockResolver {
        MockResolver {
            token_ok: token.into(),
            tenant: TenantId::parse(tenant).unwrap(),
            user_id: user.into(),
        }
    }

    #[test]
    fn saas_com_bearer_valido_vira_user_actor() {
        let r = resolver_de("00000000-0000-0000-0000-00000000e152", "42", "btvs_ok");
        let ctx = resolver_contexto(
            Mode::Saas,
            &headers_com("authorization", "Bearer btvs_ok"),
            Some(&r),
        )
        .unwrap();
        assert_eq!(ctx.tenant, r.tenant);
        assert_eq!(
            ctx.actor.as_str(),
            "user:42",
            "actor = user:{{id}} (item 6)"
        );
    }

    #[test]
    fn saas_com_cookie_valido_tambem_resolve() {
        let r = resolver_de("00000000-0000-0000-0000-00000000e152", "7", "btvs_ck");
        let ctx = resolver_contexto(
            Mode::Saas,
            &headers_com("cookie", "outra=1; btv_session=btvs_ck; z=2"),
            Some(&r),
        )
        .unwrap();
        assert_eq!(ctx.actor.as_str(), "user:7");
    }

    #[test]
    fn saas_sem_token_e_401_nunca_local() {
        let r = resolver_de("00000000-0000-0000-0000-00000000e152", "1", "btvs_ok");
        assert_eq!(
            resolver_contexto(Mode::Saas, &HeaderMap::new(), Some(&r)),
            Err(Recusa::SemSessao),
            "saas sem sessão NUNCA faz fallback para LOCAL"
        );
    }

    #[test]
    fn saas_com_token_forjado_e_401() {
        let r = resolver_de("00000000-0000-0000-0000-00000000e152", "1", "btvs_ok");
        assert_eq!(
            resolver_contexto(
                Mode::Saas,
                &headers_com("authorization", "Bearer btvs_FORJADO"),
                Some(&r),
            ),
            Err(Recusa::SessaoInvalida)
        );
    }

    #[test]
    fn saas_sem_resolver_e_500_erro_de_deploy() {
        assert_eq!(
            resolver_contexto(
                Mode::Saas,
                &headers_com("authorization", "Bearer btvs_ok"),
                None,
            ),
            Err(Recusa::SaasSemResolver)
        );
    }

    #[test]
    fn extrair_token_dos_dois_caminhos_e_vazios() {
        assert_eq!(
            extrair_token(&headers_com("authorization", "Bearer abc")).as_deref(),
            Some("abc")
        );
        assert_eq!(
            extrair_token(&headers_com("cookie", "btv_session=xyz")).as_deref(),
            Some("xyz")
        );
        // Bearer vazio e cookie ausente = None.
        assert_eq!(
            extrair_token(&headers_com("authorization", "Bearer ")),
            None
        );
        assert_eq!(extrair_token(&HeaderMap::new()), None);
    }
}
