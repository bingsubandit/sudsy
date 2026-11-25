//! A [`LanguageProvider`](provider_api::LanguageProvider) for a language parsed by tree-sitter,
//! configured by data.
//!
//! concept: tree-sitter-adapter
//! what: A language-neutral adapter from tree-sitter to the lossless tree through a node-name mapping.
//! tools: tree-sitter
//! reads: path:core_treesitter

use crate::{Composite, Engine, Mapping};
use core_config::OptionSchema;
use core_format::NodeFormatter;
use core_rules::{Family, Rule, RuleHooks};
use core_syntax::SyntaxNode;
use core_syntax::grammars::LazyGrammar;
use provider_api::{ParseError, ParserProvider, ParserSpec};

/// What makes a tree-sitter language a provider.
pub struct Spec {
    /// `env!("CARGO_PKG_NAME")` of the language's crate.
    pub crate_name: &'static str,
    /// Resolved on the first parse: a linked-in grammar, or the language's installed pack.
    pub language: fn() -> tree_sitter::Language,
    pub grammar: LazyGrammar,
    pub mapping: &'static Mapping,
    /// File extensions without the dot.
    pub extensions: &'static [&'static str],
    /// Exact file names (`.editorconfig`).
    pub file_names: &'static [&'static str],
    pub schema: OptionSchema,
    pub formatter: &'static dyn NodeFormatter,
    pub hooks: &'static dyn RuleHooks,
    /// The language's own Rust rules.
    pub rules: &'static [&'static dyn Rule],
    /// The text of the built-in pattern files (`provider_api::pattern_files!()`).
    pub pattern_files: &'static [&'static str],
    /// The families the pattern files may name.
    pub families: &'static [Family],
    /// Semantic checks the grammar cannot express; run on a tree without recovery nodes, and
    /// reported as parse errors.
    pub validate: Option<fn(&SyntaxNode, &str) -> Vec<ParseError>>,
}

pub fn provider_of(spec: Spec) -> ParserProvider {
    provider_of_composite(spec, None)
}

/// A provider whose language is assembled from several grammars.
pub fn provider_of_composite(spec: Spec, composite: Option<Composite>) -> ParserProvider {
    let engine = Engine::with_composite(spec.language, spec.grammar, spec.mapping, composite);
    let validate = spec.validate;
    let parse = move |file: &provider_api::SourceFile| {
        let continued =
            file.language_version.as_deref() == Some(core_format::EmbedMode::Continued.name());
        let mut parsed = if continued {
            engine.parse_continued(&file.text)
        } else {
            engine.parse(&file.text)
        };
        if let (true, Some(validate)) = (parsed.errors.is_empty(), validate) {
            parsed.errors = validate(&parsed.tree, &file.text);
        }
        parsed
    };
    ParserProvider::new(
        ParserSpec {
            crate_name: spec.crate_name,
            grammar: spec.grammar,
            extensions: spec.extensions,
            file_names: spec.file_names,
            schema: spec.schema,
            formatter: spec.formatter,
            hooks: spec.hooks,
            rules: spec.rules,
            pattern_files: spec.pattern_files,
            families: spec.families,
        },
        parse,
    )
}
