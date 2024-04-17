use std::sync::Arc;

use alloy::network::Ethereum;
use async_trait::async_trait;
use futures::future::join_all;

use crate::chain_list::{ChainListProvider, CHAINS};
use crate::config::Config;
use crate::storage::{PremintStorage, Reader};
use crate::types::PremintTypes;

#[derive(Debug, PartialEq, Eq)]
pub enum Evaluation {
    Accept,
    Ignore(String),
    Reject(String),
}

#[macro_export]
macro_rules! reject {
    ($($arg:tt)*) => {{
        Ok($crate::rules::Evaluation::Reject(format!($($arg)*).to_string()))
    }};
}

#[macro_export]
macro_rules! ignore {
    ($($arg:tt)*) => {{
        Ok($crate::rules::Evaluation::Ignore(format!($($arg)*).to_string()))
    }};
}

#[derive(Debug)]
pub struct RuleResult {
    pub rule_name: &'static str,
    pub result: eyre::Result<Evaluation>,
}

#[derive(Debug)]
pub struct Results(Vec<RuleResult>);

impl Results {
    pub fn is_accept(&self) -> bool {
        !self.is_reject()
    }

    pub fn is_reject(&self) -> bool {
        !self.is_err()
            && self
                .0
                .iter()
                .any(|r| matches!(r.result, Ok(Evaluation::Reject(_))))
    }

    pub fn is_err(&self) -> bool {
        self.0.iter().any(|r| r.result.is_err())
    }

    pub fn summary(&self) -> String {
        self.0
            .iter()
            .map(|r| match r.result {
                Ok(Evaluation::Accept) => format!("{}: Accept", r.rule_name),
                Ok(Evaluation::Ignore(ref reason)) => {
                    format!("{}: Ignore ({})", r.rule_name, reason)
                }
                Ok(Evaluation::Reject(ref reason)) => {
                    format!("{}: Reject ({})", r.rule_name, reason)
                }
                Err(ref e) => format!("{}: Error ({})", r.rule_name, e),
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

pub struct RuleContext<T: Reader> {
    pub storage: T,
    pub existing: Option<PremintTypes>,
    pub rpc: Option<Arc<ChainListProvider<Ethereum>>>,
}

impl<T: Reader> RuleContext<T> {
    pub fn new(
        storage: T,
        existing: Option<PremintTypes>,
        rpc: Option<Arc<ChainListProvider<Ethereum>>>,
    ) -> Self {
        Self {
            storage,
            existing,
            rpc,
        }
    }
}

#[cfg(test)]
impl RuleContext<PremintStorage> {
    pub async fn test_default() -> Self {
        let config = Config::test_default();

        Self {
            storage: PremintStorage::new(&config).await,
            existing: None,
            rpc: None,
        }
    }

    pub async fn test_default_rpc(chain_id: u64) -> Self {
        RuleContext {
            rpc: Some(CHAINS.get_rpc(chain_id).await.unwrap()),
            ..Self::test_default().await
        }
    }
}

#[async_trait]
pub trait Rule<T: Reader>: Send + Sync {
    async fn check(
        &self,
        item: &PremintTypes,
        context: &RuleContext<T>,
    ) -> eyre::Result<Evaluation>;
    fn rule_name(&self) -> &'static str;
}

#[macro_export]
macro_rules! rule {
    ($fn:tt) => {{
        struct SimpleRule;

        #[async_trait::async_trait]
        impl<T: Reader> $crate::rules::Rule<T> for SimpleRule {
            async fn check(
                &self,
                item: &$crate::types::PremintTypes,
                context: &$crate::rules::RuleContext<T>,
            ) -> eyre::Result<crate::rules::Evaluation> {
                $fn(item, context).await
            }

            fn rule_name(&self) -> &'static str {
                concat!(stringify!($fn))
            }
        }

        std::boxed::Box::new(SimpleRule {})
    }};
}

#[macro_export]
macro_rules! metadata_rule {
    ($fn:tt) => {{
        struct MetadataRule;

        #[async_trait::async_trait]
        impl<T: Reader> $crate::rules::Rule<T> for MetadataRule {
            async fn check(
                &self,
                item: &$crate::types::PremintTypes,
                context: &$crate::rules::RuleContext<T>,
            ) -> eyre::Result<crate::rules::Evaluation> {
                $fn(&item.metadata(), context).await
            }

            fn rule_name(&self) -> &'static str {
                concat!("Metadata::", stringify!($fn))
            }
        }

        std::boxed::Box::new(MetadataRule {})
    }};
}

#[macro_export]
macro_rules! typed_rule {
    ($t:path, $fn:tt) => {{
        struct TypedRule;

        #[async_trait::async_trait]
        impl<T: Reader> $crate::rules::Rule<T> for TypedRule {
            async fn check(
                &self,
                item: &$crate::types::PremintTypes,
                context: &$crate::rules::RuleContext<T>,
            ) -> eyre::Result<$crate::rules::Evaluation> {
                match item {
                    $t(premint) => $fn(&premint, context).await,
                    _ => $crate::ignore!("Wrong type"),
                }
            }

            fn rule_name(&self) -> &'static str {
                concat!(stringify!($t), "::", stringify!($fn))
            }
        }

        std::boxed::Box::new(TypedRule {})
    }};
}

pub struct RulesEngine<T: Reader> {
    rules: Vec<Box<dyn Rule<T>>>,
    use_rpc: bool,
}

pub fn all_rules<T: Reader>() -> Vec<Box<dyn Rule<T>>> {
    let mut rules: Vec<Box<dyn Rule<T>>> = Vec::new();

    rules.append(&mut general::all_rules());
    rules.append(&mut crate::premints::zora_premint_v2::rules::all_rules());

    rules
}

impl<T: Reader> RulesEngine<T> {
    pub fn new(config: &Config) -> Self {
        RulesEngine {
            rules: vec![],
            use_rpc: config.enable_rpc,
        }
    }
    pub fn add_rule(&mut self, rule: Box<dyn Rule<T>>) {
        self.rules.push(rule);
    }
    pub fn add_default_rules(&mut self) {
        self.rules.extend(all_rules());
    }
    pub async fn evaluate(&self, item: &PremintTypes, store: T) -> eyre::Result<Results> {
        let metadata = item.metadata();
        let existing = match store.get_for_id_and_kind(&metadata.id, metadata.kind).await {
            Ok(existing) => Some(existing),
            Err(report) => match report.downcast_ref::<sqlx::Error>() {
                Some(sqlx::Error::RowNotFound) => None,
                _ => return Err(report),
            },
        };

        let context = RuleContext::new(
            store,
            existing,
            // TODO: check global/per-rule configuration to determine whether to add rpc access
            match self.use_rpc && metadata.chain_id != 0 {
                true => match CHAINS.get_rpc(metadata.chain_id).await {
                    Ok(rpc) => Some(rpc),
                    Err(_) => {
                        tracing::warn!(
                            "No RPC provider for chain {} during rule evaluation",
                            metadata.chain_id
                        );
                        None
                    }
                },
                false => None,
            },
        );

        let results: Vec<_> = self
            .rules
            .iter()
            .map(|rule| rule.check(&item, &context))
            .collect();
        let all_checks = join_all(results).await;

        Ok(Results(
            all_checks
                .into_iter()
                .zip(self.rules.iter())
                .map(|(result, rule)| RuleResult {
                    rule_name: rule.rule_name(),
                    result,
                })
                .collect(),
        ))
    }
}

mod general {
    use crate::rules::Evaluation::{Accept, Ignore, Reject};
    use crate::rules::{Evaluation, Rule, RuleContext};
    use crate::storage::Reader;
    use crate::types::PremintMetadata;

