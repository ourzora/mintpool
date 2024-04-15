use async_trait::async_trait;
use futures::future::join_all;

use crate::types::{Premint, PremintTypes};

#[derive(Debug)]
pub enum Evaluation {
    Accept,
    Ignore,
    Reject(String),
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
                Ok(Evaluation::Ignore) => format!("{}: Ignore", r.rule_name),
                Ok(Evaluation::Reject(ref reason)) => {
                    format!("{}: Reject ({})", r.rule_name, reason)
                }
                Err(ref e) => format!("{}: Error ({})", r.rule_name, e),
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

#[derive(Clone)]
pub struct RuleContext {}

#[async_trait]
pub trait Rule: Send + Sync {
    async fn check(&self, item: PremintTypes, context: RuleContext) -> eyre::Result<Evaluation>;
    fn rule_name(&self) -> &'static str;
}

pub struct FnRule<T>(pub &'static str, pub T);

#[async_trait]
impl<T, Fut> Rule for FnRule<T>
where
    T: Fn(PremintTypes, RuleContext) -> Fut + Send + Sync + 'static,
    Fut: std::future::Future<Output = eyre::Result<Evaluation>> + Send,
{
    async fn check(&self, item: PremintTypes, context: RuleContext) -> eyre::Result<Evaluation> {
        self.1(item, context).await
    }

    fn rule_name(&self) -> &'static str {
        self.0
    }
}

#[macro_export]
macro_rules! rule {
    ($fn:tt) => {
        std::boxed::Box::new($crate::rules::FnRule(stringify!($fn), $fn))
    };
}

#[macro_export]
macro_rules! metadata_rule {
    ($fn:tt) => {{
        struct MetadataRule;

        #[async_trait::async_trait]
        impl $crate::rules::Rule for MetadataRule {
            async fn check(
                &self,
                item: $crate::types::PremintTypes,
                context: $crate::rules::RuleContext,
            ) -> eyre::Result<crate::rules::Evaluation> {
                $fn(item.metadata(), context).await
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
        impl $crate::rules::Rule for TypedRule {
            async fn check(
                &self,
                item: $crate::types::PremintTypes,
                context: $crate::rules::RuleContext,
            ) -> eyre::Result<$crate::rules::Evaluation> {
                match item {
                    $t(premint) => $fn(premint, context).await,
                    _ => Ok($crate::rules::Evaluation::Ignore),
                }
            }

            fn rule_name(&self) -> &'static str {
                concat!(stringify!($t), "::", stringify!($fn))
            }
        }

        std::boxed::Box::new(TypedRule {})
    }};
}

pub struct RulesEngine {
    rules: Vec<Box<dyn Rule>>,
}

pub fn all_rules() -> Vec<Box<dyn Rule>> {
    let mut rules: Vec<Box<dyn Rule>> = Vec::new();

    rules.append(&mut general::all_rules());
    rules.append(&mut crate::premints::zora_premint_v2::rules::all_rules());

    rules
}

impl RulesEngine {
    pub fn new() -> Self {
        RulesEngine { rules: vec![] }
    }
    pub fn add_rule(&mut self, rule: Box<dyn Rule>) {
        self.rules.push(rule);
    }
    pub fn add_default_rules(&mut self) {
        self.rules.extend(all_rules());
    }
    pub async fn evaluate(&self, item: PremintTypes, context: RuleContext) -> Results {
        let results: Vec<_> = self
            .rules
            .iter()
            .map(|rule| rule.check(item.clone(), context.clone()))
            .collect();
        let all_checks = join_all(results).await;

        Results(
            all_checks
                .into_iter()
                .zip(self.rules.iter())
                .map(|(result, rule)| RuleResult {
                    rule_name: rule.rule_name(),
                    result,
                })
                .collect(),
        )
    }
}

mod general {
    use crate::rules::Evaluation::{Accept, Reject};
    use crate::rules::{Evaluation, Rule, RuleContext};
    use crate::types::PremintMetadata;

    pub fn all_rules() -> Vec<Box<dyn Rule>> {
        vec![metadata_rule!(token_uri_length)]
    }

    pub async fn token_uri_length(
        meta: PremintMetadata,
        _context: RuleContext,
    ) -> eyre::Result<Evaluation> {
        let max_allowed = if meta.uri.starts_with("data:") {
            // allow some more data for data uris
            8 * 1024
        } else {
            2 * 1024
        };

        Ok(match meta.uri.len() {
            0 => Reject("Token URI is empty".to_string()),
            _ if meta.uri.len() > max_allowed => Reject(format!(
                "Token URI is too long: {} > {}",
                meta.uri.len(),
                max_allowed
            )),
            _ => Accept,
        })
    }
}

#[cfg(test)]
mod test {
    use alloy_primitives::U256;

    use crate::premints::zora_premint_v2::types::ZoraPremintV2;
    use crate::rules::Evaluation::{Accept, Reject};
    use crate::types::SimplePremint;

    use super::*;

    async fn simple_rule(item: PremintTypes, context: RuleContext) -> eyre::Result<Evaluation> {
        Ok(Accept)
    }

    async fn conditional_rule(
        item: PremintTypes,
        _context: RuleContext,
    ) -> eyre::Result<Evaluation> {
        match item {
            PremintTypes::Simple(s) => {
                if s.metadata().chain_id == U256::default() {
                    Ok(Accept)
                } else {
                    Ok(Reject("Chain ID is not default".to_string()))
                }
            }
            _ => Ok(Accept),
        }
    }

    async fn simple_typed_rule(
        _item: SimplePremint,
        _context: RuleContext,
    ) -> eyre::Result<Evaluation> {
        Ok(Accept)
    }

    async fn simple_typed_zora_rule(
        _item: ZoraPremintV2,
        _context: RuleContext,
    ) -> eyre::Result<Evaluation> {
        Ok(Accept)
    }

    #[tokio::test]
    async fn test_simple_rule() {
        let context = RuleContext {};
        let rule = rule!(simple_rule);
        let result = rule
            .check(PremintTypes::Simple(Default::default()), context)
            .await
            .unwrap();
        assert!(matches!(result, Accept));
    }

    #[tokio::test]
    async fn test_simple_rules_engine() {
        let mut re = RulesEngine::new();
        let context = RuleContext {};
        re.add_rule(rule!(simple_rule));
        re.add_rule(rule!(conditional_rule));

        let result = re
            .evaluate(PremintTypes::Simple(Default::default()), context)
            .await;

        assert!(result.is_accept());
    }

    #[tokio::test]
    async fn test_typed_rules_engine() {
        let mut re = RulesEngine::new();
        let context = RuleContext {};

        let rule = typed_rule!(PremintTypes::Simple, simple_typed_rule);
        let rule2 = typed_rule!(PremintTypes::ZoraV2, simple_typed_zora_rule);

        assert_eq!(rule.rule_name(), "PremintTypes::Simple::simple_typed_rule");
        assert_eq!(
            rule2.rule_name(),
            "PremintTypes::ZoraV2::simple_typed_zora_rule"
        );

        re.add_rule(rule);
        re.add_rule(rule2);

        let result = re
            .evaluate(PremintTypes::Simple(Default::default()), context)
            .await;

        assert!(result.is_accept());
    }
}
