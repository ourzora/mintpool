use crate::config::Config;
use crate::controller::{ControllerCommands, ControllerInterface, DBQuery};
use crate::storage;
use crate::types::PremintTypes;
use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use sqlx::SqlitePool;
use tokio::net::TcpListener;

#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,
    pub controller: ControllerInterface,
}
pub async fn make_router(_config: &Config, controller: ControllerInterface) -> Router {
    let (snd, recv) = tokio::sync::oneshot::channel();
    controller
        .send_command(ControllerCommands::Query(DBQuery::Direct(snd)))
        .await
        .unwrap();
    let db = recv.await.unwrap().expect("Failed to get db");
    Router::new()
        .route("/health", get(health))
        .route("/list-all", get(list_all))
        .route("/submit-premint", post(submit_premint))
        .with_state(AppState { db, controller })
}

pub async fn start_api(config: &Config, router: Router) -> eyre::Result<()> {
    let addr = format!("{}:{}", config.initial_network_ip(), config.api_port);
    let listener = TcpListener::bind(addr.clone()).await.unwrap();

    tracing::info!(address = addr, "Starting API server");
    tokio::spawn(async move {
        axum::serve(listener, router)
            .await
            .expect("API Server failed");
    });
    Ok(())
}

async fn list_all(
    State(state): State<AppState>,
) -> Result<Json<Vec<PremintTypes>>, (StatusCode, String)> {
    match storage::list_all(&state.db).await {
        Ok(premints) => Ok(Json(premints)),
        Err(_e) => {
            tracing::warn!("Failed to list all premints: {:?}", _e);

            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to list all premints".to_string(),
            ))
        }
    }
}

async fn health() -> &'static str {
    "OK"
}

async fn submit_premint(
    State(state): State<AppState>,
    Json(premint): Json<PremintTypes>,
) -> (StatusCode, String) {
    match state
        .controller
        .send_command(ControllerCommands::Broadcast { message: premint })
        .await
    {
        Ok(()) => (StatusCode::OK, "Premint submitted".to_string()),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    }
}
