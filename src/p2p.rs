use std::borrow::Cow;
use std::collections::HashMap;
use std::hash::Hasher;
use std::time::Duration;

use eyre::WrapErr;
use futures_ticker::Ticker;
use itertools::Itertools;
use libp2p::autonat::NatStatus;
use libp2p::futures::StreamExt;
use libp2p::gossipsub::{IdentTopic, TopicHash, Version};
use libp2p::identify::Event;
use libp2p::identity::Keypair;
use libp2p::kad::store::{MemoryStore, RecordStore};
use libp2p::kad::{Addresses, ProviderRecord, RecordKey};
use libp2p::multiaddr::Protocol;
use libp2p::request_response::{InboundRequestId, Message, ProtocolSupport};
use libp2p::swarm::behaviour::toggle::Toggle;
use libp2p::swarm::dial_opts::{DialOpts, PeerCondition};
use libp2p::swarm::DialError::DialPeerConditionFalse;
use libp2p::swarm::{ConnectionId, NetworkBehaviour, NetworkInfo, SwarmEvent};
use libp2p::{
    autonat, dcutr, gossipsub, kad, noise, relay, request_response, tcp, yamux, Multiaddr, PeerId,
    StreamProtocol,
};
use rand::prelude::SliceRandom;
use serde::{Deserialize, Serialize};
use sha256::digest;
use tokio::select;

use crate::config::Config;
use crate::controller::{P2PEvent, SwarmCommand};
use crate::multi_ticker::MultiTicker;
use crate::multiaddr_ext::MultiaddrExt;
use crate::storage::QueryOptions;
use crate::types::{
    claims_topic_hashes, InclusionClaim, MintpoolNodeInfo, PeerInclusionClaim, Premint,
    PremintName, PremintTypes,
};

#[derive(NetworkBehaviour)]
pub struct MintpoolBehaviour {
    gossipsub: gossipsub::Behaviour,
    kad: kad::Behaviour<MemoryStore>,
    identify: libp2p::identify::Behaviour,
    ping: libp2p::ping::Behaviour,
    request_response: request_response::cbor::Behaviour<QueryOptions, SyncResponse>,
    relay: Toggle<relay::Behaviour>,
    relay_client: Toggle<relay::client::Behaviour>,
    relay_manager: Toggle<libp2p_relay_manager::Behaviour>,
    autonat: autonat::Behaviour,
    dcutr: dcutr::Behaviour,
}

#[derive(Copy, Clone, PartialEq, Eq, Hash)]
enum SwarmTickers {
    Bootstrap,
    DiscoverGossipPeers,
}

pub struct SwarmController {
    swarm: libp2p::Swarm<MintpoolBehaviour>,
    command_receiver: tokio::sync::mpsc::Receiver<SwarmCommand>,
    event_sender: tokio::sync::mpsc::Sender<P2PEvent>,
    premint_names: Vec<PremintName>,
    tickers: MultiTicker<SwarmTickers>,
    config: Config,
}

/// Service for managing p2p actions and connections
impl SwarmController {
    pub fn new(
        id_keys: Keypair,
        config: &Config,
        command_receiver: tokio::sync::mpsc::Receiver<SwarmCommand>,
        event_sender: tokio::sync::mpsc::Sender<P2PEvent>,
    ) -> Self {
        let mut swarm =
            Self::make_swarm_controller(id_keys, config).expect("Invalid config for swarm");

        // add external address if configured
        config
            .external_address
            .clone()
            .map(|addr| addr.parse::<Multiaddr>())
            .and_then(|addr| match addr {
                Ok(addr) => {
                    swarm.add_external_address(addr.clone());
                    tracing::info!("Added external address: {:?}", addr);
                    Some(addr)
                }
                Err(err) => {
                    tracing::warn!("Error parsing external address: {:?}", err);
                    None
                }
            });

        Self {
            swarm,
            command_receiver,
            event_sender,
            config: config.clone(),
            premint_names: config.premint_names(),
            tickers: MultiTicker::new(vec![
                (
                    // documentation suggests bootstrapping every 5 minutes
                    SwarmTickers::Bootstrap,
                    Ticker::new(Duration::from_secs(60 * 5)),
                ),
                (
                    SwarmTickers::DiscoverGossipPeers,
                    Ticker::new(Duration::from_secs(60)),
                ),
            ]),
        }
    }

