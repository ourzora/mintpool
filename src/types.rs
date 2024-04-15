use crate::premints::zora_premint_v2::types::ZoraPremintV2;
use alloy::rpc::types::eth::{Filter, Log, Transaction};
use alloy_primitives::{Address, B256, U256};
use async_trait::async_trait;
use libp2p::{gossipsub, Multiaddr, PeerId};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;

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
    pub version: u64,
    pub kind: PremintName,
    pub signer: Address,
    pub chain_id: U256,
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
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
#[allow(clippy::large_enum_variant)]
pub enum PremintTypes {
    Simple(SimplePremint),
    ZoraV2(ZoraPremintV2),
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
            PremintTypes::ZoraV2(p) => p.metadata(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
pub struct SimplePremint {
    version: u64,
    chain_id: U256,
    sender: Address,
    token_id: u64,
    media: String,
}

impl SimplePremint {
    pub fn new(
        version: u64,
        chain_id: U256,
        sender: Address,
        token_id: u64,
        media: String,
    ) -> Self {
        Self {
            version,
            chain_id,
            sender,
            token_id,
            media,
        }
    }
}

#[async_trait]
impl Premint for SimplePremint {
    fn metadata(&self) -> PremintMetadata {
        PremintMetadata {
            id: format!("{:?}:{:?}:{:?}", self.chain_id, self.sender, self.token_id),
            version: self.version,
            kind: PremintName("simple".to_string()),
            signer: self.sender,
            chain_id: self.chain_id,
            collection_address: Address::default(),
            token_id: U256::from(self.token_id),
            uri: self.media.clone(),
        }
    }

    fn check_filter(_chain_id: u64) -> Option<Filter> {
        todo!()
    }

    fn map_claim(_chain_id: u64, _log: Log) -> eyre::Result<InclusionClaim> {
        todo!()
    }

    async fn verify_claim(
        _chain_id: u64,
        _tx: Transaction,
        _log: Log,
        _claim: InclusionClaim,
    ) -> bool {
        todo!()
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
    use crate::premints::zora_premint_v2::types::PREMINT_FACTORY_ADDR;
    use alloy_primitives::{Bytes, LogData};
    use std::str::FromStr;

    #[test]
    fn test_premint_serde() {
        let premint = PremintTypes::Simple(SimplePremint {
            version: 1,
            chain_id: U256::from(1),
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

        let premint = PremintTypes::ZoraV2(ZoraPremintV2::default());
        let json = premint.to_json().unwrap();
        println!("{}", json);
        let premint: PremintTypes = PremintTypes::from_json(json).unwrap();
        println!("{:?}", premint);
    }

    #[test]
    fn test_map_premintv2_claim() {
        let log = Log {
            inner: alloy_primitives::Log {
            address: PREMINT_FACTORY_ADDR.clone(),
            data: LogData::new(vec![B256::from_str("0xd7f3736994092942aacd1d75026379ceeaf4e28b6183b15f2decc9237334429b").unwrap(),
                             B256::from_str("0x00000000000000000000000065aae9d752ecac4965015664d0a6d0951e28d757").unwrap(),
                             B256::from_str("0x0000000000000000000000000000000000000000000000000000000000000001").unwrap(),
                             B256::from_str("0x0000000000000000000000000000000000000000000000000000000000000001").unwrap(),

                ],
                Bytes::from_str("0x0000000000000000000000000000000000000000000000000000000000000001000000000000000000000000edb81afaecc2379635b25a752b787f821a46644c0000000000000000000000000000000000000000000000000000000000000001").unwrap()
            ).unwrap()
            },
            block_hash: None,
            block_number: None,
            transaction_hash: Some(
                B256::from_str(
                    "0xb28c6c91fc5c79490c0bf2e8b26ec7ea5ca66065e14436bf5798a9feaad6e617",
                )
                .unwrap(),
            ),
            transaction_index: Some(4),
            log_index: Some(28),
            ..Default::default()
        };

        let claim = ZoraPremintV2::map_claim(7777777, log.clone()).unwrap();
        let expected = InclusionClaim {
            premint_id: "1".to_string(),
            chain_id: 7777777,
            tx_hash: log.clone().transaction_hash.unwrap(),
            log_index: 28,
            kind: "zora_premint_v2".to_string(),
        };

        assert_eq!(claim, expected);
    }

    #[tokio::test]
    async fn test_verify_premintv2_claim() {
        let tx = Transaction {
            hash: B256::from_str(
                "0xb28c6c91fc5c79490c0bf2e8b26ec7ea5ca66065e14436bf5798a9feaad6e617",
            )
            .unwrap(),
            nonce: 1,
            block_hash: Some(
                B256::from_str(
                    "0x0e918f6a5cfda90ce33ac5117880f6db97849a095379acdc162d038aaee56757",
                )
                .unwrap(),
            ),
            block_number: Some(12387768),
            transaction_index: Some(4),
            from: Address::from_str("0xeDB81aFaecC2379635B25A752b787f821a46644c").unwrap(),
            to: Some(PREMINT_FACTORY_ADDR.clone()),
            value: U256::from(777_000_000_000_000_i64),
            ..Default::default()
        };

        let log = Log {
            inner: alloy_primitives::Log {
            address: PREMINT_FACTORY_ADDR.clone(),
            data: LogData::new(vec![B256::from_str("0xd7f3736994092942aacd1d75026379ceeaf4e28b6183b15f2decc9237334429b").unwrap(),
                             B256::from_str("0x00000000000000000000000065aae9d752ecac4965015664d0a6d0951e28d757").unwrap(),
                             B256::from_str("0x0000000000000000000000000000000000000000000000000000000000000001").unwrap(),
                             B256::from_str("0x0000000000000000000000000000000000000000000000000000000000000001").unwrap(),
                ],
                Bytes::from_str("0x0000000000000000000000000000000000000000000000000000000000000001000000000000000000000000edb81afaecc2379635b25a752b787f821a46644c0000000000000000000000000000000000000000000000000000000000000001").unwrap()
            ).unwrap()
            },
            block_hash: None,
            block_number: None,
            transaction_hash: Some(
                B256::from_str(
                    "0xb28c6c91fc5c79490c0bf2e8b26ec7ea5ca66065e14436bf5798a9feaad6e617",
                )
                .unwrap(),
            ),
            transaction_index: Some(4),
            log_index: Some(28),
            ..Default::default()
        };

        let claim = ZoraPremintV2::map_claim(7777777, log.clone()).unwrap();
        assert!(ZoraPremintV2::verify_claim(7777777, tx.clone(), log.clone(), claim).await);
    }
}
