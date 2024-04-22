use std::sync::Arc;

use alloy::rpc::types::eth::{TransactionInput, TransactionRequest};
use alloy_primitives::Bytes;
use alloy_provider::Provider;
use alloy_sol_types::SolCall;
use futures_util::StreamExt;

use crate::chain_list::{ChainListProvider, CHAINS};
use crate::controller::{ControllerCommands, ControllerInterface};
use crate::premints::zora_premint_v2::types::PREMINT_FACTORY_ADDR;
use crate::types::{InclusionClaim, Premint};

pub async fn contract_call<T>(call: T, provider: &Arc<ChainListProvider>) -> eyre::Result<T::Return>
where
    T: SolCall,
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
            T::abi_decode_returns(&response, false)
                .map_err(|err| eyre::eyre!("Error decoding contract response: {:?}", err))
        })
}

/// Checks for new premints being brought onchain then sends to controller to handle
pub struct MintChecker {
    chain_id: u64,
    controller: ControllerInterface,
    rpc_url: String,
}

impl MintChecker {
    pub fn new(chain_id: u64, rpc_url: String, controller: ControllerInterface) -> Self {
        Self {
            chain_id,
            controller,
            rpc_url, // needed in case of WS disconnect so mintchecker can force a reconnect
        }
    }

    pub async fn poll_for_new_mints<T: Premint>(&self) -> eyre::Result<()> {
        let mut highest_block: Option<u64> = None;

        let mut filter = if let Some(filter) = T::check_filter(self.chain_id) {
            filter
        } else {
            let err = eyre::eyre!("No filter for chain / premint type, skipping spawning checker");
            tracing::warn!(error = err.to_string(), "checking failed");
            return Err(err);
        };

        loop {
            let rpc = match self.make_provider().await {
                Ok(rpc) => rpc,
                Err(e) => {
                    tracing::error!("Error getting provider: {}", e);
                    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                    continue;
                }
            };
            tracing::info!(
                "Starting checker for chain {}, {}",
                self.chain_id,
                self.rpc_url
            );

            // set start block in case of WS disconnect
            if let Some(highest_block) = highest_block {
                filter = filter.from_block(highest_block);
            }
            let mut stream = match rpc.subscribe_logs(&filter).await {
                Ok(t) => t.into_stream(),
                Err(e) => {
                    tracing::error!("Error subscribing to logs: {}", e);
                    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                    continue;
                }
            };

            while let Some(log) = stream.next().await {
                tracing::debug!("Saw log");
                match T::map_claim(self.chain_id, log.clone()) {
                    Ok(claim) => {
                        tracing::debug!("Found claim of inclusion {:?}", claim);
                        if let Err(err) = self
                            .controller
                            .send_command(ControllerCommands::ResolveOnchainMint(claim))
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
                    highest_block = Some(block_number);
                }
            }
        }
    }

    async fn make_provider(&self) -> eyre::Result<Arc<ChainListProvider>> {
        CHAINS.get_rpc(self.chain_id).await
    }
}

/// checks the chain to ensure an inclusion claim actually does exist so we can safely prune
pub async fn inclusion_claim_correct(claim: InclusionClaim) -> eyre::Result<bool> {
    let chain = CHAINS.get_rpc(claim.chain_id).await?;
    let tx = chain
        .get_transaction_receipt(claim.tx_hash)
        .await?
        .ok_or(eyre::eyre!("transaction not found"))?;

    let log = tx
        .inner
        .logs()
        .get(claim.log_index as usize)
        .ok_or(eyre::eyre!("log index not found: {}", claim.log_index))?;

    todo!();
}
