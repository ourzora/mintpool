use crate::controller::{P2PEvent, SwarmCommand};
use crate::types::{MintpoolNodeInfo, Premint, PremintTypes};
use eyre::WrapErr;
use libp2p::core::ConnectedPoint;
use libp2p::futures::StreamExt;
use libp2p::identity::Keypair;
use libp2p::kad::store::MemoryStore;
use libp2p::multiaddr::Protocol;
use libp2p::swarm::{NetworkBehaviour, NetworkInfo, SwarmEvent};
use libp2p::{gossipsub, kad, noise, tcp, yamux, Multiaddr};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::time::Duration;
use tokio::select;

pub fn make_swarm_controller(
    id_keys: Keypair,
    command_receiver: tokio::sync::mpsc::Receiver<SwarmCommand>,
    event_sender: tokio::sync::mpsc::Sender<P2PEvent>,
) -> eyre::Result<SwarmController> {
    let peer_id = id_keys.public().to_peer_id();
    let swarm = libp2p::SwarmBuilder::with_existing_identity(id_keys)
        .with_tokio()
        .with_tcp(
            tcp::Config::default(),
            noise::Config::new,
            yamux::Config::default,
        )?
        .with_behaviour(|key| {
            let message_id_fn = |message: &gossipsub::Message| {
                let mut s = DefaultHasher::new();
                message.data.hash(&mut s);
                gossipsub::MessageId::from(s.finish().to_string())
            };

            let mut b = kad::Behaviour::new(peer_id, MemoryStore::new(key.public().to_peer_id()));
            b.set_mode(Some(kad::Mode::Server));
            let gossipsub_config = gossipsub::ConfigBuilder::default()
                .heartbeat_interval(Duration::from_secs(10))
                .validation_mode(gossipsub::ValidationMode::Strict)
                .message_id_fn(message_id_fn)
                .build()
                .expect("valid config");

            let gs = gossipsub::Behaviour::new(
                gossipsub::MessageAuthenticity::Signed(key.clone()),
                gossipsub_config,
            )
            .expect("valid config");

            MintpoolBehaviour {
                gossipsub: gs,
                kad: b,
            }
        })?
        .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(60)))
        .build();

    Ok(SwarmController::new(
        swarm,
        "zora-premints-v2".to_string(),
        command_receiver,
        event_sender,
    ))
}
pub struct SwarmController {
    swarm: libp2p::Swarm<MintpoolBehaviour>,
    topic_name: String,
    command_receiver: tokio::sync::mpsc::Receiver<SwarmCommand>,
    event_sender: tokio::sync::mpsc::Sender<P2PEvent>,
}

impl SwarmController {
    pub fn new(
        swarm: libp2p::Swarm<MintpoolBehaviour>,
        topic_name: String,
        command_receiver: tokio::sync::mpsc::Receiver<SwarmCommand>,
        event_sender: tokio::sync::mpsc::Sender<P2PEvent>,
    ) -> Self {
        Self {
            swarm,
            topic_name,
            command_receiver,
            event_sender,
        }
    }

    pub async fn run(&mut self, port: u64, listen_ip: String) -> eyre::Result<()> {
        let registry_topic = gossipsub::IdentTopic::new("announce::premints");

        let topic = gossipsub::IdentTopic::new("zora-1155-v1-mints");
        self.swarm.behaviour_mut().gossipsub.subscribe(&topic)?;
        self.swarm
            .behaviour_mut()
            .gossipsub
            .subscribe(&registry_topic)?;

        self.swarm
            .listen_on(format!("/ip4/{listen_ip}/tcp/{port}").parse()?)?;

        self.run_loop().await;
        Ok(())
    }

    async fn run_loop(&mut self) {
        loop {
            select! {
                command = self.command_receiver.recv() => {
                    if let Some(command) = command {
                        self.handle_command(command).await;
                    }
                }
                event = self.swarm.select_next_some() => self.handle_swarm_event(event).await,
            }
        }
    }

