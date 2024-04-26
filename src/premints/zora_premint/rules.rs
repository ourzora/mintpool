use std::str::FromStr;

use alloy_primitives::Signature;
use alloy_sol_types::SolStruct;

use crate::chain::view_contract_call;
use crate::premints::zora_premint::contract::{IZoraPremintV2, PREMINT_FACTORY_ADDR};
use crate::premints::zora_premint::v2::ZoraPremintV2;
use crate::rules::Evaluation::Accept;
use crate::rules::{Evaluation, Rule, RuleContext};
use crate::storage::Reader;
use crate::types::PremintTypes;
use crate::{ignore, reject, typed_rule};

// create premint v2 rule implementations here

pub async fn is_authorized_to_create_premint<T: Reader, P>(
    premint: &P,
    context: &RuleContext<T>,
) -> eyre::Result<Evaluation> {
    let rpc = match context.rpc {
        None => return ignore!("Rule requires RPC call"),
        Some(ref rpc) => rpc,
    };

    let call = IZoraPremintV2::isAuthorizedToCreatePremintCall {
        contractAddress: premint.collection_address,
        signer: premint.collection.contractAdmin,
        premintContractConfigContractAdmin: premint.collection.contractAdmin,
    };

    let result = view_contract_call(call, rpc, PREMINT_FACTORY_ADDR).await?;

    match result.isAuthorized {
        true => Ok(Accept),
        false => reject!("Unauthorized to create premint"),
    }
}

pub async fn not_minted<T: Reader>(
    premint: &ZoraPremintV2,
    context: &RuleContext<T>,
) -> eyre::Result<Evaluation> {
    let rpc = match context.rpc {
        None => return ignore!("Rule requires RPC provider"),
        Some(ref rpc) => rpc,
    };

    let call = IZoraPremintV2::premintStatusCall {
        contractAddress: premint.collection_address,
        uid: premint.premint.uid,
    };

    let result = view_contract_call(call, rpc, PREMINT_FACTORY_ADDR).await?;

    match result.contractCreated && !result.tokenIdForPremint.is_zero() {
        false => Ok(Accept),
        true => reject!("Premint already minted"),
    }
}

pub async fn premint_version_supported<T: Reader>(
    premint: &ZoraPremintV2,
    context: &RuleContext<T>,
) -> eyre::Result<Evaluation> {
    let rpc = match context.rpc {
        None => return ignore!("Rule requires RPC provider"),
        Some(ref rpc) => rpc,
    };

    let call = IZoraPremintV2::supportedPremintSignatureVersionsCall {
        contractAddress: premint.collection_address,
    };

    let result = view_contract_call(call, rpc, PREMINT_FACTORY_ADDR).await?;

    match result.versions.contains(&"ERC20_1".to_string()) {
        true => Ok(Accept),
        false => reject!("Premint version 2 not supported by contract"),
    }
}

// * signatureIsValid ( this can be performed entirely offline )
//   * check if the signature is valid
//   * check if the signature is equal to the proposed contract admin

pub async fn is_valid_signature<T: Reader>(
    premint: &ZoraPremintV2,
    _context: &RuleContext<T>,
) -> eyre::Result<Evaluation> {
    //   * if contract exists, check if the signer is the contract admin
    //   * if contract does not exist, check if the signer is the proposed contract admin
    let signature = Signature::from_str(premint.signature.as_str())?;

    let domain = premint.eip712_domain();
    let hash = premint.premint.eip712_signing_hash(&domain);
    let signer = signature.recover_address_from_prehash(&hash)?;

    if signer != premint.collection.contractAdmin {
        reject!(
            "Invalid signature for contract admin {} vs recovered {}",
            premint.collection.contractAdmin,
            signer
        )
    } else {
        Ok(Accept)
    }
}

async fn is_chain_supported<T: Reader>(
    premint: &ZoraPremintV2,
    _context: &RuleContext<T>,
) -> eyre::Result<Evaluation> {
    let supported_chains: Vec<u64> = vec![7777777, 999999999, 8453];
    let chain_id = premint.chain_id;

    match supported_chains.contains(&chain_id) {
        true => Ok(Accept),
        false => reject!("Chain not supported"),
    }
}

pub fn all_rules<T: Reader>() -> Vec<Box<dyn Rule<T>>> {
    vec![
        typed_rule!(PremintTypes::ZoraV2, is_authorized_to_create_premint),
        typed_rule!(PremintTypes::ZoraV2, is_valid_signature),
        typed_rule!(PremintTypes::ZoraV2, is_chain_supported),
        typed_rule!(PremintTypes::ZoraV2, not_minted),
        typed_rule!(PremintTypes::ZoraV2, premint_version_supported),
    ]
}

#[cfg(test)]
mod test {
    use crate::rules::Evaluation::{Ignore, Reject};

    use super::*;

    const PREMINT_JSON: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/data/valid_zora_v2_premint.json"
    ));

    #[tokio::test]
    async fn test_is_valid_signature() {
        let premint: ZoraPremintV2 = serde_json::from_str(PREMINT_JSON).unwrap();
        let context = RuleContext::test_default().await;
        let result = is_valid_signature(&premint, &context).await;

        match result {
            Ok(Accept) => {}
            Ok(Ignore(reason)) => panic!("Should not be ignored: {}", reason),
            Ok(Reject(reason)) => panic!("Rejected: {}", reason),
            Err(err) => panic!("Error: {:?}", err),
        }
    }

    #[tokio::test]
    async fn test_is_authorized_to_create_premint() {
        let premint: ZoraPremintV2 = serde_json::from_str(PREMINT_JSON).unwrap();
        let context = RuleContext::test_default_rpc(7777777).await;
        let result = is_authorized_to_create_premint(&premint, &context).await;

        match result {
            Ok(Accept) => {}
            Ok(Ignore(reason)) => panic!("Should not be ignored: {}", reason),
            Ok(Reject(reason)) => panic!("Rejected: {}", reason),
            Err(err) => panic!("Error: {:?}", err),
        }
    }
}
