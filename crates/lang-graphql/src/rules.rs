//! The Rust rules of GraphQL; syntax rules that fit a pattern are files in `rules/`.

mod no_typename_prefix;

use crate::generated::kind;
use core_rules::Rule;
use core_syntax::{SyntaxNode, SyntaxNodeExt, SyntaxToken};

/// The name token of a definition (a variable's is its `$name`): a token of the node itself, or
/// of its `FragmentName`.
fn name_of(def: &SyntaxNode) -> Option<SyntaxToken> {
    let holder = def
        .child(kind::FRAGMENT_NAME)
        .unwrap_or_else(|| def.clone());
    holder
        .tokens()
        .find(|t| matches!(t.kind(), kind::NAME | kind::VARIABLE))
}

pub static RULES: &[&dyn Rule] = &[&no_typename_prefix::NoTypenamePrefix];
