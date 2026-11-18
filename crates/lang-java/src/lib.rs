//! Java: `parser` (a hand-written lexer and parser) builds the lossless tree; `format/` lays the
//! code out like google-java-format.
//!
//! concept: none (glue: provider entry)

mod format;
mod imports;
mod lexer;
mod parser;
mod rules;
mod scope;

pub use format::{JavaFormatter, option_schema};
pub use parser::parse;

include!("provider.generated.rs");
