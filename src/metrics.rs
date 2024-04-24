use crate::api::AppState;
use crate::config::Config;
use axum::body::Body;
use axum::extract::State;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use opentelemetry_sdk::metrics::SdkMeterProvider;
use prometheus::{Encoder, TextEncoder};
use tracing_opentelemetry::MetricsLayer;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{fmt, EnvFilter};

/// Returns a router for prometheus metrics
pub fn init_metrics_and_logging(config: &Config) -> Router<AppState> {
    let pregistry = prometheus::Registry::new();

    let exporter = opentelemetry_prometheus::exporter()
        .with_namespace("mintpool")
        .with_registry(pregistry.clone())
        .build()
        .expect("Failed to create Prometheus exporter");

    let metrics_provider = SdkMeterProvider::builder().with_reader(exporter).build();

    let opentelemetry_metrics = MetricsLayer::new(metrics_provider);

    let registry = tracing_subscriber::Registry::default()
        .with(EnvFilter::from_default_env())
        .with(opentelemetry_metrics);

    match config.interactive {
        true => registry
            .with(fmt::layer().pretty())
            .try_init()
            .expect("Unable to initialize logger"),

        false => registry
            .with(fmt::layer().json())
            .try_init()
            .expect("Unable to initialize logger"),
    };

    Router::new()
        .route("/metrics", get(metrics_route))
        .with_state(pregistry.clone())
}

async fn metrics_route(State(registry): State<prometheus::Registry>) -> impl IntoResponse {
    let mut buffer = vec![];
    let encoder = TextEncoder::new();
    let metric_families = registry.gather();
    encoder.encode(&metric_families, &mut buffer).unwrap();
    Response::new(Body::from(buffer)).into_response()
}
