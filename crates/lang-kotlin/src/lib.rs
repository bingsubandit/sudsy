//! Kotlin (`.kt`, `.kts`): ktrs-parser (a port of the Kotlin compiler's PSI parser) parses behind
//! `adapter`, which builds the lossless tree; `format.rs` lays the code out like ktlint.

mod adapter;
mod format;
mod rewrite;
mod rules;
mod scope;

pub use adapter::{MAX_BYTES, parse};
pub use format::{KotlinFormatter, option_schema};

include!("provider.generated.rs");
