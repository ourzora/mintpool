use std::fmt::Debug;

use libp2p::{Multiaddr, PeerId};

#[derive(Debug)]
pub struct MintpoolNodeInfo {
    pub peer_id: PeerId,
    pub addr: Vec<Multiaddr>,
}
