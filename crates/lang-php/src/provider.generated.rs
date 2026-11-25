// Generated from lang.yaml by `cargo xtask codegen`.
use core_syntax::grammars;
use provider_api::{ParserProvider, ParserSpec};

core_syntax::include_generated!();

static SYMBOLS: provider_api::SymbolLang = provider_api::SymbolLang::new(include_str!("../symbols.rows"));

#[rustfmt::skip]
pub fn provider() -> ParserProvider {
    ParserProvider::new(ParserSpec { crate_name: env!("CARGO_PKG_NAME"), grammar: grammars::PHP, extensions: &["php", "phtml", "inc"], file_names: &[], schema: option_schema(), formatter: &PhpFormatter, hooks: &core_rules::DEFAULT_HOOKS, rules: rules::RULES, pattern_files: provider_api::pattern_files!(), families: &[core_rules::regex_source::FAMILY, core_rules::missing_return::FAMILY, core_rules::index_rules::NAME_RESOLVES] }, adapter::parse_file).with_symbols(&SYMBOLS)
}
