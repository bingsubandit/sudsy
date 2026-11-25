// Generated from lang.yaml by `cargo xtask codegen`.
use core_syntax::grammars;
use provider_api::{ParserProvider, ParserSpec};

core_syntax::include_generated!();

static FORMAT: core_format::rules::LazyRules = core_format::rules::LazyRules::new(|| grammars::RUBY.get(), include_str!("../format.rules")).source_indent().facts(core_format::rules::RuleFacts { exact_end: &["Data"], keep_comment_gap: true, ..core_format::rules::RuleFacts::DEFAULT }).blanks(core_format::stream_blank::BlankFacts { statements: &["Statements"], guard: &["If", "Unless"], jump: &["Return", "Break", "Next", "Redo", "Retry"], jump_words: &["raise", "fail", "throw"], branch: &["Else", "If", "Unless"], directives: &["rubocop:"], modifiers: &["private", "protected", "public", "module_function"], magic: &["frozen_string_literal", "encoding", "coding", "warn_indent", "shareable_constant_value"], body: &["Class", "Module", "Def", "Block"], defs: &["Def", "Class", "Module"],  }).indents(core_format::stream_indent::IndentFacts { statements: &["Statements"], frames: &["Def", "Class", "Module", "SingletonClass", "Block", "Lambda", "Begin"], operations: &["And", "Or"], conditions: &["If", "Unless", "While", "Until"], opaque: &["Arguments", "Array", "Parentheses"], pairs: &["Assoc"], chains: &["Call"], leaders: &["Dot"], attached: &["Block"], keyword_frames: &["If", "Unless", "While", "Until", "Case", "When", "Else", "Rescue", "Ensure"], branches: &["Else", "Rescue", "Ensure", "When"], closers: &["end", "}"], heredoc_openers: &["DocStart"], heredoc_bodies: &["HeredocBody"], heredoc_wrappers: &["Arguments"], open_brackets: &["LParen", "LBracket", "LBrace", "EmbBegin"], close_brackets: &["RParen", "RBracket", "RBrace"], elements: &["Arguments", "Array", "Hash"],  });

#[rustfmt::skip]
pub fn provider() -> ParserProvider {
    ParserProvider::new(ParserSpec { crate_name: env!("CARGO_PKG_NAME"), grammar: grammars::RUBY, extensions: &["rb", "rake", "gemspec", "ru"], file_names: &["Gemfile", "Rakefile", "Guardfile", "Capfile"], schema: option_schema(), formatter: &FORMAT, hooks: &core_rules::DEFAULT_HOOKS, rules: rules::RULES, pattern_files: provider_api::pattern_files!(), families: rules::FAMILIES }, adapter::parse_file)
}
