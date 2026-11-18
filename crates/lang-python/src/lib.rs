//! The Python provider (`.py .pyi .pyw`): ruff's parser parses, `adapter` turns its AST and
//! tokens into the lossless tree, the formatter prints the file in Black's style (`ruff format`).
//!
//! concept: none (glue: provider entry)

mod adapter;
mod format;
mod forward;
mod hooks;
mod names;
mod rules;
mod semantic;
mod text;

pub use format::{PythonFormatter, option_schema};

include!("provider.generated.rs");
