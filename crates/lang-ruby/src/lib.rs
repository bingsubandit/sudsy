//! Ruby: Prism parses behind `adapter`, which builds the lossless tree; `format.rs` keeps the
//! code as written.

mod adapter;
mod format;
mod rules;

pub use adapter::parse;
pub use format::{RubyFormatter, option_schema};

include!("provider.generated.rs");
