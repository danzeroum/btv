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

use axum::extract::{FromRef, FromRequestParts, Request, State};
use axum::http::request::Parts;
use axum::http::{HeaderMap, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use btv_domain::{ActorId, TenantContext, TenantId};
use std::sync::Arc;

/// Actor do modo local — casado com o valor que os seis handlers da C3.1
/// gravam hoje (`web:btv`), para a troca da fonte na E1s.3 ser byte-idêntica.
const ACTOR_LOCAL: &str = "web:btv";

/// Modo de operação (ADR 0026 item 6). INVARIANTE: resolvido de `BTV_MODE`
/// UMA vez no arranque (na construção do `TenantResolucao`) e injetado na
/// borda — o modo é propriedade do PROCESSO, não da requisição. Ler o modo
/// por-request (a forma da E1s.3) diria implicitamente que ele PODE alternar
/// local↔saas no meio da vida do processo — e uma requisição escorregando
/// durante a transição atravessaria sem sessão; um risco de segurança, não
/// flexibilidade. A E1s.4 tornou a invariante expressa no tipo: o `Mode` mora
/// no estado injetado, não numa env lida a cada chamada.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Local,
    Saas,
}

/// Lê `BTV_MODE` — default `local` (o modo do produto local-first). Só
/// `saas` (case-insensitive) liga o modo SaaS; qualquer outro valor é local.
/// Chamado UMA vez, por quem monta o router (`TenantResolucao::from_env`);
/// nunca no caminho por-request. É a única leitura de `BTV_MODE` do binário
/// (o lint T4-D vigia essa fronteira).
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

/// Estado que a borda injeta: o modo (resolvido UMA vez no arranque — o modo é
/// propriedade do PROCESSO, não da requisição) + o resolver de sessões (Some
/// no saas, None no local). `FromRef` para o extractor e o layer o puxarem de
/// qualquer `AppState`. Construído por `from_env` em produção (uma leitura de
/// `BTV_MODE`) e por `new`/`local` nos testes (modo explícito, sem env — o que
/// deixa a varredura saas da E1s.4 determinística sem mutar a env do processo).
#[derive(Clone)]
pub struct TenantResolucao {
    mode: Mode,
    resolver: Option<Arc<dyn SessionResolver>>,
}

impl TenantResolucao {
    /// Produção: resolve o modo da env UMA vez (quem monta o router chama
    /// isto), e recebe o resolver (None hoje/local; `Some(Arc::new(PgStore))`
    /// na onda saas). É o único ponto que chama `current_mode()` fora de teste.
    pub fn from_env(resolver: Option<Arc<dyn SessionResolver>>) -> Self {
        Self {
            mode: current_mode(),
            resolver,
        }
    }

    /// Injeção explícita de modo (só testes/goldens: produção usa `from_env`):
    /// sem tocar a env. A E1s.4 monta o router real em `Saas` por aqui; os
    /// goldens montam em `Local` — a varredura fica determinística no mesmo
    /// processo dos goldens, sem `set_var`.
    #[cfg(test)]
    pub(crate) fn new(mode: Mode, resolver: Option<Arc<dyn SessionResolver>>) -> Self {
        Self { mode, resolver }
    }

    /// Atalho para o modo local sem resolver — o que os goldens usam.
    /// Determinístico: não lê a env.
    #[cfg(test)]
    pub(crate) fn local() -> Self {
        Self::new(Mode::Local, None)
    }
}

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

/// Allowlist NOMEADA de rotas alcançáveis SEM sessão no modo saas — as
/// "exceções nomeadas" da cobertura universal (E1s.3). VAZIA hoje: não há
/// endpoint de login HTTP ainda (a E1s.1 entregou só o `btv session issue`,
/// ato de OPERADOR), então no modo saas TODA rota exige sessão válida — a
/// superfície inteira nasce fail-closed. É a resposta à pergunta da cobertura:
/// o modo saas fica de fato INABILITÁVEL pela borda (401 em tudo), mas pelo
/// MECANISMO REAL da borda, não por uma recusa de arranque — e abre
/// rota-a-rota, nomeada, à medida que a E1s construir o login e o resto do
/// fluxo. A primeira entrada aqui será a rota de login.
pub(crate) const ROTAS_LIVRES: &[&str] = &[];

