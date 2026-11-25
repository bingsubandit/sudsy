//! Batchfile (`.bat`, `.cmd`): a hand-written lossless lexer and parser; the formatter is the rows of
//! `format.rules` on the line stream of `core_format`.
//!
//! concept: none (glue: provider entry)

mod commands;
mod lexer;
mod parser;

use core_config::shared;
pub use parser::parse;

include!("schema.generated.rs");
include!("provider.generated.rs");
