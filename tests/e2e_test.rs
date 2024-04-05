mod common;

use crate::common::mintpool_build;
use alloy::network::EthereumSigner;
use alloy::rpc::types::eth::TransactionRequest;
use alloy_node_bindings::Anvil;
use alloy_primitives::U256;
use alloy_provider::{Provider, ProviderBuilder};
use alloy_rpc_client::{RpcClient, WsConnect};
use alloy_signer_wallet::LocalWallet;
use mintpool::config::{ChainInclusionMode, Config};
use mintpool::controller::{ControllerCommands, DBQuery};
use mintpool::premints::zora_premint_v2::types::{ZoraPremintV2, PREMINT_FACTORY_ADDR};
use mintpool::run;
use mintpool::types::PremintTypes;
use std::env;

#[tokio::test]
#[ignore] // remove once signing logic is complete
async fn test_broadcasting_premint() {
    let anvil = Anvil::new()
        .chain_id(7777777)
        .fork("https://rpc.zora.energy")
        .fork_block_number(12665000)
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
    };

    env::set_var("CHAIN_7777777_RPC_WSS", anvil.ws_endpoint());

    let ctl = mintpool_build::make_nodes(config.peer_port, 1, config.peer_limit).await;
    let ctl = ctl.first().unwrap();
    // in real
    run::start_watch_chain::<ZoraPremintV2>(&config, ctl.clone()).await;

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
    let found = all_premints.first().unwrap();

    // ============================================================================================

    let signer: LocalWallet = anvil.keys()[0].clone().into();
    let provider = ProviderBuilder::new()
        .signer(EthereumSigner::from(signer))
        .on_client(
            RpcClient::connect_pubsub(WsConnect::new(anvil.ws_endpoint()))
                .await
                .unwrap(),
        );

    IZoraPremintV2
    
    // Someone found the premint and brought it onchain
    let tx_request = TransactionRequest {
        to: Some(PREMINT_FACTORY_ADDR),
        // data:
        nonce: Some(0),
        gas_price: Some(U256::from(20e9)),
        gas: Some(U256::from(21000)),
        ..Default::default()
    };
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
