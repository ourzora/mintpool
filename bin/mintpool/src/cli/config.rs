use envconfig::Envconfig;

#[derive(Envconfig, Debug)]
pub struct Config {
    #[envconfig(from = "SEED")]
    pub seed: u64,
    #[envconfig(from = "PORT", default = "7777")]
    pub port: u64,
    #[envconfig(from = "CONNECT_EXTERNAL", default = "true")]
    pub connect_external: bool,
}

impl Config {
    pub fn network_ip(&self) -> String {
        if self.connect_external {
            "0.0.0.0".to_string()
        } else {
            "127.0.0.1".to_string()
        }
    }
}

pub fn init() -> Config {
    Config::init_from_env().expect("Failed to load config")
}
