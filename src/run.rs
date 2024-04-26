use alloy::hex;
use libp2p::identity;
use libp2p::identity::Keypair;
use std::time::Duration;
use tracing::{info_span, Instrument};

use crate::chain::{get_contract_boot_nodes, MintChecker, MintCheckerResult};
use crate::chain_list::CHAINS;
use crate::config::{BootNodes, ChainInclusionMode, Config};
use crate::controller::{Controller, ControllerCommands, ControllerInterface};
use crate::p2p::SwarmController;
use crate::rules::RulesEngine;
use crate::storage::{PremintStorage, Reader};
use crate::types::Premint;

/// Starts the libp2p swarm, the controller, and the checkers if applicable, then wires them all up.
/// Returns an interface for interacting with the controller.
/// All interactions with the controller should be done through `ControllerInterface` for memory safety.
/// Recommended to use this function when extending mintpool as a library, but if you're feeling bold you can reproduce what its doing.
pub async fn start_p2p_services(
    config: Config,
    rules: RulesEngine<PremintStorage>,
) -> eyre::Result<ControllerInterface> {
    let id_keys = make_keypair(&config)
        .expect("Failed to create keypair, node cannot start. Confirm secret is 32 bytes of hex (0x + 64 hex chars)");
    let (event_send, event_recv) = tokio::sync::mpsc::channel(1024);
    let (swrm_cmd_send, swrm_recv) = tokio::sync::mpsc::channel(1024);
    let (ext_cmd_send, ext_cmd_recv) = tokio::sync::mpsc::channel(1024);

    let store = PremintStorage::new(&config).await;

    let mut swarm_controller = SwarmController::new(id_keys, &config, swrm_recv, event_send);
    let mut controller = Controller::new(
        config.clone(),
        swrm_cmd_send,
        event_recv,
        ext_cmd_recv,
        store,
        rules,
    );
    let controller_interface = ControllerInterface::new(ext_cmd_send);

    let node_info = swarm_controller.node_info();
    tracing::info!(
        "Starting mintpool node with id: {:?}",
        node_info.peer_id.to_string()
    );

    let port = config.peer_port;
    let network_ip = config.initial_network_ip();
    let node_id = config.node_id;

    tokio::spawn(async move {
        let future = swarm_controller.run(port, network_ip);

        match node_id {
            Some(node_id) => future.instrument(info_span!("", "node_id" = node_id)).await,
            None => future.await,
        }
        .expect("Swarm controller failed");
    });

    let node_id = config.node_id;
    tokio::spawn(async move {
        let future = controller.run_loop();

        match node_id {
            Some(node_id) => future.instrument(info_span!("", "node_id" = node_id)).await,
            None => future.await,
        }
    });

    // Connect to initial nodes
    match &config.boot_nodes {
        BootNodes::Chain => {
            tracing::info!("Fetching boot nodes from chain");
            match get_contract_boot_nodes().await {
                Ok(boot_nodes) => {
                    connect_to_boot_nodes(&controller_interface, boot_nodes.clone()).await;
                    tracing::info!(
                        nodes = serde_json::to_string(&boot_nodes).ok(),
                        "Connected to bootnodes!"
                    )
                }
                Err(err) => {
                    tracing::error!(error=err.to_string(), "Failed to get boot nodes from contract, falling back to No boot nodes. Add nodes via interactive mode or admin API.");
                }
            }
        }
        BootNodes::Custom(boot_nodes) => {
            tracing::info!("Connecting to custom boot nodes");
            connect_to_boot_nodes(&controller_interface, boot_nodes.clone()).await;
        }
        BootNodes::None => {
            tracing::info!("Starting with no boot nodes as peers");
        }
    }

    Ok(controller_interface)
}

fn make_keypair(config: &Config) -> eyre::Result<Keypair> {
    let secret_bytes = hex::decode(&config.secret)?;
    let mut bytes = [0u8; 32];
    if secret_bytes.len() < 32 {
        bytes[..secret_bytes.len()].copy_from_slice(&secret_bytes);
    } else if secret_bytes.len() > 32 {
        bytes.copy_from_slice(&secret_bytes[..32]);
    } else {
        bytes.copy_from_slice(&secret_bytes);
    };

    Ok(Keypair::ed25519_from_bytes(bytes)?)
}

async fn connect_to_boot_nodes(ctl: &ControllerInterface, boot_nodes: Vec<String>) {
    for boot_node in boot_nodes {
        if let Err(err) = ctl
            .send_command(ControllerCommands::ConnectToPeer {
                address: boot_node.clone(),
            })
            .await
        {
            tracing::error!(
                error = err.to_string(),
                boot_node = boot_node,
                "Failed to connect to bootnode"
            );
        }
    }
    // TODO: we should probably announce on ConnectToPeer
    tokio::time::sleep(Duration::from_millis(500)).await; // give nodes time to connect.
    if let Err(err) = ctl.send_command(ControllerCommands::AnnounceSelf).await {
        tracing::error!(
            error = err.to_string(),
            "Failed to announce self to boot nodes"
        );
    }
}

// Used to start processes to watch for new mint events onchain
pub async fn start_watch_chain<T: Premint>(config: &Config, controller: ControllerInterface) {
    if config.chain_inclusion_mode == ChainInclusionMode::Check {
        for chain_id in config.supported_chains() {
            let rpc_url = CHAINS.get_rpc_url(chain_id).expect(format!("Failed to get RPC URL for configured chain_id {chain_id}. Set environment variable CHAIN_{chain_id}_RPC_WSS").as_str());

            let checker = MintChecker::new(chain_id, rpc_url, controller.clone());
            tokio::spawn(async move {
                loop {
                    match checker.poll_for_new_mints::<T>().await {
                        Ok(MintCheckerResult::NoFilter) => {
                            tracing::warn!(
                                chain_id = chain_id,
                                "No filter for chain / premint type, skipping checker"
                            );
                            break;
                        }
                        Err(err) => {
                            tracing::error!(
                                error = err.to_string(),
                                chain_id = chain_id,
                                "checker failed"
                            );
                        }
                    }
                }
            });
            tracing::info!(chain_id = chain_id, "Started watching for premints onchain")
        }
    }
}
