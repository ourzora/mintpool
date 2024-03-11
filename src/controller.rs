use crate::p2p::NetworkState;
use crate::types::Premint;
use tokio::io::AsyncBufReadExt;
use tokio::sync::mpsc;
use tokio::{io, select};

#[derive(Debug)]
pub enum SwarmCommand {
    ConnectToPeer { address: String },
    ReturnNetworkState,
    AnnounceSelf,
    Broadcast { message: Premint },
}

pub enum P2PEvent {
    NetworkState(NetworkState),
    PremintReceived(Premint),
}

pub struct Controller {
    swarm_command_sender: mpsc::Sender<SwarmCommand>,
    swarm_event_receiver: mpsc::Receiver<P2PEvent>,
}

impl Controller {
    pub fn new(
        swarm_command_sender: mpsc::Sender<SwarmCommand>,
        swarm_event_receiver: mpsc::Receiver<P2PEvent>,
    ) -> Self {
        Self {
            swarm_command_sender,
            swarm_event_receiver,
        }
    }

    pub async fn run_loop(&mut self) {
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
                        self.swarm_command_sender.send(SwarmCommand::ReturnNetworkState).await.unwrap();
                    }

                    else if line.starts_with("/announce") {
                        self.swarm_command_sender.send(SwarmCommand::AnnounceSelf).await.unwrap();
                    }

                    else {
                        match self.parse_premint(line) {
                            Ok(premint) => {
                                self.swarm_command_sender.send(SwarmCommand::Broadcast { message: premint }).await.unwrap();
                            }
                            Err(e) => {
                                println!("Error parsing premint: {:?}", e);
                            }
                        }

                    }
                }
                Some(event) = self.swarm_event_receiver.recv() => {
                    self.handle_event(event).await;
                }
            }
        }
    }

    fn parse_premint(&self, line: String) -> eyre::Result<Premint> {
        let p: Premint = serde_json::from_str(&line)?;
        Ok(p)
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
}
