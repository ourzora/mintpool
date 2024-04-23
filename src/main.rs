use clap::Parser;
use mintpool::api;
use mintpool::metrics::init_metrics_and_logging;
use mintpool::premints::zora_premint::v2::ZoraPremintV2;
use mintpool::rules::RulesEngine;
use mintpool::run::{start_p2p_services, start_watch_chain};
use mintpool::stdin::watch_stdin;
use tokio::signal::unix::{signal, SignalKind};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> eyre::Result<()> {
    let config = mintpool::config::init();

    let metrics_router = init_metrics_and_logging(&config);

    tracing::info!("Starting mintpool with config: {:?}", config);

    let mut rules = RulesEngine::new(&config);
    rules.add_default_rules();
    let ctl = start_p2p_services(config.clone(), rules).await?;

    let router = api::router_with_defaults(&config).merge(metrics_router);
    api::start_api(&config, ctl.clone(), router, true).await?;

    start_watch_chain::<ZoraPremintV2>(&config, ctl.clone()).await;
    tracing::info!(monotonic_counter.chains_watched = 1, "Watching chain");
    if config.interactive {
        watch_stdin(ctl.clone()).await;
    } else {
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
    }

    Ok(())
}

#[derive(Parser, Debug)]
struct Cli {
    seed: u64,
}
