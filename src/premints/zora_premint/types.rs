#[macro_export]
macro_rules! implement_zora_premint_traits {
    ($namespace:tt, $typ:ident, $kind:literal, $version:tt) => {
        #[derive(
            std::fmt::Debug,
            std::default::Default,
            serde::Serialize,
            serde::Deserialize,
            core::clone::Clone,
            core::cmp::PartialEq,
        )]
        #[serde(rename_all = "camelCase")]
        pub struct $typ {
            pub collection_address: alloy::primitives::Address,
            pub chain_id: u64,
            pub collection: $namespace::ContractCreationConfig,
            pub premint: $namespace::CreatorAttribution,
            pub signature: String,
        }


        impl core::default::Default for $namespace::ContractCreationConfig {
            fn default() -> Self {
                Self {
                    contractAdmin: Default::default(),
                    contractURI: String::default(),
                    contractName: String::default(),
                }
            }
        }

        impl core::default::Default for $namespace::TokenCreationConfig {
            fn default() -> Self {
                Self {
                    tokenURI: Default::default(),
                    maxSupply: Default::default(),
                    maxTokensPerAddress: Default::default(),
                    pricePerToken: Default::default(),
                    mintStart: Default::default(),
                    mintDuration: Default::default(),
                    royaltyBPS: Default::default(),
                    payoutRecipient: Default::default(),
                    fixedPriceMinter: Default::default(),
                    createReferral: Default::default(),
                }
            }
        }

        impl core::default::Default for $namespace::CreatorAttribution {
            fn default() -> Self {
                Self {
                    tokenConfig: Default::default(),
                    uid: Default::default(),
                    version: Default::default(),
                    deleted: Default::default(),
                }
            }
        }

        impl $typ {
            fn event_to_guid(chain_id: u64, event: &$namespace::PremintedV2) -> String {
                format!(
                    "{:?}:{:?}:{:?}",
                    chain_id, event.contractAddress, event.uid
                )
            }

            pub fn eip712_domain(&self) -> alloy::sol_types::Eip712Domain {
                alloy::sol_types::Eip712Domain {
                    name: Some(std::borrow::Cow::from("Preminter")),
                    version: Some(std::borrow::Cow::from($version)),
                    chain_id: Some(alloy::primitives::U256::from(self.chain_id)),
                    verifying_contract: Some(self.collection_address),
                    salt: None,
                }
            }

            $crate::zora_premint_rules!($namespace, $typ, $version);
        }

        #[async_trait::async_trait]
        impl $crate::types::Premint for $typ {

            fn metadata(&self) -> $crate::types::PremintMetadata {
                let id = format!(
                    "{:?}:{:?}:{:?}",
                    self.chain_id, self.collection_address, self.premint.uid
                );

                $crate::types::PremintMetadata {
                    id,
                    version: self.premint.version as u64,
                    kind: $crate::types::PremintName($kind.to_string()),
                    signer: self.collection.contractAdmin,
                    chain_id: self.chain_id,
                    collection_address: Address::default(), // TODO: source this
                    token_id: alloy::primitives::U256::from(self.premint.uid),
                    uri: self.premint.tokenConfig.tokenURI.clone(),
                }
            }

            fn check_filter(chain_id: u64) -> Option<alloy::rpc::types::eth::Filter> {
                let supported_chains = [7777777, 8453]; // TODO: add the rest here and enable testnet mode
                if !supported_chains.contains(&chain_id) {
                    return None;
                }
                Some(
                    alloy::rpc::types::eth::Filter::new()
                        .address($crate::premints::zora_premint::contract::PREMINT_FACTORY_ADDR)
                        .event(<$namespace::PremintedV2 as alloy::sol_types::SolEvent>::SIGNATURE),
                )
            }

            fn map_claim(
                chain_id: u64,
                log: alloy::rpc::types::eth::Log,
            ) -> eyre::Result<$crate::types::InclusionClaim> {
                let event = <$namespace::PremintedV2 as alloy::sol_types::SolEvent>::decode_raw_log(
                    log.topics(),
                    log.data().data.as_ref(),
                    true,
                )?
                .into();

                let id = Self::event_to_guid(chain_id, &event);

                Ok($crate::types::InclusionClaim {
                    premint_id: id,
                    chain_id,
                    tx_hash: log.transaction_hash.unwrap_or_default(),
                    log_index: log.log_index.unwrap_or_default(),
                    kind: $kind.to_string(),
                })
            }

            async fn verify_claim(
                &self,
                chain_id: u64,
                tx: alloy::rpc::types::eth::TransactionReceipt,
                log: alloy::rpc::types::eth::Log,
                claim: $crate::types::InclusionClaim,
            ) -> bool {
                let event =
                    <$namespace::PremintedV2 as alloy::sol_types::SolEvent>::decode_raw_log(log.topics(), &log.data().data, true);

                match event {
                    Ok(event) => {
                        let conditions = vec![
                            log.address() == $crate::premints::zora_premint::contract::PREMINT_FACTORY_ADDR,
                            log.transaction_hash.unwrap_or_default() == tx.transaction_hash,
                            claim.tx_hash == tx.transaction_hash,
                            claim.log_index == log.log_index.unwrap_or_default(),
                            claim.premint_id == Self::event_to_guid(chain_id, &event),
                            claim.kind == *"zora_premint_v2",
                            claim.chain_id == chain_id,
                            self.collection_address == event.contractAddress,
                            self.premint.uid == event.uid,
                        ];

                        // confirm all conditions are true
                        conditions.into_iter().all(|x| x)
                    }
                    Err(e) => {
                        tracing::debug!("Failed to parse log: {}", e);
                        false
                    }
                }
            }
        }
    };
}
