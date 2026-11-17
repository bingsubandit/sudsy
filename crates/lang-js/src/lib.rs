//! The JavaScript and TypeScript provider (`.js .mjs .cjs .jsx .ts .mts .cts .tsx`): oxc parses,
//! `adapter` turns its AST and tokens into the lossless tree, the formatter prints the file as
//! written.

mod adapter;
mod format;
mod hooks;
mod rules;
mod safety;
mod semantic;

pub use format::{JsFormatter, option_schema};

include!("provider.generated.rs");
