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
