//! The Python provider (`.py .pyi .pyw`): ruff's parser parses, `adapter` turns its AST and
//! tokens into the lossless tree, the formatter prints the file in Black's style (`ruff format`).

mod adapter;
mod analysis;
mod format;
mod hooks;
mod rules;

pub use format::{PythonFormatter, option_schema};

include!("provider.generated.rs");
