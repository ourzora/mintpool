use alloy_primitives::U256;
use mintpool::{rule, typed_rule};
use mintpool::premints::zora_premint_v2::types::ZoraPremintV2;
use mintpool::rules::{Rule, RuleContext, RulesEngine};
use mintpool::types::{Premint, PremintTypes, SimplePremint};


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

async fn simple_typed_zora_rule(
    item: ZoraPremintV2,
    context: RuleContext,
) -> eyre::Result<bool> {
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
    assert_eq!(
        rule2.rule_name(),
        "PremintTypes::ZoraV2::simple_typed_zora_rule"
    );

    re.add_rule(rule);
    re.add_rule(rule2);

    let result = re
        .evaluate(PremintTypes::Simple(Default::default()), context)
        .await;

    assert!(result.unwrap());
}