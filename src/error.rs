use askama::Template;
use axum::{
    Json,
    http::StatusCode,
    response::{Html, IntoResponse, Response},
};
use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("Credenciais de acesso ausentes")]
    MissingAuthorization,
    #[error("Usuário ou senha inválidos")]
    InvalidCredentials,
    #[error("Este ativo não existe")]
    AssetDoesNotExist,
    #[error("Este usuário não existe")]
    UserDoesNotExist,
    #[error("Este nome de usuário já está em uso")]
    UsernameTaken,
    #[error("Você não possui este ativo na carteira")]
    HoldingDoesNotExist,
    #[error(
        "Você possui apenas {} deste ativo, e tentou vender {}",
        crate::format::quantity(*available),
        crate::format::quantity(*requested)
    )]
    InsufficientQuantity { available: f64, requested: f64 },
    #[error("A quantidade precisa ser um número maior que zero")]
    InvalidQuantity,
    #[error("O nome de usuário deve ter até 40 letras, números, ponto, hífen ou underline")]
    InvalidUsername,
    #[error("A senha precisa ter pelo menos {minimum} caracteres")]
    PasswordTooShort { minimum: usize },
    #[error(transparent)]
    Database(#[from] sqlx::Error),
    #[error(transparent)]
    Template(#[from] askama::Error),
    #[error(transparent)]
    Jwt(#[from] jwt_simple::Error),
}

impl AppError {
    const fn status(&self) -> StatusCode {
        match self {
            Self::UsernameTaken
            | Self::MissingAuthorization
            | Self::InvalidQuantity
            | Self::InvalidUsername
            | Self::PasswordTooShort { .. }
            | Self::InsufficientQuantity { .. } => StatusCode::BAD_REQUEST,
            Self::InvalidCredentials => StatusCode::UNAUTHORIZED,
            Self::AssetDoesNotExist | Self::UserDoesNotExist | Self::HoldingDoesNotExist => {
                StatusCode::NOT_FOUND
            }
            Self::Database(_) | Self::Template(_) | Self::Jwt(_) => {
                StatusCode::INTERNAL_SERVER_ERROR
            }
        }
    }

    /// Mensagem segura para mostrar a quem está usando a aplicação.
    ///
    /// Falhas internas (banco, template, JWT) viram um texto genérico: o detalhe
    /// vai para o log, não para a tela, porque costuma expor a estrutura do
    /// sistema. Os demais erros já são escritos pensando em quem vai ler.
    fn public_message(&self) -> String {
        match self {
            Self::Database(_) | Self::Template(_) | Self::Jwt(_) => {
                "Algo deu errado do nosso lado. Tente novamente em instantes.".to_string()
            }
            other => other.to_string(),
        }
    }
}

#[derive(Serialize)]
pub struct ErrorResponse {
    error: String,
}

#[derive(Template)]
#[template(path = "error.html")]
struct ErrorPage {
    status: u16,
    message: String,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = self.status();
        let message = self.public_message();

        if status.is_server_error() {
            tracing::error!(error = %self, "request failed");
        } else {
            tracing::warn!(error = %self, "request rejected");
        }

        (status, Json(ErrorResponse { error: message })).into_response()
    }
}

/// Envolve um [`AppError`] para que a resposta seja uma página HTML em vez de
/// JSON.
///
/// As rotas do navegador usam este tipo; as rotas de API devolvem `AppError`
/// direto. Assim o mesmo erro serve aos dois formatos sem duplicação.
pub struct HtmlError(pub AppError);

// As conversões são escritas uma a uma, e não com um `impl` genérico sobre
// `Into<AppError>`, porque o genérico colidiria com a conversão reflexiva que a
// própria biblioteca padrão fornece.
impl From<AppError> for HtmlError {
    fn from(error: AppError) -> Self {
        Self(error)
    }
}

impl From<sqlx::Error> for HtmlError {
    fn from(error: sqlx::Error) -> Self {
        Self(AppError::Database(error))
    }
}

impl From<askama::Error> for HtmlError {
    fn from(error: askama::Error) -> Self {
        Self(AppError::Template(error))
    }
}

impl From<jwt_simple::Error> for HtmlError {
    fn from(error: jwt_simple::Error) -> Self {
        Self(AppError::Jwt(error))
    }
}

impl IntoResponse for HtmlError {
    fn into_response(self) -> Response {
        let Self(error) = self;
        let status = error.status();
        let message = error.public_message();

        if status.is_server_error() {
            tracing::error!(error = %error, "page failed");
        } else {
            tracing::warn!(error = %error, "page rejected");
        }

        let page = ErrorPage {
            status: status.as_u16(),
            message,
        };

        match page.render() {
            Ok(html) => (status, Html(html)).into_response(),
            // Se até a página de erro falhar, não dá para renderizar nada:
            // devolve o status cru para não entrar em recursão.
            Err(err) => {
                tracing::error!(error = %err, "failed to render error page");
                status.into_response()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn internal_failures_do_not_leak_details() {
        let error = AppError::Database(sqlx::Error::RowNotFound);
        let message = error.public_message();

        assert!(!message.contains("RowNotFound"));
        assert_eq!(error.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn user_facing_errors_keep_their_message() {
        let error = AppError::InsufficientQuantity {
            available: 2.0,
            requested: 5.0,
        };

        assert!(error.public_message().contains('2'));
        assert!(error.public_message().contains('5'));
        assert_eq!(error.status(), StatusCode::BAD_REQUEST);
    }
}
