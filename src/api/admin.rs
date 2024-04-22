use crate::api::routes::APIResponse;
use crate::api::AppState;
use crate::controller::ControllerCommands;
use crate::p2p::NetworkState;
use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::Response;
use axum::Json;
use serde::Serialize;

pub async fn auth_middleware(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Response {
    let secret = match state.api_secret {
        Some(s) => s,
        None => {
            return Response::builder()
                .status(StatusCode::UNAUTHORIZED)
                .body(Body::new("Unauthorized".to_string()))
                .expect("Invalid response");
        }
    };

    match request.headers().get("Authorization") {
        Some(auth) => {
            if auth.to_str().unwrap_or_default() != secret.as_str() {
                return Response::builder()
                    .status(StatusCode::UNAUTHORIZED)
                    .body(Body::new("Unauthorized. Invalid secret".to_string()))
                    .expect("Invalid response");
            }
        }
        None => {
            return Response::builder()
                .status(StatusCode::UNAUTHORIZED)
                .body(Body::new(
                    "Unauthorized. Set Header Authorization: <secret>".to_string(),
                ))
                .expect("Invalid response");
        }
    };
    next.run(request).await
}

#[derive(serde::Deserialize)]
pub struct PeerRequest {
    peer: String,
}

// should be behind an auth middleware
pub async fn add_peer(
    State(state): State<AppState>,
    Json(request): Json<PeerRequest>,
) -> (StatusCode, Json<APIResponse>) {
    match state
        .controller
        .send_command(ControllerCommands::ConnectToPeer {
            address: request.peer.clone(),
        })
        .await
    {
        Ok(_) => (
            StatusCode::OK,
            Json(APIResponse::Success {
                message: "Peer added".into(),
            }),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(APIResponse::Error {
                message: e.to_string(),
            }),
        ),
    }
}

pub async fn node_info(
    State(state): State<AppState>,
) -> Result<Json<NodeInfoResponse>, StatusCode> {
    let (snd, rcv) = tokio::sync::oneshot::channel();
    match state
        .controller
        .send_command(ControllerCommands::ReturnNetworkState { channel: snd })
        .await
    {
        Ok(_) => match rcv.await {
            Ok(info) => Ok(Json(NodeInfoResponse::from(info))),
            Err(_e) => Err(StatusCode::INTERNAL_SERVER_ERROR),
        },
        Err(_e) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
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