use alloy::{
    rpc::types::eth::{Filter, Log, Transaction},
    sol_types::sol,
};

use alloy_primitives::{address, Address, B256, U256};
use alloy_sol_types::SolEvent;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
// use alloy_sol_types::SolEvent;
use crate::types::{InclusionClaim, Premint, PremintMetadata, PremintName};

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PremintV2Message {
    collection: PremintV2Collection,
    premint: PremintV2,
    chain_id: u64,
    signature: String,
}

sol!(
    event PremintedV2(
        address indexed contractAddress,
        uint256 indexed tokenId,
        bool indexed createdNewContract,
        uint32 uid,
        address minter,
        uint256 quantityMinted
    );
);

static PREMINT_FACTORY_ADDR: Address = address!("7777773606e7e46C8Ba8B98C08f5cD218e31d340");

// Lazy::new(|| {
//     address!("0x7777773606e7e46C8Ba8B98C08f5cD218e31d340")
// });

#[async_trait]
impl Premint for PremintV2Message {
    fn metadata(&self) -> PremintMetadata {
        PremintMetadata {
            id: self.premint.uid.to_string(),
            kind: Self::kind_id(),
            signer: self.collection.contract_admin,
            chain_id: self.chain_id as i64,
            collection_address: Address::default(), // TODO: source this
            token_id: U256::from(self.premint.uid),
            uri: self.premint.token_creation_config.token_uri.clone(),
        }
    }

    fn check_filter(chain_id: u64) -> Option<Filter> {
        let supported_chains = vec![7777777, 8423]; // TODO: add the rest here and enable testnet mode
        if !supported_chains.contains(&chain_id) {
            return None;
        }
        Some(
            Filter::new()
                .address(PREMINT_FACTORY_ADDR.clone())
                .event(PremintedV2::SIGNATURE),
        )
    }

    fn map_claim(chain_id: u64, log: Log) -> eyre::Result<InclusionClaim> {
        let event = PremintedV2::decode_raw_log(&log.topics, &log.data, true)?;

        Ok(InclusionClaim {
            premint_id: event.uid.to_string(),
            chain_id,
            tx_hash: log.transaction_hash.unwrap_or_default(),
            log_index: log.log_index.unwrap_or(U256::from(0)).to(),
            kind: "zora_premint_v2".to_string(),
        })
    }

    async fn verify_claim(chain_id: u64, tx: Transaction, log: Log, claim: InclusionClaim) -> bool {
        let event = PremintedV2::decode_raw_log(&log.topics, &log.data, true);
        match event {
            Ok(event) => {
                let conditions = vec![
                    log.address == PREMINT_FACTORY_ADDR.clone(),
                    log.transaction_hash.unwrap_or(B256::default()) == tx.hash,
                    claim.tx_hash == tx.hash,
                    claim.log_index == log.log_index.unwrap_or(U256::default()).to::<u64>(),
                    claim.premint_id == event.uid.to_string(),
                    claim.kind == "zora_premint_v2".to_string(),
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

    fn kind_id() -> PremintName {
        PremintName("zora_premint_v2".to_string())
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PremintV2Collection {
    contract_admin: Address,
    contract_uri: String,
    contract_name: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PremintV2 {
    token_creation_config: TokenCreationConfigV2,
    uid: u64,
    version: u64,
    deleted: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TokenCreationConfigV2 {
    token_uri: String,
    token_max_supply: U256,
    mint_start: u64,
    mint_duration: u64,
    max_tokens_per_address: U256,
    price_per_token: U256,
    #[serde(rename = "royaltyBPS")]
    royalty_bps: U256,
    payout_recipient: Address,
    fixed_price_minter: Address,
    creator_referral: Address,
}
