use askama::Template;
use axum::{
    Form, Router,
    extract::Query,
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
};
use axum_extra::extract::{
    CookieJar,
    cookie::{Cookie, SameSite},
};
use serde::Deserialize;
use time::Duration;

use crate::{
    app::AppState,
    auth::user::{SESSION_DURATION_MINS, UnauthenticatedUser, User},
    error::{AppError, HtmlError},
    models::{Asset, Wallet},
    repository::Repository,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(dashboard))
        .route("/login", get(login_page).post(login))
        .route("/logout", post(logout))
        .route("/carteira/comprar", post(buy))
        .route("/carteira/vender", post(sell))
}

const SESSION_COOKIE: &str = "token";

/// Monta o cookie de sessão com as proteções que o projeto base não tinha:
/// não é legível por JavaScript, não viaja em requisições vindas de outros
/// sites, e expira junto com o token que carrega.
fn session_cookie(token: String) -> Cookie<'static> {
    Cookie::build((SESSION_COOKIE, token))
        .http_only(true)
        .same_site(SameSite::Lax)
        .path("/")
        .max_age(Duration::minutes(SESSION_DURATION_MINS as i64))
        .build()
}

// -- Login --------------------------------------------------------------------

#[derive(Template)]
#[template(path = "login.html")]
struct LoginPage;

async fn login_page() -> Result<Html<String>, HtmlError> {
    let html = LoginPage.render()?;
    Ok(Html(html))
}

#[derive(Deserialize)]
struct LoginForm {
    username: String,
    password: String,
}

async fn login(
    repository: Repository,
    jar: CookieJar,
    Form(request): Form<LoginForm>,
) -> Result<impl IntoResponse, HtmlError> {
    let unauth_user = UnauthenticatedUser::new(request.username, request.password);

    // Quem ainda não tem conta é cadastrado na hora — é o fluxo do projeto base,
    // mantido para não pedir dois formulários em uma aplicação de estudo.
    let user = match unauth_user.authenticate(&repository).await {
        Ok(user) => user,
        Err(AppError::UserDoesNotExist) => unauth_user.register(&repository).await?,
        Err(other_err) => return Err(other_err.into()),
    };

    let token = user.auth_token()?;

    Ok((jar.add(session_cookie(token)), Redirect::to("/")))
}

async fn logout(jar: CookieJar) -> impl IntoResponse {
    // `remove` só apaga o cookie no navegador se o caminho bater com o que foi
    // usado na criação.
    let expired = Cookie::build(SESSION_COOKIE).path("/").build();

    (jar.remove(expired), Redirect::to("/login"))
}

// -- Dashboard ----------------------------------------------------------------

/// Aviso mostrado no topo do dashboard depois de uma ação.
///
/// Vem da URL como um código fixo, e não como texto livre, para que ninguém
/// consiga montar um link que exibe a mensagem que quiser na página.
#[derive(Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Notice {
    CompraRegistrada,
    VendaRegistrada,
    PosicaoEncerrada,
}

impl Notice {
    const fn message(self) -> &'static str {
        match self {
            Self::CompraRegistrada => "Compra registrada na sua carteira.",
            Self::VendaRegistrada => "Venda registrada na sua carteira.",
            Self::PosicaoEncerrada => "Posição encerrada: você vendeu todo o ativo.",
        }
    }

    const fn slug(self) -> &'static str {
        match self {
            Self::CompraRegistrada => "compra_registrada",
            Self::VendaRegistrada => "venda_registrada",
            Self::PosicaoEncerrada => "posicao_encerrada",
        }
    }
}

#[derive(Deserialize)]
struct DashboardQuery {
    #[serde(default)]
    aviso: Option<Notice>,
}

#[derive(Template)]
#[template(path = "dashboard.html")]
struct DashboardPage {
    username: String,
    wallet: Wallet,
    assets: Vec<Asset>,
    notice: Option<&'static str>,
}

#[tracing::instrument(skip_all)]
async fn dashboard(
    maybe_user: Option<User>,
    repository: Repository,
    Query(query): Query<DashboardQuery>,
) -> Result<Response, HtmlError> {
    let Some(user) = maybe_user else {
        return Ok(Redirect::to("/login").into_response());
    };

    let wallet = repository.get_wallet(user.id()).await?;
    let assets = repository.list_assets().await?;

    let page = DashboardPage {
        username: user.username().clone(),
        wallet,
        assets,
        notice: query.aviso.map(Notice::message),
    };

    Ok(Html(page.render()?).into_response())
}

// -- Compra e venda -----------------------------------------------------------

#[derive(Deserialize)]
struct TradeForm {
    asset_id: i64,
    quantity: f64,
}

impl TradeForm {
    /// Rejeita quantidades que não fazem sentido antes de chegar ao banco.
    ///
    /// O `CHECK` da tabela já barraria valores negativos, mas o erro do
    /// Postgres não é uma mensagem que dê para mostrar a alguém.
    fn validated_quantity(&self) -> Result<f64, AppError> {
        if self.quantity.is_finite() && self.quantity > 0.0 {
            Ok(self.quantity)
        } else {
            Err(AppError::InvalidQuantity)
        }
    }
}

#[tracing::instrument(skip_all)]
async fn buy(
    user: User,
    repository: Repository,
    Form(request): Form<TradeForm>,
) -> Result<Redirect, HtmlError> {
    let quantity = request.validated_quantity()?;

    repository
        .buy_asset(user.id(), request.asset_id, quantity)
        .await?;

    Ok(redirect_with(Notice::CompraRegistrada))
}

#[tracing::instrument(skip_all)]
async fn sell(
    user: User,
    repository: Repository,
    Form(request): Form<TradeForm>,
) -> Result<Redirect, HtmlError> {
    let quantity = request.validated_quantity()?;

    let remaining = repository
        .sell_asset(user.id(), request.asset_id, quantity)
        .await?;

    let notice = if remaining.is_some() {
        Notice::VendaRegistrada
    } else {
        Notice::PosicaoEncerrada
    };

    Ok(redirect_with(notice))
}

/// Redireciona de volta ao dashboard depois de uma ação que alterou dados.
///
/// Evita que recarregar a página repita a compra ou a venda.
fn redirect_with(notice: Notice) -> Redirect {
    Redirect::to(&format!("/?aviso={}", notice.slug()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn trade(quantity: f64) -> TradeForm {
        TradeForm {
            asset_id: 1,
            quantity,
        }
    }

    #[test]
    fn accepts_positive_quantity() {
        assert_eq!(trade(1.5).validated_quantity().expect("válida"), 1.5);
    }

    #[test]
    fn rejects_zero_and_negative_quantities() {
        assert!(trade(0.0).validated_quantity().is_err());
        assert!(trade(-1.0).validated_quantity().is_err());
    }

    #[test]
    fn rejects_nan_and_infinity() {
        assert!(trade(f64::NAN).validated_quantity().is_err());
        assert!(trade(f64::INFINITY).validated_quantity().is_err());
    }

    #[test]
    fn every_notice_has_a_stable_slug() {
        for notice in [
            Notice::CompraRegistrada,
            Notice::VendaRegistrada,
            Notice::PosicaoEncerrada,
        ] {
            let slug = notice.slug();
            let parsed: Notice = serde_json::from_value(serde_json::json!(slug))
                .expect("o slug precisa ser desserializável de volta");

            assert!(
                parsed == notice,
                "slug {slug} não voltou para o mesmo aviso"
            );
            assert!(!notice.message().is_empty());
        }
    }
}
