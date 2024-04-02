use std::collections::HashMap;
use std::fmt::Debug;
use alloy_primitives::Address;
use alloy_signer::k256::U256;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

pub mod zora_premint_v2;
pub mod simple_premint;

pub use simple_premint::types::SimplePremint;
pub use zora_premint_v2::types::ZoraPremintConfigV2;

#[derive(Debug)]
pub struct PremintMetadata {
    pub id: String,
    pub kind: String,
    pub signer: Address,
    pub chain_id: i64,
    pub collection_address: Address,
    pub token_id: U256,
    pub uri: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct ERC721Metadata {
    pub name: String,
    pub description: String,
    pub image: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct ERC1155Metadata {
    pub name: String,
    pub description: String,
    pub image: String,
    pub decimals: u64,
    pub properties: Map<String, Value>
}

enum Metadata {
    ERC721(ERC721Metadata),
    ERC1155(ERC1155Metadata),
}

pub trait Premint {
    fn metadata(&self) -> PremintMetadata;
}

struct Envelope {
    premint: PremintTypes,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum PremintTypes {
    Simple(SimplePremint),
    ZoraV2(ZoraPremintConfigV2),
}

impl PremintTypes {
    pub fn from_json(line: String) -> eyre::Result<Self> {
        let p: PremintTypes = serde_json::from_str(&line)?;
        Ok(p)
    }

    pub fn to_json(&self) -> eyre::Result<String> {
        let p: String = serde_json::to_string(&self)?;
        Ok(p)
    }
}

impl Premint for PremintTypes {
    fn metadata(&self) -> PremintMetadata {
        match self {
            PremintTypes::Simple(p) => p.metadata(),
            PremintTypes::ZoraV2(p) => p.metadata(),
        }
    }
}

#[cfg(test)]
mod test {
    use ruint::aliases::U256;
    use crate::premints::zora_premint_v2::types::TokenCreationConfig;
    use super::*;

    fn get_config() -> ZoraPremintConfigV2 {
        let config = TokenCreationConfig {
            tokenURI: "ipfs://tokenIpfsId0".to_string(),
            maxSupply: U256::from(100000000000000000u128),
            maxTokensPerAddress: 10,
            pricePerToken: 0,
            mintStart: 0,
            mintDuration: 100,
            royaltyBPS: 8758,
            payoutRecipient: "0x70997970C51812dc3A010C7d01b50e0d17dc79C8".parse().unwrap(),
            fixedPriceMinter: "0x7e5A9B6F4bB9efC27F83E18F29e4326480668f87".parse().unwrap(),
            createReferral: "0x63779E68424A0746cF04B2bc51f868185a7660dF".parse().unwrap(),
        };

        ZoraPremintConfigV2 {
            tokenConfig: config,
            uid: 105,
            version: 0,
            deleted: false,
        }
    }

    #[tokio::test]
    async fn test_signature() {
        let config = get_config();

        let typ = PremintTypes::ZoraV2(config);

        println!("{}", typ.to_json().unwrap());
    }

    #[test]
    fn test_premint_serde() {
        let premint = PremintTypes::Simple(SimplePremint {
            chain_id: 1,
            sender: "0x66f9664f97F2b50F62D13eA064982f936dE76657"
                .parse()
                .unwrap(),
            token_id: 1,
            media: "https://ipfs.io/ipfs/Qm".to_string(),
        });

        let json = premint.to_json().unwrap();
        println!("{}", json);
        let premint = PremintTypes::from_json(json).unwrap();
        println!("{:?}", premint);

        let premint = PremintTypes::ZoraV2(Default::default());
        let json = premint.to_json().unwrap();
        println!("{}", json);
        let premint: PremintTypes = PremintTypes::from_json(json).unwrap();
        println!("{:?}", premint);
    }
}