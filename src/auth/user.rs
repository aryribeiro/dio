use std::convert::Infallible;

use axum::extract::FromRequestParts;
use axum_extra::extract::CookieJar;
use jwt_simple::{
    claims::Claims,
    prelude::{Duration, MACLike},
};
use password_auth::VerifyError;
use serde::{Deserialize, Serialize};

use crate::{app::AppState, config, error::AppError, repository::Repository};

/// Por quanto tempo uma sessão continua válida sem novo login.
pub const SESSION_DURATION_MINS: u64 = 60 * 12;

/// Tamanho mínimo de senha aceito no cadastro.
const MIN_PASSWORD_LEN: usize = 8;

/// Tamanho máximo de nome de usuário, para não gravar lixo no banco.
const MAX_USERNAME_LEN: usize = 40;

pub struct UnauthenticatedUser {
    username: String,
    password: String,
}

impl UnauthenticatedUser {
    pub fn new(username: String, password: String) -> Self {
        Self { username, password }
    }

    /// Confere se as credenciais têm forma aceitável antes de tocar no banco.
    ///
    /// Só se aplica ao cadastro: no login a senha é comparada com o hash
    /// existente, e recusar uma senha antiga por ser curta trancaria a pessoa
    /// para fora da própria conta.
    pub fn validate_for_signup(&self) -> Result<(), AppError> {
        let username = self.username.trim();

        if username.is_empty() || username.len() > MAX_USERNAME_LEN {
            return Err(AppError::InvalidUsername);
        }

        if !username
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '-' || c == '.')
        {
            return Err(AppError::InvalidUsername);
        }

        if self.password.len() < MIN_PASSWORD_LEN {
            return Err(AppError::PasswordTooShort {
                minimum: MIN_PASSWORD_LEN,
            });
        }

        Ok(())
    }

    pub async fn authenticate(&self, repository: &Repository) -> Result<User, AppError> {
        let user_record = match repository.get_user_by_name(&self.username).await? {
            Some(user_record) => user_record,
            None => return Err(AppError::UserDoesNotExist),
        };

        match password_auth::verify_password(&self.password, &user_record.password_hash) {
            Ok(()) => Ok(User::new(user_record.id, user_record.username)),
            Err(VerifyError::PasswordInvalid) => Err(AppError::InvalidCredentials),
            Err(VerifyError::Parse(err)) => panic!("Hashing algorithm failed: {err}"),
        }
    }

    pub async fn register(self, repository: &Repository) -> Result<User, AppError> {
        self.validate_for_signup()?;

        let password_hash = password_auth::generate_hash(&self.password);
        let username = self.username.trim();
        let user_record = match repository.add_user(username, &password_hash).await {
            Ok(user_record) => user_record,
            Err(sqlx::Error::Database(db_err)) if db_err.is_unique_violation() => {
                return Err(AppError::UsernameTaken);
            }
            Err(err) => return Err(AppError::Database(err)),
        };

        Ok(User::new(user_record.id, user_record.username))
    }
}

pub struct User {
    id: i64,
    username: String,
}

impl User {
    fn new(id: i64, username: String) -> Self {
        Self { id, username }
    }

    pub const fn username(&self) -> &String {
        &self.username
    }

    pub const fn id(&self) -> i64 {
        self.id
    }

    pub fn auth_token(self) -> Result<String, AppError> {
        let claims = Claims::with_custom_claims(
            UserClaims::from(self),
            Duration::from_mins(SESSION_DURATION_MINS),
        );
        let token = config::secrets().jwt_key().authenticate(claims)?;
        Ok(token)
    }

    pub fn from_auth_token(token: &str) -> Result<Self, AppError> {
        let claims: UserClaims = config::secrets()
            .jwt_key()
            .verify_token::<UserClaims>(token, None)?
            .custom;
        Ok(Self::new(claims.id, claims.username))
    }
}

impl FromRequestParts<AppState> for User {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        _state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let jar = CookieJar::from_headers(&parts.headers);

        let token = match jar.get("token") {
            Some(token) => token.value(),
            None => return Err(AppError::MissingAuthorization),
        };

        User::from_auth_token(token)
    }
}

impl FromRequestParts<AppState> for Option<User> {
    type Rejection = Infallible;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        Ok(User::from_request_parts(parts, state).await.ok())
    }
}

#[derive(Serialize, Deserialize)]
struct UserClaims {
    id: i64,
    username: String,
}

impl From<User> for UserClaims {
    fn from(User { id, username }: User) -> Self {
        Self { id, username }
    }
}
