pub mod factories;
pub mod mintpool_build {
    use chrono::format;
    use mintpool::config::{BootNodes, ChainInclusionMode, Config};
    use mintpool::controller::{ControllerCommands, ControllerInterface};
    use mintpool::rules::RulesEngine;
    use rand::Rng;
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

    pub fn make_config(port: u64, peer_limit: u64) -> Config {
        let mut config = Config::test_default();
        config.secret = format!("0x{}", rand::thread_rng().gen_range(10..99));
        config.peer_port = port;
        config.peer_limit = peer_limit;

        config
    }

    pub async fn make_nodes(
        start_port: u64,
        num_nodes: u64,
        peer_limit: u64,
    ) -> Vec<ControllerInterface> {
        let mut nodes = Vec::new();
        for i in 0..num_nodes {
            let config = make_config(start_port + i, peer_limit);

            let ctl = mintpool::run::start_p2p_services(
                &config,
                RulesEngine::new_with_default_rules(&config),
            )
            .await
            .unwrap();
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

pub mod asserts {
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
