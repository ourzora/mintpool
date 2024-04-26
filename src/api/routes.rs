use crate::api::AppState;
use crate::controller::ControllerCommands;
use crate::p2p::NetworkState;
use crate::rules::Results;
use crate::storage;
use crate::storage::{get_for_id_and_kind, QueryOptions};
use crate::types::{PremintName, PremintTypes};
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde::Serialize;
use sqlx::{Executor, Row};

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

pub async fn get_by_id_and_kind(
    State(state): State<AppState>,
    Path((kind, id)): Path<(String, String)>,
) -> Result<Json<PremintTypes>, (StatusCode, String)> {
    match get_for_id_and_kind(&state.db, &id, PremintName(kind)).await {
        Ok(premint) => Ok(Json(premint)),
        Err(_e) => Err((StatusCode::NOT_FOUND, "Failed to get premint".to_string())),
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

pub async fn summary(State(state): State<AppState>) -> Result<Json<SummaryResponse>, StatusCode> {
    let (snd, rcv) = tokio::sync::oneshot::channel();
    match state
        .controller
        .send_command(ControllerCommands::ReturnNetworkState { channel: snd })
        .await
    {
        Ok(_) => match rcv.await {
            Ok(info) => {
                let total = state
                    .db
                    .fetch_one("SELECT COUNT(*) as count FROM premints")
                    .await
                    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
                    .get::<i64, _>("count");
                let active = state
                    .db
                    .fetch_one("SELECT COUNT(*) as count FROM premints WHERE seen_on_chain = false")
                    .await
                    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
                    .get::<i64, _>("count");

                Ok(Json(SummaryResponse {
                    commit_sha: crate::built_info::GIT_COMMIT_HASH_SHORT
                        .unwrap_or_default()
                        .to_string(),
                    pkg_version: crate::built_info::PKG_VERSION.to_string(),
                    active_premint_count: active as u64,
                    total_premint_count: total as u64,
                    node_info: info.into(),
                }))
            }
            Err(_e) => Err(StatusCode::INTERNAL_SERVER_ERROR),
        },
        Err(_e) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

#[derive(Serialize)]
pub struct SummaryResponse {
    pub commit_sha: String,
    pub pkg_version: String,
    pub active_premint_count: u64,
    pub total_premint_count: u64,
    pub node_info: NodeInfoResponse,
}

#[derive(Serialize)]
pub struct NodeInfoResponse {
    pub local_peer_id: String,
    pub num_peers: u64,
    pub dht_peers: Vec<Vec<String>>,
    pub gossipsub_peers: Vec<String>,
    pub all_external_addresses: Vec<Vec<String>>,
}

impl From<NetworkState> for NodeInfoResponse {
    fn from(state: NetworkState) -> Self {
        let NetworkState {
            local_peer_id,
            network_info,
            dht_peers,
            gossipsub_peers,
            all_external_addresses,
            ..
        } = state;
        let dht_peers = dht_peers
            .into_iter()
            .map(|peer| peer.iter().map(|p| p.to_string()).collect())
            .collect();
        let gossipsub_peers = gossipsub_peers.into_iter().map(|p| p.to_string()).collect();
        let all_external_addresses = all_external_addresses
            .into_iter()
            .map(|peer| peer.into_iter().map(|p| p.to_string()).collect())
            .collect();
        Self {
            local_peer_id: local_peer_id.to_string(),
            num_peers: network_info.num_peers() as u64,
            dht_peers,
            gossipsub_peers,
            all_external_addresses,
        }
    }
}
