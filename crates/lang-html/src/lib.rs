//! HTML, Vue, Svelte and Angular: one hand-written parser (`parser`), one tree, the formatter
//! prints the file as written.
//!
//! concept: none (glue: provider entry)

mod access;
mod checks;
mod content;
mod dialect;
mod document;
mod dom;
mod format;
mod hooks;
mod meta;
mod ng_checks;
mod parser;
mod patterns;
mod rules;
mod script;
mod template;

pub use dialect::{angular_provider, provider, svelte_provider, vue_provider};
pub use format::{HtmlFormatter, option_schema};

core_syntax::include_generated!();
