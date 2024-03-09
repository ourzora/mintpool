use libp2p::core::ConnectedPoint;
use libp2p::futures::StreamExt;
use libp2p::gossipsub::Topic;
use libp2p::identity::Keypair;
use libp2p::kad::store::MemoryStore;
use libp2p::kad::{GetRecordOk, QueryResult, Quorum, Record};
use libp2p::multiaddr::Protocol;
use libp2p::swarm::{NetworkBehaviour, SwarmEvent};
use libp2p::{gossipsub, kad, noise, tcp, yamux, Multiaddr};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::ops::Add;
use std::time::{Duration, Instant};
use tokio::io::AsyncBufReadExt;
use tokio::{io, select};
use tracing_subscriber::fmt::format;

pub fn make_swarm_controller(id_keys: Keypair) -> eyre::Result<SwarmController> {
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

    Ok(SwarmController::new(swarm, "zora-premints-v2".to_string()))
}
pub struct SwarmController {
    pub swarm: libp2p::Swarm<MintpoolBehaviour>,
    topic_name: String,
}

impl SwarmController {
    pub fn new(swarm: libp2p::Swarm<MintpoolBehaviour>, topic_name: String) -> Self {
        Self { swarm, topic_name }
    }

    pub async fn run(&mut self, port: u64) -> eyre::Result<()> {
        let mut stdin = io::BufReader::new(io::stdin()).lines();

        let registry_topic = gossipsub::IdentTopic::new("announce::premints");

        let topic = gossipsub::IdentTopic::new("zora-1155-v1-mints");
        self.swarm.behaviour_mut().gossipsub.subscribe(&topic)?;
        self.swarm
            .behaviour_mut()
            .gossipsub
            .subscribe(&registry_topic)?;

        self.swarm
            .listen_on(format!("/ip4/0.0.0.0/tcp/{port}").parse()?)?;

        loop {
            select! {
                Ok(Some(line)) = stdin.next_line() => {
                    if line.is_empty() {
                        continue;
                    }
                    if line.starts_with("/ip4") {
                        tracing::info!("Adding peer: {:?}", line);
                        let addr: Multiaddr = line.parse().unwrap();

                        let Some(Protocol::P2p(peer_id)) = addr.iter().last() else {
                            tracing::error!("Invalid peer id: {:?}", addr);
                            continue;
                        };
                        self.swarm.add_external_address(addr.clone().with(Protocol::P2p(peer_id)));
                        self.swarm.dial(addr.clone().with(Protocol::P2p(peer_id)))?;
                        // self.swarm.behaviour_mut().kad.add_address(&peer_id, addr.clone());

                    }

                    else if line.starts_with("/peers") {
                        self.log_peers();
                    }

                    else if line.starts_with("/check") {
                        // self.command_check_peers_for_topic().await;
                        self.announce_self();
                    }

                    else if let Err(e) = self.swarm
                        .behaviour_mut().gossipsub
                        .publish(topic.clone(), line.as_bytes()) {
                        println!("Publish error: {e:?}");
                    }
                }

                event = self.swarm.select_next_some() => self.handle_event(event).await,
            }
        }
    }

    async fn handle_event(&mut self, event: SwarmEvent<MintpoolBehaviourEvent>) {
        match event {
            SwarmEvent::NewListenAddr { address, .. } => {
                let pid = self.swarm.local_peer_id();
                eprintln!("Listening:: {:?}", address.with(Protocol::P2p(pid.clone())));
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

                        //
                        // match b.kad.bootstrap() {
                        //     Err(err) => tracing::info!(
                        //         "Failed to boostrap while connecting to a new node {:?}",
                        //         err
                        //     ),
                        //     Ok(q) => {
                        //         tracing::info!("Bootstrapped successfully: {:?}", q);
                        //         self.log_peers();
                        //     }
                        // }

                        tracing::info!("Dialed: {:?}", addr);
                        // self.register_to_dht()
                        // self.announce_self();
                    }
                    ConnectedPoint::Listener {
                        local_addr,
                        send_back_addr,
                    } => {
                        let addr = send_back_addr.with(Protocol::P2p(peer_id));
                        // self.swarm.add_external_address(addr.clone());
                        // self.swarm
                        //     .behaviour_mut()
                        //     .kad
                        //     .add_address(&peer_id, addr.clone());
                        tracing::info!("Was connected to by: {:?} local: {local_addr}", addr);
                    }
                }

                tracing::info!("Connection established with peer: {:?}", peer_id);
            }
            SwarmEvent::ConnectionClosed { .. } => tracing::info!("Connection closed"),
            SwarmEvent::OutgoingConnectionError { peer_id, error, .. } => {
                if let Some(peer_id) = peer_id {
                    tracing::warn!("connection error to peer {:?}: {:?}", peer_id, error);
                }
            }

