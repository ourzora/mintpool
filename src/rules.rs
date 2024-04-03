use async_trait::async_trait;
use futures::future::join_all;

use crate::types::{Premint, PremintTypes};

#[derive(Clone)]
pub struct RuleContext {}

#[async_trait]
trait Rule: Send + Sync {
    async fn check(&self, item: PremintTypes, context: RuleContext) -> eyre::Result<bool>;
    fn rule_name(&self) -> &'static str;
}

struct FnRule<T>(pub &'static str, pub T);

#[async_trait]
impl<T, Fut> Rule for FnRule<T>
    where
        T: Fn(PremintTypes, RuleContext) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output=eyre::Result<bool>> + Send,
{
    async fn check(&self, item: PremintTypes, context: RuleContext) -> eyre::Result<bool> {
        self.1(item, context).await
    }

    fn rule_name(&self) -> &'static str { self.0 }
}

macro_rules! rule {
    ($fn:tt) => {
        FnRule(stringify!($fn), $fn)
    };
}

macro_rules! typed_rule {
    ($t:path, $fn:tt) => {{
        struct TypedRule;

        #[async_trait]
        impl crate::rules::Rule for TypedRule{
            async fn check(&self, item: crate::types::PremintTypes, context: crate::rules::RuleContext) -> eyre::Result<bool> {
                match item {
                    $t(premint) => $fn(premint, context).await,
                    _ => Ok(true),
                }
            }

            fn rule_name(&self) -> &'static str { concat!(stringify!($t), "::", stringify!($fn)) }
        }

        TypedRule{}
    }};
}

struct RulesEngine {
    rules: Vec<Box<dyn Rule>>,
}

impl RulesEngine {
    pub fn new() -> Self {
        RulesEngine { rules: vec![] }
    }
    pub fn add_rule(&mut self, rule: impl Rule + 'static) {
        self.rules.push(Box::new(rule));
    }

    pub async fn evaluate(&self, item: PremintTypes, context: RuleContext) -> eyre::Result<bool> {
        let results: Vec<_> = self
            .rules
            .iter()
            .map(|rule| rule.check(item.clone(), context.clone()))
            .collect();
        let all_checks = join_all(results).await;

        // TODO: ideally we'd want to return a list of all errors
        //       so that a caller could determine which rules failed and why
        for error in all_checks.into_iter() {
            match error {
                Err(e) => {
                    return Err(e);
                }
                Ok(pass) => {
                    if !pass {
                        return Ok(false)
                    }
                }
            }
        }

        Ok(true)
    }
}

#[cfg(test)]
mod test {
    use alloy_primitives::U256;

    use crate::premints::zora_premint_v2::types::ZoraPremintV2;
    use crate::types::SimplePremint;

    use super::*;

    async fn simple_rule(item: PremintTypes, context: RuleContext) -> eyre::Result<bool> {
        Ok(true)
    }

    async fn conditional_rule(item: PremintTypes, context: RuleContext) -> eyre::Result<bool> {
        match item {
            PremintTypes::Simple(s) => Ok(s.metadata().chain_id == U256::default()),
            _ => Ok(true),
        }
    }

    async fn simple_typed_rule(item: SimplePremint, context: RuleContext) -> eyre::Result<bool> {
        Ok(true)
    }

    async fn simple_typed_zora_rule(item: ZoraPremintV2, context: RuleContext) -> eyre::Result<bool> {
        Ok(true)
    }

    #[tokio::test]
    async fn test_simple_rule() {
        let context = RuleContext {};
        let rule = rule!(simple_rule);
        let result = rule
            .check(PremintTypes::Simple(Default::default()), context)
            .await
            .unwrap();
        assert!(result);
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

        assert!(result.unwrap());
    }

    #[tokio::test]
    async fn test_typed_rules_engine() {
        let mut re = RulesEngine::new();
        let context = RuleContext {};

        let rule = typed_rule!(PremintTypes::Simple, simple_typed_rule);
        let rule2 = typed_rule!(PremintTypes::ZoraV2, simple_typed_zora_rule);

        assert_eq!(rule.rule_name(), "PremintTypes::Simple::simple_typed_rule");
        assert_eq!(rule2.rule_name(), "PremintTypes::ZoraV2::simple_typed_zora_rule");

        re.add_rule(rule);
        re.add_rule(rule2);

        let result = re
            .evaluate(PremintTypes::Simple(Default::default()), context)
            .await;

        assert!(result.unwrap());
    }
}
