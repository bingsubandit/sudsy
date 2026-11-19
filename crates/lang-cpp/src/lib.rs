//! C and C++ (`.c .h`, `.cc .cpp .cxx .hpp .hh .hxx .ipp`): read without preprocessing, as
//! clang-format reads them. `lexer` gives every token, `structure` the unwrapped lines, bracket
//! groups, directives and conditional groups; the two providers (`cpp`, and `c` as its dialect)
//! share both. The formatter prints the file as written.
//!
//! concept: none (glue: provider entry)

core_syntax::include_generated!();
mod format;
mod lexer;
mod structure;

pub use format::{CppFormatter, option_schema};

use core_syntax::grammars;
use provider_api::adapter::{Tok, replay_with};
use provider_api::{ParseError, ParseResult, ParserProvider, ParserSpec, SourceFile};

/// The keywords a file is lexed with: C++ words such as `class`, `new` and `this` are names in C.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dialect {
    C,
    Cpp,
}

/// The provider of a dialect; `c` takes `cpp` as its config base. `.h` is C: a project whose
/// headers are C++ writes its `cpp` options where the C ones would apply, or names the files.
pub fn provider(dialect: Dialect) -> ParserProvider {
    static C: CppFormatter = CppFormatter::new(Dialect::C);
    static CPP: CppFormatter = CppFormatter::new(Dialect::Cpp);
    let spec = ParserSpec {
        crate_name: env!("CARGO_PKG_NAME"),
        grammar: grammars::CPP,
        extensions: match dialect {
            Dialect::C => &["c", "h"],
            Dialect::Cpp => &["cc", "cpp", "cxx", "hpp", "hh", "hxx", "ipp"],
        },
        file_names: &[],
        schema: option_schema(),
        formatter: match dialect {
            Dialect::C => &C,
            Dialect::Cpp => &CPP,
        },
        hooks: &core_rules::DEFAULT_HOOKS,
        rules: &[],
        pattern_files: provider_api::pattern_files!(),
        families: &[],
    };
    let provider = ParserProvider::new(spec, move |file| parse_file(file, dialect));
    match dialect {
        Dialect::Cpp => provider,
        Dialect::C => provider.with_dialect("c", "cpp"),
    }
}

fn parse_file(file: &SourceFile, dialect: Dialect) -> ParseResult {
    parse(&file.text, dialect)
}

pub fn parse(src: &str, dialect: Dialect) -> ParseResult {
    let lexed = lexer::lex(src, dialect);
    let nodes = structure::build(src, &lexed.toks);
    let tokens: Vec<Tok> = (lexed.toks.iter())
        .map(|t| Tok {
            kind: t.kind,
            start: t.start,
            end: t.end,
        })
        .collect();
    let mut result = replay_with(src, grammars::CPP.get(), true, &tokens, &nodes, |t, _| t);
    let errors = (lexed.errors.iter()).map(|&(at, len, msg)| ParseError::syntax(at, len, msg));
    result.errors.extend(errors);
    result.errors.sort_by_key(|e| e.start);
    result
}