    pub fn node_info(&self) -> MintpoolNodeInfo {
        let peer_id = *self.swarm.local_peer_id();
        let addr: Vec<Multiaddr> = self.swarm.listeners().cloned().collect();
        MintpoolNodeInfo { peer_id, addr }
    }

    fn make_swarm_controller(
        id_keys: Keypair,
        config: &Config,
    ) -> eyre::Result<libp2p::Swarm<MintpoolBehaviour>> {
        let peer_id = id_keys.public().to_peer_id();
        let public_key = id_keys.public();
        let swarm = libp2p::SwarmBuilder::with_existing_identity(id_keys)
            .with_tokio()
            .with_tcp(
                tcp::Config::default().port_reuse(true).nodelay(true),
                noise::Config::new,
                yamux::Config::default,
            )?
            .with_quic()
            .with_dns()?
            .with_relay_client(noise::Config::new, yamux::Config::default)?
            .with_behaviour(|key, client| {
                let kad = kad::Behaviour::new(peer_id, MemoryStore::new(key.public().to_peer_id()));
                let gossipsub_config = gossipsub::ConfigBuilder::default()
                    .heartbeat_interval(Duration::from_secs(10))
                    .validation_mode(gossipsub::ValidationMode::Strict)
                    .protocol_id("/mintpool/0.1.0", Version::V1_1)
                    .message_id_fn(gossipsub_message_id)
                    .build()
                    .expect("valid config");

                let gossipsub = gossipsub::Behaviour::new(
                    gossipsub::MessageAuthenticity::Signed(key.clone()),
                    gossipsub_config,
                )
                .expect("valid config");

                let identify = libp2p::identify::Behaviour::new(libp2p::identify::Config::new(
                    "mintpool/0.1.0".to_string(),
                    public_key,
                ));

                let ping = libp2p::ping::Behaviour::new(libp2p::ping::Config::new());

                let request_response = request_response::cbor::Behaviour::new(
                    [(
                        StreamProtocol::new("/mintpool-sync/1"),
                        ProtocolSupport::Full,
                    )],
                    request_response::Config::default(),
                );

                let autonat = autonat::Behaviour::new(
                    peer_id,
                    autonat::Config {
                        boot_delay: Duration::from_secs(15),
                        ..Default::default()
                    },
                );

                let dcutr = dcutr::Behaviour::new(peer_id);

                // if we're configured to be relay server,
                // assume that we don't want to use the relay client
                let (relay, relay_client, relay_manager) = if config.enable_relay_server {
                    tracing::info!("Enabling relay server");
                    (
                        Toggle::from(Some(relay::Behaviour::new(peer_id, Default::default()))),
                        Toggle::from(None),
                        Toggle::from(None),
                    )
                } else {
                    (
                        Toggle::from(None),
                        Toggle::from(Some(client)),
                        Toggle::from(Some(libp2p_relay_manager::Behaviour::new(
                            libp2p_relay_manager::Config {
                                auto_connect: true,
                                auto_relay: true,
                                limit: Some(5),
                                backoff: Duration::from_secs(15),
                            },
                        ))),
                    )
                };

                MintpoolBehaviour {
                    gossipsub,
                    kad,
                    identify,
                    ping,
                    request_response,
                    relay,
                    relay_client,
                    relay_manager,
                    autonat,
                    dcutr,
                }
            })?
            .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(60)))
            .build();

        Ok(swarm)
    }

    /// Starts the swarm controller listening and runs the run_loop awaiting incoming actions
    pub async fn run(&mut self, port: u64, listen_ip: String) -> eyre::Result<()> {
        self.swarm
            .listen_on(format!("/ip4/{listen_ip}/udp/{port}/quic-v1").parse()?)?;
        self.swarm
            .listen_on(format!("/ip4/{listen_ip}/tcp/{port}").parse()?)?;

        let registry_topic = announce_topic();
        self.gossip_subscribe(&registry_topic)?;

        // subscribe to all relevant topics
        self.premint_names
            .iter()
            .flat_map(|name| vec![name.msg_topic(), name.claims_topic()])
            .collect::<Vec<IdentTopic>>()
            .iter()
            .for_each(|topic| match self.gossip_subscribe(&topic) {
                Ok(_) => {}
                Err(err) => {
                    tracing::error!("Error subscribing to topic: {:?}", err);
                }
            });

        self.run_loop().await;
        Ok(())
    }

    /// Core run loop for the swarm controller, should run forever in a thread
    async fn run_loop(&mut self) {
        loop {
            select! {
                command = self.command_receiver.recv() => {
                    if let Some(command) = command {
                        self.handle_command(command).await;
                    }
                }
                event = self.swarm.select_next_some() => self.handle_swarm_event(event).await,
                tick = self.tickers.select_next_some() => {
                    match tick {
                        (SwarmTickers::Bootstrap, _) => {
                            match self.swarm.behaviour_mut().kad.bootstrap() {
                                Ok(_) => {}
                                Err(err) => {
                                    tracing::error!("Error bootstrapping kad: {:?}", err);
                                }
                            }
                        }
                        (SwarmTickers::DiscoverGossipPeers, _) => {
                            let b = self.swarm.behaviour_mut();

                            b.gossipsub.topics().for_each(|topic| {
                                // kad will automatically dial all discovered peers,
                                // gossipsub will automatically sync topics with new peers
                                b.kad.get_providers(Self::topic_to_record_key(topic));
                            });
                        }
                    }
                }
            }
        }
    }

    /// Handles swarm actions sent by the controller
    async fn handle_command(&mut self, command: SwarmCommand) {
        tracing::debug!("Received command: {:?}", command);
        match command {
            SwarmCommand::ConnectToPeer { address } => match address.parse() {
                Ok(addr) => {
                    self.safe_dial(addr).await;
                }
                Err(err) => {
                    tracing::warn!("Error parsing address: {:?}", err);
                }
            },
            SwarmCommand::ReturnNetworkState { channel } => {
                let network_state = self.make_network_state();
                if channel.send(network_state).is_err() {
                    tracing::error!("Error sending network state from swarm",);
                }
            }
            SwarmCommand::AnnounceSelf => {
                self.announce_self();
            }
            SwarmCommand::Broadcast { message } => {
                if let Err(err) = self.broadcast_message(message) {
                    tracing::error!("Error broadcasting message: {:?}", err);
                }
            }
            SwarmCommand::ReturnNodeInfo { channel } => {
                if channel.send(self.node_info()).is_err() {
                    tracing::error!("Error sending node info from swarm",);
                }
            }
            SwarmCommand::SendOnchainMintFound(claim) => {
                if let Err(err) = self.broadcast_claim(claim) {
                    tracing::error!("Error broadcasting claim: {:?}", err);
                }
            }
            SwarmCommand::Sync { query } => self.do_sync(query).await,
        }
    }

    async fn handle_swarm_event(&mut self, event: SwarmEvent<MintpoolBehaviourEvent>) {
        match event {
            SwarmEvent::NewListenAddr { address, .. } => {
                let pid = self.swarm.local_peer_id();
                let local_address = if address.is_relayed() {
                    // if it's a relay address, let's assume it's an external address

                    if !self.swarm.external_addresses().contains(&address) {
                        self.swarm.add_external_address(address.clone());
                    }

                    address.to_string()
                } else {
                    address.with(Protocol::P2p(*pid)).to_string()
                };
                tracing::info!(local_address = local_address, "Started listening");
            }

            SwarmEvent::ExpiredListenAddr { address, .. } => {
                tracing::info!(address = address.to_string(), "Expired listen address");

                if address.is_relayed() {
                    self.swarm.remove_external_address(&address);
                }
            }

            SwarmEvent::ListenerClosed {
                addresses, reason, ..
            } => {
                tracing::info!(
                    addresses = addresses
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<String>>()
                        .join(", "),
                    reason = format!("{:?}", reason),
                    "Listener closed"
                );

                for address in addresses {
                    if address.is_relayed() {
                        self.swarm.remove_external_address(&address);
                    }
                }
            }

            SwarmEvent::IncomingConnection {
                connection_id,
                local_addr,
                send_back_addr,
            } => {
                tracing::info!("Incoming connection: {connection_id}, local_addr: {local_addr}, send_back_addr: {send_back_addr}");
                self.reject_connection_if_over_max(connection_id);
            }
            SwarmEvent::ConnectionEstablished {
                peer_id,
                connection_id,
                ..
            } => {
                self.reject_connection_if_over_max(connection_id);
                tracing::info!("Connection established with peer: {:?}", peer_id);
                tracing::info!(counter.connections = 1);
            }

            SwarmEvent::ConnectionClosed { peer_id, cause, .. } => {
                tracing::info!("Connection closed: {:?}, cause: {:?}", peer_id, cause);
                tracing::info!(counter.connections = -1);
            }

            SwarmEvent::OutgoingConnectionError { peer_id, error, .. } => {
                if let Some(peer_id) = peer_id {
                    tracing::warn!(
                        peer_id = peer_id.to_string(),
                        error = error.to_string(),
                        "connection error to peer "
                    );
                }
            }

            SwarmEvent::Behaviour(MintpoolBehaviourEvent::Kad(kad_event)) => {
                if let Err(err) = self.handle_kad_event(kad_event).await {
                    tracing::error!(
                        error = err.to_string(),
                        "Error processing kad behavior event",
                    );
                }
            }
            SwarmEvent::Behaviour(MintpoolBehaviourEvent::Gossipsub(gossipsub_event)) => {
                if let Err(err) = self.handle_gossipsub_event(gossipsub_event).await {
                    tracing::error!(
                        error = err.to_string(),
                        "Error processing gossipsub behavior event",
                    );
                }
            }

            SwarmEvent::Behaviour(MintpoolBehaviourEvent::RequestResponse(event)) => {
                match self.handle_request_response_event(event).await {
                    Ok(_) => {}
                    Err(err) => {
                        tracing::error!("Error handling request response event: {:?}", err);
                    }
                }
            }

            SwarmEvent::Behaviour(MintpoolBehaviourEvent::RelayClient(event)) => {
                match self.handle_relay_client_event(event).await {
                    Ok(_) => {}
                    Err(err) => {
                        tracing::error!("Error handling relay client event: {:?}", err);
                    }
                }
            }

            SwarmEvent::Dialing { peer_id, .. } => {
                tracing::info!("Dialing: {:?}", peer_id)
            }

            SwarmEvent::NewExternalAddrCandidate { address } => {
                tracing::info!("New external address candidate: {:?}", address)
            }

            SwarmEvent::ExternalAddrConfirmed { address } => {
                tracing::info!("External address confirmed: {:?}", address);
            }

            SwarmEvent::Behaviour(MintpoolBehaviourEvent::Identify(event)) => match event {
                Event::Received { peer_id, info } => {
                    let is_relay = info.protocols.contains(&relay::HOP_PROTOCOL_NAME);

                    if let (Some(relay_manager), true) =
                        (self.swarm.behaviour_mut().relay_manager.as_mut(), is_relay)
                    {
                        tracing::debug!("Discovered relay peer: {:?}", info);

                        for addr in info.listen_addrs {
                            tracing::debug!("Adding relay address: {:?}", addr);
                            relay_manager.add_address(peer_id, addr);
                        }
                    }
                }
                _ => {
                    tracing::info!("Identify event: {:?}", event);
                }
            },

            SwarmEvent::Behaviour(MintpoolBehaviourEvent::Ping(event)) => {
                match event.result {
                    Ok(rtt) => {
                        if let Some(relay_manager) =
                            self.swarm.behaviour_mut().relay_manager.as_mut()
                        {
                            relay_manager.set_peer_rtt(event.peer, event.connection, rtt);
                        }
                    }
                    _ => {}
                }
                tracing::trace!("Ping event: {:?}", event);
            }

            SwarmEvent::Behaviour(MintpoolBehaviourEvent::Relay(event)) => {
                tracing::debug!("Relay event: {:?}", event);
            }

            SwarmEvent::Behaviour(MintpoolBehaviourEvent::Autonat(event)) => {
                match self.handle_autonat_event(event).await {
                    Ok(_) => {}
                    Err(err) => {
                        tracing::error!("Error handling autonat event: {:?}", err);
                    }
                }
            }

            SwarmEvent::Behaviour(MintpoolBehaviourEvent::Dcutr(event)) => {
                tracing::debug!("Dcutr event: {:?}", event);
            }

            other => {
                tracing::debug!("Unhandled swarm event: {:?}", other)
            }
        }
    }

    fn broadcast_message(&mut self, message: PremintTypes) -> eyre::Result<()> {
        let topic = message.metadata().kind.msg_topic();
        let msg = message.to_json().wrap_err("failed to serialize message")?;

        self.swarm
            .behaviour_mut()
            .gossipsub
            .publish(topic, msg.as_bytes())
            .wrap_err(format!("failed to publish message to topic {:?}", message))?;
        Ok(())
    }

    fn broadcast_claim(&mut self, claim: InclusionClaim) -> eyre::Result<()> {
        let topic = PremintName(claim.kind.clone()).claims_topic();
        let msg = serde_json::to_string(&claim).wrap_err("failed to serialize claim")?;
        self.swarm
            .behaviour_mut()
            .gossipsub
            .publish(topic, msg.as_bytes())
            .wrap_err(format!(
                "failed to publish claim message to topic {:?}",
                claim
            ))?;
        Ok(())
    }

    fn announce_self(&mut self) {
        let peer_id = *self.swarm.local_peer_id();
        let listening_on = self.swarm.listeners().collect::<Vec<_>>();
        tracing::info!("announcing, listening on: {:?}", listening_on);
        let value = if let Some(addr) = self.swarm.listeners().collect::<Vec<_>>().first() {
            let m = (*addr).clone().with(Protocol::P2p(peer_id));
            tracing::info!("sending full address: {:?}", m.to_string());
            m.to_string()
        } else {
            peer_id.to_string()
        };
        let registry_topic = announce_topic();

        if let Err(err) = self
            .swarm
            .behaviour_mut()
            .gossipsub
            .publish(registry_topic, value.as_bytes())
        {
            tracing::error!(error = err.to_string(), "Error announcing self");
        };
    }

    async fn handle_gossipsub_event(&mut self, event: gossipsub::Event) -> eyre::Result<()> {
        tracing::debug!("Gossipsub event: {:?}", event);
        let registry_topic = announce_topic();
        match event {
            gossipsub::Event::Message {
                message,
                propagation_source,
                ..
            } => {
                let msg = String::from_utf8_lossy(&message.data);
                // Handle announcements
                if message.topic == registry_topic.hash() {
                    tracing::info!("New peer: {:?}", msg);
                    let addr: Multiaddr = msg
                        .to_string()
                        .parse()
                        .wrap_err(format!("invalid address found from announce: {}", msg))?;

                    self.safe_dial(addr).await;
                    tracing::info!(histogram.peer_announced = 1);
                // Handle inclusion claims
                } else if claims_topic_hashes(self.premint_names.clone()).contains(&message.topic) {
                    let claim = serde_json::from_str::<InclusionClaim>(&msg)
                        .wrap_err("Error parsing inclusion claim")?;

                    self.event_sender
                        .send(P2PEvent::MintSeenOnchain(PeerInclusionClaim {
                            claim,
                            from_peer_id: propagation_source,
                        }))
                        .await
                        .wrap_err("failed to send mint seen onchain event")?;
                    tracing::info!(counter.seen_on_chain_peer_claim = 1);
                // Handle premints
                } else {
                    match serde_json::from_str::<PremintTypes>(&msg) {
                        Ok(premint) => {
                            let id = premint.metadata().id;
                            tracing::info!(id = id, "Received new premint");
                            self.event_sender
                                .send(P2PEvent::PremintReceived(premint.clone()))
                                .await
                                .wrap_err("failed to send premint event")?;
                            tracing::debug!("premint event sent: {:?}", premint);
                            tracing::info!(counter.p2p_premint_received = 1);
                        }
                        Err(err) => {
                            tracing::error!("Error parsing premint: {:?}", err);
                        }
                    }
                }
            }
            gossipsub::Event::Subscribed { peer_id, topic } => {
                tracing::info!("Subscribed to topic: {:?} by peer: {:?}", topic, peer_id);
            }
            gossipsub::Event::Unsubscribed { peer_id, topic } => {
                tracing::info!(
                    "Unsubscribed from topic: {:?} by peer: {:?}",
                    topic,
                    peer_id
                );
            }
            gossipsub::Event::GossipsubNotSupported { peer_id } => {
                tracing::info!("Gossipsub not supported by peer: {:?}", peer_id);
            }
        }
        Ok(())
    }

    async fn handle_kad_event(&mut self, event: kad::Event) -> eyre::Result<()> {
        match event {
            kad::Event::InboundRequest { request } => {
                tracing::info!("Inbound kad request: {:?}", request);
            }
            kad::Event::RoutingUpdated {
                peer,
                addresses,
                is_new_peer,
                ..
            } => {
                tracing::info!(
                    "Routing updated, peer: {:?}, addresses: {:?}",
                    peer,
                    addresses
                );

                if is_new_peer && self.swarm.behaviour_mut().kad.kbuckets().count() == 1 {
                    tracing::info!("First peer, bootstrapping kad");
                    match self.swarm.behaviour_mut().kad.bootstrap() {
                        Ok(_) => {}
                        Err(err) => {
                            tracing::error!("Error bootstrapping kad: {:?}", err);
                        }
                    }
                }
            }
            other => {
                tracing::info!("Kad event: {:?}", other);
            }
        }
        Ok(())
    }

    // Returns True if the connection was rejected
    fn reject_connection_if_over_max(&mut self, connection_id: ConnectionId) -> bool {
        let state = self.make_network_state();
        if self.config.peer_limit < state.network_info.num_peers() as u64 {
            tracing::warn!("Max peers reached, rejecting connection",);
            self.swarm.close_connection(connection_id);
            return true;
        }
        false
    }

    async fn safe_dial(&mut self, address: Multiaddr) {
        let state = self.make_network_state();
        let peers = state.gossipsub_peers.len();
        let max_peers = self.config.peer_limit;

        if peers as u64 >= max_peers {
            tracing::warn!(
                peers = peers,
                max_peers = max_peers,
                "Max peers reached, not connecting to peer"
            );
            return;
        }

        let opts = if let Some(peer_id) = address.peer_id() {
            DialOpts::peer_id(peer_id)
                .addresses(vec![address])
                .condition(PeerCondition::DisconnectedAndNotDialing)
                .build()
        } else {
            DialOpts::from(address)
        };

        match self.swarm.dial(opts) {
            Ok(_) | Err(DialPeerConditionFalse(PeerCondition::DisconnectedAndNotDialing)) => {}

            Err(err) => {
                tracing::error!("Error dialing peer: {:?}", err);
            }
        }
    }

    fn make_network_state(&mut self) -> NetworkState {
        let external_addresses = self.swarm.external_addresses().cloned().collect();
        let local_peer_id = *self.swarm.local_peer_id();
        let network_info = self.swarm.network_info();
        let listeners = self.swarm.listeners().cloned().collect();

        let b = self.swarm.behaviour_mut();
        let dht_peers: Vec<_> = b
            .kad
            .kbuckets()
            .flat_map(|x| x.iter().map(|x| x.node.value.clone()).collect::<Vec<_>>())
            .collect();
        let providing = b.kad.store_mut().provided().map(Cow::into_owned).collect();
        let gossipsub_peers = b.gossipsub.all_mesh_peers().cloned().collect::<Vec<_>>();

        let nat_status = if let Some(addr) = &self.config.external_address {
            match addr.parse() {
                Ok(addr) => NatStatus::Public(addr),
                Err(_) => NatStatus::Unknown,
            }
        } else {
            b.autonat.nat_status()
        };

        NetworkState {
            local_peer_id,
            network_info,
            dht_peers,
            gossipsub_peers,
            external_addresses,
            providing,
            listeners,
            nat_status,
        }
    }

    async fn do_sync(&mut self, query: QueryOptions) {
        // select random peer
        let state = self.make_network_state();

        let peer_id = state
            .gossipsub_peers
            .choose(&mut rand::thread_rng())
            .cloned();

        if let Some(peer_id) = peer_id {
            let id = self
                .swarm
                .behaviour_mut()
                .request_response
                .send_request(&peer_id, query);

            tracing::info!(request_id = id.to_string(), "sent sync request");
        } else {
            tracing::info!("No peers to sync with");
        }
    }

    async fn handle_request_response_event(
        &mut self,
        event: request_response::Event<QueryOptions, SyncResponse>,
    ) -> eyre::Result<()> {
        match event {
            request_response::Event::Message { message, .. } => match message {
                Message::Request {
                    request_id,
                    request,
                    channel,
                } => {
                    let resp = self.make_sync_response(request_id, request).await;
                    self.swarm
                        .behaviour_mut()
                        .request_response
                        .send_response(channel, resp)
                        .map_err(|e| eyre::eyre!("Error sending response: {:?}", e))?;
                }
                Message::Response {
                    request_id,
                    response,
                } => {
                    tracing::info!(
                        request_id = request_id.to_string(),
                        "received response for sync"
                    );
                    match response {
                        SyncResponse::Premints(premints) => {
                            self.event_sender
                                .send(P2PEvent::SyncResponse { premints })
                                .await?;
                        }
                        SyncResponse::Error(err) => {
                            tracing::error!(
                                request_id = request_id.to_string(),
                                error = err,
                                "error received to our sync request"
                            );
                        }
                    }
                }
            },
            other => tracing::info!("mintpool sync request/response event: {:?}", other),
        }
        Ok(())
    }

    async fn handle_relay_client_event(&mut self, event: relay::client::Event) -> eyre::Result<()> {
        match event {
            relay::client::Event::ReservationReqAccepted { relay_peer_id, .. } => {
                tracing::info!("Relay reservation request accepted: {:?}", relay_peer_id);
            }

            other => {
                tracing::info!("Relay client event: {:?}", other);
            }
        }

        Ok(())
    }

    async fn handle_autonat_event(&mut self, event: autonat::Event) -> eyre::Result<()> {
        if let autonat::Event::StatusChanged { old, new } = event {
            tracing::info!("Autonat status changed: {:?} -> {:?}", old, new);
            match new {
                NatStatus::Private => {
                    tracing::info!("Autonat status is private");
                    if let Some(manager) = self.swarm.behaviour_mut().relay_manager.as_mut() {
                        tracing::info!("Relay peer: {:?}", manager.random_select());
                    }
                }
                NatStatus::Public(multiaddr) => {
                    tracing::info!("Autonat status is public: {}", multiaddr);
                }
                NatStatus::Unknown => {
                    tracing::info!("Autonat status is unknown");
                }
            }
        }

        Ok(())
    }

    fn gossip_subscribe(&mut self, topic: &IdentTopic) -> eyre::Result<()> {
        tracing::info!("Subscribing to topic: {}", topic.to_string());
        let b = self.swarm.behaviour_mut();

        b.gossipsub.subscribe(&topic)?;
        b.kad
            .start_providing(Self::topic_to_record_key(&topic.hash()))?;

        Ok(())
    }

    fn gossip_unsubscribe(&mut self, topic: &IdentTopic) -> eyre::Result<()> {
        let b = self.swarm.behaviour_mut();

        b.gossipsub.unsubscribe(topic)?;
        b.kad
            .stop_providing(&Self::topic_to_record_key(&topic.hash()));

        Ok(())
    }

    fn topic_to_record_key(topic: &TopicHash) -> RecordKey {
        RecordKey::new(&format!("topic::{}", topic.to_string()).as_bytes())
    }

    // Makes a Response for a request to sync from another node
    async fn make_sync_response(
        &mut self,
        request_id: InboundRequestId,
        request: QueryOptions,
    ) -> SyncResponse {
        tracing::info!(
            request_id = request_id.to_string(),
            "processing request for sync"
        );
        match self.make_sync_response_query(request).await {
            Ok(premints) => SyncResponse::Premints(premints),
            Err(err) => {
                tracing::error!(
                    request_id = request_id.to_string(),
                    error = err.to_string(),
                    "error processing sync request"
                );
                SyncResponse::Error(err.to_string())
            }
        }
    }

    // inner function to make propagating errors that occur during query easier to work with
    async fn make_sync_response_query(
        &mut self,
        request: QueryOptions,
    ) -> eyre::Result<Vec<PremintTypes>> {
        let (snd, recv) = tokio::sync::oneshot::channel();
        self.event_sender
            .send(P2PEvent::SyncRequest {
                query: request,
                channel: snd,
            })
            .await
            .map_err(|_| eyre::eyre!("Controller error"))?;
        let result = recv
            .await
            .map_err(|_| eyre::eyre!("Channel error"))?
            .map_err(|_| eyre::eyre!("Query error"))?;
        Ok(result)
    }
}