    pub fn all_rules<T: Reader>() -> Vec<Box<dyn Rule<T>>> {
        vec![
            metadata_rule!(token_uri_length),
            metadata_rule!(existing_token_uri),
            metadata_rule!(signer_matches),
            metadata_rule!(version_is_higher),
        ]
    }

    pub async fn token_uri_length<T: Reader>(
        meta: &PremintMetadata,
        _context: &RuleContext<T>,
    ) -> eyre::Result<Evaluation> {
        let max_allowed = if meta.uri.starts_with("data:") {
            // allow some more data for data uris
            8 * 1024
        } else {
            2 * 1024
        };

        match meta.uri.len() {
            0 => reject!("Token URI is empty"),
            _ if meta.uri.len() > max_allowed => reject!(
                "Token URI is too long: {} > {}",
                meta.uri.len(),
                max_allowed
            ),
            _ => Ok(Accept),
        }
    }

    pub async fn existing_token_uri<T: Reader>(
        meta: &PremintMetadata,
        context: &RuleContext<T>,
    ) -> eyre::Result<Evaluation> {
        let existing = context.storage.get_for_token_uri(&meta.uri).await;

        match existing {
            Err(report) => match report.downcast_ref::<sqlx::Error>() {
                // if the token uri doesn't exist, that's good!
                Some(sqlx::Error::RowNotFound) => Ok(Accept),

                // all other errors should be reported
                _ => Err(report),
            },
            Ok(existing) => {
                let metadata = existing.metadata();

                if metadata.id == meta.id {
                    // it's okay if the token uri exists for another version of the same token.
                    // other rules should ensure that we're only overwriting it if signer matches
                    Ok(Accept)
                } else {
                    reject!("Token URI already exists")
                }
            }
        }
    }

    pub async fn signer_matches<T: Reader>(
        meta: &PremintMetadata,
        context: &RuleContext<T>,
    ) -> eyre::Result<Evaluation> {
        match &context.existing {
            None => ignore!("No existing premint"),
            Some(existing) => {
                if existing.metadata().signer == meta.signer {
                    Ok(Accept)
                } else {
                    reject!("Signer does not match")
                }
            }
        }
    }

