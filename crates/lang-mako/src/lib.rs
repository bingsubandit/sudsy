//! Mako templates (`*.mako`): a hand-written lossless lexer and parser; the formatter is the rows
//! of `format.rules` on the line stream of `core_format`. The Python and the HTML of a template
//! are parsed by their own providers (the `embed` rows of the grammar).
//!
//! concept: none (glue: provider entry)

mod blocks;
mod lexer;
mod parser;

use core_config::shared;

pub use blocks::BLOCKS;
pub use parser::parse;

include!("schema.generated.rs");
include!("provider.generated.rs");
