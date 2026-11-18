//! What the rules read off the text of the tree: dotted names, string literals and their values.

use crate::format::syntax;
use crate::generated::kind as k;
use core_syntax::{SyntaxNode, TextSize};
use std::borrow::Cow;

pub use core_syntax::roles::code_text;

/// One quoted piece of a string literal.
pub struct Part {
    /// Where the text between the quotes starts and what it is.
    pub inner_start: TextSize,
    pub inner: String,
    pub raw: bool,
    pub bytes: bool,
}

/// The pieces of a string or bytes literal node (`ExprStringLiteral`, `ExprBytesLiteral`).
pub fn parts(n: &SyntaxNode) -> Vec<Part> {
    n.children()
        .filter(|c| matches!(c.kind(), k::STRING_LITERAL | k::BYTES_LITERAL))
        .filter_map(|c| {
            let t = c
                .descendants_with_tokens()
                .filter_map(|e| e.into_token())
                .find(|t| t.kind() == k::STRING)?;
            let lit = syntax().split(t.text())?;
            let start = lit.prefix.len() + lit.open.len();
            Some(Part {
                inner_start: t.text_range().start() + TextSize::try_from(start).ok()?,
                inner: lit.inner.to_owned(),
                raw: lit.prefix.contains(['r', 'R']),
                bytes: lit.prefix.contains(['b', 'B']),
            })
        })
        .collect()
}

/// The value of a string literal: the pieces joined, escapes resolved (`\N{...}` is one `?`).
pub fn string_value(n: &SyntaxNode) -> String {
    let quote = '\0';
    parts(n)
        .iter()
        .map(|p| match p.raw {
            true => Cow::Borrowed(p.inner.as_str()),
            false => syntax().decode(&p.inner, quote).unwrap_or_default(),
        })
        .collect()
}
