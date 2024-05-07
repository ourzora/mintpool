use crate::{implement_zora_premint_traits, typed_rule};
use alloy::primitives::Address;

use crate::premints::zora_premint::contract::IZoraPremintERC20V1;

use crate::rules::Rule;
use crate::storage::Reader;
use crate::types::PremintTypes;

impl Default for IZoraPremintERC20V1::ContractCreationConfig {
    fn default() -> Self {
        Self {
            contractAdmin: Default::default(),
            contractURI: Default::default(),
            contractName: Default::default(),
        }
    }
}

impl Default for IZoraPremintERC20V1::TokenCreationConfig {
    fn default() -> Self {
        Self {
            tokenURI: Default::default(),
            maxSupply: Default::default(),
            maxTokensPerAddress: Default::default(),
            pricePerToken: Default::default(),
            mintStart: Default::default(),
            mintDuration: Default::default(),
            royaltyBPS: Default::default(),
            fixedPriceMinter: Default::default(),
            royaltyMintSchedule: Default::default(),
            royaltyRecipient: Default::default(),
        }
    }
}

impl Default for IZoraPremintERC20V1::CreatorAttribution {
    fn default() -> Self {
        Self {
            tokenConfig: Default::default(),
            uid: Default::default(),
            version: Default::default(),
            deleted: Default::default(),
        }
    }
}

implement_zora_premint_traits!(
    IZoraPremintERC20V1,
    ERC20V1,
    "zora_premint_erc20v1",
    "ERC20_1"
);

pub fn all_v2_rules<T: Reader>() -> Vec<Box<dyn Rule<T>>> {
    vec![
        typed_rule!(
            PremintTypes::ZoraERC20V1,
            ERC20V1::is_authorized_to_create_premint
        ),
        typed_rule!(PremintTypes::ZoraERC20V1, ERC20V1::is_valid_signature),
        typed_rule!(PremintTypes::ZoraERC20V1, ERC20V1::is_chain_supported),
        typed_rule!(PremintTypes::ZoraERC20V1, ERC20V1::not_minted),
        typed_rule!(
            PremintTypes::ZoraERC20V1,
            ERC20V1::premint_version_supported
        ),
    ]
}
