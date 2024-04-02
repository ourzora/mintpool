use crate::config::Config;
use crate::controller::{ControllerCommands, ControllerInterface, DBQuery};
use crate::storage;
use crate::types::PremintTypes;
use aide::axum::ApiRouter;
use aide::openapi::{OpenApi, StatusCode};
use axum::extract::State;
use axum::handler::Handler;
use axum::{Extension, Json};
use sqlx::SqlitePool;
use std::sync::Arc;
use tokio::net::TcpListener;

#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,
}
pub async fn make_router(controller: ControllerInterface) -> ApiRouter<AppState> {
    let router = ApiRouter::new();

    let (snd, recv) = tokio::sync::oneshot::channel();
    controller
        .send_command(ControllerCommands::Query(DBQuery::Direct(snd)))
        .await
        .unwrap();
    let db = recv.await.unwrap().expect("Failed to get db");
    router.with_state(AppState { db })
}

pub async fn start_api(config: &Config, router: ApiRouter) -> eyre::Result<()> {
    let mut api = OpenApi::default();

    let addr = format!("{}:{}", config.initial_network_ip(), config.api_port);
    let listener = TcpListener::bind(addr).await?;
    let router = router.finish_api(&mut api).layer(Extension(Arc::new(api)));

    axum::serve(listener, router.into_make_service()).await?;

    Ok(())
}

async fn list_all(State(state): State<AppState>) -> Result<Json<Vec<PremintTypes>>, StatusCode> {
    storage::list_all(&state.db)
        .await
        .map_err(|| StatusCode::Code(500))
}
