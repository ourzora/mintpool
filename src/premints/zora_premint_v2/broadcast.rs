use crate::premints::zora_premint_v2::types::{ZoraPremintV2, PREMINT_FACTORY_ADDR};
use alloy::network::Ethereum;
use alloy::pubsub::PubSubFrontend;
use alloy::rpc::types::eth::TransactionRequest;

use alloy_provider::{Provider, RootProvider};
use alloy_sol_macro::sol;

sol! {
    IZoraPremintV2,
    "src/premints/zora_premint_v2/zora1155PremintExecutor.json"
}

// async fn broadcast_premint_v2(
//     premint: ZoraPremintV2,
//     provider: RootProvider<Ethereum, PubSubFrontend>,
// ) -> eyre::Result<()> {
//     let tx = TransactionRequest::to(Some(PREMINT_FACTORY_ADDR));
//
//     provider.send_transaction()
// }

async fn premint_to_call(premint: ZoraPremintV2) -> IZoraPremintV2::premintV2Call {
    IZoraPremintV2::premintV2Call {
        contractConfig: IZoraPremintV2::ContractCreationConfig {
            contractAdmin: Default::default(),
            contractURI: "".to_string(),
            contractName: "".to_string(),
        },
        premintConfig: IZoraPremintV2::PremintConfigV2 {
            tokenConfig: premint.premint.tokenConfig,
            uid: 0,
            version: 0,
            deleted: false,
        },
        signature: Default::default(),
        quantityToMint: Default::default(),
        mintArguments: IZoraPremintV2::MintArguments {
            mintRecipient: Default::default(),
            mintComment: "".to_string(),
            mintRewardsRecipients: vec![],
        },
    }
}
