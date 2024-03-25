use crate::types::PremintName;
use envconfig::Envconfig;

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
    premint_types: String,
}

impl Config {
    pub fn initial_network_ip(&self) -> String {
        if self.connect_external {
            "0.0.0.0".to_string()
        } else {
            "127.0.0.1".to_string()
        }
    }

    pub fn premint_types(&self) -> Vec<PremintName> {
        self.premint_types
            .split(",")
            .map(|s| PremintName(s.to_string()))
            .collect()
    }
}

pub fn init() -> Config {
    Config::init_from_env().expect("Failed to load config")
}
