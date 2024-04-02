use alloy_signer::Signer;
use alloy_signer_wallet::LocalWallet;
use alloy_sol_macro::sol;
use serde::{Deserialize, Serialize};

use crate::premints::PremintTypes;

sol! {
    #[derive(Debug, Serialize, Deserialize, PartialEq, Default)]
    struct TokenCreationConfig {
        // Metadata URI for the created token
        string tokenURI;
        // Max supply of the created token
        uint256 maxSupply;
        // Max tokens that can be minted for an address, 0 if unlimited
        uint64 maxTokensPerAddress;
        // Price per token in eth wei. 0 for a free mint.
        uint96 pricePerToken;
        // The start time of the mint, 0 for immediate.  Prevents signatures from being used until the start time.
        uint64 mintStart;
        // The duration of the mint, starting from the first mint of this token. 0 for infinite
        uint64 mintDuration;
        // RoyaltyBPS for created tokens. The royalty amount in basis points for secondary sales.
        uint32 royaltyBPS;
        // This is the address that will be set on the `royaltyRecipient` for the created token on the 1155 contract,
        // which is the address that receives creator rewards and secondary royalties for the token,
        // and on the `fundsRecipient` on the ZoraCreatorFixedPriceSaleStrategy contract for the token,
        // which is the address that receives paid mint funds for the token.
        address payoutRecipient;
        // Fixed price minter address
        address fixedPriceMinter;
        // create referral
        address createReferral;
    }

    #[derive(Debug, Serialize, Deserialize, PartialEq, Default)]
    struct CreatorAttribution {
        // The config for the token to be created
        TokenCreationConfig tokenConfig;
        // Unique id of the token, used to ensure that multiple signatures can't be used to create the same intended token.
        // only one signature per token id, scoped to the contract hash can be executed.
        uint32 uid;
        // Version of this premint, scoped to the uid and contract.  Not used for logic in the contract, but used externally to track the newest version
        uint32 version;
        // If executing this signature results in preventing any signature with this uid from being minted.
        bool deleted;
    }
}

// renaming solidity type not implemented yet in alloy-sol-types,
// so we alias the generated type.
pub type ZoraPremintConfigV2 = CreatorAttribution;

#[cfg(test)]
mod test {
    use std::str::FromStr;

    use alloy_sol_types::{eip712_domain, SolStruct};
    use const_hex::ToHexExt;
    use ruint::Uint;

    use super::*;

    fn get_config() -> ZoraPremintConfigV2 {
        let config = TokenCreationConfig {
            tokenURI: "ipfs://tokenIpfsId0".to_string(),
            maxSupply: Uint::<256, 4>::from(100000000000000000u128),
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
                maxSupply: Uint::<256, 4>::from(100000000000000000u128),
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

        let typ = PremintTypes::ZoraV2(premint);

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

        println!("struct hash: {}", premint.eip712_hash_struct());
        println!("type hash: {}", premint.eip712_type_hash());

        let signature = wallet.sign_typed_data(&premint, &domain).await.expect("TODO: panic message");

        println!("0x{}", signature.as_bytes().encode_hex());
    }
}
