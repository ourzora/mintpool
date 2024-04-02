use std::future::Future;
use std::pin::Pin;

use futures::future::join_all;
use crate::premints::zora_premint_v2::types::ZoraPremintV2;

use crate::types::Premint;

struct RuleContext {}

// define a rule as an async function signature
// pub type RuleCheck = dyn Fn(PremintTypes) -> Box<dyn Future<Output=bool>>;
pub type SpecificRuleCheck<T> = dyn Fn(&T) -> Pin<Box<dyn Future<Output=bool>>> + Send + Sync;

pub struct RulesEngine<T: Premint> {
    rules: Vec<Box<SpecificRuleCheck<T>>>,
}

impl<T: Premint> RulesEngine<T> {
    pub async fn validate(&self, premint: &T) -> bool {
        let results: Vec<_> = self.rules.
            iter().
            map(|rule| {
                rule(premint)
            }).collect();

        let all_checks = join_all(results).await;

        all_checks.iter().all(|&check| check)
    }
}


async fn check<T: Premint>(premint: T) {
    let premint_v2_rules = RulesEngine {
        rules: vec![Box::new(is_authorized_to_create_premint)]
    };

    // TODO: apply rules based on type
}

