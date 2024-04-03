use async_trait::async_trait;
use futures::future::join_all;

use crate::types::{Premint, PremintTypes};

#[derive(Clone)]
pub struct RuleContext {}

#[async_trait]
trait Rule: Send + Sync {
    async fn check(&self, item: PremintTypes, context: RuleContext) -> eyre::Result<bool>;
}

struct FnRule<T>(pub T);

#[async_trait]
impl<T, Fut> Rule for FnRule<T>
where
    T: Fn(PremintTypes, RuleContext) -> Fut + Send + Sync + 'static,
    Fut: std::future::Future<Output = eyre::Result<bool>> + Send,
{
    async fn check(&self, item: PremintTypes, context: RuleContext) -> eyre::Result<bool> {
        self.0(item, context).await
    }
}

macro_rules! rule {
    ($fn:tt) => {
        FnRule($fn)
    };
}

macro_rules! typed_rule {
    ($t:path, $fn:tt) => {
        crate::rules::FnRule(
            |item: crate::types::PremintTypes,
             context: crate::rules::RuleContext|
             -> std::pin::Pin<
                std::boxed::Box<dyn std::future::Future<Output = eyre::Result<bool>> + Send + Sync>,
            > {
                std::boxed::Box::pin(async {
                    match item {
                        $t(premint) => $fn(premint, context).await,
                        _ => Ok(true),
                    }
                })
            },
        )
    };
}

struct RE {
    rules: Vec<Box<dyn Rule>>,
}

impl RE {
    pub fn new() -> Self {
        RE { rules: vec![] }
    }
    pub fn add_rule(&mut self, rule: impl Rule + 'static) {
        self.rules.push(Box::new(rule));
    }

    pub async fn evaluate(&self, item: PremintTypes, context: RuleContext) -> bool {
        let results: Vec<_> = self
            .rules
            .iter()
            .map(|rule| rule.check(item.clone(), context.clone()))
            .collect();
        let all_checks = join_all(results).await;

        // TODO: handle errors
        all_checks
            .iter()
            .all(|check| check.is_ok() && check.as_ref().unwrap().clone())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::types::SimplePremint;
    use alloy_primitives::U256;

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
        let mut re = RE::new();
        let context = RuleContext {};
        re.add_rule(rule!(simple_rule));
        re.add_rule(rule!(conditional_rule));

        let result = re
            .evaluate(PremintTypes::Simple(Default::default()), context)
            .await;

        assert!(result);
    }

    #[tokio::test]
    async fn test_typed_rules_engine() {
        let mut re = RE::new();
        let context = RuleContext {};

        let rule = typed_rule!(PremintTypes::Simple, simple_typed_rule);

        re.add_rule(rule);

        let result = re
            .evaluate(PremintTypes::Simple(Default::default()), context)
            .await;

        assert!(result);
    }
}
