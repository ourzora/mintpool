use crate::premints::zora_premint::contract::IZoraPremintERC20V1;

// aliasing the types here for readability. the original name need to stay
// because they impact signature generation
pub type PremintConfigERC20V1 = IZoraPremintERC20V1::CreatorAttribution;
pub type TokenCreationConfigERC20V1 = IZoraPremintERC20V1::TokenCreationConfig;
pub type ContractCreationConfigERC20V1 = IZoraPremintERC20V1::ContractCreationConfig;