fn gossipsub_message_id(message: &gossipsub::Message) -> gossipsub::MessageId {
    if message.topic == announce_topic().hash() {
        let s = String::from_utf8_lossy(&message.data);
        let hash = digest(s.to_string());
        gossipsub::MessageId::from(hash)
    } else {
        let s = String::from_utf8_lossy(&message.data);
        match PremintTypes::from_json(s.to_string()) {
            Ok(premint) => {
                let metadata = premint.metadata();
                let hash = digest(metadata.id);
                gossipsub::MessageId::from(hash)
            }
            Err(_) => gossipsub::MessageId::from("likely_spam".to_string()),
        }
    }
}

#[derive(Debug)]
pub struct NetworkState {
    pub local_peer_id: PeerId,
    pub network_info: NetworkInfo,
    pub dht_peers: Vec<Addresses>,
    pub gossipsub_peers: Vec<PeerId>,
    pub external_addresses: Vec<Multiaddr>,
    pub providing: Vec<ProviderRecord>,
    pub listeners: Vec<Multiaddr>,
    pub nat_status: NatStatus,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum SyncResponse {
    Premints(Vec<PremintTypes>),
    Error(String),
}

fn announce_topic() -> gossipsub::IdentTopic {
    gossipsub::IdentTopic::new("mintpool::announce")
}
