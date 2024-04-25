use std::borrow::Cow;

use alloy::sol_types::private::U256;
use alloy_primitives::Address;
use alloy_sol_types::Eip712Domain;
use serde::{Deserialize, Serialize};

use crate::premints::zora_premint::contract::IZoraPremintERC20V1;

// aliasing the types here for readability. the original name need to stay
// because they impact signature generation
pub type PremintConfigERC20V1 = IZoraPremintERC20V1::CreatorAttribution;
pub type TokenCreationConfigERC20V1 = IZoraPremintERC20V1::TokenCreationConfig;
pub type ContractCreationConfigERC20V1 = IZoraPremintERC20V1::ContractCreationConfig;

// modelled after the PremintRequest API type
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ZoraPremintERC20V1 {
    pub collection: ContractCreationConfigERC20V1,
    pub premint: PremintConfigERC20V1,
    pub collection_address: Address,
    pub chain_id: u64,
    pub signature: String,
}

impl Default for ZoraPremintERC20V1 {
    fn default() -> Self {
        Self {
            collection: ContractCreationConfigERC20V1 {
                contractAdmin: Default::default(),
                contractURI: "".to_string(),
                contractName: "".to_string(),
            },
            premint: PremintConfigERC20V1 {
                tokenConfig: TokenCreationConfigERC20V1 {
                    tokenURI: "".to_string(),
                    maxSupply: Default::default(),
                    maxTokensPerAddress: 0,
                    currency: Default::default(),
                    pricePerToken: U256::try_from(0).unwrap(),
                    mintStart: 0,
                    mintDuration: 0,
                    royaltyBPS: 0,
                    payoutRecipient: Default::default(),
                    createReferral: Default::default(),
                    erc20Minter: Default::default(),
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

impl ZoraPremintERC20V1 {
    pub fn eip712_domain(&self) -> Eip712Domain {
        Eip712Domain {
            name: Some(Cow::from("Preminter")),
            version: Some(Cow::from("ERC20_1")),
            chain_id: Some(U256::from(self.chain_id)),
            verifying_contract: Some(self.collection_address),
            salt: None,
        }
    }

    /// Recreate a deterministic GUID for a premint
    fn event_to_guid(chain_id: u64, event: &IZoraPremintERC20V1::Preminted) -> String {
        format!("{:?}:{:?}:{:?}", chain_id, event.contractAddress, event.uid)
    }
}
