//! Gradle build scripts (Groovy DSL): tree-sitter parses (through `core-treesitter`, mapping in
//! `lang.yaml`) and `format.rules` lays the file out line by line, every line indented by its brackets.
//!
//! concept: none (glue: provider entry)

mod rules;
mod scope;

use core_config::shared;

include!("schema.generated.rs");
include!("provider.generated.rs");
