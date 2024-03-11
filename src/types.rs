use alloy_primitives::{Address, U256};
use libp2p::{Multiaddr, PeerId};
use serde::{Deserialize, Serialize};

#[derive(Debug)]
pub struct MintpoolNodeInfo {
    pub peer_id: PeerId,
    pub addr: Vec<Multiaddr>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub enum Premint {
    Simple(SimplePremint),
    V2(PremintV2Message),
}

impl Premint {
    pub fn from_json(line: String) -> eyre::Result<Self> {
        let p: Premint = serde_json::from_str(&line)?;
        Ok(p)
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct SimplePremint {
    chain_id: u64,
    sender: Address,
    token_id: u64,
    media: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct PremintV2Message {
    collection: PremintV2Collection,
    premint: PremintV2,
    chain_id: u64,
    signature: String,
}
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct PremintV2Collection {
    contract_admin: Address,
    contract_uri: String,
    contract_name: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct PremintV2 {
    token_creation_config: TokenCreationConfigV2,
    uid: u64,
    version: u64,
    deleted: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
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
        let premint = Premint::Simple(SimplePremint {
            chain_id: 1,
            sender: "0x66f9664f97F2b50F62D13eA064982f936dE76657"
                .parse()
                .unwrap(),
            token_id: 1,
            media: "https://ipfs.io/ipfs/Qm".to_string(),
        });

        let json = serde_json::to_string(&premint).unwrap();
        println!("{}", json);
        let premint: Premint = serde_json::from_str(&json).unwrap();
        println!("{:?}", premint);

        let premint = Premint::V2(PremintV2Message::default());
        let json = serde_json::to_string(&premint).unwrap();
        println!("{}", json);
        let premint: Premint = serde_json::from_str(&json).unwrap();
        println!("{:?}", premint);
    }
}
