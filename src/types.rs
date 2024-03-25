use alloy_primitives::private::derive_more::Display;
use alloy_primitives::{Address, U256};
use libp2p::{gossipsub, Multiaddr, PeerId};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;

#[derive(Debug, Display)]
pub struct PremintName(pub String);

impl PremintName {
    pub fn msg_topic(&self) -> gossipsub::IdentTopic {
        gossipsub::IdentTopic::new(format!("mintpool::{:?}", self))
    }
}

#[derive(Debug)]
pub struct MintpoolNodeInfo {
    pub peer_id: PeerId,
    pub addr: Vec<Multiaddr>,
}

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

pub trait Premint: Serialize + DeserializeOwned + Debug + Clone {
    fn metadata(&self) -> PremintMetadata;
    fn kind_id() -> PremintName;
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum PremintTypes {
    Simple(SimplePremint),
    V2(PremintV2Message),
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

impl PremintTypes {
    pub fn metadata(&self) -> PremintMetadata {
        match self {
            PremintTypes::Simple(p) => p.metadata(),
            PremintTypes::V2(p) => p.metadata(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
pub struct SimplePremint {
    chain_id: u64,
    sender: Address,
    token_id: u64,
    media: String,
}

impl Premint for SimplePremint {
    fn metadata(&self) -> PremintMetadata {
        PremintMetadata {
            id: format!("{:?}:{:?}:{:?}", self.chain_id, self.sender, self.token_id),
            kind: "simple".to_string(),
            signer: self.sender,
            chain_id: self.chain_id as i64,
            collection_address: Address::default(),
            token_id: U256::from(self.token_id),
            uri: self.media.clone(),
        }
    }

    fn kind_id() -> PremintName {
        PremintName("simple".to_string())
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PremintV2Message {
    collection: PremintV2Collection,
    premint: PremintV2,
    chain_id: u64,
    signature: String,
}

impl Premint for PremintV2Message {
    fn metadata(&self) -> PremintMetadata {
        PremintMetadata {
            id: self.premint.uid.to_string(),
            kind: "zora_premint_v2".to_string(),
            signer: self.collection.contract_admin,
            chain_id: self.chain_id as i64,
            collection_address: Address::default(), // TODO: source this
            token_id: U256::from(self.premint.uid),
            uri: self.premint.token_creation_config.token_uri.clone(),
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

#[cfg(test)]
mod test {
    use super::*;
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

        let premint = PremintTypes::V2(PremintV2Message::default());
        let json = premint.to_json().unwrap();
        println!("{}", json);
        let premint: PremintTypes = PremintTypes::from_json(json).unwrap();
        println!("{:?}", premint);
    }
}
