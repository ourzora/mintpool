use clap::Parser;
use mintpool::run::start_swarm_and_controller;
use mintpool::stdin::watch_stdin;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> eyre::Result<()> {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .pretty()
        .try_init();

    let config = mintpool::config::init();

    tracing::info!("Starting mintpool with config: {:?}", config);

    let ctl = start_swarm_and_controller(&config)?;
    watch_stdin(ctl).await;

    Ok(())
}

#[derive(Parser, Debug)]
struct Cli {
    seed: u64,
}
