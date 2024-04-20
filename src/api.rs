use crate::config::Config;
use crate::controller::{ControllerCommands, ControllerInterface, DBQuery};
use crate::rules::Results;
use crate::storage;
use crate::types::PremintTypes;
use axum::error_handling::HandleErrorLayer;
use axum::extract::State;
use axum::http::StatusCode;
use axum::middleware::from_fn_with_state;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Serialize;
use sqlx::SqlitePool;
use std::time::Duration;
use tokio::net::TcpListener;
use tower::{BoxError, ServiceBuilder};

#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,
    pub controller: ControllerInterface,
    pub api_secret: Option<String>,
}

impl AppState {
    pub async fn from(config: &Config, controller: ControllerInterface) -> Self {
        let (snd, recv) = tokio::sync::oneshot::channel();
        controller
            .send_command(ControllerCommands::Query(DBQuery::Direct(snd)))
            .await
            .unwrap();
        let db = recv.await.unwrap().expect("Failed to get db");

        Self {
            db,
            controller,
            api_secret: config.admin_api_secret.clone(),
        }
    }
}

pub fn router_with_defaults(config: &Config) -> Router<AppState> {
    Router::new()
        .route("/health", get(health))
        .route("/list-all", get(list_all))
        .route("/submit-premint", post(submit_premint))
        .layer(
            ServiceBuilder::new()
                .layer(HandleErrorLayer::new(|error: BoxError| async move {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        format!("Unhandled error: {:?}", error),
                    )
                }))
                .layer(tower_http::trace::TraceLayer::new_for_http())
                .layer(tower::timeout::TimeoutLayer::new(Duration::from_secs(10)))
                .layer(tower_http::cors::CorsLayer::new().allow_origin(tower_http::cors::Any))
                .layer(tower_http::compression::CompressionLayer::new().gzip(true)),
        )
}

pub fn with_admin_routes(state: AppState, router: Router<AppState>) -> Router<AppState> {
    let admin = Router::new()
        .route("/admin/node", get(admin::node_info))
        .route("/admin/add-peer", post(admin::add_peer))
        // admin submit premint route is not rate limited (allows for operator to send high volume of premints)
        .route("/admin/submit-premint", post(submit_premint))
        .layer(from_fn_with_state(state, admin::auth_middleware));

    router.merge(admin)
}

pub async fn start_api(
    config: &Config,
    controller: ControllerInterface,
    router: Router<AppState>,
    use_admin_routes: bool,
) -> eyre::Result<()> {
    let app_state = AppState::from(config, controller).await;
    let mut router = router;
    if use_admin_routes {
        router = with_admin_routes(app_state.clone(), router);
    }

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

pub mod admin {
    use crate::api::{APIResponse, AppState};
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
                Err(e) => Err(StatusCode::INTERNAL_SERVER_ERROR),
            },
            Err(e) => Err(StatusCode::INTERNAL_SERVER_ERROR),
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
}
