use alloy::network::Ethereum;
use alloy::rpc::types::eth::{TransactionInput, TransactionRequest};
use alloy_primitives::Bytes;
use alloy_provider::Provider;
use alloy_sol_types::SolCall;
use futures_util::StreamExt;
use tokio::sync::mpsc::Sender;

use crate::chain_list::{ChainListProvider, CHAINS};
use crate::controller::ControllerCommands;
use crate::premints::zora_premint_v2::types::PREMINT_FACTORY_ADDR;
use crate::types::Premint;

pub async fn contract_call<T, P>(call: T, provider: P) -> eyre::Result<T::Return>
where
    T: SolCall,
    P: Provider<Ethereum>,
{
    provider
        .call(
            &TransactionRequest {
                to: Some(PREMINT_FACTORY_ADDR),
                input: TransactionInput::new(Bytes::from(call.abi_encode())),
                ..Default::default()
            },
            None,
        )
        .await
        .map_err(|err| eyre::eyre!("Error calling contract: {:?}", err))
        .and_then(|response| {
            T::abi_decode_returns(&**response, false)
                .map_err(|err| eyre::eyre!("Error decoding contract response: {:?}", err))
        })
}

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

    async fn make_provider(&self) -> eyre::Result<ChainListProvider> {
        let chain = CHAINS.get_chain_by_id(self.chain_id as i64);

        match chain {
            Some(c) => c.get_rpc(true).await,
            None => Err(eyre::eyre!("Chain not found for id {}", self.chain_id)),
        }
    }
}
