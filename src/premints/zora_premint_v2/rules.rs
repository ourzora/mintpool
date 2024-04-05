use alloy::rpc::types::eth::{TransactionInput, TransactionRequest};
use std::str::FromStr;

use crate::chain_list::CHAINS;
use alloy_primitives::{Bytes, Signature};
use alloy_provider::Provider;
use alloy_sol_macro::sol;
use alloy_sol_types::{SolCall, SolStruct};

use crate::premints::zora_premint_v2::types::{ZoraPremintV2, PREMINT_FACTORY_ADDR};
use crate::rules::Evaluation::{Accept, Reject};
use crate::rules::{Evaluation, Rule, RuleContext};
use crate::typed_rule;
use crate::types::{Premint, PremintTypes};

sol! {
    contract PremintExecutor {
        function isAuthorizedToCreatePremint(
            address signer,
            address premintContractConfigContractAdmin,
            address contractAddress
        ) external view returns (bool isAuthorized);
    }
}

// create premint v2 rule implementations here

pub async fn is_authorized_to_create_premint(
    premint: ZoraPremintV2,
    context: RuleContext,
) -> eyre::Result<Evaluation> {
    let call = PremintExecutor::isAuthorizedToCreatePremintCall {
        contractAddress: premint.collection_address,
        signer: premint.collection.contractAdmin,
        premintContractConfigContractAdmin: premint.collection.contractAdmin,
    };

    let chain = CHAINS.get_chain_by_id(premint.chain_id.to());

    match chain {
        Some(chain) => {
            let provider = chain.get_rpc(false).await?;
            let response = provider
                .call(
                    &TransactionRequest {
                        to: Some(PREMINT_FACTORY_ADDR),
                        input: TransactionInput {
                            input: Some(Bytes::from(call.abi_encode())),
                            data: None,
                        },
                        ..Default::default()
                    },
                    None,
                )
                .await;

            match response {
                Ok(response) => {
                    let response =
                        PremintExecutor::isAuthorizedToCreatePremintReturn::from(response.output);
                    let result = call.abi_decode(&response.output);
                    if result.is_err() {
                        return Ok(Reject("Unauthorized to create premint".to_string()));
                    }
                    if result.unwrap() {
                        return Ok(Accept);
                    }
                }
                Err(_) => {
                    return Ok(Reject("Unauthorized to create premint".to_string()));
                }
            }
        }
        None => {
            return Ok(Reject("Chain not supported".to_string()));
        }
    }

    //   * if contract exists, check if the signer is the contract admin
    //   * if contract does not exist, check if the signer is the proposed contract admin
    //   * this logic exists as a function on the premint executor contract
}

// * signatureIsValid ( this can be performed entirely offline )
//   * check if the signature is valid
//   * check if the signature is equal to the proposed contract admin

pub async fn is_valid_signature(
    premint: ZoraPremintV2,
    context: RuleContext,
) -> eyre::Result<Evaluation> {
    //   * if contract exists, check if the signer is the contract admin
    //   * if contract does not exist, check if the signer is the proposed contract admin

    let signature = Signature::from_str(premint.signature.as_str())?;

    let domain = premint.eip712_domain();
    let hash = premint.premint.eip712_signing_hash(&domain);
    let signer = signature.recover_address_from_prehash(&hash)?;

    if signer != premint.collection.contractAdmin {
        return Ok(Reject(format!(
            "Invalid signature for contract admin {}",
            premint.collection.contractAdmin
        )));
    }

    Ok(Accept)
}

pub fn all_rules() -> Vec<Box<dyn Rule>> {
    vec![
        Box::new(typed_rule!(
            PremintTypes::ZoraV2,
            is_authorized_to_create_premint
        )),
        Box::new(typed_rule!(PremintTypes::ZoraV2, is_valid_signature)),
    ]
}

#[cfg(test)]
mod test {
    use std::str::FromStr;

    use super::*;

    const PREMINT_JSON: &str = r#"
{
  "collection": {
    "contractAdmin": "0xa771209423284bace9a24a06d166a11196724b53",
    "contractURI": "ipfs://bafkreic4fnavhtymee7makmk7wp257nloh5y5ysc2fcwa5rpg6v6f3jhly",
    "contractName": "Karate sketch"
  },
  "premint": {
    "tokenConfig": {
      "tokenURI": "ipfs://bafkreier5h4a6btu24fsitbjdvpyak7moi6wkp33wlqmx2kfwgpq2lvx4y",
      "maxSupply": 18446744073709551615,
      "maxTokensPerAddress": 0,
      "pricePerToken": 0,
      "mintStart": 1702541688,
      "mintDuration": 2592000,
      "royaltyBPS": 500,
      "fixedPriceMinter": "0x04e2516a2c207e84a1839755675dfd8ef6302f0a",
      "payoutRecipient": "0xa771209423284bace9a24a06d166a11196724b53",
      "createReferral": "0x0000000000000000000000000000000000000000"
    },
    "uid": 2,
    "version": 1,
    "deleted": false
  },
  "collectionAddress": "0x42e108d1ed954b0adbd53ea118ba7614622d10d0",
  "chainId": 7777777,
  "signature": "0x894405d100900e6823385ca881c91d5ca7137a326f0c7d27edfd2907d9669cea55626bbd807a36cea815eceeac6634f45cfec54d7157c35f496b999e7b9451de1c"
}"#;

    #[tokio::test]
    async fn test_is_valid_signature() {
        let premint: ZoraPremintV2 = serde_json::from_str(PREMINT_JSON).unwrap();
        assert!(matches!(
            is_valid_signature(premint, RuleContext {}).await,
            Ok(Accept)
        ));
    }
}
