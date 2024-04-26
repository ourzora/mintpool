use std::ops::Sub;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use chrono::{DateTime, Utc};
use eyre::WrapErr;
use futures_ticker::Ticker;
use futures_util::StreamExt;
use libp2p::PeerId;
use sqlx::SqlitePool;
use tokio::select;
use tokio::sync::{mpsc, oneshot, Semaphore};

use crate::chain::inclusion_claim_correct;
use crate::config::{ChainInclusionMode, Config};
use crate::p2p::NetworkState;
use crate::rules::{Results, RulesEngine};
use crate::storage::{list_all_with_options, PremintStorage, QueryOptions, Reader, Writer};
use crate::types::{
    InclusionClaim, MintpoolNodeInfo, PeerInclusionClaim, PremintName, PremintTypes,
};

/// Represents commands that can be sent to the p2p swarm
#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
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
    SendOnchainMintFound(InclusionClaim),
    Sync {
        query: QueryOptions,
    },
}

/// Event types that may be received from the p2p swarm that need to be handled by the controller
pub enum P2PEvent {
    NetworkState(NetworkState),
    PremintReceived(PremintTypes),
    MintSeenOnchain(PeerInclusionClaim),
    SyncRequest {
        query: QueryOptions,
        channel: oneshot::Sender<eyre::Result<Vec<PremintTypes>>>,
    },
    SyncResponse {
        premints: Vec<PremintTypes>,
    },
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
        channel: oneshot::Sender<eyre::Result<()>>,
    },
    ReturnNodeInfo {
        channel: oneshot::Sender<MintpoolNodeInfo>,
    },
    Query(DBQuery),
    ResolveOnchainMint(InclusionClaim),
    Sync,
}

pub enum DBQuery {
    ListAll(oneshot::Sender<eyre::Result<Vec<PremintTypes>>>),
    Direct(oneshot::Sender<eyre::Result<SqlitePool>>),
}

/// Central hub for processing incoming events and commands from peers and other inputs
pub struct Controller {
    swarm_command_sender: mpsc::Sender<SwarmCommand>,
    swarm_event_receiver: mpsc::Receiver<P2PEvent>,
    external_commands: mpsc::Receiver<ControllerCommands>,
    store: PremintStorage,
    rules: RulesEngine<PremintStorage>,
    sync_ticker: Ticker,

    config: Config,
}

