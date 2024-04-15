use std::borrow::Cow;

use crate::premints::zora_premint_v2::types::IZoraPremintV2::PremintedV2;
use crate::types::{InclusionClaim, Premint, PremintMetadata, PremintName};
use alloy::rpc::types::eth::{Filter, Log, Transaction};
use alloy::sol_types::private::U256;
use alloy_primitives::{address, Address};
use alloy_sol_macro::sol;
use alloy_sol_types::{Eip712Domain, SolEvent};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

sol! {
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    IZoraPremintV2,
    "src/premints/zora_premint_v2/zora1155PremintExecutor.json"
}

// aliasing the types here for readability. the original name need to stay
// because they impact signature generation
pub type PremintConfig = IZoraPremintV2::CreatorAttribution;
pub type TokenCreationConfig = IZoraPremintV2::TokenCreationConfig;
pub type ContractCreationConfig = IZoraPremintV2::ContractCreationConfig;

// modelled after the PremintRequest API type
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ZoraPremintV2 {
    pub collection: ContractCreationConfig,
    pub premint: PremintConfig,
    pub collection_address: Address,
    pub chain_id: u64,
    pub signature: String,
}

impl Default for ZoraPremintV2 {
    fn default() -> Self {
        Self {
            collection: ContractCreationConfig {
                contractAdmin: Default::default(),
                contractURI: "".to_string(),
                contractName: "".to_string(),
            },
            premint: PremintConfig {
                tokenConfig: TokenCreationConfig {
                    tokenURI: "".to_string(),
                    maxSupply: Default::default(),
                    maxTokensPerAddress: 0,
                    pricePerToken: 0,
                    mintStart: 0,
                    mintDuration: 0,
                    royaltyBPS: 0,
                    payoutRecipient: Default::default(),
                    fixedPriceMinter: Default::default(),
                    createReferral: Default::default(),
                },
                uid: 0,
                version: 0,
                deleted: false,
            },
            collection_address: Address::default(),
            chain_id: 0,
            signature: String::default(),
        }
    }
}

pub static PREMINT_FACTORY_ADDR: Address = address!("7777773606e7e46C8Ba8B98C08f5cD218e31d340");

impl ZoraPremintV2 {
    pub fn eip712_domain(&self) -> Eip712Domain {
        Eip712Domain {
            name: Some(Cow::from("Preminter")),
            version: Some(Cow::from("2")),
            chain_id: Some(U256::from(self.chain_id)),
            verifying_contract: Some(self.collection_address),
            salt: None,
        }
    }

    /// Recreate a deterministic GUID for a premint
    fn event_to_guid(chain_id: u64, event: &PremintedV2) -> String {
        format!("{:?}:{:?}:{:?}", chain_id, event.contractAddress, event.uid)
    }
}

#[async_trait]
impl Premint for ZoraPremintV2 {
    fn metadata(&self) -> PremintMetadata {
        let id = format!(
            "{:?}:{:?}:{:?}",
            self.chain_id, self.collection_address, self.premint.uid
        );

        println!("Premint ID: {}", id);

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

        let id = Self::event_to_guid(chain_id, &event);

        println!("Retreived premint ID: {}", id);

        Ok(InclusionClaim {
            premint_id: id,
            chain_id,
            tx_hash: log.transaction_hash.unwrap_or_default(),
            log_index: log.log_index.unwrap_or_default(),
            kind: "zora_premint_v2".to_string(),
        })
    }

    async fn verify_claim(chain_id: u64, tx: Transaction, log: Log, claim: InclusionClaim) -> bool {
        let event =
            IZoraPremintV2::PremintedV2::decode_raw_log(log.topics(), &log.data().data, true);
        match event {
            Ok(event) => {
                let conditions = vec![
                    log.address() == PREMINT_FACTORY_ADDR,
                    log.transaction_hash.unwrap_or_default() == tx.hash,
                    claim.tx_hash == tx.hash,
                    claim.log_index == log.log_index.unwrap_or_default(),
                    claim.premint_id == Self::event_to_guid(chain_id, &event),
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
