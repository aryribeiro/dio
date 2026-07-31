use axum::{extract::FromRequestParts, http::header::AUTHORIZATION};

use crate::{app::AppState, config, error::AppError};

pub struct Admin;

impl FromRequestParts<AppState> for Admin {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        _state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let Some(auth) = parts.headers.get(AUTHORIZATION) else {
            return Err(AppError::MissingAuthorization);
        };

        if auth == config::secrets().admin_secret() {
            Ok(Admin)
        } else {
            Err(AppError::InvalidCredentials)
        }
    }
}
