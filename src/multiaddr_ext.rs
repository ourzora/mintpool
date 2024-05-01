use libp2p::multiaddr::Protocol;
use libp2p::PeerId;

pub trait MultiaddrExt {
    // get the last peer id from this address
    fn peer_id(&self) -> Option<PeerId>;

    fn is_relayed(&self) -> bool;
}

impl MultiaddrExt for libp2p::Multiaddr {
    fn peer_id(&self) -> Option<PeerId> {
        let mut last = None;
        self.iter().for_each(|component| {
            if let Protocol::P2p(key) = component {
                last = Some(key.clone());
            }
        });

        last
    }

    fn is_relayed(&self) -> bool {
        self.iter().any(|addr| matches!(addr, Protocol::P2pCircuit))
    }
}
