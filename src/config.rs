use envconfig::Envconfig;

#[derive(Envconfig, Debug)]
pub struct Config {
    pub seed: u64,
    #[envconfig(from = "PORT", default = "7777")]
    pub port: u64,
}

pub fn init() -> Config {
    Config::init_from_env().expect("Failed to load config")
}
