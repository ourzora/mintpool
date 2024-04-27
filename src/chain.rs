use crate::chain_list::{ChainListProvider, CHAINS};
use crate::controller::{ControllerCommands, ControllerInterface};
use crate::premints::zora_premint_v2::types::PREMINT_FACTORY_ADDR;
use crate::types::{InclusionClaim, Premint, PremintTypes};
use alloy::primitives::{address, Address, Bytes, TxKind};
use alloy::providers::Provider;
use alloy::rpc::types::eth::{BlockId, Filter, TransactionInput, TransactionRequest};
use alloy::sol;
use alloy::sol_types::{SolCall, SolEvent};
use futures_util::StreamExt;
use std::sync::Arc;

/// Helper function for calling view functions for SolCall types
pub async fn view_contract_call<T>(
    call: T,
    provider: &Arc<ChainListProvider>,
    address: Address,
) -> eyre::Result<T::Return>
where
    T: SolCall,
{
    provider
        .call(
            &TransactionRequest {
                to: Some(TxKind::Call(address)),
                input: TransactionInput::new(Bytes::from(call.abi_encode())),
                ..Default::default()
            },
            BlockId::latest(),
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

    /// Polls for new mints based on a filter defined by the PremintType
    pub async fn poll_for_new_mints<T: Premint>(&self) -> eyre::Result<MintCheckerResult> {
        let mut highest_block: Option<u64> = None;

        let mut filter = if let Some(filter) = T::check_filter(self.chain_id) {
            filter
        } else {
            let err = eyre::eyre!("No filter for chain / premint type, skipping spawning checker");
            tracing::warn!(error = err.to_string(), "checking failed");
            return Ok(MintCheckerResult::NoFilter);
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

pub enum MintCheckerResult {
    NoFilter,
}

/// checks the chain to ensure an inclusion claim actually does exist so we can safely prune
pub async fn inclusion_claim_correct(
    premint: &PremintTypes,
    claim: &InclusionClaim,
) -> eyre::Result<bool> {
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

    Ok(premint
        .verify_claim(claim.chain_id, tx.clone(), log.clone(), claim.clone())
        .await)
}

sol! {
    #[derive(Debug)]
    MintpoolTrustedBootnodes,
    "contracts/artifacts/abi.json"
}

const BOOTNODES_CONTRACT_ADDRESS: Address = address!("777777A1476296E9F7835C91e2B917ecbfAf83a1");
const BOOTNODES_CONTRACT_DEPLOY_BLOCK: u64 = 13_699_230;

pub async fn get_contract_boot_nodes() -> eyre::Result<Vec<String>> {
    let chain = CHAINS.get_rpc(7777777).await?;

    let filter = Filter::new()
        .address(BOOTNODES_CONTRACT_ADDRESS)
        .event(MintpoolTrustedBootnodes::TrustedNodeAdded::SIGNATURE)
        .from_block(BOOTNODES_CONTRACT_DEPLOY_BLOCK);

    let logs = chain.get_logs(&filter).await?;
    let events: Vec<MintpoolTrustedBootnodes::TrustedNodeAdded> = logs
        .iter()
        .filter_map(|log| {
            MintpoolTrustedBootnodes::TrustedNodeAdded::decode_raw_log(
                log.topics(),
                log.data().data.as_ref(),
                true,
            )
            .ok()
        })
        .collect();

    let nodes = events
        .iter()
        .map(|event| event.node.to_string())
        .collect::<Vec<String>>();

    let result = view_contract_call(
        MintpoolTrustedBootnodes::isTrustedNode_1Call {
            _nodes: nodes.clone(),
        },
        &chain,
        BOOTNODES_CONTRACT_ADDRESS,
    )
    .await?;

    let valid_nodes = result
        ._0
        .into_iter()
        .zip(nodes.iter())
        .filter_map(
            |(is_trusted, node)| {
                if is_trusted {
                    Some(node.clone())
                } else {
                    None
                }
            },
        )
        .collect::<Vec<String>>();

    Ok(valid_nodes)
}

#[cfg(test)]
mod test {
    use crate::chain::get_contract_boot_nodes;

    #[tokio::test]
    async fn test_get_bootnodes_from_chain() {
        let nodes = get_contract_boot_nodes().await.unwrap();

        assert_eq!(
            nodes,
            vec![
                "/dnsaddr/mintpool-1.zora.co".to_string(),
                "/dnsaddr/mintpool-2.zora.co".to_string(),
                "/dnsaddr/mintpool-3.zora.co".to_string()
            ]
        );
    }
}
