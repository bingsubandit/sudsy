//! Procfile (`Procfile`, `Procfile.dev`): the shared line parser of `core_syntax::key_value_lines`;
//! the formatter is the rows of `format.rules` and the shared key-value shape.
//!
//! concept: none (glue: provider entry)

mod parser;

include!("schema.generated.rs");
include!("provider.generated.rs");
