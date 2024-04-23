use std::sync::Arc;

use alloy::rpc::types::eth::{TransactionInput, TransactionRequest};
use alloy_primitives::{address, Address, Bytes};
use alloy_provider::Provider;
use alloy_sol_macro::sol;
use alloy_sol_types::SolCall;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};

use crate::chain_list::ChainListProvider;

pub static PREMINT_FACTORY_ADDR: Address = address!("7777773606e7e46C8Ba8B98C08f5cD218e31d340");

sol! {
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    IZoraPremintERC20V1,
    "src/premints/zora_premint/zora1155PremintExecutor_erc20v1.json"
}

sol! {
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    IZoraPremintV2,
    "src/premints/zora_premint/zora1155PremintExecutor_v2.json"
}

pub trait ZoraPremint {
    fn collection_address(&self) -> Address;
    fn chain_id(&self) -> u64;
    fn signature(&self) -> String;
}

pub async fn contract_call<T>(call: T, provider: &Arc<ChainListProvider>) -> eyre::Result<T::Return>
where
    T: SolCall,
{
    provider
        .call(
            &TransactionRequest {
                to: Some(PREMINT_FACTORY_ADDR),
                input: TransactionInput::new(Bytes::from(call.abi_encode())),
                ..Default::default()
            },
            None,
        )
        .await
        .map_err(|err| eyre::eyre!("Error calling contract: {:?}", err))
        .and_then(|response| {
            T::abi_decode_returns(&response, false)
                .map_err(|err| eyre::eyre!("Error decoding contract response: {:?}", err))
        })
}
