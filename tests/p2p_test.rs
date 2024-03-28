use mintpool::controller::ControllerCommands;
use mintpool::controller::ControllerCommands::Broadcast;
use mintpool::types::PremintTypes;
use tokio::time;

#[tokio::test]
// test to make sure that nodes can connect to a specified host
async fn test_connecting_to_other_nodes() {
    let num_nodes = 10;

    let nodes = build::make_nodes(2000, num_nodes, 1000).await;
    build::connect_all_to_first(nodes.clone()).await;

    let (first, nodes) = build::split_first_rest(nodes).await;

    // Expect the first node to be connected to all other nodes,
    // expect all other nodes to just be connected to the first node.
    asserts::expect_n_connections(&first, (num_nodes - 1) as usize).await;
    for node in nodes {
        asserts::expect_n_connections(&node, 1).await;
    }
}

#[tokio::test]
// test announcing self to the network
async fn test_announcing_to_network() {
    let num_nodes = 3;

    let nodes = build::make_nodes(2300, num_nodes, 1000).await;
    build::connect_all_to_first(nodes.clone()).await;

    let (first, nodes) = build::split_first_rest(nodes).await;
    time::sleep(time::Duration::from_secs(1)).await;

    // have each node broadcast its presence to the network
    build::announce_all(nodes.clone()).await;
    time::sleep(time::Duration::from_secs(2)).await;

    // Expect all nodes to be connected to all other nodes
    asserts::expect_n_connections(&first, (num_nodes - 1) as usize).await;
    for node in nodes {
        asserts::expect_n_connections(&node, (num_nodes - 1) as usize).await;
    }
}

#[tokio::test]
// After a premint is announced, all connected nodes should be able to list it
async fn test_list_all_premints() {
    let num_nodes = 3;

    let nodes = build::gen_fully_connected_swarm(2310, num_nodes).await;
    let (first, nodes) = build::split_first_rest(nodes).await;

    first
        .send_command(Broadcast {
            message: PremintTypes::V2(Default::default()),
        })
        .await
        .unwrap();

    first
        .send_command(Broadcast {
            message: PremintTypes::Simple(Default::default()),
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

#[tokio::test]
// Connections should not be able to exceed max_connections config
async fn test_max_connections() {
    let num_nodes = 5;
    let limit = 3;

    let nodes = build::make_nodes(2350, num_nodes, limit).await;
    build::connect_all_to_first(nodes.clone()).await;

    build::announce_all(nodes.clone()).await;

    for node in nodes {
        asserts::expect_lte_than_connections(&node, limit as usize).await;
    }
}

mod build {
    use mintpool::config::Config;
    use mintpool::controller::{ControllerCommands, ControllerInterface};
    use tokio::time;

    pub async fn announce_all(nodes: Vec<ControllerInterface>) {
        for node in nodes {
            node.send_command(ControllerCommands::AnnounceSelf)
                .await
                .unwrap();
            time::sleep(time::Duration::from_millis(500)).await;
        }
    }

    pub async fn split_first_rest(
        nodes: Vec<ControllerInterface>,
    ) -> (ControllerInterface, Vec<ControllerInterface>) {
        if let Some((first, nodes)) = nodes.split_first() {
            (first.clone(), nodes.to_vec())
        } else {
            panic!("nodes is empty")
        }
    }

    pub async fn make_nodes(
        start_port: u64,
        num_nodes: u64,
        peer_limit: u64,
    ) -> Vec<ControllerInterface> {
        let mut nodes = Vec::new();
        let rand_n = rand::random::<u64>();
        for i in 0..num_nodes {
            let config = Config {
                seed: rand_n + i,
                port: start_port + i,
                connect_external: false,
                db_url: None,
                persist_state: false,
                prune_minted_premints: false,
                peer_limit,
                premint_types: "simple,zora_premint_v2".to_string(),
            };

            let ctl = mintpool::run::start_services(&config).await.unwrap();
            nodes.push(ctl);
        }
        nodes
    }

    pub async fn gen_fully_connected_swarm(
        start_port: u64,
        num_nodes: u64,
    ) -> Vec<ControllerInterface> {
        let nodes = make_nodes(start_port, num_nodes, 1000).await;
        connect_all_to_first(nodes.clone()).await;
        time::sleep(time::Duration::from_secs(1)).await;

        // have each node broadcast its presence to the network
        announce_all(nodes.clone()).await;
        time::sleep(time::Duration::from_secs(1)).await;
        nodes
    }

    pub async fn connect_all_to_first(nodes: Vec<ControllerInterface>) {
        if let Some((first, nodes)) = nodes.split_first() {
            let n1_info = first
                .get_node_info()
                .await
                .expect("failed to get node info from n1");
            let n1_local_addr = n1_info
                .addr
                .first()
                .unwrap()
                .clone()
                .with_p2p(n1_info.peer_id)
                .unwrap()
                .to_string();

            for node in nodes {
                node.send_command(ControllerCommands::ConnectToPeer {
                    address: n1_local_addr.clone(),
                })
                .await
                .unwrap();
            }

            // Give connections time to establish
            time::sleep(time::Duration::from_secs(1)).await;
        } else {
            panic!("nodes is empty")
        }
    }
}

mod asserts {
    pub async fn expect_n_connections(ctl: &mintpool::controller::ControllerInterface, n: usize) {
        let state = ctl
            .get_network_state()
            .await
            .expect("failed to get network state");

        assert_eq!(state.network_info.num_peers(), n);
        assert_eq!(state.gossipsub_peers.len(), n);
    }

    // expect less than or equal to n connections
    pub async fn expect_lte_than_connections(
        ctl: &mintpool::controller::ControllerInterface,
        n: usize,
    ) {
        let state = ctl
            .get_network_state()
            .await
            .expect("failed to get network state");
        println!("{:?}", state);

        let peers = state.network_info.num_peers();
        assert!(peers <= n);
        let peers = state.gossipsub_peers.len();
        assert!(peers <= n);
    }
}
