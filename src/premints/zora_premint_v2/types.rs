use std::borrow::Cow;

use alloy::rpc::types::eth::{Filter, Log, Transaction};
use alloy::sol_types::private::U256;
use alloy_primitives::{address, Address};
use alloy_signer::Signer;
use alloy_signer_wallet::LocalWallet;
use alloy_sol_macro::sol;
use alloy_sol_types::{eip712_domain, Eip712Domain, SolEvent};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::types::{InclusionClaim, Premint, PremintMetadata, PremintName};

sol! {
    event PremintedV2(
        address indexed contractAddress,
        uint256 indexed tokenId,
        bool indexed createdNewContract,
        uint32 uid,
        address minter,
        uint256 quantityMinted
    );

    #[derive(Debug, Serialize, Deserialize, PartialEq, Default)]
    struct ContractCreationConfig {
        // Creator/admin of the created contract.  Must match the account that signed the message
        address contractAdmin;
        // Metadata URI for the created contract
        string contractURI;
        // Name of the created contract
        string contractName;
    }

    #[derive(Debug, Serialize, Deserialize, PartialEq, Default)]
    struct TokenCreationConfig {
        // Metadata URI for the created token
        string tokenURI;
        // Max supply of the created token
        uint256 maxSupply;
        // Max tokens that can be minted for an address, 0 if unlimited
        uint64 maxTokensPerAddress;
        // Price per token in eth wei. 0 for a free mint.
        uint96 pricePerToken;
        // The start time of the mint, 0 for immediate.  Prevents signatures from being used until the start time.
        uint64 mintStart;
        // The duration of the mint, starting from the first mint of this token. 0 for infinite
        uint64 mintDuration;
        // RoyaltyBPS for created tokens. The royalty amount in basis points for secondary sales.
        uint32 royaltyBPS;
        // This is the address that will be set on the `royaltyRecipient` for the created token on the 1155 contract,
        // which is the address that receives creator rewards and secondary royalties for the token,
        // and on the `fundsRecipient` on the ZoraCreatorFixedPriceSaleStrategy contract for the token,
        // which is the address that receives paid mint funds for the token.
        address payoutRecipient;
        // Fixed price minter address
        address fixedPriceMinter;
        // create referral
        address createReferral;
    }

    #[derive(Debug, Serialize, Deserialize, PartialEq, Default)]
    struct CreatorAttribution {
        // The config for the token to be created
        TokenCreationConfig tokenConfig;
        // Unique id of the token, used to ensure that multiple signatures can't be used to create the same intended token.
        // only one signature per token id, scoped to the contract hash can be executed.
        uint32 uid;
        // Version of this premint, scoped to the uid and contract.  Not used for logic in the contract, but used externally to track the newest version
        uint32 version;
        // If executing this signature results in preventing any signature with this uid from being minted.
        bool deleted;
    }
}

// renaming solidity type not implemented yet in alloy-sol-types,
// so we alias the generated type.
pub type ZoraPremintConfigV2 = CreatorAttribution;

// modelled after the PremintRequest API type
#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ZoraPremintV2 {
    pub collection: ContractCreationConfig,
    pub premint: ZoraPremintConfigV2,
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
        PremintMetadata {
            id: self.premint.uid.to_string(),
            kind: Self::kind_id(),
            signer: self.collection.contractAdmin,
            chain_id: self.chain_id,
            collection_address: Address::default(), // TODO: source this
            token_id: U256::from(self.premint.uid),
            uri: self.premint.tokenConfig.tokenURI.clone(),
        }
    }

    fn guid(&self) -> String {
        format!(
            "{:?}:{:?}:{:?}:{:?}",
            self.chain_id, self.collection_address, self.premint.uid, self.premint.version
        )
    }

    fn check_filter(chain_id: u64) -> Option<Filter> {
        let supported_chains = [7777777, 8423]; // TODO: add the rest here and enable testnet mode
        if !supported_chains.contains(&chain_id) {
            return None;
        }
        Some(
            Filter::new()
                .address(PREMINT_FACTORY_ADDR)
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
                    log.address == PREMINT_FACTORY_ADDR,
                    log.transaction_hash.unwrap_or_default() == tx.hash,
                    claim.tx_hash == tx.hash,
                    claim.log_index == log.log_index.unwrap_or_default().to::<u64>(),
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

    fn kind_id() -> PremintName {
        PremintName("zora_premint_v2".to_string())
    }
}
