//! `tests/rules/gradle/<rule>/*`: see docs/ai/development/testing.md.

#[test]
fn rule_fixtures() {
    provider_api::fixtures::rule_fixtures(&lang_gradle::provider());
}
