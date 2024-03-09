use crate::p2p::NetworkState;
use tokio::io::AsyncBufReadExt;
use tokio::sync::oneshot;
use tokio::{io, select};

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
        message: String,
    },
}

pub enum SwarmEvent {}

pub struct Controller {
    swarm_command_sender: tokio::sync::mpsc::Sender<SwarmCommand>,
}

impl Controller {
    pub fn new(swarm_command_sender: tokio::sync::mpsc::Sender<SwarmCommand>) -> Self {
        Self {
            swarm_command_sender,
        }
    }

    pub async fn run_loop(&self) {
        let mut stdin = io::BufReader::new(io::stdin()).lines();

        loop {
            select! {
                Ok(Some(line)) = stdin.next_line() => {
                    if line.is_empty() {
                        continue;
                    }
                    if line.starts_with("/ip4") {
                        self.swarm_command_sender.send(SwarmCommand::ConnectToPeer { address: line  }).await.unwrap();
                    }

                    else if line.starts_with("/peers") {
                        let (sndr, recv) = oneshot::channel();
                        self.swarm_command_sender.send(SwarmCommand::ReturnNetworkState { channel: sndr  }).await.unwrap();
                        let state = recv.await;
                        match state {
                            Ok(state) => {
                                tracing::info!("Current network state: {:?}", state);
                            },
                            Err(e) => tracing::error!("Error getting network state: {:?}", e),
                        }
                    }

                    else if line.starts_with("/announce") {
                        self.swarm_command_sender.send(SwarmCommand::AnnounceSelf).await.unwrap();
                    }

                    else {
                        self.swarm_command_sender.send(SwarmCommand::Broadcast { message: line }).await.unwrap();
                    }
                }
            }
        }
    }
}
