use std::any::{Any, TypeId};
use std::future::Future;
use std::ops::Deref;
use std::pin::Pin;
use std::str::FromStr;
use futures::future::join_all;

use alloy_signer::Signer;
use alloy_signer_wallet::LocalWallet;
use crate::premints::{Premint, PremintTypes, SimplePremint};
use crate::premints::PremintTypes::Simple;
use crate::premints::zora_premint_v2::types::ZoraPremintConfigV2;

const premintJson: &str = r#"
{
  "tokenConfig": {
    "tokenURI": "ipfs://tokenIpfsId0",
    "maxSupply": 100000000000000000,
    "maxTokensPerAddress": 10,
    "pricePerToken": 0,
    "mintStart": 0,
    "mintDuration": 100,
    "royaltyBPS": 9343,
    "payoutRecipient": "0x70997970C51812dc3A010C7d01b50e0d17dc79C8",
    "fixedPriceMinter": "0x62437dFD73292ee3aDd7630cd782C895BA17EDb7",
    "createReferral": "0xd7C218E1d720C9f1533D1B7E952D6510b6e8FffC"
  },
  "uid": 105,
  "version": 0,
  "deleted": false
}
"#;

struct RuleContext {}

// define a rule as an async function signature
type RuleCheck = dyn Fn(PremintTypes) -> Box<dyn Future<Output=bool>>;
type SpecificRuleCheck<T> = dyn Fn(T) -> Box<dyn Future<Output=bool>>;

fn is_authorized_to_create_premint(premint: &PremintTypes) -> Pin<Box<dyn Future<Output=bool>>> {
    Box::pin(async move {
        true
    })
}

pub async fn test() {
    let premint = PremintTypes::ZoraV2(serde_json::from_str(premintJson).unwrap());

    println!("types: {:?}, premint: {:?}, simple: {:?}", TypeId::of::<PremintTypes>(), premint.type_id(), TypeId::of::<SimplePremint>());

    let checks = vec![is_authorized_to_create_premint];

    let results: Vec<_> = checks.
        iter().
        map(|check| {
            check(&premint)
        }).collect();

    let all_checks = join_all(results).await;

    println!("{:?}", premint);
}


#[cfg(test)]
mod test {
    use super::*;

    #[tokio::test]
    async fn test_signature() {
        test().await;
    }
}
