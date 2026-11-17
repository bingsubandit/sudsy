// Generated from lang.yaml by `cargo xtask codegen`.
use core_syntax::grammars;
use provider_api::{ParserProvider, ParserSpec};

core_syntax::include_generated!();

#[rustfmt::skip]
pub fn provider() -> ParserProvider {
    ParserProvider::new(ParserSpec { crate_name: env!("CARGO_PKG_NAME"), grammar: grammars::RUBY, extensions: &["rb", "rake", "gemspec", "ru"], file_names: &["Gemfile", "Rakefile", "Guardfile", "Capfile"], schema: option_schema(), formatter: &RubyFormatter, hooks: &core_rules::DEFAULT_HOOKS, rules: rules::RULES, pattern_files: provider_api::pattern_files!(), families: rules::FAMILIES }, adapter::parse_file)
}
