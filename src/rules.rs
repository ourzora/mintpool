use async_trait::async_trait;
use std::pin::Pin;

use crate::premints::zora_premint_v2::rules::is_authorized_to_create_premint;
use crate::premints::zora_premint_v2::types::ZoraPremintV2;
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
        Fut: std::future::Future<Output=eyre::Result<bool>> + Send,
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
    ($t:expr, $fn:tt) => {
        FnRule(
            |item: PremintTypes, context: RuleContext| -> Pin<Box<dyn std::future::Future<Output=eyre::Result<bool>> + Send + Sync>> {
                Box::pin(async {
                    match item {
                        $t(premint) => {
                            $fn(premint, context).await
                        }
                        _ => { Ok(true) }
                    }
                })
            })
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
        all_checks.iter().all(|check| check.is_ok() && check.as_ref().unwrap().clone())
    }
}

// fn init_rules() {
//     let mut r = RE::new();
//     r.add_rule(&rule!(simple_rule));
// }
//
// // define a rule as an async function signature
// // pub type RuleCheck = dyn Fn(PremintTypes) -> Box<dyn Future<Output=bool>>;
// pub type SpecificRuleCheck<T> = fn(&T) -> (dyn Future<Output = bool> + Send);
//
// pub struct RulesEngine<T: Premint> {
//     rules: Vec<Box<SpecificRuleCheck<T>>>,
// }
//
// impl<T: Premint> RulesEngine<T> {
//     pub async fn validate(&self, premint: &T) -> bool {
//         // let results: Vec<_> = self.rules.iter().map(|rule| rule(premint)).collect();
//         //
//         // let all_checks = join_all(results).await;
//         //
//         // all_checks.iter().all(|&check| check)
//         todo!("implement")
//     }
// }
//
// async fn check<T: Premint>(premint: T) {
//     let premint_v2_rules = RulesEngine {
//         rules: vec![is_authorized_to_create_premint],
//     };
//
//     // TODO: apply rules based on type
// }

#[cfg(test)]
mod test {
    use std::future::{ready, Ready};
    use super::*;
    use alloy_primitives::U256;
    use alloy_signer::k256::sha2::digest::Output;
    use tracing_subscriber::filter::FilterExt;
    use crate::premints::zora_premint_v2::rules::is_valid_signature;

    async fn simple_rule(item: PremintTypes, context: RuleContext) -> eyre::Result<bool> {
        Ok(true)
    }

    async fn conditional_rule(item: PremintTypes, context: RuleContext) -> eyre::Result<bool> {
        match item {
            PremintTypes::Simple(s) => Ok(s.metadata().chain_id == U256::default()),
            _ => Ok(true),
        }
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

    async fn return_true() -> eyre::Result<bool> {
        Ok(true)
    }

    pub async fn test_signature(premint: PremintTypes, context: RuleContext) -> eyre::Result<bool> {
        Ok(true)
    }

    #[tokio::test]
    async fn test_typed_rules_engine() {
        let mut re = RE::new();
        let context = RuleContext {};

        let rule = typed_rule!(PremintTypes::ZoraV2, is_valid_signature);
        let rule = FnRule(
            |item: PremintTypes, context: RuleContext| -> Pin<Box<dyn std::future::Future<Output=eyre::Result<bool>> + Send + Sync>>   {
                Box::pin(async {
                    match item {
                        PremintTypes::ZoraV2(premint) => {
                            is_valid_signature(premint, context).await
                        }
                        _ => { Ok(true) }
                    }
                })
            });

        re.add_rule(rule);

        let result = re
            .evaluate(PremintTypes::Simple(Default::default()), context)
            .await;

        assert!(result);
    }
}
