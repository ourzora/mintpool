use crate::types::{InclusionClaim, Premint};
use ethers::prelude::{Filter, Log, Middleware, Provider, StreamExt, Ws};
use std::sync::Arc;
use tokio::sync::mpsc::Sender;

/// Checks for new premints being brought onchain then sends to controller to handle
struct MintChecker {
    chain_id: u64,
    rpc: Arc<Provider<Ws>>,
    channel: Sender<InclusionClaim>,
}

impl MintChecker {
    pub async fn poll_for_new_mints<T: Premint>(&self) -> eyre::Result<()> {
        let mut highest_block: Option<u64> = None;

        loop {
            let mut filter = if let Some(filter) = T::check_filter(self.chain_id) {
                filter
            } else {
                tracing::warn!("No filter for chain / premint type, skipping spawning checker");
                return Err(eyre::eyre!(
                    "No filter for chain / premint type, skipping spawning checker"
                ));
            };

            if let Some(highest_block) = highest_block {
                filter = filter.from_block(highest_block);
            }

            let mut stream = self.rpc.subscribe_logs(&filter).await?;
            while let Some(log) = stream.next().await {
                match self.log_to_claim::<T>(log.clone()).await {
                    Ok(claim) => {
                        if let Err(err) = self.channel.send(claim).await {
                            tracing::error!("Error sending claim to controller: {}", err);
                        }
                    }
                    Err(e) => {
                        tracing::error!("Error processing log while checking premint: {}", e);
                    }
                }
                if let Some(block_number) = log.block_number {
                    highest_block = Some(block_number.as_u64());
                }
            }
        }
    }

    async fn log_to_claim<T: Premint>(&self, log: Log) -> eyre::Result<InclusionClaim> {
        let tx_hash = log
            .transaction_hash
            .ok_or(eyre::eyre!("No tx hash in log"))?;

        let tx = self
            .rpc
            .get_transaction(tx_hash)
            .await?
            .ok_or(eyre::eyre!("No tx found"))?;

        let claim = T::map_claim(self.chain_id, tx, log)?;
        Ok(claim)
    }
}
