// Generated from lang.yaml by `cargo xtask codegen`.
use core_syntax::grammars;
use provider_api::{ParserProvider, ParserSpec};

core_syntax::include_generated!();

static FORMAT: core_format::rules::LazyRules = core_format::rules::LazyRules::new(|| grammars::BATCH.get(), include_str!("../format.rules")).source_indent().facts(core_format::rules::RuleFacts { exact_end: &[], keep_comment_gap: false, ..core_format::rules::RuleFacts::DEFAULT }).lines(core_format::stream::LineFacts { flush: &["Label"], align_closers: true, option_blanks: &[("ColonColon", "CommentText", "space_after_colons"), ] });

#[rustfmt::skip]
pub fn provider() -> ParserProvider {
    ParserProvider::new(ParserSpec { crate_name: env!("CARGO_PKG_NAME"), grammar: grammars::BATCH, extensions: &["bat", "cmd"], file_names: &[], schema: option_schema(), formatter: &FORMAT, hooks: &core_rules::DEFAULT_HOOKS, rules: &[], pattern_files: provider_api::pattern_files!(), families: &[] }, parser::parse_file)
}
