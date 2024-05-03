use crate::{implement_zora_premint_traits, typed_rule};
use alloy::primitives::Address;

use crate::premints::zora_premint::contract::IZoraPremintV2;

use crate::rules::Rule;
use crate::storage::Reader;
use crate::types::PremintTypes;

impl Default for IZoraPremintV2::ContractCreationConfig {
    fn default() -> Self {
        Self {
            contractAdmin: Default::default(),
            contractURI: Default::default(),
            contractName: Default::default(),
        }
    }
}

impl Default for IZoraPremintV2::TokenCreationConfig {
    fn default() -> Self {
        Self {
            tokenURI: Default::default(),
            maxSupply: Default::default(),
            maxTokensPerAddress: Default::default(),
            pricePerToken: Default::default(),
            mintStart: Default::default(),
            mintDuration: Default::default(),
            royaltyBPS: Default::default(),
            payoutRecipient: Default::default(),
            fixedPriceMinter: Default::default(),
            createReferral: Default::default(),
        }
    }
}

impl Default for IZoraPremintV2::CreatorAttribution {
    fn default() -> Self {
        Self {
            tokenConfig: Default::default(),
            uid: Default::default(),
            version: Default::default(),
            deleted: Default::default(),
        }
    }
}

implement_zora_premint_traits!(IZoraPremintV2, V2, "zora_premint_v2", "2");

pub fn all_v2_rules<T: Reader>() -> Vec<Box<dyn Rule<T>>> {
    vec![
        typed_rule!(PremintTypes::ZoraV2, V2::is_authorized_to_create_premint),
        typed_rule!(PremintTypes::ZoraV2, V2::is_valid_signature),
        typed_rule!(PremintTypes::ZoraV2, V2::is_chain_supported),
        typed_rule!(PremintTypes::ZoraV2, V2::not_minted),
        typed_rule!(PremintTypes::ZoraV2, V2::premint_version_supported),
    ]
}

#[cfg(test)]
mod test {
    use crate::rules::Evaluation::{Accept, Ignore, Reject};
    use crate::rules::RuleContext;

    use super::*;

    const PREMINT_JSON: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/data/valid_zora_v2_premint.json"
    ));

    #[tokio::test]
    async fn test_is_valid_signature() {
        let premint: V2 = serde_json::from_str(PREMINT_JSON).unwrap();
        let context = RuleContext::test_default().await;
        let result = V2::is_valid_signature(&premint, &context).await;

        match result {
            Ok(Accept) => {}
            Ok(Ignore(reason)) => panic!("Should not be ignored: {}", reason),
            Ok(Reject(reason)) => panic!("Rejected: {}", reason),
            Err(err) => panic!("Error: {:?}", err),
        }
    }

    #[tokio::test]
    async fn test_is_authorized_to_create_premint() {
        let premint: V2 = serde_json::from_str(PREMINT_JSON).unwrap();
        let context = RuleContext::test_default_rpc(7777777).await;
        let result = V2::is_authorized_to_create_premint(&premint, &context).await;

        match result {
            Ok(Accept) => {}
            Ok(Ignore(reason)) => panic!("Should not be ignored: {}", reason),
            Ok(Reject(reason)) => panic!("Rejected: {}", reason),
            Err(err) => panic!("Error: {:?}", err),
        }
    }
}
