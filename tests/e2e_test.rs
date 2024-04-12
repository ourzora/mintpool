mod common;

use crate::common::mintpool_build;
use alloy::network::EthereumSigner;
use alloy::rpc::types::eth::{TransactionInput, TransactionRequest};
use alloy_node_bindings::{Anvil, WEI_IN_ETHER};
use alloy_primitives::{hex, Bytes, U256};
use alloy_provider::{Provider, ProviderBuilder};
use alloy_rpc_client::BuiltInConnectionString::Http;
use alloy_rpc_client::{RpcClient, WsConnect};
use alloy_signer::k256::ecdsa::SigningKey;
use alloy_signer_wallet::LocalWallet;
use alloy_sol_types::SolCall;
use mintpool::config::{ChainInclusionMode, Config};
use mintpool::controller::{ControllerCommands, DBQuery};
use mintpool::premints::zora_premint_v2::broadcast::premint_to_call;
use mintpool::premints::zora_premint_v2::types::IZoraPremintV2::MintArguments;
use mintpool::premints::zora_premint_v2::types::{
    IZoraPremintV2, ZoraPremintV2, PREMINT_FACTORY_ADDR,
};
use mintpool::run;
use mintpool::types::PremintTypes;
use std::env;
use std::str::FromStr;
use std::time::Duration;

#[tokio::test]
#[ignore] // remove once signing logic is complete
async fn test_broadcasting_premint() {
    // let anvil = Anvil::new()
    //     .chain_id(7777777)
    //     .fork("https://rpc.zora.energy")
    //     .fork_block_number(12665000)
    //     .block_time(1)
    //     .spawn();

    let mut config = Config {
        seed: 0,
        peer_port: 7778,
        connect_external: false,
        db_url: None,
        persist_state: false,
        prune_minted_premints: false,
        api_port: 0,
        peer_limit: 10,
        supported_premint_types: "".to_string(),
        chain_inclusion_mode: ChainInclusionMode::Check,
        supported_chain_ids: "7777777".to_string(),
        trusted_peers: None,
    };

    env::set_var("CHAIN_7777777_RPC_WSS", "ws://localhost:8545");

    let ctl = mintpool_build::make_nodes(config.peer_port, 1, config.peer_limit).await;
    let ctl = ctl.first().unwrap();
    // in real
    run::start_watch_chain::<ZoraPremintV2>(&config, ctl.clone()).await;

    // end creation of services

    // Push a message to the mintpool
    let premint: ZoraPremintV2 = serde_json::from_str(PREMINT_JSON).unwrap();
    ctl.send_command(ControllerCommands::Broadcast {
        message: PremintTypes::ZoraV2(premint),
    })
    .await
    .unwrap();

    let (send, recv) = tokio::sync::oneshot::channel();

    // Read the premint from DB
    ctl.send_command(ControllerCommands::Query(DBQuery::ListAll(send)))
        .await
        .unwrap();
    let all_premints = recv.await.unwrap().unwrap();

    assert_eq!(all_premints.len(), 1);

    // query for message from mintpool

    let found = all_premints.first().unwrap();

    // ============================================================================================

    let signer: LocalWallet =
        LocalWallet::from_str("0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80")
            .unwrap();
    let provider = ProviderBuilder::new()
        // .with_recommended_layers()
        .signer(EthereumSigner::from(signer.clone()))
        .on_client(RpcClient::new_http(
            reqwest::Url::from_str("http://localhost:8545").unwrap(),
        ));

    let premint = match found {
        PremintTypes::ZoraV2(premint) => premint,
        _ => panic!("unexpected premint type"),
    };

    let calldata = premint_to_call(
        premint.clone(),
        U256::from(1),
        MintArguments {
            mintRecipient: signer.address(),
            mintComment: "".to_string(),
            mintRewardsRecipients: vec![],
        },
    );

    // IZoraPremintV2::
    println!("fetching gas");
    let gas_price = provider.get_gas_price().await.unwrap();
    println!("gas_price: {:?}", gas_price);
    let max_fee_per_gas = provider.get_max_priority_fee_per_gas().await.unwrap();
    println!("max_fee_per_gas: {:?}", max_fee_per_gas);

    println!("calldata: {:?}", calldata.abi_encode());

    // Someone found the premint and brought it onchain
    let mut tx_request = TransactionRequest {
        to: Some(PREMINT_FACTORY_ADDR),
        input: TransactionInput::new(Bytes::from(calldata.abi_encode())),
        value: Some(U256::from(0.000777 * (10 ^ 18) as f32)),
        nonce: Some(13),
        max_priority_fee_per_gas: Some(20 * (10 ^ 12)),
        gas_price: Some(gas_price),
        gas: Some(30_000_000),
        max_fee_per_gas: Some(max_fee_per_gas),
        chain_id: Some(7777777),
        ..Default::default()
    };

    // let gas = provider.estimate_gas(&tx_request, None).await.unwrap();
    // tx_request.gas = Some(gas);

    println!("tx_request: {:?}", tx_request);
    let mut tx = provider.send_transaction(tx_request).await.unwrap();
    // tx.set_required_confirmations(1);
    // tx.set_timeout(Some(Duration::from_secs(2)));
    println!("tx: {:?}", tx);
    tx.get_receipt().await.unwrap();

    println!("tx processed");

    // Check that premint has been removed from mintpool
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
