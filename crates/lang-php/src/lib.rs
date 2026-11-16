//! PHP: mago-syntax parses behind `adapter`, which builds the lossless tree; `format.rs` lays the
//! code out.

mod adapter;
mod format;
mod rules;

pub use adapter::parse;
pub use format::{PhpFormatter, option_schema};

include!("provider.generated.rs");
