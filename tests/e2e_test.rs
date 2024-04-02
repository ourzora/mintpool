mod common;
use crate::common::mintpool_build;
use alloy_node_bindings::Anvil;
use mintpool::premints::zora_v2::PremintV2Message;
use mintpool::run;

#[tokio::test]
#[ignore] // remove once signing logic is complete
async fn test_broadcasting_premint() {
    let anvil = Anvil::new()
        .chain_id(7777777)
        .fork("https://rpc.zora.energy")
        .fork_block_number(12665000)
        .spawn();

    let config = mintpool_build::make_config(2222, 1000);
    let ctl = mintpool_build::make_nodes(config.port, 1, config.peer_limit).await;
    let ctl = ctl.first().unwrap();
    // in real
    run::start_watch_chain::<PremintV2Message>(&config, ctl.clone()).await;

    // todo!("This test needs to be completed once signing logic is complete");
}
