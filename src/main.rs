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

    let mut swarm_controller = mintpool::p2p::make_swarm_controller(id_keys)?;

    let port = 7000 + cli_opts.seed;

    swarm_controller.run(port).await
}

#[derive(Parser, Debug)]
struct Cli {
    seed: u64,
}
