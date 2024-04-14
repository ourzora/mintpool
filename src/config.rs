use std::collections::HashMap;
use std::env;
use std::str::FromStr;

use envconfig::Envconfig;
use rand::Rng;

use crate::chain_list::CHAINS;
use crate::types::PremintName;

#[derive(Envconfig, Debug)]
pub struct Config {
    #[envconfig(from = "SEED")]
    pub seed: u64,

    #[envconfig(from = "PEER_PORT", default = "7778")]
    pub peer_port: u64,

    #[envconfig(from = "CONNECT_EXTERNAL", default = "true")]
    pub connect_external: bool,

    #[envconfig(from = "DATABASE_URL")]
    pub db_url: Option<String>,

    #[envconfig(from = "PERSIST_STATE", default = "false")]
    pub persist_state: bool,

    #[envconfig(from = "PRUNE_MINTED_PREMINTS", default = "true")]
    pub prune_minted_premints: bool,

    #[envconfig(from = "API_PORT", default = "7777")]
    pub api_port: u64,

    #[envconfig(from = "PEER_LIMIT", default = "1000")]
    pub peer_limit: u64,

    // Comma separated list of default premint types to process
    #[envconfig(from = "PREMINT_TYPES", default = "zora_premint_v2")]
    pub supported_premint_types: String,

    #[envconfig(from = "CHAIN_INCLUSION_MODE", default = "verify")]
    pub chain_inclusion_mode: ChainInclusionMode,

    #[envconfig(from = "SUPPORTED_CHAIN_IDS", default = "7777777,8453")]
    pub supported_chain_ids: String,
    // Dynamic configuration: RPC urls take the form of CHAIN_<chain_id>_RPC_WSS
    // If not provided in the environment, the default is to use the public node
    #[envconfig(from = "TRUSTED_PEERS")]
    pub trusted_peers: Option<String>,

    // node_id will only be used for logging purposes, if set
    #[envconfig(from = "NODE_ID")]
    pub node_id: Option<u64>,

    #[envconfig(from = "EXTERNAL_ADDRESS")]
    pub external_address: Option<String>,

    #[envconfig(from = "INTERACTIVE", default = "false")]
    pub interactive: bool,
}

impl Config {
    pub fn test_default() -> Self {
        Config {
            seed: rand::random(),
            peer_port: rand::thread_rng().gen_range(5000..=10000),
            connect_external: false,
            db_url: None,
            persist_state: false,
            prune_minted_premints: false,
            api_port: 0,
            peer_limit: 1000,
            supported_premint_types: "simple,zora_premint_v2".to_string(),
            chain_inclusion_mode: ChainInclusionMode::Check,
            supported_chain_ids: "7777777,999999999".to_string(),
            trusted_peers: None,
            node_id: None,
            external_address: None,
            interactive: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ChainInclusionMode {
    Check,  // node will check chains for new premints getting included
    Verify, // node will verify that premints are included on chain based on messages from other nodes
    Trust, // node will trust that premints are included on chain based on messages from other trusted nodes
}

impl FromStr for ChainInclusionMode {
    type Err = eyre::Report;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "check" => Ok(Self::Check),
            "verify" => Ok(Self::Verify),
            "trust" => Ok(Self::Trust),
            _ => Err(eyre::eyre!("Invalid chain inclusion mode")),
        }
    }
}

impl Config {
    pub fn initial_network_ip(&self) -> String {
        if self.connect_external {
            "0.0.0.0".to_string()
        } else {
            "127.0.0.1".to_string()
        }
    }

    pub fn premint_names(&self) -> Vec<PremintName> {
        self.supported_premint_types
            .split(',')
            .map(|s| PremintName(s.to_string()))
            .collect()
    }

    pub fn supported_chains(&self) -> Vec<u64> {
        self.supported_chain_ids
            .split(',')
            .map(|s| s.parse().unwrap())
            .collect()
    }

    pub fn trusted_peers(&self) -> Vec<String> {
        match &self.trusted_peers {
            None => vec![],
            Some(peers) => peers.split(',').map(|s| s.to_string()).collect(),
        }
    }

    pub fn rpc_url(&self, chain_id: u64) -> eyre::Result<String> {
        let defaults = HashMap::from([
            (7777777, "wss://rpc.zora.co"),
            (8423, "wss://base-rpc.publicnode.com"),
        ]);

        match env::var(format!("CHAIN_{}_RPC_WSS", chain_id)) {
            Ok(url) => Ok(url),
            Err(_) => match defaults.get(&chain_id) {
                Some(url) => Ok(url.to_string()),
                None => Err(eyre::eyre!("No default RPC URL for chain {}", chain_id)),
            },
        }
    }

    pub fn validate(self) -> Self {
        for chain_id in self.supported_chains() {
            CHAINS
                .get_chain_by_id(chain_id as i64)
                .expect(format!("Chain ID {} is not supported", chain_id).as_str());
        }
        self
    }
}

pub fn init() -> Config {
    Config::init_from_env()
        .expect("Failed to load config")
        .validate()
}

#[cfg(test)]
mod test {
    use crate::config::ChainInclusionMode;

    #[test]
    fn test_premint_names() {
        let config = super::Config {
            seed: 0,
            peer_port: 7777,
            connect_external: false,
            db_url: None,
            persist_state: false,
            prune_minted_premints: false,
            api_port: 0,
            peer_limit: 1000,
            supported_premint_types: "simple,zora_premint_v2".to_string(),
            chain_inclusion_mode: ChainInclusionMode::Check,
            supported_chain_ids: "7777777".to_string(),
            trusted_peers: None,
            node_id: None,
            external_address: None,
            interactive: false,
        };

        let names = config.premint_names();
        assert_eq!(names.len(), 2);
        assert_eq!(names[0].0, "simple");
        assert_eq!(names[1].0, "zora_premint_v2");

        let config = super::Config {
            seed: 0,
            peer_port: 7777,
            connect_external: false,
            db_url: None,
            persist_state: false,
            prune_minted_premints: false,
            api_port: 0,
            peer_limit: 1000,
            supported_premint_types: "zora_premint_v2".to_string(),
            chain_inclusion_mode: ChainInclusionMode::Check,
            supported_chain_ids: "7777777".to_string(),
            trusted_peers: None,
            node_id: None,
            external_address: None,
            interactive: false,
        };

        let names = config.premint_names();
        assert_eq!(names.len(), 1);
        assert_eq!(names[0].0, "zora_premint_v2");
    }
}
