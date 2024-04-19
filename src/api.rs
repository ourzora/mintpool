use crate::config::Config;
use crate::controller::{ControllerCommands, ControllerInterface, DBQuery};
use crate::rules::Results;
use crate::storage;
use crate::types::PremintTypes;
use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Serialize;
use sqlx::SqlitePool;
use tokio::net::TcpListener;

#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,
    pub controller: ControllerInterface,
}

impl AppState {
    pub async fn from(controller: ControllerInterface) -> Self {
        let (snd, recv) = tokio::sync::oneshot::channel();
        controller
            .send_command(ControllerCommands::Query(DBQuery::Direct(snd)))
            .await
            .unwrap();
        let db = recv.await.unwrap().expect("Failed to get db");
        Self { db, controller }
    }
}

pub async fn router_with_defaults() -> Router<AppState> {
    Router::new()
        .route("/health", get(health))
        .route("/list-all", get(list_all))
        .route("/submit-premint", post(submit_premint))
        .route("/submit-premintz", post(submit_premint))
}

pub async fn start_api(
    config: &Config,
    controller: ControllerInterface,
    router: Router<AppState>,
) -> eyre::Result<()> {
    let app_state = AppState::from(controller).await;
    let router = router.with_state(app_state);
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
) -> (StatusCode, Json<APIResponse>) {
    let (snd, recv) = tokio::sync::oneshot::channel();
    match state
        .controller
        .send_command(ControllerCommands::Broadcast {
            message: premint,
            channel: snd,
        })
        .await
    {
        Ok(()) => match recv.await {
            Ok(Ok(_)) => (
                StatusCode::OK,
                Json(APIResponse::Success {
                    message: "Premint submitted".to_string(),
                }),
            ),
            Ok(Err(e)) => match e.downcast_ref::<Results>() {
                Some(res) => (
                    StatusCode::BAD_REQUEST,
                    Json(APIResponse::RulesError {
                        evaluation: res.clone(),
                    }),
                ),
                None => (
                    StatusCode::BAD_REQUEST,
                    Json(APIResponse::Error {
                        message: e.to_string(),
                    }),
                ),
            },
            Err(e) => {
                tracing::warn!("Failed to submit premint: {:?}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(APIResponse::Error {
                        message: e.to_string(),
                    }),
                )
            }
        },
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(APIResponse::Error {
                message: e.to_string(),
            }),
        ),
    }
}

#[derive(Serialize)]
pub enum APIResponse {
    RulesError { evaluation: Results },
    Error { message: String },
    Success { message: String },
}
