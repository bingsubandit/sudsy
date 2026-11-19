//! POSIX sh and Bash: tree-sitter parses (through `core-treesitter`, mapping in `lang.yaml`),
//! the formatter in `format.rs` lays out the statements and keeps every word as written.
//!
//! concept: none (glue: provider entry)

mod format;
mod rules;
mod scope;

pub use format::{ShellFormatter, option_schema};

include!("provider.generated.rs");
