use async_trait::async_trait;
use ethers::contract::EthEvent;
use ethers::prelude::{abigen, parse_log, Address, Filter, Log, Transaction, H256, U256};
use libp2p::{gossipsub, Multiaddr, PeerId};
use once_cell::sync::Lazy;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fmt::{Debug, Display};

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
    fn map_claim(chain_id: u64, tx: Transaction, log: Log) -> eyre::Result<InclusionClaim>;
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

    fn map_claim(chain_id: u64, tx: Transaction, log: Log) -> eyre::Result<InclusionClaim> {
        todo!()
    }

    async fn verify_claim(chain_id: u64, tx: Transaction, log: Log, claim: InclusionClaim) -> bool {
        todo!()
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

abigen!(
    PremintV2,
    r"[event PremintedV2(address indexed contractAddress,uint256 indexed tokenId,bool indexed createdNewContract,uint32 uid,address minter,uint256 quantityMinted)]"
);

static PREMINT_FACTORY_ADDR: Lazy<Address> = Lazy::new(|| {
    "0x7777773606e7e46C8Ba8B98C08f5cD218e31d340"
        .parse()
        .unwrap()
});

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

    fn kind_id() -> PremintName {
        PremintName("zora_premint_v2".to_string())
    }

    fn check_filter(chain_id: u64) -> Option<Filter> {
        let supported_chains = vec![7777777, 8423]; // TODO: add the rest here and enable testnet mode

        if !supported_chains.contains(&chain_id) {
            return None;
        }

        Some(
            Filter::new()
                .address(PREMINT_FACTORY_ADDR.clone())
                .event(PremintedV2Filter::abi_signature().to_string().as_str()),
        )
    }

    fn map_claim(chain_id: u64, tx: Transaction, log: Log) -> eyre::Result<InclusionClaim> {
        let event: PremintedV2Filter = parse_log(log.clone())?;
        Ok(InclusionClaim {
            premint_id: event.uid.to_string(),
            chain_id,
            tx_hash: tx.hash,
            log_index: log.log_index.unwrap_or(U256::zero()).as_u64(),
            kind: "zora_premint_v2".to_string(),
        })
    }

    async fn verify_claim(chain_id: u64, tx: Transaction, log: Log, claim: InclusionClaim) -> bool {
        let event = parse_log::<PremintedV2Filter>(log.clone());
        match event {
            Ok(event) => {
                let conditions = vec![
                    log.address == PREMINT_FACTORY_ADDR.clone(),
                    log.transaction_hash.unwrap_or(H256::zero()) == tx.hash,
                    claim.tx_hash == tx.hash,
                    claim.log_index == log.log_index.unwrap_or(U256::zero()).as_u64(),
                    claim.premint_id == event.uid.to_string(),
                    claim.kind == "zora_premint_v2".to_string(),
                    claim.chain_id == chain_id,
                ];

                // confirm all conditions are true
                conditions.into_iter().all(|x| x)
            }
            Err(e) => {
                tracing::debug!("Failed to parse log: {}", e);
                return false;
            }
        }
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

#[derive(Debug, Clone, PartialEq)]
pub struct InclusionClaim {
    pub premint_id: String,
    pub chain_id: u64,
    pub tx_hash: H256,
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
