// pub mod admin;
pub mod admin;
pub mod routes;

use crate::config::Config;
use crate::controller::{ControllerCommands, ControllerInterface, DBQuery};
use axum::error_handling::HandleErrorLayer;
use axum::http::StatusCode;
use axum::middleware::from_fn_with_state;
use axum::routing::{get, post};
use axum::Router;
use sqlx::SqlitePool;
use std::time::Duration;
use tokio::net::TcpListener;
use tower::buffer::BufferLayer;
use tower::limit::RateLimitLayer;
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
        .route("/health", get(routes::health))
        .route("/list-all", get(routes::list_all))
        .route("/submit-premint", post(routes::submit_premint))
        .layer(
            ServiceBuilder::new()
                .layer(HandleErrorLayer::new(|error: BoxError| async move {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        format!("Unhandled error: {:?}", error),
                    )
                }))
                .layer(BufferLayer::new(10000))
                .layer(RateLimitLayer::new(60, Duration::from_secs(60))),
        )
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
        .route("/admin/submit-premint", post(routes::submit_premint))
        .layer(from_fn_with_state(state, admin::auth_middleware))
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
        );

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
