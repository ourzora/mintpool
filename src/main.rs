use clap::Parser;
use libp2p::identity;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> eyre::Result<()> {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .pretty()
        .try_init();

    let cli_opts = Cli::parse();
    let mut bytes = [0u8; 32];
    bytes[0] = cli_opts.seed as u8;

    let id_keys = identity::Keypair::ed25519_from_bytes(bytes).unwrap();

    let (sender, receiver) = tokio::sync::mpsc::channel(32);

    let mut swarm_controller = mintpool::p2p::make_swarm_controller(id_keys, receiver)?;

    let controller = mintpool::controller::Controller::new(sender);

    let port = 7000 + cli_opts.seed;

    tokio::spawn(async move {
        swarm_controller
            .run(port)
            .await
            .expect("Swarm controller failed");
    });

    controller.run_loop().await;
    Ok(())
}

#[derive(Parser, Debug)]
struct Cli {
    seed: u64,
}
