//! Dockerfile and Containerfile: tree-sitter parses (through `core-treesitter`, mapping in
//! `lang.yaml`), the formatter in `format.rs` re-lays each instruction on its source tokens.
//!
//! concept: none (glue: provider entry)

mod format;
mod rewrite;
mod rules;

pub use format::{DockerfileFormatter, option_schema};

include!("provider.generated.rs");
