use async_trait::async_trait;
use futures::future::join_all;

use crate::types::{Premint, PremintTypes};

#[derive(Clone)]
pub struct RuleContext {}

#[async_trait]
pub trait Rule: Send + Sync {
    async fn check(&self, item: PremintTypes, context: RuleContext) -> eyre::Result<bool>;
    fn rule_name(&self) -> &'static str;
}

pub struct FnRule<T>(pub &'static str, pub T);

#[async_trait]
impl<T, Fut> Rule for FnRule<T>
where
    T: Fn(PremintTypes, RuleContext) -> Fut + Send + Sync + 'static,
    Fut: std::future::Future<Output = eyre::Result<bool>> + Send,
{
    async fn check(&self, item: PremintTypes, context: RuleContext) -> eyre::Result<bool> {
        self.1(item, context).await
    }

    fn rule_name(&self) -> &'static str {
        self.0
    }
}

#[macro_export]
macro_rules! rule {
    ($fn:tt) => {
        mintpool::rules::FnRule(stringify!($fn), $fn)
    };
}

#[macro_export]
macro_rules! typed_rule {
    ($t:path, $fn:tt) => {{
        struct TypedRule;

        #[async_trait::async_trait]
        impl mintpool::rules::Rule for TypedRule {
            async fn check(
                &self,
                item: mintpool::types::PremintTypes,
                context: mintpool::rules::RuleContext,
            ) -> eyre::Result<bool> {
                match item {
                    $t(premint) => $fn(premint, context).await,
                    _ => Ok(true),
                }
            }

            fn rule_name(&self) -> &'static str {
                concat!(stringify!($t), "::", stringify!($fn))
            }
        }

        TypedRule {}
    }};
}

pub struct RulesEngine {
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
                        return Ok(false);
                    }
                }
            }
        }

        Ok(true)
    }
}