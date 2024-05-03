use crate::premints::zora_premint::v2::V2;
use crate::rules::Rule;
use crate::storage::Reader;
use crate::typed_rule;
use crate::types::PremintTypes;
#[macro_export]
macro_rules! zora_premint_rules {
    ($namespace:tt, $typ:ty, $version:literal) => {
        pub async fn is_authorized_to_create_premint<T: $crate::storage::Reader>(
            premint: &$typ,
            context: &$crate::rules::RuleContext<T>,
        ) -> eyre::Result<$crate::rules::Evaluation> {
            let rpc = match context.rpc {
                None => return $crate::ignore!("Rule requires RPC call"),
                Some(ref rpc) => rpc,
            };

            let call = $namespace::isAuthorizedToCreatePremintCall {
                contractAddress: premint.collection_address,
                signer: premint.collection.contractAdmin,
                premintContractConfigContractAdmin: premint.collection.contractAdmin,
            };

            let result = $crate::chain::view_contract_call(
                call,
                rpc,
                $crate::premints::zora_premint::contract::PREMINT_FACTORY_ADDR,
            )
            .await?;

            match result.isAuthorized {
                true => Ok($crate::rules::Evaluation::Accept),
                false => $crate::reject!("Unauthorized to create premint"),
            }
        }

        pub async fn not_minted<T: $crate::storage::Reader>(
            premint: &$typ,
            context: &$crate::rules::RuleContext<T>,
        ) -> eyre::Result<$crate::rules::Evaluation> {
            let rpc = match context.rpc {
                None => return $crate::ignore!("Rule requires RPC provider"),
                Some(ref rpc) => rpc,
            };

            let call = $namespace::premintStatusCall {
                contractAddress: premint.collection_address,
                uid: premint.premint.uid,
            };

            let result = $crate::chain::view_contract_call(
                call,
                rpc,
                $crate::premints::zora_premint::contract::PREMINT_FACTORY_ADDR,
            )
            .await?;

            match result.contractCreated && !result.tokenIdForPremint.is_zero() {
                false => Ok($crate::rules::Evaluation::Accept),
                true => $crate::reject!("Premint already minted"),
            }
        }

        pub async fn premint_version_supported<T: $crate::storage::Reader>(
            premint: &$typ,
            context: &$crate::rules::RuleContext<T>,
        ) -> eyre::Result<$crate::rules::Evaluation> {
            let rpc = match context.rpc {
                None => return $crate::ignore!("Rule requires RPC provider"),
                Some(ref rpc) => rpc,
            };

            let call = $namespace::supportedPremintSignatureVersionsCall {
                contractAddress: premint.collection_address,
            };

            let result = $crate::chain::view_contract_call(
                call,
                rpc,
                $crate::premints::zora_premint::contract::PREMINT_FACTORY_ADDR,
            )
            .await?;

            match result.versions.contains(&$version.to_string()) {
                true => Ok($crate::rules::Evaluation::Accept),
                false => $crate::reject!(concat!(
                    "Premint version ",
                    $version,
                    " not supported by contract"
                )),
            }
        }

        // * signatureIsValid ( this can be performed entirely offline )
        //   * check if the signature is valid
        //   * check if the signature is equal to the proposed contract admin

        pub async fn is_valid_signature<T: $crate::storage::Reader>(
            premint: &$typ,
            _context: &$crate::rules::RuleContext<T>,
        ) -> eyre::Result<$crate::rules::Evaluation> {
            //   * if contract exists, check if the signer is the contract admin
            //   * if contract does not exist, check if the signer is the proposed contract admin

            let signature: alloy::signers::Signature =
                core::str::FromStr::from_str(premint.signature.as_str())?;

            let domain = premint.eip712_domain();
            let hash = alloy::sol_types::SolStruct::eip712_signing_hash(&premint.premint, &domain);
            let signer: alloy::primitives::Address =
                signature.recover_address_from_prehash(&hash)?;

            if signer != premint.collection.contractAdmin {
                $crate::reject!(
                    "Invalid signature for contract admin {} vs recovered {}",
                    premint.collection.contractAdmin,
                    signer
                )
            } else {
                Ok($crate::rules::Evaluation::Accept)
            }
        }

        pub async fn is_chain_supported<T: $crate::storage::Reader>(
            premint: &$typ,
            _context: &$crate::rules::RuleContext<T>,
        ) -> eyre::Result<$crate::rules::Evaluation> {
            let supported_chains: Vec<u64> = vec![7777777, 999999999, 8453];
            let chain_id = premint.chain_id;

            match supported_chains.contains(&chain_id) {
                true => Ok($crate::rules::Evaluation::Accept),
                false => $crate::reject!("Chain not supported"),
            }
        }
    };
}
