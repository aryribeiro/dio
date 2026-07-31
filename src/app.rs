use axum::Router;
use sqlx::PgPool;
use tokio::net::TcpListener;
use tracing::info;
use tracing_subscriber::{
    Layer, fmt::format::FmtSpan, layer::SubscriberExt, util::SubscriberInitExt,
};

use crate::{config, routes};

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
}

impl AppState {
    async fn new() -> color_eyre::Result<Self> {
        let database_url = std::env::var("DATABASE_URL")?;
        let db = PgPool::connect(&database_url).await?;

        // Aplica as migrações pendentes no boot, para que subir a aplicação em
        // uma máquina nova não exija rodar o sqlx-cli à mão.
        sqlx::migrate!().run(&db).await?;
        info!("Migrations applied");

        Ok(Self { db })
    }
}

pub struct App;

impl App {
    pub async fn start() -> color_eyre::Result<()> {
        let layer = tracing_subscriber::fmt::layer()
            .with_span_events(FmtSpan::NEW)
            .boxed();

        tracing_subscriber::registry().with(layer).init();

        // O `.env` é opcional: em produção as variáveis costumam vir do próprio
        // ambiente, e a ausência do arquivo não deveria derrubar o processo.
        if let Err(err) = dotenvy::dotenv() {
            info!("No .env file loaded: {err}");
        }

        config::init()?;

        let state = AppState::new().await?;

        let listener = TcpListener::bind("0.0.0.0:3000").await?;
        let router = Router::new()
            .nest("/api", routes::api::router())
            .merge(routes::frontend::router())
            .with_state(state);

        info!("Listening on http://localhost:3000");

        axum::serve(listener, router).await?;

        Ok(())
    }
}
