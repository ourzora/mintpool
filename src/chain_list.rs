use std::marker::PhantomData;

use alloy::network::{Ethereum, Network};
use alloy::pubsub::PubSubFrontend;
use alloy_provider::layers::{GasEstimatorProvider, ManagedNonceProvider};
use alloy_provider::{Provider, ProviderBuilder, RootProvider};
use alloy_rpc_client::WsConnect;
use alloy_transport::BoxTransport;
use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};

const CHAINS_JSON: &str = include_str!("../data/chains.json");

pub struct Chains(Vec<Chain>);

pub static CHAINS: Lazy<Chains> = Lazy::new(|| Chains::new());
static VARIABLE_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"\$\{(.+?)}").unwrap());

impl Chains {
    fn new() -> Self {
        Chains(serde_json::from_str::<Vec<Chain>>(CHAINS_JSON).unwrap())
    }

    pub fn get_chain_by_id(&self, chain_id: i64) -> Option<Chain> {
        self.0
            .iter()
            .find(|chain| chain.chain_id == chain_id)
            .cloned()
    }
}

// types created by https://transform.tools/json-to-rust-serde
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Chain {
    pub name: String,
    pub chain: String,
    pub icon: Option<String>,
    pub rpc: Vec<String>,
    #[serde(default)]
    pub features: Vec<Feature>,
    pub faucets: Vec<String>,
    pub native_currency: NativeCurrency,
    #[serde(rename = "infoURL")]
    pub info_url: String,
    pub short_name: String,
    pub chain_id: i64,
    pub network_id: i64,
    pub slip44: Option<i64>,
    pub ens: Option<Ens>,
    #[serde(default)]
    pub explorers: Vec<Explorer>,
    pub title: Option<String>,
    pub status: Option<String>,
    #[serde(default)]
    pub red_flags: Vec<String>,
    pub parent: Option<Parent>,
}

pub type ChainListProvider = RootProvider<PubSubFrontend, Ethereum>;

// Note: this ideally should just return <P: Provider> but alloy is doing something weird where it
// doesn't recognize RootProvider as impl Provider
async fn connect(url: &String) -> eyre::Result<ChainListProvider> {
    if VARIABLE_REGEX.is_match(url) {
        return Err(eyre::eyre!("URL contains variables"));
    }

    let conn = WsConnect::new(url);
    let x = ProviderBuilder::new().on_ws(conn).await?;
    Ok(x)
}

impl Chain {
    pub async fn get_rpc(&self, need_pub_sub: bool) -> eyre::Result<ChainListProvider> {
        for rpc in self.rpc.iter() {
            if need_pub_sub && !rpc.starts_with("ws") {
                continue;
            }

            tracing::info!("Trying to connect to {}", rpc);
            let provider = connect(rpc).await;
            if provider.is_ok() {
                return provider;
            }
        }

        Err(eyre::eyre!("No suitable RPC URL found for chain"))
    }
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Feature {
    pub name: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeCurrency {
    pub name: String,
    pub symbol: String,
    pub decimals: i64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Ens {
    pub registry: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Explorer {
    pub name: String,
    pub url: String,
    pub standard: String,
    pub icon: Option<String>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Parent {
    #[serde(rename = "type")]
    pub type_field: String,
    pub chain: String,
    #[serde(default)]
    pub bridges: Vec<Bridge>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Bridge {
    pub url: String,
}

#[cfg(test)]
mod test {
    use alloy::network::Ethereum;

    use super::*;

    #[test]
    fn test_chains_new() {
        let _chains = Chains::new();
    }

    #[test]
    fn test_get_chain_by_id() {
        let chain = CHAINS.get_chain_by_id(7777777);
        assert!(chain.is_some());
        assert_eq!(chain.unwrap().name, "Zora".to_string());
    }

    #[tokio::test]
    async fn test_chain_connect() {
        let chain = CHAINS.get_chain_by_id(7777777).unwrap();
        let provider = connect(&chain.rpc[1]).await.unwrap();

        // quick integration test here
        let number = provider.get_block_number().await.unwrap();
        assert!(number > 0);
    }

    #[tokio::test]
    async fn test_chain_connect_variable() {
        let url = "https://mainnet.infura.io/v3/${INFURA_API_KEY}".to_string();
        let provider = connect(&url).await;

        assert!(provider.is_err());
        match provider {
            Ok(_) => panic!("Expected error"),
            Err(e) => assert!(e.to_string().contains("URL contains variables")),
        }
    }
}
