use crate::premints::zora_premint_v2::types::{IZoraPremintV2, ZoraPremintV2};
use alloy_primitives::{Bytes, U256};

use crate::premints::zora_premint_v2::types::IZoraPremintV2::MintArguments;

pub fn premint_to_call(
    premint: ZoraPremintV2,
    quantity: U256,
    mint_args: MintArguments,
) -> IZoraPremintV2::premintV2Call {
    let sig = Bytes::from(premint.signature);
    IZoraPremintV2::premintV2Call {
        contractConfig: premint.collection,
        premintConfig: premint.premint,
        signature: sig,
        quantityToMint: quantity,
        mintArguments: mint_args,
    }
}
