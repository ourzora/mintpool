use alloy::network::{Ethereum, EthereumSigner};
use alloy::primitives::{Bytes, Sign, TxKind, B256, U256, address};
use alloy::providers::fillers::{
    ChainIdFiller, FillProvider, GasFiller, JoinFill, NonceFiller, SignerFiller,
};
use alloy::providers::{Identity, Provider, ProviderBuilder, RootProvider, WalletProvider};
use alloy::rpc::client::RpcClient;
use alloy::rpc::types::eth::{BlockId, TransactionRequest};
use alloy::signers::k256::ecdsa::SigningKey;
use alloy::signers::wallet::Wallet;
use alloy::signers::Signer;
use alloy::sol_types::SolCall;
use alloy::transports::http::Http;
use lazy_static::lazy_static;
use mintpool::api::start_api;
use mintpool::{metadata_rule, typed_rule};
use mintpool::premints::zora_premint_v2::types::IZoraPremintV2::MintArguments;
use mintpool::premints::zora_premint_v2::types::{
    IZoraPremintV2, ZoraPremintV2, PREMINT_FACTORY_ADDR,
};
use mintpool::rules::{Evaluation, Rule, RuleContext, RulesEngine};
use mintpool::storage::Reader;
use mintpool::types::{Premint, PremintMetadata, PremintTypes};
use reqwest::Client;
use std::collections::HashMap;
use async_trait::async_trait;
use tokio::signal::unix::{signal, SignalKind};

#[tokio::main]
async fn main() -> eyre::Result<()> {
    let config = mintpool::config::init();

    let minter = Minter::new();
    let start_balance = minter.current_balance(7777777).await?;
    println!("start balance: {:?}", display_eth(start_balance));

    let mut rules = RulesEngine::new_with_default_rules(&config);
    rules.add_rule(Box::new(minter));

    let ctl = mintpool::run::start_p2p_services(config.clone(), rules).await?;
    let router = mintpool::api::router_with_defaults(&config);
    start_api(&config, ctl.clone(), router, true).await?;



    let mut sigint = signal(SignalKind::interrupt())?;
    let mut sigterm = signal(SignalKind::terminate())?;
    tokio::select! {
        _ = sigint.recv() => {
            tracing::info!("Received SIGINT, shutting down");
        }
        _ = sigterm.recv() => {
            tracing::info!("Received SIGTERM, shutting down");
        }
    }
    Ok(())
}

fn display_eth(v: U256) -> String {
    let i: f64 = match v.try_into(){
        Ok(v) => v,
        Err(_) => return format!("Too big: {:?}", v),
    };

    format!("{:.6}", i / 1e18)
}



// Passing an alloy type currently is pretty annoying
type MintProvider = FillProvider<
    JoinFill<
        JoinFill<JoinFill<JoinFill<Identity, GasFiller>, NonceFiller>, ChainIdFiller>,
        SignerFiller<EthereumSigner>,
    >,
    RootProvider<Http<Client>>,
    Http<Client>,
    Ethereum,
>;

struct Minter {
    signer: Wallet<SigningKey>,
    providers: HashMap<u64, MintProvider>,
}

#[async_trait]
impl<T: Reader> Rule<T> for Minter {
    async fn check(&self, item: &PremintTypes, context: &RuleContext<T>) -> eyre::Result<Evaluation> {
        match item {
            PremintTypes::ZoraV2(premint) => {
                // If from jacob.eth, mint it
                if premint.metadata().signer == address!("17cd072cBd45031EFc21Da538c783E0ed3b25DCc") {
                    match self.mint_zora_premint(premint.clone()).await {
                        Ok(_) => {
                            println!("Minted premint: {:?}", premint);
                        }
                        Err(e) => {
                            println!("Failed to mint premint: {:?}", e);
                        }
                    }
                    Ok(Evaluation::Reject("Minted".to_string()))
                } else {
                    Ok(Evaluation::Accept)
                }
            },
            _ => Ok(Evaluation::Ignore("not a zora premint".to_string())),
        }
    }

    fn rule_name(&self) -> &'static str {
        "simple minter"
    }
}

impl Minter {
    pub fn new() -> Self {
        let pkey = hex::decode(std::env::var("PRIVATE_KEY").unwrap().strip_prefix("0x").unwrap())
            .expect("failed to decode private key");
        let pkey = B256::from_slice(pkey.as_slice());
        let signer = Wallet::from_bytes(&pkey).expect("failed to create wallet");

        let mut providers = HashMap::new();

        Minter { signer, providers }
    }

    fn make_provider_for_chain(
        &self,
        chain_id: u64,
        signer: Wallet<SigningKey>,
    ) -> eyre::Result<MintProvider> {
        let provider = ProviderBuilder::new()
            .with_recommended_fillers()
            .signer(EthereumSigner::from(signer.clone()))
            .on_client(RpcClient::new_http("https://rpc.zora.energy".parse()?));
        Ok(provider)
    }

    async fn current_balance(&self, chain_id: u64) -> eyre::Result<U256> {
        let provider = self.make_provider_for_chain(chain_id, self.signer.clone())?;
        let balance = provider
            .get_balance(self.signer.address(), BlockId::latest())
            .await?;
        Ok(balance)
    }
}

impl Minter {
    async fn mint_zora_premint(&self, premint: ZoraPremintV2) -> eyre::Result<()> {
        let mut signer = self.signer.clone();
        let signer = signer.with_chain_id(Some(premint.chain_id));
        let provider = self.make_provider_for_chain(premint.chain_id, signer.clone())?;

        let calldata = {
            let s = premint.clone().signature;
            let h = hex::decode(s).unwrap();
            let sig = Bytes::from(h);
            IZoraPremintV2::premintV2Call {
                contractConfig: premint.clone().collection,
                premintConfig: premint.clone().premint,
                signature: sig,
                quantityToMint: U256::from(1),
                mintArguments: MintArguments {
                    mintRecipient: signer.address(),
                    mintComment: "".to_string(),
                    mintRewardsRecipients: vec![],
                },
            }
        };

        let gas_price = provider.get_gas_price().await?;
        let max_fee_per_gas = provider.get_max_priority_fee_per_gas().await?;

        let value: u64 = 777_000_000_000_000;
        // Someone found the premint and brought it onchain
        let tx_request = TransactionRequest {
            from: Some(signer.address()),
            to: Some(TxKind::Call(PREMINT_FACTORY_ADDR)),
            input: Some(Bytes::from(calldata.abi_encode())).into(),
            value: Some(U256::from(value)),
            chain_id: Some(7777777),
            gas_price: Some(gas_price),
            max_fee_per_gas: Some(max_fee_per_gas),
            ..Default::default()
        };

        let tx = provider.send_transaction(tx_request).await?;
        let receipt = tx.get_receipt().await?;
        println!("receipt: {:?}", receipt);

        Ok(())
    }
}
