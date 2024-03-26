use crate::types::PremintName;
use crate::types::PremintName;
use envconfig::Envconfig;
use std::collections::HashMap;
use std::env;

#[derive(Envconfig, Debug)]
pub struct Config {
    #[envconfig(from = "SEED")]
    pub seed: u64,

    #[envconfig(from = "PORT", default = "7777")]
    pub port: u64,

    #[envconfig(from = "CONNECT_EXTERNAL", default = "true")]
    pub connect_external: bool,

    #[envconfig(from = "DATABASE_URL")]
    pub db_url: Option<String>,

    #[envconfig(from = "PERSIST_STATE", default = "false")]
    pub persist_state: bool,

    #[envconfig(from = "PRUNE_MINTED_PREMINTS", default = "true")]
    pub prune_minted_premints: bool,

    #[envconfig(from = "PEER_LIMIT", default = "1000")]
    pub peer_limit: u64,

    // Comma separated list of default premint types to process
    #[envconfig(from = "PREMINT_TYPES", default = "zora_premint_v2")]
    pub premint_types: String,

    #[envconfig(from = "CHAIN_INCLUSION_MODE", default = "verify")]
    pub chain_inclusion_mode: ChainInclusionMode,

    #[envconfig(from = "SUPPORTED_CHAIN_IDS", default = "777777,8423")]
    pub supported_chain_ids: String,
    // Dynamic configuration: RPC urls take the form of CHAIN_<chain_id>_RPC_WSS
    // If not provided in the environment, the default is to use the public node
}

enum ChainInclusionMode {
    Check,
    Verify,
    Trust,
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
        self.premint_types
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

    pub fn rpc_url(&self, chain_id: u64) -> eyre::Result<String> {
        let defaults = HashMap::from([
            (777777, "wss://rpc.zora.co"),
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
            self.rpc_url(chain_id).expect(format!("Failed to get RPC URL for configured chain_id {chain_id}. Set environment variable CHAIN_{chain_id}_RPC_WSS").as_str());
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
            port: 7777,
            connect_external: false,
            db_url: None,
            persist_state: false,
            prune_minted_premints: false,
            peer_limit: 1000,
            premint_types: "simple,zora_premint_v2".to_string(),
            chain_inclusion_mode: ChainInclusionMode::Check,
            supported_chain_ids: "7777777".to_string(),
        };

        let names = config.premint_names();
        assert_eq!(names.len(), 2);
        assert_eq!(names[0].0, "simple");
        assert_eq!(names[1].0, "zora_premint_v2");

        let config = super::Config {
            seed: 0,
            port: 7777,
            connect_external: false,
            db_url: None,
            persist_state: false,
            prune_minted_premints: false,
            peer_limit: 1000,
            premint_types: "simple,zora_premint_v2".to_string(),
            chain_inclusion_mode: ChainInclusionMode::Check,
            supported_chain_ids: "7777777".to_string(),
        };

        let names = config.premint_names();
        assert_eq!(names.len(), 1);
        assert_eq!(names[0].0, "zora_premint_v2");
    }
}
