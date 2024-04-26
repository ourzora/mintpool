mod common;

use crate::common::{asserts, helpers, mintpool_build};
use alloy_signer::Signer;
use common::factories::Factory;
use mintpool::api::routes::submit_premint;
use mintpool::controller::ControllerCommands;
use mintpool::controller::ControllerCommands::Broadcast;
use mintpool::premints::zora_premint_v2::types::ZoraPremintV2;
use mintpool::types::{PremintTypes, SimplePremint};
use tokio::time;

#[test_log::test(tokio::test)]
// test to make sure that nodes can connect to a specified host
async fn test_connecting_to_other_nodes() {
    let num_nodes = 10;

    let nodes = mintpool_build::make_nodes(2000, num_nodes, 1000).await;
    mintpool_build::connect_all_to_first(nodes.clone()).await;
    tokio::time::sleep(time::Duration::from_secs(1)).await;

    let (first, nodes) = mintpool_build::split_first_rest(nodes).await;

    // Expect the first node to be connected to all other nodes,
    // expect all other nodes to just be connected to the first node.
    asserts::expect_n_connections(&first, (num_nodes - 1) as usize).await;
    for node in nodes {
        asserts::expect_n_connections(&node, 1).await;
    }
}

#[test_log::test(tokio::test)]
// test announcing self to the network
async fn test_announcing_to_network() {
    let num_nodes = 3;

    let nodes = mintpool_build::make_nodes(2300, num_nodes, 1000).await;
    mintpool_build::connect_all_to_first(nodes.clone()).await;

    let (first, nodes) = mintpool_build::split_first_rest(nodes).await;
    time::sleep(time::Duration::from_secs(1)).await;

    // have each node broadcast its presence to the network
    mintpool_build::announce_all(nodes.clone()).await;
    time::sleep(time::Duration::from_secs(2)).await;

    // Expect all nodes to be connected to all other nodes
    asserts::expect_n_connections(&first, (num_nodes - 1) as usize).await;
    for node in nodes {
        asserts::expect_n_connections(&node, (num_nodes - 1) as usize).await;
    }
}

#[test_log::test(tokio::test)]
// After a premint is announced, all connected nodes should be able to list it
async fn test_list_all_premints() {
    let num_nodes = 3;

    let nodes = mintpool_build::gen_fully_connected_swarm(2310, num_nodes).await;
    let (first, nodes) = mintpool_build::split_first_rest(nodes).await;
    let (snd, rcv) = tokio::sync::oneshot::channel();
    first
        .send_command(Broadcast {
            message: PremintTypes::ZoraV2(Default::default()),
            channel: snd,
        })
        .await
        .unwrap();
    let (snd, rcv) = tokio::sync::oneshot::channel();

    first
        .send_command(Broadcast {
            message: PremintTypes::Simple(SimplePremint::build_default()),
            channel: snd,
        })
        .await
        .unwrap();

    time::sleep(time::Duration::from_millis(500)).await;

    for node in nodes {
        let (snd, recv) = tokio::sync::oneshot::channel();
        node.send_command(ControllerCommands::Query(
            mintpool::controller::DBQuery::ListAll(snd),
        ))
        .await
        .unwrap();
        let premints = recv.await.unwrap().unwrap();
        assert_eq!(premints.len(), 2);
    }
}

#[test_log::test(tokio::test)]
// Connections should not be able to exceed max_connections config
async fn test_max_connections() {
    let num_nodes = 5;
    let limit = 3;

    let nodes = mintpool_build::make_nodes(2350, num_nodes, limit).await;
    mintpool_build::connect_all_to_first(nodes.clone()).await;

    mintpool_build::announce_all(nodes.clone()).await;

    for node in nodes {
        asserts::expect_lte_than_connections(&node, limit as usize).await;
    }
}

const PREMINT_JSON: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/data/valid_zora_v2_premint.json"
));

#[test_log::test(tokio::test)]
async fn test_node_sync() {
    let num_nodes = 3;

    let nodes = mintpool_build::gen_fully_connected_swarm(2350, num_nodes).await;
    let (first, nodes) = mintpool_build::split_first_rest(nodes).await;
    let second = mintpool_build::make_nodes(3350, 1, 3).await;
    let second = second.into_iter().next().unwrap();

    mintpool_build::connect_all_to_first(nodes.clone()).await;

    // add two premints to the pool
    let premints = vec![
        PremintTypes::Simple(SimplePremint::build_default()),
        PremintTypes::ZoraV2(serde_json::from_str(PREMINT_JSON).unwrap()),
    ];

    futures_util::future::join_all(
        premints
            .iter()
            .map(|premint| helpers::must_submit_premint(&first, premint.clone())),
    )
    .await;

    // check that all premints are in the pool
    let stored_premints = first
        .get_all_premints()
        .await
        .expect("failed to get all premints");

    assert_eq!(stored_premints.len(), premints.len());

    // check that the second node has no premints
    assert_eq!(
        second
            .get_all_premints()
            .await
            .expect("failed to get all premints")
            .len(),
        0
    );

    // after adding the premints, connect the nodes
    second
        .send_command(ControllerCommands::ConnectToPeer {
            address: mintpool_build::get_local_address(&first).await,
        })
        .await
        .unwrap();
    time::sleep(time::Duration::from_secs(1)).await;

    second
        .send_command(ControllerCommands::Sync)
        .await
        .expect("failed to sync");

    // give time for the nodes to sync
    time::sleep(time::Duration::from_secs(3)).await;

    // check that all premints are in the second node
    let stored_premints = second
        .get_all_premints()
        .await
        .expect("failed to get all premints");

    assert_eq!(stored_premints.len(), premints.len());

    for premint in premints {
        let stored_premint = stored_premints
            .iter()
            .find(|p| p == &&premint)
            .expect("premint not found in stored premints");
        assert_eq!(stored_premint, &premint);
    }
}
