use crate::p2p::NetworkState;
use crate::types::{MintpoolNodeInfo, Premint};
use tokio::select;
use tokio::sync::{mpsc, oneshot};

#[derive(Debug)]
pub enum SwarmCommand {
    ConnectToPeer {
        address: String,
    },
    ReturnNetworkState {
        channel: oneshot::Sender<NetworkState>,
    },
    AnnounceSelf,
    Broadcast {
        message: Premint,
    },
    ReturnNodeInfo {
        channel: oneshot::Sender<MintpoolNodeInfo>,
    },
}

pub enum P2PEvent {
    NetworkState(NetworkState),
    PremintReceived(Premint),
}

pub enum ControllerCommands {
    ConnectToPeer {
        address: String,
    },
    ReturnNetworkState {
        channel: oneshot::Sender<NetworkState>,
    },
    AnnounceSelf,
    Broadcast {
        message: Premint,
    },
    ReturnNodeInfo {
        channel: oneshot::Sender<MintpoolNodeInfo>,
    },
}

pub struct Controller {
    swarm_command_sender: mpsc::Sender<SwarmCommand>,
    swarm_event_receiver: mpsc::Receiver<P2PEvent>,
    external_commands: mpsc::Receiver<ControllerCommands>,
}

impl Controller {
    pub fn new(
        swarm_command_sender: mpsc::Sender<SwarmCommand>,
        swarm_event_receiver: mpsc::Receiver<P2PEvent>,
        external_commands: mpsc::Receiver<ControllerCommands>,
    ) -> Self {
        Self {
            swarm_command_sender,
            swarm_event_receiver,
            external_commands,
        }
    }

    pub async fn run_loop(&mut self) {
        loop {
            select! {
                Some(command) = self.external_commands.recv() => {
                    if let Err(err) = self.handle_command(command).await {
                        tracing::error!("Error handling command to controller: {:?}", err);
                    };
                }
                Some(event) = self.swarm_event_receiver.recv() => {
                    self.handle_event(event).await;
                }
            }
        }
    }

    async fn handle_event(&self, event: P2PEvent) {
        match event {
            P2PEvent::NetworkState(network_state) => {
                tracing::info!("Current network state: {:?}", network_state);
            }
            P2PEvent::PremintReceived(premint) => {
                tracing::info!("Received premint: {:?}", premint);
            }
        }
    }

    async fn handle_command(&mut self, command: ControllerCommands) -> eyre::Result<()> {
        match command {
            ControllerCommands::ConnectToPeer { address } => {
                self.swarm_command_sender
                    .send(SwarmCommand::ConnectToPeer { address })
                    .await?;
            }
            ControllerCommands::ReturnNetworkState { channel } => {
                self.swarm_command_sender
                    .send(SwarmCommand::ReturnNetworkState { channel })
                    .await?;
            }
            ControllerCommands::AnnounceSelf => {
                self.swarm_command_sender
                    .send(SwarmCommand::AnnounceSelf)
                    .await?;
            }
            ControllerCommands::Broadcast { message } => {
                self.swarm_command_sender
                    .send(SwarmCommand::Broadcast { message })
                    .await?;
            }
            ControllerCommands::ReturnNodeInfo { channel } => {
                self.swarm_command_sender
                    .send(SwarmCommand::ReturnNodeInfo { channel })
                    .await?;
            }
        }
        Ok(())
    }
}

#[derive(Clone)]
pub struct ControllerInterface {
    command_sender: mpsc::Sender<ControllerCommands>,
}

impl ControllerInterface {
    pub fn new(command_sender: mpsc::Sender<ControllerCommands>) -> Self {
        Self { command_sender }
    }

    pub async fn send_command(&self, command: ControllerCommands) -> eyre::Result<()> {
        self.command_sender.send(command).await?;
        Ok(())
    }

    pub async fn get_node_info(&self) -> eyre::Result<MintpoolNodeInfo> {
        let (snd, recv) = oneshot::channel();
        self.send_command(ControllerCommands::ReturnNodeInfo { channel: snd })
            .await?;
        Ok(recv.await?)
    }

    pub async fn get_network_state(&self) -> eyre::Result<NetworkState> {
        let (snd, recv) = oneshot::channel();
        self.send_command(ControllerCommands::ReturnNetworkState { channel: snd })
            .await?;
        Ok(recv.await?)
    }
}
