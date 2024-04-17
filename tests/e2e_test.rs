#![cfg(test)]
mod common;

use alloy::hex;
use alloy::network::EthereumSigner;
use alloy::rpc::types::eth::TransactionRequest;
use alloy_json_rpc::RpcError;
use alloy_node_bindings::Anvil;
use alloy_primitives::{Bytes, U256};
use alloy_provider::{Provider, ProviderBuilder};
use alloy_rpc_client::RpcClient;
use alloy_signer::Signer;
use alloy_signer_wallet::LocalWallet;
use alloy_sol_types::{SolCall, SolValue};
use alloy_transport::TransportErrorKind;
use mintpool::config::{ChainInclusionMode, Config};
use mintpool::controller::{ControllerCommands, DBQuery};
use mintpool::premints::zora_premint_v2::rules::is_valid_signature;
use mintpool::premints::zora_premint_v2::types::IZoraPremintV2::MintArguments;
use mintpool::premints::zora_premint_v2::types::{
    IZoraPremintV2, ZoraPremintV2, PREMINT_FACTORY_ADDR,
};
use mintpool::rules::Evaluation::Accept;
use mintpool::rules::RuleContext;
use mintpool::run;
use mintpool::types::PremintTypes;
use std::env;
use std::fmt::Debug;
use std::str::FromStr;
use std::time::Duration;

/// This test does the full round trip lifecycle of a premint
/// 1. Premint is broadcasted to mintpool
/// 2. Premint is fetched from DB (similating a client fetching from API)
/// 3. Premint is brought onchain by a client
/// 4. Premint is removed from mintpool when an event is seen onchain
#[test_log::test(tokio::test)]
async fn test_zora_premint_v2_e2e() {
    // ============================================================================================
    // Configure and launch anvil, watcher, and mintpool
    // ============================================================================================

    let fork_block = 13253646;
    let anvil = Anvil::new()
        .chain_id(7777777)
        .fork_block_number(fork_block)
        .fork("https://rpc.zora.energy")
        .spawn();

    let config = Config {
        seed: 0,
        peer_port: 7778,
        connect_external: false,
        db_url: None,
        persist_state: false,
        prune_minted_premints: false, // important so we can query table
        api_port: 0,
        peer_limit: 10,
        supported_premint_types: "zora_premint_v2".to_string(),
        chain_inclusion_mode: ChainInclusionMode::Check,
        supported_chain_ids: "7777777".to_string(),
        trusted_peers: None,
        node_id: None,
        external_address: None,
        interactive: false,
        enable_rpc: true,
    };

    // set this so CHAINS will use the anvil rpc rather than the one in chains.json
    env::set_var("CHAIN_7777777_RPC_WSS", anvil.ws_endpoint());

    let ctl = run::start_services(&config).await.unwrap();
    run::start_watch_chain::<ZoraPremintV2>(&config, ctl.clone()).await;

    // ============================================================================================
    // Publish a premint to the mintpool
    // ============================================================================================

    // Push a message to the mintpool
    let premint: ZoraPremintV2 = serde_json::from_str(PREMINT_JSON).unwrap();

    ctl.send_command(ControllerCommands::Broadcast {
        message: PremintTypes::ZoraV2(premint),
    })
    .await
    .unwrap();

    // Read the premint from DB, expect there to be 1
    let (send, recv) = tokio::sync::oneshot::channel();
    ctl.send_command(ControllerCommands::Query(DBQuery::ListAll(send)))
        .await
        .unwrap();
    let all_premints = recv.await.unwrap().unwrap();
    assert_eq!(all_premints.len(), 1);

    // ============================================================================================
    // query for message from mintpool
    // ============================================================================================

    let found = all_premints.first().unwrap();
    let premint = match found {
        PremintTypes::ZoraV2(premint) => premint,
        _ => panic!("unexpected premint type"),
    };

    // ============================================================================================
    // bring premint onchain
    // ============================================================================================

    let signer: LocalWallet = anvil.keys()[0].clone().into();
    let signer = signer.with_chain_id(Some(7777777));

    let provider = ProviderBuilder::new()
        .with_recommended_layers()
        .signer(EthereumSigner::from(signer.clone()))
        .on_client(RpcClient::new_http(anvil.endpoint_url()));

    let calldata = {
        let s = premint.clone().signature;
        let h = hex::decode(s).unwrap();
        let sig = Bytes::from(h);
        IZoraPremintV2::premintV2Call {
            contractConfig: premint.clone().collection,
            premintConfig: premint.clone().premint,
            signature: sig,
            quantityToMint: U256::from(1),
            mintArguments: MintArguments {
                mintRecipient: signer.address(),
                mintComment: "".to_string(),
                mintRewardsRecipients: vec![],
            },
        }
    };

    let gas_price = provider.get_gas_price().await.unwrap();
    let max_fee_per_gas = provider.get_max_priority_fee_per_gas().await.unwrap();

    let value: u64 = 777_000_000_000_000;
    // Someone found the premint and brought it onchain
    let tx_request = TransactionRequest {
        from: Some(signer.address()),
        to: Some(PREMINT_FACTORY_ADDR),
        input: Some(Bytes::from(calldata.abi_encode())).into(),
        value: Some(U256::from(value)),
        chain_id: Some(7777777),
        gas_price: Some(gas_price),
        max_fee_per_gas: Some(max_fee_per_gas),
        ..Default::default()
    };

    fn map_call_error(err: RpcError<TransportErrorKind>) -> String {
        match err {
            RpcError::ErrorResp(err) => {
                println!("Error: {:?}", err.clone());
                let b = err.clone().data.unwrap();

                let msg =
                    IZoraPremintV2::premintV2Call::abi_decode_returns(&b.get().abi_encode(), false)
                        .unwrap();
                format!("Error: {:?}, returns: {:?}", err, msg)
            }
            _ => {
                format!("unexpected error, could not parse: {:?}", err)
            }
        }
    }

    let tx = provider.send_transaction(tx_request).await;
    let tx = match tx {
        Ok(tx) => tx,
        Err(e) => panic!("{}", map_call_error(e)),
    };

    match tx.get_receipt().await {
        Ok(_receipt) => {
            println!("receipt found");
        }
        Err(e) => panic!("{}", map_call_error(e)),
    }
    tokio::time::sleep(Duration::from_secs(1)).await;

    // ============================================================================================
    // Confirm is either marked as pruned or removed from DB
    // ============================================================================================
    let (send, recv) = tokio::sync::oneshot::channel();
    ctl.send_command(ControllerCommands::Query(DBQuery::ListAll(send)))
        .await
        .unwrap();
    let all_premints = recv.await.unwrap().unwrap();
    assert_eq!(all_premints.len(), 0);
}

