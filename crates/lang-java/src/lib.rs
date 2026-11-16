//! Java: rezel-lang-java parses behind `adapter`, which builds the lossless tree; `format.rs` lays
//! the code out like google-java-format.

mod adapter;
mod format;
mod rules;

pub use adapter::parse;
pub use format::{JavaFormatter, option_schema};

include!("provider.generated.rs");
