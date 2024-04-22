use crate::api::AppState;
use crate::controller::ControllerCommands;
use crate::rules::Results;
use crate::storage;
use crate::storage::QueryOptions;
use crate::types::PremintTypes;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde::Serialize;

pub async fn list_all(
    State(state): State<AppState>,
    Query(params): Query<QueryOptions>,
) -> Result<Json<Vec<PremintTypes>>, (StatusCode, String)> {
    match storage::list_all_with_options(&state.db, &params).await {
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

pub async fn get_one(
    State(state): State<AppState>,
    Query(params): Query<QueryOptions>,
) -> Result<Json<PremintTypes>, (StatusCode, String)> {
    match storage::get_one(&state.db, &params).await {
        Ok(premint) => Ok(Json(premint)),
        Err(_e) => {
            tracing::warn!("Failed to get one premint: {:?}", _e);

            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to get one premint".to_string(),
            ))
        }
    }
}

pub async fn health() -> &'static str {
    "OK"
}

/// Route for handling premint submission
pub async fn submit_premint(
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