const PREMINT_JSON: &str = r#"
{
  "collection": {
    "contractAdmin": "0xd272a3cb66bea1fa7547dad5b420d5ebe14222e5",
    "contractURI": "ipfs://bafkreicuxlqqgoo6fxlmijqvilckvwj6ey26yvzpwg73ybcltvvek2og6i",
    "contractName": "Fancy title"
  },
  "premint": {
    "tokenConfig": {
      "tokenURI": "ipfs://bafkreia474gkk2ak5eeqstp43nqeiunqkkfeblctna3y54av7bt6uwehmq",
      "maxSupply": 18446744073709551615,
      "maxTokensPerAddress": 0,
      "pricePerToken": 0,
      "mintStart": 1708100240,
      "mintDuration": 2592000,
      "royaltyBPS": 500,
      "fixedPriceMinter": "0x04e2516a2c207e84a1839755675dfd8ef6302f0a",
      "payoutRecipient": "0xd272a3cb66bea1fa7547dad5b420d5ebe14222e5",
      "createReferral": "0x0000000000000000000000000000000000000000"
    },
    "uid": 1,
    "version": 1,
    "deleted": false
  },
  "collectionAddress": "0x0cfbce0e2ea475d6413e2f038b2b62e64106ad1f",
  "chainId": 7777777,
  "signer": "0xd272a3cb66bea1fa7547dad5b420d5ebe14222e5",
  "signature": "0x2eb4d27a5b04fd41bdd33f66a18a4993c0116724c5fe5b8dc20bf22f45455c621139eabdbd27434e240938a60b1952979c9dc9c8a141cc71764786fe4d3f909f1c"
}"#;
