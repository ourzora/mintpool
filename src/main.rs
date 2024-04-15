use clap::Parser;
use mintpool::api;
use mintpool::premints::zora_premint_v2::types::ZoraPremintV2;
use mintpool::run::{start_services, start_watch_chain};
use mintpool::stdin::watch_stdin;
use tokio::signal::unix::{signal, SignalKind};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> eyre::Result<()> {
    let config = mintpool::config::init();

    let subscriber = tracing_subscriber::fmt().with_env_filter(EnvFilter::from_default_env());

    match config.interactive {
        true => subscriber.pretty().try_init(),
        false => subscriber.json().try_init(),
    }
    .expect("Unable to initialize logger");

    tracing::info!("Starting mintpool with config: {:?}", config);

    let ctl = start_services(&config).await?;

    let router = api::make_router(&config, ctl.clone()).await;
    api::start_api(&config, router).await?;

    start_watch_chain::<ZoraPremintV2>(&config, ctl.clone()).await;
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
