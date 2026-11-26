// Generated from lang.yaml by `cargo xtask codegen`.
use core_syntax::grammars;
use provider_api::{ParserProvider, ParserSpec};

core_syntax::include_generated!();

static FORMAT: core_format::rules::LazyRules = core_format::rules::LazyRules::new(|| grammars::MAKO.get(), include_str!("../format.rules")).source_indent().facts(core_format::rules::RuleFacts { exact_end: &[], keep_comment_gap: false, ..core_format::rules::RuleFacts::DEFAULT }).lines(core_format::stream::LineFacts { flush: &[], align_closers: false, option_blanks: &[("Percent", "ControlOpen", "space_after_percent"), ("Percent", "ControlArm", "space_after_percent"), ("Percent", "ControlClose", "space_after_percent"), ("Percent", "ControlWord", "space_after_percent"), ("HashHash", "CommentText", "space_after_hashes"), ], trim_bracket_edges: false });

#[rustfmt::skip]
pub fn provider() -> ParserProvider {
    ParserProvider::new(ParserSpec { crate_name: env!("CARGO_PKG_NAME"), grammar: grammars::MAKO, extensions: &["mako"], file_names: &[], schema: option_schema(), formatter: &FORMAT, hooks: &core_rules::DEFAULT_HOOKS, rules: &[], pattern_files: provider_api::pattern_files!(), families: &[core_rules::blank_lines::FAMILY, core_rules::unique::FAMILY] }, parser::parse_file).with_format_hosts(&["html", "htm", "xhtml", "xml", "svg"]).with_embedded_rules(&[("python", &[("no-wildcard-imports", core_config::Level::Error)])])
}
