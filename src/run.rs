use crate::config::Config;
use crate::controller::{Controller, ControllerInterface};
use crate::p2p::make_swarm_controller;
use libp2p::identity;

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
    tokio::spawn(async move {
        swarm_controller
            .run(port)
            .await
            .expect("Swarm controller failed");
    });

    tokio::spawn(async move {
        controller.run_loop().await;
    });

    Ok(controller_interface)
}