    async fn handle_command(&mut self, command: SwarmCommand) {
        tracing::info!("Received command: {:?}", command);
        match command {
            SwarmCommand::ConnectToPeer { address } => {
                let addr: Multiaddr = address.parse().unwrap();
                if let Err(err) = self.swarm.dial(addr) {
                    tracing::error!("Error dialing peer: {:?}", err);
                }
            }
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
                let peer_id = *self.swarm.local_peer_id();
                let addr: Vec<Multiaddr> = self.swarm.listeners().cloned().collect();
                if channel.send(MintpoolNodeInfo { peer_id, addr }).is_err() {
                    tracing::error!("Error sending node info from swarm",);
                }
            }
        }
    }

    async fn handle_swarm_event(&mut self, event: SwarmEvent<MintpoolBehaviourEvent>) {
        match event {
            SwarmEvent::NewListenAddr { address, .. } => {
                let pid = self.swarm.local_peer_id();
                let local_address = address.with(Protocol::P2p(pid.clone())).to_string();
                tracing::info!(local_address = local_address, "Started listening");
            }

            SwarmEvent::IncomingConnection {
                connection_id,
                local_addr,
                send_back_addr,
            } => {
                tracing::info!("Incoming connection: {connection_id}, local_addr: {local_addr}, send_back_addr: {send_back_addr}");
            }
            SwarmEvent::ConnectionEstablished {
                peer_id, endpoint, ..
            } => {
                match endpoint {
                    ConnectedPoint::Dialer { address, .. } => {
                        let addr = address;
                        self.swarm.add_external_address(addr.clone());
                        let b = self.swarm.behaviour_mut();
                        b.kad.add_address(&peer_id, addr.clone());
                        tracing::info!("Dialed: {:?}", addr);
                    }
                    ConnectedPoint::Listener {
                        local_addr,
                        send_back_addr,
                    } => {
                        let addr = send_back_addr.with(Protocol::P2p(peer_id));
                        tracing::info!("Was connected to by: {:?} local: {local_addr}", addr);
                    }
                }

                tracing::info!("Connection established with peer: {:?}", peer_id);
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

            SwarmEvent::Dialing { peer_id, .. } => {
                tracing::info!("Dialing: {:?}", peer_id)
            }
            other => {
                tracing::info!("Unhandled swarm event: {:?}", other)
            }
        }
    }

    fn broadcast_message(&mut self, message: PremintTypes) -> eyre::Result<()> {
        let topic = gossipsub::IdentTopic::new("zora-1155-v1-mints");
        let msg = message.to_json().wrap_err("failed to serialize message")?;

        self.swarm
            .behaviour_mut()
            .gossipsub
            .publish(topic, msg.as_bytes())
            .wrap_err(format!("failed to publish message to topic {:?}", message))?;
        Ok(())
    }

    fn announce_self(&mut self) {
        let peer_id = *self.swarm.local_peer_id();
        let listening_on = self.swarm.listeners().collect::<Vec<_>>();
        tracing::info!("announcing, listening on: {:?}", listening_on);
        let value = if let Some(addr) = self.swarm.listeners().collect::<Vec<_>>().first() {
            let m = (*addr).clone().with(Protocol::P2p(peer_id.clone()));
            tracing::info!("sending full address: {:?}", m.to_string());
            m.to_string()
        } else {
            peer_id.to_string()
        };
        let registry_topic = gossipsub::IdentTopic::new("announce::premints");

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
        tracing::info!("Gossipsub event: {:?}", event);
        let registry_topic = gossipsub::IdentTopic::new("announce::premints");
        match event {
            gossipsub::Event::Message { message, .. } => {
                let msg = String::from_utf8_lossy(&message.data);
                if message.topic == registry_topic.hash() {
                    tracing::info!("New peer: {:?}", msg);
                    let addr: Multiaddr = msg
                        .to_string()
                        .parse()
                        .wrap_err(format!("invalid address found from announce: {}", msg))?;

                    self.swarm.dial(addr)?;
                } else {
                    match serde_json::from_str::<PremintTypes>(&msg) {
                        Ok(premint) => {
                            self.event_sender
                                .send(P2PEvent::PremintReceived(premint.clone()))
                                .await
                                .wrap_err("failed to send premint event")?;
                            tracing::debug!("premint event sent: {:?}", premint);
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
                peer, addresses, ..
            } => {
                tracing::info!(
                    "Routing updated, peer: {:?}, addresses: {:?}",
                    peer,
                    addresses
                );
            }
            other => {
                tracing::info!("Kad event: {:?}", other);
            }
        }
        Ok(())
    }

    fn make_network_state(&mut self) -> NetworkState {
        let dht_peers: Vec<_> = self
            .swarm
            .behaviour_mut()
            .kad
            .kbuckets()
            .flat_map(|x| {
                x.iter()
                    .map(|x| format!("{:?}", x.node.value))
                    .collect::<Vec<_>>()
            })
            .collect();

        let my_id = self.swarm.local_peer_id().to_string();

        let gossipsub_peers = self
            .swarm
            .behaviour_mut()
            .gossipsub
            .all_mesh_peers()
            .map(|p| p.to_string())
            .collect::<Vec<_>>();

        NetworkState {
            local_peer_id: my_id,
            network_info: self.swarm.network_info(),
            dht_peers,
            gossipsub_peers,
            all_external_addresses: self.swarm.external_addresses().cloned().collect(),
        }
    }
}

#[derive(NetworkBehaviour)]
pub struct MintpoolBehaviour {
    gossipsub: gossipsub::Behaviour,
    kad: kad::Behaviour<MemoryStore>,
}

#[derive(Debug)]
pub struct NetworkState {
    pub local_peer_id: String,
    pub network_info: NetworkInfo,
    pub dht_peers: Vec<String>,
    pub gossipsub_peers: Vec<String>,
    pub all_external_addresses: Vec<Multiaddr>,
}
