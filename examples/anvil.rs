use alloy::network::EthereumSigner;
use alloy::rpc::types::eth::TransactionRequest;
use alloy::sol_types::private::U256;
use alloy_node_bindings::Anvil;
use alloy_provider::{Provider, ProviderBuilder};
use alloy_rpc_client::RpcClient;
use alloy_signer_wallet::LocalWallet;
use mintpool::premints::zora_premint_v2::types::PREMINT_FACTORY_ADDR;
use std::str::FromStr;

#[tokio::main]
async fn main() {
    let anvil = Anvil::new()
        .chain_id(7777777)
        .fork("https://rpc.zora.energy")
        .fork_block_number(12665000)
        .block_time(1)
        .spawn();

    let signer: LocalWallet = anvil.keys()[0].clone().into();

    let provider = ProviderBuilder::new()
        .with_recommended_layers()
        .signer(EthereumSigner::from(signer.clone()))
        .on_client(RpcClient::new_http(anvil.endpoint_url()));

    let gas_price = provider.get_gas_price().await.unwrap();
    println!("gas_price: {:?}", gas_price);
    let max_fee_per_gas = provider.get_max_priority_fee_per_gas().await.unwrap();

    let mut tx_request = TransactionRequest {
        to: Some(PREMINT_FACTORY_ADDR),
        value: Some(U256::from(1)),
        nonce: Some(14),
        max_priority_fee_per_gas: Some(20 * (10 ^ 12)),
        gas_price: Some(gas_price),
        gas: Some(30_000_000),
        max_fee_per_gas: Some(max_fee_per_gas),
        chain_id: Some(7777777),
        ..Default::default()
    };

    let mut tx = provider.send_transaction(tx_request).await.unwrap();
    // tx.set_required_confirmations(1);
    // tx.set_timeout(Some(Duration::from_secs(2)));
    println!("tx: {:?}", tx);
    tx.get_receipt().await.unwrap();
}
