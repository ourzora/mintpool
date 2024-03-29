use alloy::rpc::types::eth::{Filter, Log, Transaction};
use alloy_primitives::{Address, B256, U256};
use async_trait::async_trait;
use libp2p::{gossipsub, Multiaddr, PeerId};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;

use crate::premints::zora_v2::PremintV2Message;

#[derive(Debug)]
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
    pub kind: PremintName,
    pub signer: Address,
    pub chain_id: i64,
    pub collection_address: Address,
    pub token_id: U256,
    pub uri: String,
}

#[async_trait]
pub trait Premint: Serialize + DeserializeOwned + Debug + Clone {
    fn metadata(&self) -> PremintMetadata;

    fn check_filter(chain_id: u64) -> Option<Filter>;
    fn map_claim(chain_id: u64, log: Log) -> eyre::Result<InclusionClaim>;
    async fn verify_claim(chain_id: u64, tx: Transaction, log: Log, claim: InclusionClaim) -> bool;
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

#[async_trait]
impl Premint for SimplePremint {
    fn metadata(&self) -> PremintMetadata {
        PremintMetadata {
            id: format!("{:?}:{:?}:{:?}", self.chain_id, self.sender, self.token_id),
            kind: Self::kind_id(),
            signer: self.sender,
            chain_id: self.chain_id as i64,
            collection_address: Address::default(),
            token_id: U256::from(self.token_id),
            uri: self.media.clone(),
        }
    }

    fn check_filter(chain_id: u64) -> Option<Filter> {
        todo!()
    }

    fn map_claim(chain_id: u64, log: Log) -> eyre::Result<InclusionClaim> {
        todo!()
    }

    async fn verify_claim(chain_id: u64, tx: Transaction, log: Log, claim: InclusionClaim) -> bool {
        todo!()
    }

    fn kind_id() -> PremintName {
        PremintName("simple".to_string())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct InclusionClaim {
    pub premint_id: String,
    pub chain_id: u64,
    pub tx_hash: B256,
    pub log_index: u64,
    pub kind: String,
}

#[cfg(test)]
mod test {
    use super::*;
    use ethers::prelude::{Bytes, U64};
    use std::str::FromStr;
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

    #[test]
    fn test_map_premintv2_claim() {
        let tx = Transaction {
            hash: H256::from_str(
                "0xb28c6c91fc5c79490c0bf2e8b26ec7ea5ca66065e14436bf5798a9feaad6e617",
            )
            .unwrap(),
            nonce: U256::from(37),
            block_hash: Some(
                H256::from_str(
                    "0x0e918f6a5cfda90ce33ac5117880f6db97849a095379acdc162d038aaee56757",
                )
                .unwrap(),
            ),
            block_number: Some(U64::from(12387768)),
            transaction_index: Some(U64::from(4)),
            from: Address::from_str("0xeDB81aFaecC2379635B25A752b787f821a46644c").unwrap(),
            to: Some(PREMINT_FACTORY_ADDR.clone()),
            value: U256::from(777_000_000_000_000_i64),
            ..Default::default()
        };

        let log = Log {
            address: PREMINT_FACTORY_ADDR.clone(),
            topics: vec![H256::from_str("0xd7f3736994092942aacd1d75026379ceeaf4e28b6183b15f2decc9237334429b").unwrap(),
                         H256::from_str("0x00000000000000000000000065aae9d752ecac4965015664d0a6d0951e28d757").unwrap(),
                         H256::from_str("0x0000000000000000000000000000000000000000000000000000000000000001").unwrap(),
                         H256::from_str("0x0000000000000000000000000000000000000000000000000000000000000001").unwrap(),
            ],
            data: Bytes::from_str("0x0000000000000000000000000000000000000000000000000000000000000001000000000000000000000000edb81afaecc2379635b25a752b787f821a46644c0000000000000000000000000000000000000000000000000000000000000001").unwrap(),
            block_hash: tx.block_hash,
            block_number: tx.block_number,
            transaction_hash: Some(tx.hash.clone()),
            transaction_index: tx.transaction_index,
            log_index: Some(U256::from(28)),
            transaction_log_index: Some(U256::from(28)),
            log_type: None,
            removed: None,
        };

        let claim = PremintV2Message::map_claim(7777777, tx.clone(), log).unwrap();
        let expected = InclusionClaim {
            premint_id: "1".to_string(),
            chain_id: 7777777,
            tx_hash: tx.clone().hash,
            log_index: 28,
            kind: "zora_premint_v2".to_string(),
        };

        assert_eq!(claim, expected);
    }

    #[tokio::test]
    async fn test_verify_premintv2_claim() {
        let tx = Transaction {
            hash: H256::from_str(
                "0xb28c6c91fc5c79490c0bf2e8b26ec7ea5ca66065e14436bf5798a9feaad6e617",
            )
            .unwrap(),
            nonce: U256::from(37),
            block_hash: Some(
                H256::from_str(
                    "0x0e918f6a5cfda90ce33ac5117880f6db97849a095379acdc162d038aaee56757",
                )
                .unwrap(),
            ),
            block_number: Some(U64::from(12387768)),
            transaction_index: Some(U64::from(4)),
            from: Address::from_str("0xeDB81aFaecC2379635B25A752b787f821a46644c").unwrap(),
            to: Some(PREMINT_FACTORY_ADDR.clone()),
            value: U256::from(777_000_000_000_000_i64),
            ..Default::default()
        };

        let log = Log {
            address: PREMINT_FACTORY_ADDR.clone(),
            topics: vec![H256::from_str("0xd7f3736994092942aacd1d75026379ceeaf4e28b6183b15f2decc9237334429b").unwrap(),
                         H256::from_str("0x00000000000000000000000065aae9d752ecac4965015664d0a6d0951e28d757").unwrap(),
                         H256::from_str("0x0000000000000000000000000000000000000000000000000000000000000001").unwrap(),
                         H256::from_str("0x0000000000000000000000000000000000000000000000000000000000000001").unwrap(),
            ],
            data: Bytes::from_str("0x0000000000000000000000000000000000000000000000000000000000000001000000000000000000000000edb81afaecc2379635b25a752b787f821a46644c0000000000000000000000000000000000000000000000000000000000000001").unwrap(),
            block_hash: tx.block_hash,
            block_number: tx.block_number,
            transaction_hash: Some(tx.hash.clone()),
            transaction_index: tx.transaction_index,
            log_index: Some(U256::from(28)),
            transaction_log_index: Some(U256::from(28)),
            log_type: None,
            removed: None,
        };

        let claim = PremintV2Message::map_claim(7777777, tx.clone(), log.clone()).unwrap();
        assert!(PremintV2Message::verify_claim(7777777, tx.clone(), log.clone(), claim).await);
    }
}
