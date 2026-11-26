//! Helm chart templates (Go template actions in YAML): tree-sitter parses the actions (through
//! `core-treesitter`, mapping in `lang.yaml`), the shared template layout formats them.
//!
//! concept: none (glue: provider entry)

mod format;
mod rules;

pub use format::{HelmFormatter, option_schema};

include!("provider.generated.rs");
