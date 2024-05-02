use alloy::primitives::{address, Address};
use alloy::sol;
use serde::{Deserialize, Serialize};

pub static PREMINT_FACTORY_ADDR: Address = address!("7777773606e7e46C8Ba8B98C08f5cD218e31d340");

// we need to use separate namespaces for each premint version,
// because they all need to have the type names for the signatures
// to calculate correctly
sol! {
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    IZoraPremintERC20V1,
    "src/premints/zora_premint/zora1155PremintExecutor_erc20_1.json"
}

sol! {
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    IZoraPremintV2,
    "src/premints/zora_premint/zora1155PremintExecutor_v2.json"
}
