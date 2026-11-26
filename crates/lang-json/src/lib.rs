//! The JSON and JSONC provider: a hand-written lossless parser, and formatting that is almost
//! entirely the shared shapes' tag defaults.
//!
//! concept: none (glue: provider entry)

core_syntax::include_generated!();
mod format;
mod lexer;
mod parser;
mod rules;
mod value_rules;

pub use format::{COLLAPSE, DIALECT, JsonFormatter, TRAILING_COMMAS, option_schema};
pub use parser::{Dialect, MAX_DEPTH};

use core_syntax::grammars;
use provider_api::{FileMatcher, FilePattern, ParseResult, ParserProvider, ParserSpec, SourceFile};
use std::path::Path;
use std::sync::LazyLock;

/// The file matchers, each with the dialect it selects.
static MATCHERS: LazyLock<Vec<FileMatcher>> = LazyLock::new(|| {
    let matcher = |pattern, dialect: &str| FileMatcher {
        pattern,
        dialect: Some(dialect.into()),
    };
    let ext = |e: &str| FilePattern::Extension(e.into());
    vec![
        matcher(ext("json"), "json"),
        matcher(ext("jsonc"), "jsonc"),
        // Unity assembly definitions and input actions are strict JSON.
        matcher(ext("asmdef"), "json"),
        matcher(ext("asmref"), "json"),
        matcher(ext("inputactions"), "json"),
        matcher(FilePattern::FileName("tsconfig.json".into()), "jsonc"),
        matcher(
            FilePattern::InDirectory {
                dir: ".vscode".into(),
                extension: "json".into(),
            },
            "jsonc",
        ),
    ]
});

pub fn provider() -> ParserProvider {
    let spec = ParserSpec {
        crate_name: env!("CARGO_PKG_NAME"),
        grammar: grammars::JSON,
        extensions: &[],
        file_names: &[],
        schema: option_schema(),
        formatter: &JsonFormatter,
        hooks: &core_rules::DEFAULT_HOOKS,
        rules: rules::RULES,
        pattern_files: provider_api::pattern_files!(),
        families: &[core_rules::regex_source::FAMILY],
    };
    ParserProvider::new(spec, parse_file).with_matchers(MATCHERS.clone())
}

/// The dialect a file is parsed as: the one chosen in `file.language_version` (set from the
/// `dialect` option or the matcher), else decided by the file name.
pub fn parse_file(file: &SourceFile) -> ParseResult {
    let named = (file.language_version.as_deref()).and_then(Dialect::from_name);
    let dialect = named.unwrap_or_else(|| {
        let path = Path::new(&file.id);
        MATCHERS
            .iter()
            .filter(|m| m.matches(path))
            .max_by_key(|m| m.specificity())
            .and_then(|m| m.dialect.as_deref())
            .and_then(Dialect::from_name)
            .unwrap_or(Dialect::Json)
    });
    let parsed = parser::parse(&file.text, dialect);
    ParseResult {
        tree: parsed.tree,
        errors: parsed.errors,
    }
}
