use crate::controller::ControllerCommands;
use crate::types::{InclusionClaim, Premint};
// use alloy::network::Ethereum;
use ethers::prelude::{Log, Middleware, Provider, StreamExt, Ws};
use std::sync::Arc;
use tokio::sync::mpsc::Sender;
// Temp Fix
// use alloy_provider::{Provider, RootProvider};
// use alloy_rpc_client::{RpcClient, WsConnect};

/// Checks for new premints being brought onchain then sends to controller to handle
struct MintChecker {
    chain_id: u64,
    rpc_url: String,
    channel: Sender<ControllerCommands>,
}

impl MintChecker {
    pub async fn poll_for_new_mints<T: Premint>(&self) -> eyre::Result<()> {
        let mut highest_block: Option<u64> = None;

        loop {
            let rpc = if let Ok(p) = Provider::<Ws>::connect(&self.rpc_url).await {
                Arc::new(p)
            } else {
                tracing::error!("Failed to connect to RPC, retrying...");
                tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                continue;
            };

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

            let mut stream = rpc.subscribe_logs(&filter).await?;
            while let Some(log) = stream.next().await {
                match self.log_to_claim::<T>(rpc.clone(), log.clone()).await {
                    Ok(claim) => {
                        if let Err(err) = self
                            .channel
                            .send(ControllerCommands::ResolveOnchainMint(claim))
                            .await
                        {
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

    // async fn make_provider(&self) {
    //     let ws_transport = WsConnect::new(self.rpc_url.clone());
    //
    //     // Connect to the WS client.
    //     let rpc_client = RpcClient::connect_pubsub(ws_transport).await?;
    //
    //     // Create the provider.
    //     let provider = RootProvider::<Ethereum, _>::new(rpc_client);
    // }

    async fn log_to_claim<T: Premint>(
        &self,
        rpc: Arc<Provider<Ws>>,
        log: Log,
    ) -> eyre::Result<InclusionClaim> {
        let tx_hash = log
            .transaction_hash
            .ok_or(eyre::eyre!("No tx hash in log"))?;

        let tx = rpc
            .get_transaction(tx_hash)
            .await?
            .ok_or(eyre::eyre!("No tx found"))?;

        let claim = T::map_claim(self.chain_id, tx, log)?;
        Ok(claim)
    }
}