impl Controller {
    pub fn new(
        config: Config,
        swarm_command_sender: mpsc::Sender<SwarmCommand>,
        swarm_event_receiver: mpsc::Receiver<P2PEvent>,
        external_commands: mpsc::Receiver<ControllerCommands>,
        store: PremintStorage,
        rules: RulesEngine<PremintStorage>,
    ) -> Self {
        // sync every 60 minutes, also sync 5 seconds after startup (gives some time to connect to peers)
        let sync_ticker =
            Ticker::new_with_next(Duration::from_secs(60 * 60), Duration::from_secs(5));

        Self {
            swarm_command_sender,
            swarm_event_receiver,
            external_commands,
            store,
            rules,
            sync_ticker,
            config,
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
                _ = self.sync_ticker.next() => {
                    self.request_sync().await;
                }
            }
        }
    }

    async fn request_sync(&self) {
        let from = Some(
            chrono::Utc::now() - Duration::from_secs(60 * 60 * self.config.sync_lookback_hours),
        );

        let query = QueryOptions {
            from,
            ..Default::default()
        };

        self.swarm_command_sender
            .send(SwarmCommand::Sync { query })
            .await
            .expect("Error sending sync command to swarm");
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
                tracing::info!(histogram.premint_received = 1);
            }
            P2PEvent::MintSeenOnchain(claim) => {
                if let Err(err) = self.handle_event_onchain_claim(claim).await {
                    tracing::error!("Error handling onchain claim: {:?}", err);
                }
            }
            P2PEvent::SyncRequest { query, channel } => {
                let events = list_all_with_options(&self.store.db(), &query).await;
                if let Err(Err(err)) = channel.send(events) {
                    tracing::error!("Error sending sync response: {:?}", err);
                }
                tracing::info!(histogram.sync_request_processed = 1);
            }
            P2PEvent::SyncResponse { premints } => {
                let sem = Semaphore::new(10);
                futures_util::future::join_all(premints.into_iter().map(|p| async {
                    let permit = sem.acquire().await.unwrap();
                    let _ = self.validate_and_insert(p).await;
                    drop(permit);
                }))
                .await;
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
            ControllerCommands::Broadcast { message, channel } => {
                match self.validate_and_insert(message.clone()).await {
                    Ok(_result) => {
                        if let Err(err) = self
                            .swarm_command_sender
                            .send(SwarmCommand::Broadcast { message })
                            .await
                        {
                            channel
                                .send(Err(eyre::eyre!("Error broadcasing premint: {:?}", err)))
                                .map_err(|err| {
                                    eyre::eyre!("error broadcasting via channel: {:?}", err)
                                })?;
                        } else {
                            tracing::info!(histogram.premint_broadcasted = 1);
                            channel.send(Ok(())).map_err(|err| {
                                eyre::eyre!("error broadcasting via channel: {:?}", err)
                            })?;
                        }
                    }
                    Err(err) => {
                        channel.send(Err(err)).map_err(|err| {
                            eyre::eyre!("error broadcasting via channel: {:?}", err)
                        })?;
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
            ControllerCommands::ResolveOnchainMint(claim) => {
                tracing::debug!("Received command to resolve onchain mint, {:?}", claim);
                // This comes from trusted internal checks run by the running node, so safe to trust
                // likely want to add some checks here to ensure the claim is valid in future
                if let Err(err) = self.store.mark_seen_on_chain(claim.clone()).await {
                    tracing::error!(
                        error = err.to_string(),
                        "Error marking premint as seen on chain"
                    );
                } else {
                    tracing::debug!("Marked as seen onchain {:?}", claim.clone());
                }

                if self.config.chain_inclusion_mode == ChainInclusionMode::Check {
                    if let Err(err) = self
                        .swarm_command_sender
                        .send(SwarmCommand::SendOnchainMintFound(claim))
                        .await
                    {
                        tracing::error!("Error sending onchain mint found to swarm: {:?}", err);
                    }
                }
            }
            ControllerCommands::Sync => {
                self.request_sync().await;
            }
        }
        Ok(())
    }

    async fn validate_and_insert(&self, premint: PremintTypes) -> eyre::Result<Results> {
        let evaluation = self.rules.evaluate(&premint, self.store.clone()).await?;

        if evaluation.is_accept() {
            tracing::info!(histogram.rules_accepted = 1);

            self.store
                .store(premint)
                .await
                .map(|_r| evaluation)
                .wrap_err("Failed to store premint")
        } else {
            tracing::info!("Premint failed validation: {:?}", premint);
            tracing::info!(histogram.rules_rejected = 1);

            Err(evaluation).wrap_err("Premint failed validation")
        }
    }

    async fn handle_event_onchain_claim(&self, peer_claim: PeerInclusionClaim) -> eyre::Result<()> {
        match self.config.chain_inclusion_mode {
            ChainInclusionMode::Check | ChainInclusionMode::Verify => {
                let claim = peer_claim.claim;
                let premint = self
                    .store
                    .get_for_id_and_kind(&claim.premint_id, PremintName(claim.kind.clone()))
                    .await
                    .map_err(|err| {
                        eyre::eyre!("Error getting premint for onchain claim: {:?}", err)
                    })?;

                match inclusion_claim_correct(&premint, &claim).await {
                    Ok(true) => {
                        self.store.mark_seen_on_chain(claim.clone()).await?;
                        Ok(())
                    }
                    _ => {
                        tracing::info!("Ignoring inclusion claim received from peer");
                        Ok(())
                    }
                }
            }
            ChainInclusionMode::Trust => {
                if self
                    .config
                    .trusted_peers()
                    .contains(&peer_claim.from_peer_id)
                {
                    self.store
                        .mark_seen_on_chain(peer_claim.claim.clone())
                        .await?;
                    tracing::info!(
                        "Marked premint as seen via claim from trusted peer {:?}",
                        peer_claim
                    )
                } else {
                    tracing::debug!(
                        "Ignoring inclusion claim received from peer {:?}",
                        peer_claim
                    );
                }
                Ok(())
            }
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

    pub async fn get_all_premints(&self) -> eyre::Result<Vec<PremintTypes>> {
        let (snd, recv) = oneshot::channel();
        self.send_command(ControllerCommands::Query(DBQuery::ListAll(snd)))
            .await?;

        Ok(recv.await??)
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
