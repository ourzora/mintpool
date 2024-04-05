use alloy::network::{Ethereum, EthereumSigner};
use alloy::rpc::types::eth::{TransactionInput, TransactionRequest};
use alloy_node_bindings::Anvil;
use alloy_primitives::{Bytes, U256};
use alloy_provider::{Provider, ProviderBuilder, RootProvider};
use alloy_rpc_client::RpcClient;
use alloy_signer::Signer;
use alloy_signer_wallet::LocalWallet;
use alloy_sol_types::SolCall;
use mintpool::premints::zora_premint_v2::broadcast::premint_to_call;
use mintpool::premints::zora_premint_v2::types::IZoraPremintV2::MintArguments;
use mintpool::premints::zora_premint_v2::types::{ZoraPremintV2, PREMINT_FACTORY_ADDR};
use mintpool::types::PremintTypes;
use std::str::FromStr;

#[tokio::main]
async fn main() -> eyre::Result<()> {
    // /*
    let anvil = Anvil::new()
        .chain_id(7777777)
        .fork("https://rpc.zora.energy")
        // .fork("https://eth.merkle.io")
        // .fork_block_number(12665000)
        .try_spawn()
        .unwrap();

    let signer: LocalWallet = anvil.keys()[0].clone().into();
    let signer = signer.with_chain_id(Some(7777777));

    let provider = ProviderBuilder::new()
        .with_recommended_layers()
        .signer(EthereumSigner::from(signer.clone()))
        .on_client(RpcClient::new_http(anvil.endpoint_url()));

    let premint: ZoraPremintV2 = serde_json::from_str(PREMINT_JSON).unwrap();

    let calldata = premint_to_call(
        premint.clone(),
        U256::from(1),
        MintArguments {
            mintRecipient: signer.address(),
            mintComment: "".to_string(),
            mintRewardsRecipients: vec![],
        },
    );

    let gas_price = provider.get_gas_price().await.unwrap();
    println!("gas_price: {:?}", gas_price);
    let max_fee_per_gas = provider.get_max_priority_fee_per_gas().await.unwrap();

    let tx_request = TransactionRequest {
        from: Some(signer.address()),
        to: Some(PREMINT_FACTORY_ADDR),
        input: TransactionInput::new(Bytes::from(calldata.abi_encode())),
        value: Some(U256::from(0.000777 * (10 ^ 18) as f32)),
        gas_price: Some(gas_price),
        max_fee_per_gas: Some(max_fee_per_gas),
        chain_id: Some(7777777),
        ..Default::default()
    };

    // .max_priority_fee_per_gas(20 * (10 ^ 12))
    // .gas_price(gas_price)
    // .max_fee_per_gas(max_fee_per_gas);
    // .chain_id(7777777);

    let mut tx = provider.send_transaction(tx_request).await.unwrap();
    // tx.set_required_confirmations(1);
    // tx.set_timeout(Some(Duration::from_secs(2)));
    println!("tx: {:?}", tx);
    tx.get_receipt().await.unwrap();

    println!("Worked!");
    Ok(())
    // */r
}

const PREMINT_JSON: &str = r#"
{
  "collection": {
    "contractAdmin": "0xa771209423284bace9a24a06d166a11196724b53",
    "contractURI": "ipfs://bafkreic4fnavhtymee7makmk7wp257nloh5y5ysc2fcwa5rpg6v6f3jhly",
    "contractName": "Karate sketch"
  },
  "premint": {
    "tokenConfig": {
      "tokenURI": "ipfs://bafkreier5h4a6btu24fsitbjdvpyak7moi6wkp33wlqmx2kfwgpq2lvx4y",
      "maxSupply": 18446744073709551615,
      "maxTokensPerAddress": 0,
      "pricePerToken": 0,
      "mintStart": 1702541688,
      "mintDuration": 2592000,
      "royaltyBPS": 500,
      "fixedPriceMinter": "0x04e2516a2c207e84a1839755675dfd8ef6302f0a",
      "payoutRecipient": "0xa771209423284bace9a24a06d166a11196724b53",
      "createReferral": "0x0000000000000000000000000000000000000000"
    },
    "uid": 2,
    "version": 1,
    "deleted": false
  },
  "collectionAddress": "0x42e108d1ed954b0adbd53ea118ba7614622d10d0",
  "chainId": 7777777,
  "signature": "0x894405d100900e6823385ca881c91d5ca7137a326f0c7d27edfd2907d9669cea55626bbd807a36cea815eceeac6634f45cfec54d7157c35f496b999e7b9451de1c"
}"#;
