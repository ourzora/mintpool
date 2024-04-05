use libp2p::identity;

use crate::config::{ChainInclusionMode, Config};
use crate::controller::{Controller, ControllerInterface};
use crate::p2p::SwarmController;
use crate::rules::RulesEngine;
use crate::storage::PremintStorage;

/// Starts the libp2p swarm, the controller, and the checkers if applicable.
/// Returns an interface for interacting with the controller.
/// All interactions with the controller should be done through `ControllerInterface` for memory safety.
pub async fn start_services(config: &Config) -> eyre::Result<ControllerInterface> {
    let mut bytes = [0u8; 32];
    bytes[0] = config.seed as u8;

    let id_keys = identity::Keypair::ed25519_from_bytes(bytes).unwrap();

    let (event_send, event_recv) = tokio::sync::mpsc::channel(1024);
    let (swrm_cmd_send, swrm_recv) = tokio::sync::mpsc::channel(1024);
    let (ext_cmd_send, ext_cmd_recv) = tokio::sync::mpsc::channel(1024);

    let store = PremintStorage::new(config).await;

    // configure rules
    let mut rules = RulesEngine::new();
    rules.add_default_rules();

    let mut swarm_controller = SwarmController::new(id_keys, config, swrm_recv, event_send);
    let mut controller = Controller::new(swrm_cmd_send, event_recv, ext_cmd_recv, store, rules);
    let controller_interface = ControllerInterface::new(ext_cmd_send);

    let port = config.peer_port;
    let network_ip = config.initial_network_ip();
    tokio::spawn(async move {
        swarm_controller
            .run(port, network_ip)
            .await
            .expect("Swarm controller failed");
    });

    tokio::spawn(async move {
        controller.run_loop().await;
    });

    if config.chain_inclusion_mode == ChainInclusionMode::Check {
        for chain_id in config.supported_chains() {
            let rpc_url = config.rpc_url(chain_id).expect(format!("Failed to get RPC URL for configured chain_id {chain_id}. Set environment variable CHAIN_{chain_id}_RPC_WSS").as_str());
        }
    }

    Ok(controller_interface)
}
