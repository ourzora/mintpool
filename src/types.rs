use alloy_primitives::{Address, U256};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct SimplePremint {
    chain_id: u64,
    sender: Address,
    media: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PremintV2Message {
    collection: PremintV2Collection,
    premint: PremintV2,
    chain_id: u64,
    signature: String,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PremintV2Collection {
    contract_admin: Address,
    contract_uri: String,
    contract_name: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PremintV2 {
    token_creation_config: TokenCreationConfigV2,
    uid: u64,
    version: u64,
    deleted: bool,
}

#[derive(Serialize)]
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
