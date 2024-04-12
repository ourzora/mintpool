mod common;

use crate::common::mintpool_build;
use alloy::network::EthereumSigner;
use alloy::rpc::types::eth::{TransactionInput, TransactionRequest};
use alloy_json_rpc::RpcError;
use alloy_node_bindings::Anvil;
use alloy_primitives::{Bytes, U256};
use alloy_provider::{Provider, ProviderBuilder};
use alloy_rpc_client::RpcClient;
use alloy_signer::Signer;
use alloy_signer_wallet::LocalWallet;
use alloy_sol_types::{SolCall, SolValue};
use mintpool::config::{ChainInclusionMode, Config};
use mintpool::controller::{ControllerCommands, DBQuery};
use mintpool::premints::zora_premint_v2::broadcast::premint_to_call;
use mintpool::premints::zora_premint_v2::types::IZoraPremintV2::MintArguments;
use mintpool::premints::zora_premint_v2::types::{
    IZoraPremintV2, ZoraPremintV2, PREMINT_FACTORY_ADDR,
};
use mintpool::run;
use mintpool::types::PremintTypes;
use serde::Deserialize;
use std::env;
use std::str::FromStr;
use std::time::Duration;

#[tokio::test]
#[ignore]
/// This test does the full round trip lifecycle of a premint
/// 1. Premint is broadcasted to mintpool
/// 2. Premint is fetched from DB (similating a client fetching from API)
/// 3. Premint is brought onchain by a client
/// 4. Premint is removed from mintpool when an event is seen onchain
async fn test_broadcasting_premint() {
    let anvil = Anvil::new()
        .chain_id(7777777)
        .fork_block_number(12665000)
        .fork("https://rpc.zora.energy")
        .spawn();

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
        node_id: None,
        external_address: None,
        interactive: false,
    };

    env::set_var("CHAIN_7777777_RPC_WSS", anvil.ws_endpoint());

    let ctl = mintpool_build::make_nodes(config.peer_port, 1, config.peer_limit).await;
    let ctl = ctl.first().unwrap();
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

    // ============================================================================================
    // query for message from mintpool
    // ============================================================================================

    let found = all_premints.first().unwrap();

    // ============================================================================================
    // bring premint onchain
    // ============================================================================================

    let signer: LocalWallet = anvil.keys()[0].clone().into();
    let signer = signer.with_chain_id(Some(7777777));

    let provider = ProviderBuilder::new()
        .with_recommended_layers()
        .signer(EthereumSigner::from(signer.clone()))
        .on_client(RpcClient::new_http(anvil.endpoint_url()));

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

    let gas_price = provider.get_gas_price().await.unwrap();
    let max_fee_per_gas = provider.get_max_priority_fee_per_gas().await.unwrap();

    // Someone found the premint and brought it onchain
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

    let tx = provider.send_transaction(tx_request).await;
    let tx = match tx {
        Ok(tx) => tx,
        Err(e) => match e {
            RpcError::ErrorResp(err) => {
                let b = err.data.unwrap();

                let msg =
                    IZoraPremintV2::premintV2Call::abi_decode_returns(&b.get().abi_encode(), false)
                        .unwrap();
                panic!("unexpected error: {:?}", msg)
            }
            _ => {
                panic!("unexpected error: {:?}", e);
            }
        },
    };

    match tx.get_receipt().await {
        Ok(receipt) => {
            println!("receipt: {:?}", receipt);
        }
        Err(e) => match e {
            RpcError::ErrorResp(err) => {
                let b = err.data.unwrap();

                let msg =
                    IZoraPremintV2::premintV2Call::abi_decode_returns(&b.get().abi_encode(), false)
                        .unwrap();
                panic!("unexpected error: {:?}", msg)
            }
            _ => {
                panic!("unexpected error: {:?}", e);
            }
        },
    }
    // NOTE: this currently revents I suspect because this premint is already onchain, we should grab a different one

    println!("tx processed");

    // TODO: check that the premint was removed from the mintpool, giving it a second to process
    tokio::time::sleep(Duration::from_secs(1)).await;
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
