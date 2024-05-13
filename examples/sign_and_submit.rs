use alloy::hex;
use alloy::primitives::U256;
use alloy::signers::wallet::LocalWallet;
use alloy::signers::Signer;
use mintpool::chain::view_contract_call;
use mintpool::chain_list::CHAINS;
use mintpool::config::Config;
use mintpool::premints::zora_premint::contract::IZoraPremintV2::{
    ContractCreationConfig, CreatorAttribution, TokenCreationConfig,
};
use mintpool::premints::zora_premint::contract::{IZoraPremintV2, PREMINT_FACTORY_ADDR};
use mintpool::premints::zora_premint::v2::V2;
use mintpool::rules::RulesEngine;
use mintpool::storage::PremintStorage;
use mintpool::types::PremintTypes;

/// Create a premint, sign it, and simulate submitting it
/// This involves creating a Zora V2 premint type, then signing the `CreatorAttribution` struct
/// using typed messaged with the EIP-712 domain.
#[tokio::main]
async fn main() -> eyre::Result<()> {
    let chain_id = 7777777;

    // end users wallet that signs the premint (doesn't need gas)
    let user_wallet = LocalWallet::random();
    let user_address = user_wallet.address();

    let chain = CHAINS.get_rpc(chain_id).await?;
    let contract_config = ContractCreationConfig {
        contractAdmin: user_address,
        // Uploading to IPFS should be a separate step
        contractURI: "ipfs://someCollectionCid".to_string(),
        contractName: "Example Zora premint mintpool message".to_string(),
    };

    // Get the collection address the premint would be deployed at onchain
    let collection_address = {
        let addr = view_contract_call(
            IZoraPremintV2::getContractAddressCall {
                contractConfig: contract_config.clone(),
            },
            &chain,
            PREMINT_FACTORY_ADDR,
        )
        .await?;
        addr._0
    };

    // this can be any available uint, ideally monotonically increasing.
    // If contract doesn't exist use 1, else use # tokens + 1
    let uid = 1;

    // Token creation settings. Importantly includes the token uri.
    let token_creation_config = TokenCreationConfig {
        // Uploading to IPFS should be a separate step
        tokenURI: "ipfs://tokenIPFSCid".to_string(),
        maxSupply: U256::from(10000),
        maxTokensPerAddress: 10,
        pricePerToken: 0,
        mintStart: Default::default(),
        mintDuration: Default::default(),
        royaltyBPS: 0,
        payoutRecipient: user_address,
        fixedPriceMinter: Default::default(),
        createReferral: Default::default(), // this could be you!
    };

    // Creator & contract attributes
    let creator_attribution = CreatorAttribution {
        tokenConfig: token_creation_config,
        uid,
        version: 2,
        deleted: false,
    };

    // Fully formed type
    let mut premint = V2 {
        collection_address,
        chain_id,
        collection: contract_config.clone(),
        premint: creator_attribution.clone(),
        signature: "".to_string(),
    };

    // compute and set the domain
    premint.signature = {
        let domain = premint.eip712_domain();
        let sig = user_wallet
            .sign_typed_data::<CreatorAttribution>(&creator_attribution, &domain)
            .await?;
        hex::encode(sig.as_bytes())
    };

    // Premint now should be valid
    println!("Premint: {:?}", premint);

    let premint_item = PremintTypes::ZoraV2(premint);

    // optional: validate the premint against the default mintpool rules
    let config = Config::test_default();
    let store = PremintStorage::new(&config).await;
    let mut rules_engine = RulesEngine::new(&config);
    rules_engine.add_default_rules();
    let result = rules_engine.evaluate(&premint_item, store).await?;
    println!("Result: {:?}", result);
    assert!(result.is_accept());

    // Submit premint to a premint node
    // can be any mintpool node
    let client = reqwest::Client::new();
    client
        .post("http://mintpool.zora.co/submit-premint")
        .json(&premint_item)
        .send()
        .await?;

    Ok(())
}
