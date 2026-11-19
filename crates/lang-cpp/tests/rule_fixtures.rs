//! `tests/rules/cpp/<rule>/*` and `tests/rules/c/<rule>/*`: see docs/ai/development/testing.md.

use lang_cpp::{Dialect, provider};

#[test]
fn rule_fixtures() {
    provider_api::fixtures::rule_fixtures_dialects(
        &provider(Dialect::Cpp),
        &[&provider(Dialect::C)],
    );
}
