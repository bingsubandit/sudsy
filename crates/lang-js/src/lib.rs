//! The JavaScript and TypeScript provider (`.js .mjs .cjs .jsx .ts .mts .cts .tsx`): oxc parses,
//! `adapter` turns its AST and tokens into the lossless tree, the formatter prints the file as
//! written.
//!
//! concept: none (glue: provider entry)

mod adapter;
mod format;
mod rules;
mod safety;
mod semantic;

pub use format::{JsFormatter, option_schema};
pub use semantic::bind_embedded;

include!("provider.generated.rs");
