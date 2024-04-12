use std::borrow::Cow;

use crate::types::{InclusionClaim, Premint, PremintMetadata, PremintName};
use alloy::rpc::types::eth::{Filter, Log, Transaction};
use alloy::sol_types::private::U256;
use alloy_primitives::{address, Address};
use alloy_signer::Signer;
use alloy_signer_wallet::LocalWallet;
use alloy_sol_macro::sol;
use alloy_sol_types::{Eip712Domain, SolCall, SolEvent};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

sol! {
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    IZoraPremintV2,
    "src/premints/zora_premint_v2/zora1155PremintExecutor.json"
}

// modelled after the PremintRequest API type
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ZoraPremintV2 {
    pub collection: IZoraPremintV2::ContractCreationConfig,
    pub premint: IZoraPremintV2::PremintConfigV2,
    // pub premint: ZoraPremintConfigV2,
    pub collection_address: Address,
    pub chain_id: U256,
    pub signature: String,
}

pub static PREMINT_FACTORY_ADDR: Address = address!("7777773606e7e46C8Ba8B98C08f5cD218e31d340");

impl ZoraPremintV2 {
    pub fn eip712_domain(&self) -> Eip712Domain {
        Eip712Domain {
            name: Some(Cow::from("Preminter")),
            version: Some(Cow::from("2")),
            chain_id: Some(self.chain_id),
            verifying_contract: Some(self.collection_address),
            salt: None,
        }
    }
}

#[async_trait]
impl Premint for ZoraPremintV2 {
    fn metadata(&self) -> PremintMetadata {
        let id = format!(
            "{:?}:{:?}:{:?}",
            self.chain_id, self.collection_address, self.premint.uid
        );

        PremintMetadata {
            id,
            version: self.premint.version as u64,
            kind: PremintName("zora_premint_v2".to_string()),
            signer: self.collection.contractAdmin,
            chain_id: self.chain_id,
            collection_address: Address::default(), // TODO: source this
            token_id: U256::from(self.premint.uid),
            uri: self.premint.tokenConfig.tokenURI.clone(),
        }
    }

    fn check_filter(chain_id: u64) -> Option<Filter> {
        let supported_chains = [7777777, 8423]; // TODO: add the rest here and enable testnet mode
        if !supported_chains.contains(&chain_id) {
            return None;
        }
        Some(
            Filter::new()
                .address(PREMINT_FACTORY_ADDR)
                .event(IZoraPremintV2::PremintedV2::SIGNATURE),
        )
    }

    fn map_claim(chain_id: u64, log: Log) -> eyre::Result<InclusionClaim> {
        let event = IZoraPremintV2::PremintedV2::decode_raw_log(
            log.topics(),
            log.data().data.as_ref(),
            true,
        )?;

        Ok(InclusionClaim {
            premint_id: event.uid.to_string(),
            chain_id,
            tx_hash: log.transaction_hash.unwrap_or_default(),
            log_index: log.log_index.unwrap_or_default().to(),
            kind: "zora_premint_v2".to_string(),
        })
    }

    async fn verify_claim(chain_id: u64, tx: Transaction, log: Log, claim: InclusionClaim) -> bool {
        let event = IZoraPremintV2::PremintedV2::decode_raw_log(log.topics, &**log.data, true);
        match event {
            Ok(event) => {
                let conditions = vec![
                    log.address == PREMINT_FACTORY_ADDR,
                    log.transaction_hash.unwrap_or_default() == tx.hash,
                    claim.tx_hash == tx.hash,
                    claim.log_index == log.log_index.unwrap_or_default(),
                    claim.premint_id == event.uid.to_string(),
                    claim.kind == *"zora_premint_v2",
                    claim.chain_id == chain_id,
                ];

                // confirm all conditions are true
                conditions.into_iter().all(|x| x)
            }
            Err(e) => {
                tracing::debug!("Failed to parse log: {}", e);
                false
            }
        }
    }
}
