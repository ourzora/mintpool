use std::sync::Arc;
use std::time::Duration;

use alloy::network::{Ethereum, Network};
use alloy::pubsub::PubSubFrontend;
use alloy_provider::layers::{GasEstimatorProvider, ManagedNonceProvider};
use alloy_provider::{Identity, ProviderBuilder, RootProvider};
use alloy_rpc_client::WsConnect;
use eyre::ContextCompat;
use mini_moka::sync::Cache;
use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};

const CHAINS_JSON: &str = include_str!("../data/chains.json");

pub type ChainListProvider<N = Ethereum> = GasEstimatorProvider<
    PubSubFrontend,
    ManagedNonceProvider<PubSubFrontend, RootProvider<PubSubFrontend, N>, N>,
    N,
>;

pub struct Chains<N>(Vec<Chain>, Cache<String, Arc<ChainListProvider<N>>>)
where
    N: Network;

pub static CHAINS: Lazy<Chains<Ethereum>> = Lazy::new(Chains::new);
static VARIABLE_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"\$\{(.+?)}").unwrap());

impl<N: Network> Chains<N>
where
    N: Network,
{
    fn new() -> Self {
        Chains(
            serde_json::from_str::<Vec<Chain>>(CHAINS_JSON).unwrap(),
            Cache::builder()
                .time_to_idle(Duration::from_secs(5 * 60))
                .build(),
        )
    }

    pub fn get_chain_by_id(&self, chain_id: i64) -> Option<Chain> {
        self.0
            .iter()
            .find(|chain| chain.chain_id == chain_id)
            .cloned()
    }

    pub async fn get_rpc(&self, chain_id: i64) -> eyre::Result<Arc<ChainListProvider<N>>> {
        let chain = self
            .get_chain_by_id(chain_id)
            .wrap_err(format!("Chain id {} not found", chain_id))?;

        for rpc in chain.rpc.iter() {
            if !rpc.starts_with("ws") {
                continue;
            }

            tracing::info!("Trying to connect to {}", rpc);
            let provider = self.connect(rpc).await;
            if provider.is_ok() {
                return provider;
            }
        }

        Err(eyre::eyre!("No suitable RPC URL found for chain"))
    }

    async fn connect(&self, url: &String) -> eyre::Result<Arc<ChainListProvider<N>>> {
        if VARIABLE_REGEX.is_match(url) {
            return Err(eyre::eyre!("URL contains variables"));
        }

        let cached = self.1.get(url);
        match cached {
            Some(provider) => Ok(provider),
            None => {
                let conn = WsConnect::new(url);
                let provider: ChainListProvider<N> = ProviderBuilder::<Identity, N>::default()
                    .with_recommended_layers()
                    .on_ws(conn)
                    .await?;

                let arc = Arc::new(provider);

                // keep a copy in the cache
                self.1.insert(url.clone(), arc.clone());

                Ok(arc)
            }
        }
    }
}

// types created by https://transform.tools/json-to-rust-serde
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Chain {
    pub name: String,
    pub chain: String,
    pub rpc: Vec<String>,
    pub chain_id: i64,
    pub network_id: i64,
}

#[cfg(test)]
mod test {
    use super::*;
    use alloy_provider::Provider;

    #[test]
    fn test_chains_new() {
        let _chains = Chains::<Ethereum>::new();
    }

    #[test]
    fn test_get_chain_by_id() {
        let chain = CHAINS.get_chain_by_id(7777777);
        assert!(chain.is_some());
        assert_eq!(chain.unwrap().name, "Zora".to_string());
    }

    #[tokio::test]
    async fn test_chain_connect() {
        let provider = CHAINS
            .get_rpc(7777777)
            .await
            .expect("Zora Chain should exist");

        // quick integration test here
        let number = provider.get_block_number().await.unwrap();
        assert!(number > 0);
    }

    #[tokio::test]
    async fn test_chain_connect_variable() {
        let url = "https://mainnet.infura.io/v3/${INFURA_API_KEY}".to_string();
        let provider = CHAINS.connect(&url).await;

        assert!(provider.is_err());
        match provider {
            Ok(_) => panic!("Expected error"),
            Err(e) => assert!(e.to_string().contains("URL contains variables")),
        }
    }
}
