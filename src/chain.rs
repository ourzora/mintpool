use ethers::prelude::{Filter, Provider, Ws};
use std::sync::Arc;

/// Checks for new premints being brought onchain then sends to controller to handle
struct MintChecker {
    chain_id: u64,
    rpc: Arc<Provider<Ws>>,
}

impl MintChecker {
    pub async fn poll_for_new_mints(&self, event_signature: String) {}
}
