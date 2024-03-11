use tokio::time;

#[tokio::test]
// test to make sure that nodes can connect to a specified host
async fn test_connecting_to_other_nodes() {
    let num_nodes = 10;

    let nodes = build::make_nodes(2000, num_nodes).await;
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
// test
async fn test_announcing_to_network() {
    let num_nodes = 3;

    let nodes = build::make_nodes(2300, num_nodes).await;
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

mod build {
    use mintpool::config::Config;
    use mintpool::controller::{ControllerCommands, ControllerInterface};
    use tokio::time;

    pub async fn announce_all(nodes: Vec<ControllerInterface>) {
        for node in nodes {
            node.send_command(ControllerCommands::AnnounceSelf)
                .await
                .unwrap();
            time::sleep(time::Duration::from_millis(1000)).await;
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

    pub async fn make_nodes(start_port: u64, num_nodes: u64) -> Vec<ControllerInterface> {
        let mut nodes = Vec::new();
        for i in 0..num_nodes {
            let config = Config {
                seed: i,
                port: start_port + i,
                connect_external: false,
            };

            let ctl = mintpool::run::start_swarm_and_controller(&config).unwrap();
            nodes.push(ctl);
        }
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
}
