// Generated from lang.yaml by `cargo xtask codegen`.
use core_syntax::grammars;
use provider_api::{ParserProvider, ParserSpec};

core_syntax::include_generated!();

#[rustfmt::skip]
pub fn provider() -> ParserProvider {
    ParserProvider::new(ParserSpec { crate_name: env!("CARGO_PKG_NAME"), grammar: grammars::SQL, extensions: &["sql"], file_names: &[], schema: option_schema(), formatter: &SqlFormatter, hooks: &hooks::SqlHooks, rules: rules::RULES, pattern_files: provider_api::pattern_files!(), families: &[core_rules::unique::FAMILY, core_rules::token_case::FAMILY, core_rules::word_names::KEYWORD_NAME, core_rules::word_names::NEEDLESS_QUOTES, core_rules::query_scope::UNKNOWN_COLUMNS, core_rules::query_scope::REFERENCE_QUALIFICATION, core_rules::query_scope::REFERENCE_CONSISTENCY, core_rules::query_scope::UNIQUE_ALIASES, core_rules::query_scope::UNUSED_ALIAS, core_rules::detached_token::FAMILY, core_rules::clause_style::FAMILY] }, parse::parse_file)
}
