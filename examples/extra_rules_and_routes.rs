use async_trait::async_trait;
use axum::extract::State;
use axum::routing::get;
use axum::Json;
use mintpool::api::{start_api, AppState};
use mintpool::metadata_rule;
use mintpool::rules::{Evaluation, Rule, RuleContext, RulesEngine};
use mintpool::storage::Reader;
use mintpool::types::{PremintMetadata, PremintTypes};
use reqwest::StatusCode;
use serde_json::Value;
use sqlx::{Executor, Row};
use tokio::signal::unix::{signal, SignalKind};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> eyre::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .pretty()
        .init();

    let config = mintpool::config::init();

    // Add some custom rules in addition to the defaults
    let mut rules = RulesEngine::new_with_default_rules(&config);
    rules.add_rule(metadata_rule!(only_odd_token_ids));
    rules.add_rule(Box::new(MustStartWithA {}));

    let ctl = mintpool::run::start_p2p_services(&config, rules).await?;

    // Add some custom routes in addition to the defaults. You could also add middleware or anything else you can do with axum.
    let mut router = mintpool::api::router_with_defaults().await;
    router = router
        .route("/simple", get(my_simple_route))
        .route("/count", get(query_route));

    start_api(&config, ctl.clone(), router).await?;

    let mut sigint = signal(SignalKind::interrupt())?;
    let mut sigterm = signal(SignalKind::terminate())?;

    tokio::select! {
        _ = sigint.recv() => {
            tracing::info!("Received SIGINT, shutting down");
        }
        _ = sigterm.recv() => {
            tracing::info!("Received SIGTERM, shutting down");
        }
    }
    Ok(())
}

// rules can be made a few different ways

// This is the most basic way to create a rule, implement the Rule trait for a struct
struct MustStartWithA;

#[async_trait]
impl<T: Reader> Rule<T> for MustStartWithA {
    async fn check(
        &self,
        item: &PremintTypes,
        _context: &RuleContext<T>,
    ) -> eyre::Result<Evaluation> {
        match item {
            PremintTypes::ZoraV2(premint) => {
                if premint
                    .collection_address
                    .to_string()
                    .to_lowercase()
                    .starts_with("0xa")
                {
                    Ok(Evaluation::Accept)
                } else {
                    Ok(Evaluation::Reject(
                        "collection address must start with 0xa".to_string(),
                    ))
                }
            }
            _ => Ok(Evaluation::Ignore("not a zora v2 premint".to_string())),
        }
    }

    fn rule_name(&self) -> &'static str {
        "collection address must start with 0xa"
    }
}

// if you only want your rule to act on metadata, you can use the metadata_rule! macro and write a function that takes a PremintMetadata and RuleContext
async fn only_odd_token_ids<T: Reader>(
    metadata: &PremintMetadata,
    _context: &RuleContext<T>,
) -> eyre::Result<Evaluation> {
    if metadata.token_id.to::<u128>() % 2 == 1 {
        Ok(Evaluation::Accept)
    } else {
        Ok(Evaluation::Reject("token id must be odd".to_string()))
    }
}

async fn my_simple_route() -> &'static str {
    "wow so simple"
}

// Routes are just axum routes, so you can use the full power of axum to define them.
// routes can use AppState which gives access to the commands channel, and a db connection for queries
// AppState is connected when `start_api(config, controller, router)` is called.
async fn query_route(State(state): State<AppState>) -> (StatusCode, Json<Value>) {
    let res = state
        .db
        .fetch_one("SELECT count(*) as count FROM premints")
        .await;

    let row = match res {
        Ok(row) => row,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("failed to get count, {}", e)})),
            )
        }
    };

    match row.try_get::<i64, _>("count") {
        Ok(count) => (StatusCode::OK, Json(serde_json::json!({"count": count}))),
        Err(e) => {
            tracing::error!("Failed to get count: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("failed to get count, {}", e)})),
            )
        }
    }
}
