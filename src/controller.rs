use crate::p2p::NetworkState;
use crate::rules::{RuleContext, RulesEngine};
use crate::storage::PremintStorage;
use crate::types::{InclusionClaim, MintpoolNodeInfo, PremintTypes};
use sqlx::SqlitePool;
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
        message: PremintTypes,
    },
    ReturnNodeInfo {
        channel: oneshot::Sender<MintpoolNodeInfo>,
    },
}

pub enum P2PEvent {
    NetworkState(NetworkState),
    PremintReceived(PremintTypes),
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
        message: PremintTypes,
    },
    ReturnNodeInfo {
        channel: oneshot::Sender<MintpoolNodeInfo>,
    },
    Query(DBQuery),
    ResolveOnchainMint(InclusionClaim),
}

pub enum DBQuery {
    ListAll(oneshot::Sender<eyre::Result<Vec<PremintTypes>>>),
    Direct(oneshot::Sender<eyre::Result<SqlitePool>>),
}

pub struct Controller {
    swarm_command_sender: mpsc::Sender<SwarmCommand>,
    swarm_event_receiver: mpsc::Receiver<P2PEvent>,
    external_commands: mpsc::Receiver<ControllerCommands>,
    store: PremintStorage,
    rules: RulesEngine,
}

impl Controller {
    pub fn new(
        swarm_command_sender: mpsc::Sender<SwarmCommand>,
        swarm_event_receiver: mpsc::Receiver<P2PEvent>,
        external_commands: mpsc::Receiver<ControllerCommands>,
        store: PremintStorage,
        rules: RulesEngine,
    ) -> Self {
        Self {
            swarm_command_sender,
            swarm_event_receiver,
            external_commands,
            store,
            rules,
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

    pub async fn handle_event(&self, event: P2PEvent) {
        match event {
            P2PEvent::NetworkState(network_state) => {
                tracing::info!("Current network state: {:?}", network_state);
            }
            P2PEvent::PremintReceived(premint) => {
                tracing::debug!(premint = premint.to_json().ok(), "Received premint");

                // TODO: handle error? respond with error summary?
                let _ = self.validate_and_insert(premint).await;
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
                match self.validate_and_insert(message.clone()).await {
                    Ok(_) => {
                        self.swarm_command_sender
                            .send(SwarmCommand::Broadcast { message })
                            .await?;
                    }
                    Err(err) => {
                        tracing::warn!("Invalid premint, not broadcasting: {:?}", err);
                    }
                }
            }
            ControllerCommands::ReturnNodeInfo { channel } => {
                self.swarm_command_sender
                    .send(SwarmCommand::ReturnNodeInfo { channel })
                    .await?;
            }
            ControllerCommands::Query(query) => match query {
                DBQuery::ListAll(chan) => {
                    let res = self.store.list_all().await;
                    if let Err(_err) = chan.send(res) {
                        tracing::error!("Error sending list all response back to command sender");
                    }
                }
                DBQuery::Direct(chan) => {
                    if let Err(_err) = chan.send(Ok(self.store.db())) {
                        tracing::error!("Error sending db arc response back to command sender");
                    };
                }
            },
            ControllerCommands::ResolveOnchainMint(claim) => {}
        }
        Ok(())
    }

    async fn validate_and_insert(&self, premint: PremintTypes) -> eyre::Result<()> {
        let evaluation = self.rules.evaluate(premint.clone(), RuleContext {}).await;

        if evaluation.is_accept() {
            self.store.store(premint).await
        } else {
            tracing::warn!(
                "Premint failed validation: {:?}, evaluation: {:?}",
                premint,
                evaluation.summary()
            );
            Err(eyre::eyre!(evaluation.summary()))
        }
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
