use crate::chain_list::Chains;
use crate::controller::ControllerCommands;
use crate::types::Premint;
use alloy::network::Ethereum;
use alloy_provider::Provider;
use futures_util::StreamExt;
use tokio::sync::mpsc::Sender;

/// Checks for new premints being brought onchain then sends to controller to handle
struct MintChecker {
    chain_id: u64,
    rpc_url: String,
    channel: Sender<ControllerCommands>,
}

impl MintChecker {
    pub async fn poll_for_new_mints<T: Premint>(&self) -> eyre::Result<()> {
        let mut highest_block: Option<u64> = None;

        let rpc = self.make_provider().await?;
        let mut filter = if let Some(filter) = T::check_filter(self.chain_id) {
            filter
        } else {
            let err = eyre::eyre!("No filter for chain / premint type, skipping spawning checker");
            tracing::warn!(error = err.to_string(), "checking failed");
            return Err(err);
        };

        loop {
            // set start block incase of WS disconnect
            if let Some(highest_block) = highest_block {
                filter = filter.from_block(highest_block);
            }
            let mut stream = rpc.subscribe_logs(&filter).await?.into_stream();

            while let Some(log) = stream.next().await {
                match T::map_claim(self.chain_id, log.clone()) {
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
                    highest_block = Some(block_number.to());
                }
            }
        }
    }

    async fn make_provider(&self) -> eyre::Result<Box<dyn Provider<Ethereum>>> {
        let chains = Chains::new();

        let chain = chains.get_chain_by_id(self.chain_id as i64);

        match chain {
            Some(c) => c.get_rpc(true).await,
            None => Err(eyre::eyre!("Chain not found for id {}", self.chain_id)),
        }
    }
}
