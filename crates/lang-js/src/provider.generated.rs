// Generated from lang.yaml by `cargo xtask codegen`.
use core_syntax::grammars;
use provider_api::{ParserProvider, ParserSpec};

core_syntax::include_generated!();

#[rustfmt::skip]
pub fn provider() -> ParserProvider {
    ParserProvider::new(ParserSpec { crate_name: env!("CARGO_PKG_NAME"), grammar: grammars::JS, extensions: &["js", "mjs", "cjs", "jsx", "ts", "mts", "cts", "tsx"], file_names: &[], schema: option_schema(), formatter: &JsFormatter, hooks: &hooks::JsHooks, rules: rules::RULES, pattern_files: provider_api::pattern_files!(), families: rules::FAMILIES }, adapter::parse_file)
}
