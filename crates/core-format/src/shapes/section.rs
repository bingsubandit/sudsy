use super::statement_sequence;
use crate::FormatCtx;
use crate::doc::{self, Doc};
use crate::error::FormatError;
use core_syntax::{SyntaxElement, SyntaxKind, SyntaxNode};

/// A header and the entries under it (`[table]` and its pairs, INI and EditorConfig sections):
/// the header's tokens and nodes, then the entries of the given kinds one per line, without
/// indentation (no braces), blank lines between them as the source has, up to `max_blank_lines`.
pub fn section(
    cx: &FormatCtx<'_>,
    node: &SyntaxNode,
    entries: &[SyntaxKind],
) -> Result<Doc, FormatError> {
    let mut parts = Vec::new();
    let mut items = Vec::new();
    for e in node.children_with_tokens() {
        match e {
            SyntaxElement::Token(t) if !cx.grammar.is_trivia(t.kind()) => parts.push(cx.token(&t)),
            SyntaxElement::Node(n) if entries.contains(&n.kind()) => items.push(n),
            SyntaxElement::Node(n) => parts.push(cx.format(&n)?),
            SyntaxElement::Token(_) => {}
        }
    }
    if !items.is_empty() {
        parts.push(doc::hardline());
        parts.push(statement_sequence(
            cx,
            &items,
            cx.options.max_blank_lines(),
        )?);
    }
    Ok(doc::concat(parts))
}
