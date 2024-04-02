use std::future::Future;
use std::pin::Pin;
use std::str::FromStr;
use alloy_primitives::Signature;

use alloy_signer_wallet::LocalWallet;
use alloy_sol_types::{eip712_domain, SolStruct};

use crate::premints::zora_premint_v2::types::ZoraPremintV2;


// create premint v2 rule implementations here


// TODO: is there any rust sugar to make this more concise?
//       as it stands, it's not defined as an async function, so can't use async stuff
fn is_authorized_to_create_premint(premint: &ZoraPremintV2) -> Pin<Box<dyn Future<Output=bool>>> {
//   * if contract exists, check if the signer is the contract admin
//   * if contract does not exist, check if the signer is the proposed contract admin
//   * this logic exists as a function on the premint executor contract
    Box::pin(async move {
        true
    })
}

// * signatureIsValid ( this can be performed entirely offline )
//   * check if the signature is valid
//   * check if the signature is equal to the proposed contract admin

fn is_valid_signature(premint: &ZoraPremintV2) -> Pin<Box<dyn Future<Output=bool>>> {
//   * if contract exists, check if the signer is the contract admin
//   * if contract does not exist, check if the signer is the proposed contract admin
    Box::pin(async move {
        let signature = Signature::from_str(premint.signature.as_str()).unwrap();

        let hash = premint.premint.eip712_signing_hash(&eip712_domain! {
            name: "Preminter",
            version: "2",
            chain_id: premint.chain_id,
            verifying_contract: premint.collection_address,
        });

        signature.recover_address_from_prehash(&hash).unwrap() == premint.collection.contractAdmin
    })
}

#[cfg(test)]
mod test {
    use std::str::FromStr;
    use alloy_primitives::U256;
    use alloy_signer::Signer;
    use alloy_sol_types::{eip712_domain, SolStruct};
    use crate::premints::zora_premint_v2::types::{TokenCreationConfig, ZoraPremintConfigV2};
    use super::*;

    fn get_config() -> ZoraPremintConfigV2 {
        let config = TokenCreationConfig {
            tokenURI: "ipfs://tokenIpfsId0".to_string(),
            maxSupply: U256::from(100000000000000000u128),
            maxTokensPerAddress: 10,
            pricePerToken: 0,
            mintStart: 0,
            mintDuration: 100,
            royaltyBPS: 8758,
            payoutRecipient: "0x70997970C51812dc3A010C7d01b50e0d17dc79C8".parse().unwrap(),
            fixedPriceMinter: "0x7e5A9B6F4bB9efC27F83E18F29e4326480668f87".parse().unwrap(),
            createReferral: "0x63779E68424A0746cF04B2bc51f868185a7660dF".parse().unwrap(),
        };

        let premint = ZoraPremintConfigV2 {
            tokenConfig: TokenCreationConfig {
                tokenURI: "ipfs://tokenIpfsId0".to_string(),
                maxSupply: U256::from(100000000000000000u128),
                maxTokensPerAddress: 10,
                pricePerToken: 0,
                mintStart: 0,
                mintDuration: 100,
                royaltyBPS: 8758,
                payoutRecipient: "0x70997970C51812dc3A010C7d01b50e0d17dc79C8".parse().unwrap(),
                fixedPriceMinter: "0x7e5A9B6F4bB9efC27F83E18F29e4326480668f87".parse().unwrap(),
                createReferral: "0x63779E68424A0746cF04B2bc51f868185a7660dF".parse().unwrap(),
            },
            uid: 105,
            version: 0,
            deleted: false,
        };

        ZoraPremintConfigV2 {
            tokenConfig: config,
            uid: 105,
            version: 0,
            deleted: false,
        }
    }

    #[tokio::test]
    async fn test_signing() {
        let premint = get_config();

        let wallet = LocalWallet::from_str("0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d").unwrap();
        let domain = eip712_domain! {
            name: "Preminter",
            version: "2",
            chain_id: 999999999,
            verifying_contract: "0x53870714E9ecF43fF76358064DeF05e1b1FAE2e9".parse().unwrap(),
        };

        wallet.sign_typed_data(&premint, &domain).await.expect("TODO: panic message");

        // let signature = wallet.sign_typed_data(&premint, &domain).await.expect("TODO: panic message");
        // println!("0x{}", signature.as_bytes().encode_hex());
    }
}