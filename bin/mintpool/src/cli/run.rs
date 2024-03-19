use super::config::Config;

use libp2p::identity;
use mintpool_controller::controller::{Controller, ControllerInterface};
use mintpool_controller::p2p::make_swarm_controller;

/// Starts the libp2p swarm and the controller, returns an interface for interacting with the controller.
/// All interactions with the controller should be done through `ControllerInterface` for memory safety.
pub fn start_swarm_and_controller(config: &Config) -> eyre::Result<ControllerInterface> {
    let mut bytes = [0u8; 32];
    bytes[0] = config.seed as u8;

    let id_keys = identity::Keypair::ed25519_from_bytes(bytes).unwrap();

    let (event_send, event_recv) = tokio::sync::mpsc::channel(1024);
    let (swrm_cmd_send, swrm_recv) = tokio::sync::mpsc::channel(1024);

    let (ext_cmd_send, ext_cmd_recv) = tokio::sync::mpsc::channel(1024);

    let mut swarm_controller = make_swarm_controller(id_keys, swrm_recv, event_send)?;
    let mut controller = Controller::new(swrm_cmd_send, event_recv, ext_cmd_recv);
    let controller_interface = ControllerInterface::new(ext_cmd_send);

    let port = config.port;
    let network_ip = config.network_ip();
    tokio::spawn(async move {
        swarm_controller
            .run(port, network_ip)
            .await
            .expect("Swarm controller failed");
    });

    tokio::spawn(async move {
        controller.run_loop().await;
    });

    Ok(controller_interface)
}