            SwarmEvent::Behaviour(MintpoolBehaviourEvent::Kad(kad_event)) => match kad_event {
                kad::Event::InboundRequest { request } => {
                    tracing::info!("Inbound request: {:?}", request);
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
                kad::Event::UnroutablePeer { .. } => {}
                kad::Event::RoutablePeer { .. } => {
                    tracing::info!("Routable peer");
                }
                kad::Event::PendingRoutablePeer { .. } => {
                    tracing::info!("Pending routable peer");
                }
                kad::Event::ModeChanged { .. } => {}
                kad::Event::OutboundQueryProgressed { result, .. } => {
                    tracing::info!("Outbound query progressed: {:?}", result);
                    match result {
                        kad::QueryResult::GetRecord(Ok(query_ok)) => {
                            tracing::info!("GetRecordOk: {:?}", query_ok);
                            match query_ok {
                                GetRecordOk::FoundRecord(record) => {
                                    let peer = String::from_utf8_lossy(&record.record.value);

                                    tracing::info!("Peer found: {:?}", peer);
                                    let known = self.swarm.external_addresses().collect::<Vec<_>>();
                                    tracing::info!("Known peers: {:?}", known);

                                    let ma = Multiaddr::try_from(peer.to_string())
                                        .expect("failed to parse multiaddr for peer");
                                    self.swarm.add_external_address(ma.clone());
                                    self.swarm.dial(ma).expect("Failed to dial");
                                    let known = self.swarm.external_addresses().collect::<Vec<_>>();
                                    tracing::info!("Known peers: {:?}", known);
                                }

                                GetRecordOk::FinishedWithNoAdditionalRecord {
                                    cache_candidates,
                                } => {
                                    tracing::info!(
                                        "Finished with no additional record: {:?}",
                                        cache_candidates
                                    );
                                }
                            }
                        }
                        kad::QueryResult::PutRecord(Err(err)) => {
                            tracing::info!("PutRecord error: {:?}", err);
                        }
                        QueryResult::Bootstrap(_) => {
                            tracing::info!("bootstrap")
                        }
                        QueryResult::GetClosestPeers(_) => {
                            tracing::info!("get closest peers")
                        }
                        QueryResult::GetProviders(_) => {
                            tracing::info!("get providers")
                        }
                        QueryResult::StartProviding(_) => {
                            tracing::info!("start providing")
                        }
                        QueryResult::RepublishProvider(_) => {
                            tracing::info!("republish provider")
                        }
                        QueryResult::RepublishRecord(_) => {
                            tracing::info!("replish record")
                        }
                        QueryResult::GetRecord(Err(err)) => {
                            tracing::info!("get record error: {:?}", err)
                        }
                        QueryResult::PutRecord(Ok(v)) => {
                            tracing::info!("put record ok: {:?}", v)
                        }
                    }
                }
            },
            SwarmEvent::Behaviour(MintpoolBehaviourEvent::Gossipsub(gossipsub_event)) => {
                let registry_topic = gossipsub::IdentTopic::new("announce::premints");
                match gossipsub_event {
                    gossipsub::Event::Message { message, .. } => {
                        let msg = String::from_utf8_lossy(&message.data);
                        if message.topic == registry_topic.hash() {
                            tracing::info!("Need peer: {:?}", msg);
                            let addr: Multiaddr = msg
                                .to_string()
                                .parse()
                                .expect("couldnt parse multiaddr for register");
                            self.swarm.dial(addr).expect("couldnt dial new addr");
                        } else {
                            tracing::info!("Received message: {:?}", msg);
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
            }

            SwarmEvent::IncomingConnectionError { .. } => {
                tracing::info!("Incoming connection error")
            }
            SwarmEvent::ExpiredListenAddr { .. } => {
                tracing::info!("Expired listen address")
            }
            SwarmEvent::ListenerClosed { .. } => {
                tracing::info!("Listener closed")
            }
            SwarmEvent::ListenerError { .. } => {
                tracing::info!("Listener error")
            }
            SwarmEvent::Dialing { peer_id, .. } => {
                tracing::info!("Dialing: {:?}", peer_id)
            }
            SwarmEvent::NewExternalAddrCandidate { .. } => {
                tracing::info!("New external address candidate")
            }
            SwarmEvent::ExternalAddrConfirmed { .. } => {
                tracing::info!("External address confirmed")
            }
            SwarmEvent::ExternalAddrExpired { .. } => {
                tracing::info!("External address expired")
            }
            _ => {
                tracing::info!("Unhandled event: {:?}", event)
            }
        }
    }

    fn announce_self(&mut self) {
        let peer_id = self.swarm.local_peer_id().clone();
        let listening_on = self.swarm.listeners().collect::<Vec<_>>();
        tracing::info!("announcing, listening on: {:?}", listening_on);
        let value = if let Some(addr) = self.swarm.listeners().collect::<Vec<_>>().first() {
            let m = addr.clone().clone().with(Protocol::P2p(peer_id.clone()));
            tracing::info!("sending full address: {:?}", m.to_string());
            m.to_string()
        } else {
            peer_id.to_string()
        };
        let registry_topic = gossipsub::IdentTopic::new("announce::premints");

        self.swarm
            .behaviour_mut()
            .gossipsub
            .publish(registry_topic, value.as_bytes())
            .expect("failed to publish");
    }

    fn register_to_dht(&mut self) {
        let peer_id = self.swarm.local_peer_id().clone();
        let listening_on = self.swarm.listeners().collect::<Vec<_>>();
        tracing::info!("Registering to dht, listening on: {:?}", listening_on);
        let value = if let Some(addr) = self.swarm.listeners().collect::<Vec<_>>().first() {
            let m = addr.clone().clone().with(Protocol::P2p(peer_id.clone()));
            tracing::info!("sending full address: {:?}", m.to_string());
            m.to_string()
        } else {
            peer_id.to_string()
        };

        let key = "zora-premints-v2".as_bytes();

        let record = Record {
            key: kad::RecordKey::new(&key), // TODO make this just any topic
            value: value.to_string().into_bytes(),
            publisher: Some(peer_id),
            // TODO: node will need to re-register periodically
            expires: Some(Instant::now().add(Duration::from_secs(60 * 60))), // Expires once an hour and needs to be re-broadcast
        };

        if let Err(err) = self
            .swarm
            .behaviour_mut()
            .kad
            .put_record(record, Quorum::One)
        {
            tracing::error!("Failed to register to DHT: {:?}", err);
        } else {
            tracing::info!("Registered to DHT: {:?}", peer_id);
        }
    }

    async fn command_check_peers_for_topic(&mut self) {
        let key = "zora-premints-v2".as_bytes();
        let query_result = self
            .swarm
            .behaviour_mut()
            .kad
            .get_record(kad::RecordKey::new(&key));
    }

    fn log_peers(&mut self) {
        let peers: Vec<_> = self
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
        let n_peers = peers.len();
        let peers = serde_json::to_string(&peers).ok();

        let my_id = self.swarm.local_peer_id().to_string();
        tracing::info!(
            node_id = my_id,
            peers = peers,
            "Connected to {} peers via DHT",
            n_peers
        );

        let peers = self
            .swarm
            .behaviour_mut()
            .gossipsub
            .all_mesh_peers()
            .map(|p| p.to_string())
            .collect::<Vec<_>>();
        let n_peers = peers.len();
        tracing::info!(
            peers = serde_json::to_string(&peers).ok(),
            "Connected to {} peers via gossipsub",
            n_peers
        );

        tracing::info!(
            "Swarm has the following connections {:?}",
            self.swarm.external_addresses().collect::<Vec<_>>()
        );

        tracing::info!("Network info: {:?}", self.swarm.network_info());
    }
}

#[derive(NetworkBehaviour)]
pub struct MintpoolBehaviour {
    gossipsub: gossipsub::Behaviour,
    kad: kad::Behaviour<MemoryStore>,
}