/// A decisão da borda UNIVERSAL (o que o `guarda_tenant` aplica a TODA rota,
/// função pura testável sem env, como o `resolver_contexto`): local nunca
/// recusa; saas deixa passar só as `ROTAS_LIVRES` sem sessão e exige sessão
/// válida em todo o resto. Reusa `resolver_contexto` — a MESMA decisão de
/// auth dos seis handlers, agora estendida à superfície inteira.
pub(crate) fn autoriza_borda(
    mode: Mode,
    path: &str,
    headers: &HeaderMap,
    resolver: Option<&dyn SessionResolver>,
) -> Result<(), Recusa> {
    if mode == Mode::Saas && ROTAS_LIVRES.contains(&path) {
        return Ok(());
    }
    resolver_contexto(mode, headers, resolver).map(|_| ())
}

/// Layer de cobertura UNIVERSAL (E1s.3, ADR 0029): o extractor `Tenant` só
/// guarda as rotas que o DECLARAM (os seis consumidores estrangulados); este
/// middleware fecha o RESTO da superfície (personas/templates/users/... ainda
/// não estranguladas) para que o modo saas não nasça com a maioria das rotas
/// sem borda. Em modo local é no-op (passa direto) — a composição local fica
/// byte-idêntica. Montado em `web_agent::merged_router` ao lado da guarda de
/// `Origin`. O modo vem do estado INJETADO (resolvido no arranque), não de uma
/// env por-request (E1s.4 — invariante de processo expressa no tipo).
pub(crate) async fn guarda_tenant(
    State(tr): State<TenantResolucao>,
    req: Request,
    next: Next,
) -> Response {
    match autoriza_borda(
        tr.mode,
        req.uri().path(),
        req.headers(),
        tr.resolver.as_deref(),
    ) {
        Ok(()) => next.run(req).await,
        Err(recusa) => recusa.into_response(),
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
        let tr = TenantResolucao::from_ref(state);
        // Modo do estado injetado (resolvido no arranque), não da env por
        // request — mesma invariante de processo do layer (E1s.4).
        resolver_contexto(tr.mode, &parts.headers, tr.resolver.as_deref())
            .map(Tenant)
            .map_err(IntoResponse::into_response)
    }
}

/// O adapter saas da E1s.3: o `PgStore` (da E1s.1) vira o `SessionResolver`
/// que a borda pluga no modo saas. Orphan rule OK — o trait é local a este
/// crate. `resolve_session` já é fail-closed (None em ausente/expirado/
/// revogado); aqui um erro de storage também vira None (a borda responde
/// 401, nunca vaza contexto). É a costura de wiring: no modo saas o
/// `TenantResolucao` carrega `Some(Arc::new(PgStore))` — hoje `main.rs`
/// injeta `None` (local), a onda saas troca a fonte sem tocar a borda.
#[cfg(feature = "pg")]
impl SessionResolver for btv_store::pg::PgStore {
    fn resolve(&self, token: &str) -> Option<(TenantId, String)> {
        match self.resolve_session(token) {
            Ok(Some(s)) => Some((s.tenant, s.user_id)),
            Ok(None) | Err(_) => None,
        }
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
    fn borda_universal_local_deixa_qualquer_rota_passar_sem_sessao() {
        // Modo local: a borda universal é no-op — nenhuma rota recusa, com ou
        // sem token. É o que garante a composição local byte-idêntica.
        assert_eq!(
            autoriza_borda(Mode::Local, "/api/btv/templates", &HeaderMap::new(), None),
            Ok(())
        );
        assert_eq!(
            autoriza_borda(Mode::Local, "/api/personas", &HeaderMap::new(), None),
            Ok(())
        );
    }

    #[test]
    fn borda_universal_saas_recusa_rota_nao_estrangulada_sem_sessao() {
        // O coração da cobertura universal: uma rota que NÃO declara o
        // extractor (ex.: templates/personas) é recusada na borda no modo saas
        // sem sessão — senão o saas nasceria com a superfície sem borda.
        let r = resolver_de("00000000-0000-0000-0000-00000000e153", "9", "btvs_ok");
        assert_eq!(
            autoriza_borda(
                Mode::Saas,
                "/api/btv/templates",
                &HeaderMap::new(),
                Some(&r)
            ),
            Err(Recusa::SemSessao),
            "rota não estrangulada exige sessão no saas (cobertura universal)"
        );
        // Com sessão válida, a mesma rota passa.
        assert_eq!(
            autoriza_borda(
                Mode::Saas,
                "/api/btv/templates",
                &headers_com("authorization", "Bearer btvs_ok"),
                Some(&r),
            ),
            Ok(())
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
