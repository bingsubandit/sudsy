//! Shared syntax tree model for every language Sudsy handles.
//!
//! * [`kind`]: the 32-bit `SyntaxKind` namespace scheme.
//! * [`tree`]: the `rowan`-based lossless tree and a small builder.
//! * [`trivia`]: the single trivia attachment rule and a checker for it.
//! * [`grammar`]: the `.grammar` description format, its loader and content hash.
//! * [`ast`]: support code used by the generated typed wrappers.
//! * [`codegen`]: generates those wrappers (driven by `cargo xtask codegen`).
//! * [`roles`]: the role vocabulary grammars tag their node kinds with.
//! * [`shape`]: checks trees against the bodies of a grammar.
//! * [`parse`]: the kit of the hand-written parsers (lexer base, tokens with trivia, cursor).
//! * [`scan`]: the `trivia` and `limits` rows adapters of external parsers read.
//! * [`strings`]: the string-literal model every language's `string` rows describe.
//! * [`wire`]: the versioned tree encoding used across process boundaries.
//!
//! concept: none (glue: crate root)

pub mod ast;
pub mod block_parse;
pub mod codegen;
pub mod containers;
pub mod embed_rows;
pub mod embedded;
pub mod grammar;
pub mod grammars;
pub mod kind;
pub mod parse;
pub mod regex;
pub mod roles;
pub mod scan;
pub mod shape;
pub mod spans;
pub mod strings;
pub mod tree;
pub mod trivia;
pub mod wire;

pub use embedded::Embedded;
pub use grammar::{
    Assoc, FieldAt, FormatRow, FormatVia, Grammar, ImportSyntax, KeySyntax, Precedence,
};
pub use kind::{LanguageId, SyntaxKind};
pub use rowan::{Checkpoint, GreenNode, TextRange, TextSize, WalkEvent};
pub use tree::{
    SudsyLang, SyntaxElement, SyntaxNode, SyntaxNodeExt, SyntaxToken, TreeBuilder, code_span,
    dump_tree, first_code_token, print,
};

/// The typed wrappers the crate's build script generates from its grammar (`build/lang-crate.rs`).
#[macro_export]
macro_rules! include_generated {
    () => {
        #[allow(clippy::all, dead_code, unused_imports)]
        pub mod generated {
            include!(concat!(env!("OUT_DIR"), "/generated.rs"));
        }
    };
}
