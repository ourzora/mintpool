use std::str::FromStr;

use alloy_primitives::Signature;
use alloy_sol_macro::sol;
use alloy_sol_types::SolStruct;

use crate::chain::contract_call;
use crate::chain_list::CHAINS;
use crate::premints::zora_premint_v2::types::ZoraPremintV2;
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
    _context: RuleContext,
) -> eyre::Result<Evaluation> {
    let call = PremintExecutor::isAuthorizedToCreatePremintCall {
        contractAddress: premint.collection_address,
        signer: premint.collection.contractAdmin,
        premintContractConfigContractAdmin: premint.collection.contractAdmin,
    };

    let provider = CHAINS.get_rpc(premint.chain_id.to()).await?;
    let result = contract_call(call, provider).await?;

    match result.isAuthorized {
        true => Ok(Accept),
        false => Ok(Reject("Unauthorized to create premint".to_string())),
    }
}

// * signatureIsValid ( this can be performed entirely offline )
//   * check if the signature is valid
//   * check if the signature is equal to the proposed contract admin

pub async fn is_valid_signature(
    premint: ZoraPremintV2,
    _context: RuleContext,
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

async fn is_chain_supported(
    premint: ZoraPremintV2,
    _context: RuleContext,
) -> eyre::Result<Evaluation> {
    let supported_chains: Vec<u64> = vec![7777777, 999999999, 8453];
    let chain_id = premint.chain_id.to();

    match supported_chains.contains(&chain_id) {
        true => Ok(Accept),
        false => Ok(Reject("Chain not supported".to_string())),
    }
}

pub fn all_rules() -> Vec<Box<dyn Rule>> {
    vec![
        typed_rule!(PremintTypes::ZoraV2, is_authorized_to_create_premint),
        typed_rule!(PremintTypes::ZoraV2, is_valid_signature),
        typed_rule!(PremintTypes::ZoraV2, is_chain_supported),
    ]
}

#[cfg(test)]
mod test {

    use super::*;

    const PREMINT_JSON: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/data/valid_zora_v2_premint.json"
    ));

    #[tokio::test]
    async fn test_is_valid_signature() {
        let premint: ZoraPremintV2 = serde_json::from_str(PREMINT_JSON).unwrap();
        let result = is_valid_signature(premint, RuleContext {}).await;

        match result {
            Ok(Accept) => {}
            Ok(Ignore) => panic!("Should not be ignored"),
            Ok(Reject(reason)) => panic!("Rejected: {}", reason),
            Err(err) => panic!("Error: {:?}", err),
        }
    }

    #[tokio::test]
    async fn test_is_authorized_to_create_premint() {
        let premint: ZoraPremintV2 = serde_json::from_str(PREMINT_JSON).unwrap();
        let result = is_authorized_to_create_premint(premint, RuleContext {}).await;

        match result {
            Ok(Accept) => {}
            Ok(Ignore) => panic!("Should not be ignored"),
            Ok(Reject(reason)) => panic!("Rejected: {}", reason),
            Err(err) => panic!("Error: {:?}", err),
        }
    }
}
