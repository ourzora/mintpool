use crate::premints::zora_premint::v2;
use alloy::primitives::{Address, B256, U256};
use alloy::rpc::types::eth::{Filter, Log, TransactionReceipt};
use async_trait::async_trait;
use libp2p::gossipsub::TopicHash;
use libp2p::{gossipsub, Multiaddr, PeerId};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use std::format;

#[derive(Debug, Clone)]
pub struct PremintName(pub String);

impl PremintName {
    pub fn msg_topic(&self) -> gossipsub::IdentTopic {
        gossipsub::IdentTopic::new(format!("mintpool::premint::{:?}", self))
    }

    pub fn claims_topic(&self) -> gossipsub::IdentTopic {
        gossipsub::IdentTopic::new(format!("chain::claims::{:?}", self))
    }
}

pub fn claims_topic_hashes(names: Vec<PremintName>) -> Vec<TopicHash> {
    names.iter().map(|n| n.claims_topic().hash()).collect()
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
    pub chain_id: u64,
    pub collection_address: Address,
    pub token_id: U256,
    pub uri: String,
}

#[async_trait]
pub trait Premint: Serialize + DeserializeOwned + Debug + Clone {
    fn metadata(&self) -> PremintMetadata;
    fn check_filter(chain_id: u64) -> Option<Filter>;
    fn map_claim(chain_id: u64, log: Log) -> eyre::Result<InclusionClaim>;
    async fn verify_claim(
        &self,
        chain_id: u64,
        tx: TransactionReceipt,
        log: Log,
        claim: InclusionClaim,
    ) -> bool;
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
#[allow(clippy::large_enum_variant)]
pub enum PremintTypes {
    Simple(SimplePremint),
    ZoraV2(v2::V2),
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

macro_rules! every_arm_fn {

    (PremintTypes, fn $fn:ident($($arg:ident: $arg_type:ty),*) -> $return:ty) => {
        impl PremintTypes {
            pub fn $fn(&self, $($arg: $arg_type),*) -> $return {
                match self {
                    PremintTypes::Simple(p) => p.$fn($($arg),*),
                    PremintTypes::ZoraV2(p) => p.$fn($($arg),*),
                }
            }
        }
    };

    (PremintTypes, async fn $fn:ident($($arg:ident: $arg_type:ty),*) -> $return:ty) => {
        impl PremintTypes {
            pub async fn $fn(&self, $($arg: $arg_type),*) -> $return {
                match self {
                    PremintTypes::Simple(p) => p.$fn($($arg),*).await,
                    PremintTypes::ZoraV2(p) => p.$fn($($arg),*).await,
                }
            }
        }
    };
}

every_arm_fn!(PremintTypes, fn metadata() -> PremintMetadata);
every_arm_fn!(PremintTypes, async fn verify_claim(chain_id: u64, tx: TransactionReceipt, log: Log, claim: InclusionClaim) -> bool);

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
pub struct SimplePremint {
    version: u64,
    chain_id: u64,
    sender: Address,
    token_id: u64,
    media: String,
}

impl SimplePremint {
    pub fn new(version: u64, chain_id: u64, sender: Address, token_id: u64, media: String) -> Self {
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
        &self,
        _chain_id: u64,
        _tx: TransactionReceipt,
        _log: Log,
        _claim: InclusionClaim,
    ) -> bool {
        todo!()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InclusionClaim {
    pub premint_id: String,
    pub chain_id: u64,
    pub tx_hash: B256,
    pub log_index: u64,
    pub kind: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PeerInclusionClaim {
    pub claim: InclusionClaim,
    pub from_peer_id: PeerId,
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::premints::zora_premint::contract::IZoraPremintV2::{
        ContractCreationConfig, CreatorAttribution, TokenCreationConfig,
    };
    use crate::premints::zora_premint::contract::{IZoraPremintV2, PREMINT_FACTORY_ADDR};
    use alloy::primitives::{Bytes, LogData};
    use alloy::rpc::types::eth::ReceiptEnvelope;
    use alloy::sol_types::SolEvent;
    use std::str::FromStr;

    #[test]
    fn test_premint_serde() {
        let premint = PremintTypes::Simple(SimplePremint {
            version: 1,
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

        let premint = PremintTypes::ZoraV2(v2::V2::default());
        let json = premint.to_json().unwrap();
        println!("{}", json);
        let premint: PremintTypes = PremintTypes::from_json(json).unwrap();
        println!("{:?}", premint);
    }

    #[test]
    fn test_map_premintv2_claim() {
        let log = Log {
            inner: alloy::primitives::Log {
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

        let claim = v2::V2::map_claim(7777777, log.clone()).unwrap();
        let expected = InclusionClaim {
            premint_id: "7777777:0x65aae9d752ecac4965015664d0a6d0951e28d757:1".to_string(),
            chain_id: 7777777,
            tx_hash: log.clone().transaction_hash.unwrap(),
            log_index: 28,
            kind: "zora_premint_v2".to_string(),
        };

        assert_eq!(claim, expected);
    }

    #[tokio::test]
    async fn test_verify_premintv2_claim() {
        let tx = TransactionReceipt {
            inner: ReceiptEnvelope::Eip4844(Default::default()),
            transaction_hash: B256::from_str(
                "0xb28c6c91fc5c79490c0bf2e8b26ec7ea5ca66065e14436bf5798a9feaad6e617",
            )
            .unwrap(),
            block_hash: Some(
                B256::from_str(
                    "0x0e918f6a5cfda90ce33ac5117880f6db97849a095379acdc162d038aaee56757",
                )
                .unwrap(),
            ),
            block_number: Some(12387768),
            gas_used: 0,
            effective_gas_price: 0,
            blob_gas_used: None,
            transaction_index: Some(4),
            from: Address::from_str("0xeDB81aFaecC2379635B25A752b787f821a46644c").unwrap(),
            to: Some(PREMINT_FACTORY_ADDR.clone()),

            contract_address: None,
            blob_gas_price: None,
            state_root: None,
        };

        let log = Log {
            inner: alloy::primitives::Log {
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

        let event =
            IZoraPremintV2::PremintedV2::decode_raw_log(log.topics(), &log.data().data, true)
                .unwrap();

        let claim = <v2::V2 as Premint>::map_claim(7777777, log.clone()).unwrap();
        let premint = v2::V2 {
            collection: ContractCreationConfig {
                contractAdmin: Default::default(),
                contractURI: "".to_string(),
                contractName: "".to_string(),
            },
            premint: CreatorAttribution {
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
                uid: event.uid,
                version: 1,
                deleted: false,
            },
            collection_address: event.contractAddress,
            chain_id: 0,
            signature: "".to_string(),
        };

        assert!(
            premint
                .verify_claim(7777777, tx.clone(), log.clone(), claim)
                .await
        );
    }
}