    pub async fn version_is_higher<T: Reader>(
        meta: &PremintMetadata,
        context: &RuleContext<T>,
    ) -> eyre::Result<Evaluation> {
        match &context.existing {
            None => ignore!("No existing premint"),
            Some(existing) => {
                if meta.version > existing.metadata().version {
                    Ok(Accept)
                } else {
                    reject!(
                        "Existing premint with higher version {} exists",
                        existing.metadata().version
                    )
                }
            }
        }
    }
}

#[cfg(test)]
mod test {
    use crate::premints::zora_premint_v2::types::ZoraPremintV2;
    use crate::rules::general::existing_token_uri;
    use crate::rules::Evaluation::{Accept, Reject};
    use crate::storage::Writer;
    use crate::types::{Premint, SimplePremint};

    use super::*;

    async fn test_rules_engine() -> (RulesEngine<PremintStorage>, PremintStorage) {
        let config = Config::test_default();
        let storage = PremintStorage::new(&config).await;
        let re = RulesEngine::new(&config);

        (re, storage)
    }

    async fn simple_rule<T: Reader>(
        item: &PremintTypes,
        context: &RuleContext<T>,
    ) -> eyre::Result<Evaluation> {
        Ok(Accept)
    }

    async fn conditional_rule<T: Reader>(
        item: &PremintTypes,
        _context: &RuleContext<T>,
    ) -> eyre::Result<Evaluation> {
        match item {
            PremintTypes::Simple(s) => {
                if s.metadata().chain_id == 0 {
                    Ok(Accept)
                } else {
                    reject!("Chain ID is not default")
                }
            }
            _ => Ok(Accept),
        }
    }

    async fn simple_typed_rule<T: Reader>(
        _item: &SimplePremint,
        _context: &RuleContext<T>,
    ) -> eyre::Result<Evaluation> {
        Ok(Accept)
    }

    async fn simple_typed_zora_rule<T: Reader>(
        _item: &ZoraPremintV2,
        _context: &RuleContext<T>,
    ) -> eyre::Result<Evaluation> {
        Ok(Accept)
    }

    #[tokio::test]
    async fn test_simple_rule() {
        let context = RuleContext::test_default().await;
        let rule = rule!(simple_rule);
        let result = rule
            .check(&PremintTypes::Simple(Default::default()), &context)
            .await
            .unwrap();
        assert!(matches!(result, Accept));
    }

    #[tokio::test]
    async fn test_simple_rules_engine() {
        let (mut engine, storage) = test_rules_engine().await;
        engine.add_rule(rule!(simple_rule));
        engine.add_rule(rule!(conditional_rule));

        let result = engine
            .evaluate(&PremintTypes::Simple(Default::default()), storage.clone())
            .await
            .expect("Evaluation should not fail");

        assert!(result.is_accept());
    }

    #[tokio::test]
    async fn test_typed_rules_engine() {
        let (mut engine, storage) = test_rules_engine().await;
        let context = RuleContext::test_default().await;

        let rule: Box<dyn Rule<PremintStorage>> =
            typed_rule!(PremintTypes::Simple, simple_typed_rule);
        let rule2: Box<dyn Rule<PremintStorage>> =
            typed_rule!(PremintTypes::ZoraV2, simple_typed_zora_rule);

        assert_eq!(rule.rule_name(), "PremintTypes::Simple::simple_typed_rule");
        assert_eq!(
            rule2.rule_name(),
            "PremintTypes::ZoraV2::simple_typed_zora_rule"
        );

        engine.add_rule(rule);
        engine.add_rule(rule2);

        let result = engine
            .evaluate(&PremintTypes::Simple(Default::default()), storage)
            .await
            .expect("Evaluation should not fail");

        assert!(result.is_accept());
    }

    #[tokio::test]
    async fn test_token_uri_exists_rule() {
        let storage = PremintStorage::new(&Config::test_default()).await;
        let context = RuleContext {
            storage: storage.clone(),
            existing: None,
            rpc: None,
        };
        let premint = PremintTypes::Simple(SimplePremint::default());

        let evaluation = existing_token_uri(&premint.metadata(), &context)
            .await
            .expect("Rule execution should not fail");

        assert!(matches!(evaluation, Accept));

        // now we'll store the token in the database
        storage
            .store(premint.clone())
            .await
            .expect("Simple premint should be stored");

        let evaluation = existing_token_uri(&premint.metadata(), &context)
            .await
            .expect("Rule execution should not fail");

        // rule should still pass, because it's the same token id
        assert!(matches!(evaluation, Accept));

        // now we'll try a different token with the same token uri
        let premint2 = SimplePremint::new(
            1,
            Default::default(),
            Default::default(),
            10,
            premint.metadata().uri,
        );

        let evaluation = existing_token_uri(&premint2.metadata(), &context)
            .await
            .expect("Rule execution should not fail");

        assert!(matches!(evaluation, Reject(_)));
    }
}
