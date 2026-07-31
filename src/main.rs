use crate::app::App;

mod app;
pub mod auth;
pub mod config;
pub mod error;
pub mod format;
pub mod models;
pub mod repository;
pub mod routes;

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;
    App::start().await
}
