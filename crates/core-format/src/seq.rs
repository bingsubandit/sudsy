//! The children of a node as a sequence: the helpers every layout over children needs (`kids`,
//! `flat`, `first`, `last`, `el`) and [`plain`], the children one after the other with the blanks
//! of the `[space]` rows and nothing else (no line breaks, no hanging; see
//! `shapes::flow::Flow` for those).

use crate::FormatCtx;
use crate::comments;
use crate::doc::{self, Doc};
use crate::error::FormatError;
use crate::spacing::SpaceRules;
use core_syntax::{SyntaxElement, SyntaxNode, SyntaxToken, TextRange};

/// The children that are not trivia (zero-width ones included: the end-of-file token carries
/// the trivia at the end of a file).
pub fn children(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Vec<SyntaxElement> {
    node.children_with_tokens()
        .filter(|e| !cx.grammar.is_trivia(e.kind()))
        .collect()
}

/// [`children`] without the empty ones (parsers leave zero-width nodes).
pub fn kids(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Vec<SyntaxElement> {
    let mut els = children(cx, node);
    els.retain(|e| !e.text_range().is_empty());
    els
}

/// [`kids`], with the nodes tagged `#wrapper` (a grammar's structuring nodes that stand for their
/// children) replaced by their own children.
pub fn flat(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Vec<SyntaxElement> {
    let mut out = Vec::new();
    for e in kids(cx, node) {
        match &e {
            SyntaxElement::Node(n) if cx.grammar.has_tag(n.kind(), "wrapper") => {
                out.extend(flat(cx, n));
            }
            _ => out.push(e),
        }
    }
    out
}

pub fn first(cx: &FormatCtx<'_>, e: &SyntaxElement) -> Option<SyntaxToken> {
    match e {
        SyntaxElement::Token(t) => Some(t.clone()),
        SyntaxElement::Node(n) => cx.first_token(n),
    }
}

pub fn last(cx: &FormatCtx<'_>, e: &SyntaxElement) -> Option<SyntaxToken> {
    match e {
        SyntaxElement::Token(t) => Some(t.clone()),
        SyntaxElement::Node(n) => cx.last_token(n),
    }
}

/// The element as a document: a node formatted, a token with its comments.
pub fn el(cx: &FormatCtx<'_>, e: &SyntaxElement) -> Result<Doc, FormatError> {
    match e {
        SyntaxElement::Node(n) => cx.format(n),
        SyntaxElement::Token(t) => Ok(cx.token(t)),
    }
}

/// Does the source have a line break after the element?
pub fn break_after(cx: &FormatCtx<'_>, e: &SyntaxElement) -> bool {
    last(cx, e).is_some_and(|t| comments::break_after(cx.grammar, &t))
}

/// Does the source have a line break between the first and the last token of the node?
pub fn multiline(cx: &FormatCtx<'_>, node: &SyntaxNode) -> bool {
    match (cx.first_token(node), cx.last_token(node)) {
        (Some(a), Some(b)) if a != b => cx.has_line_break(
            node,
            TextRange::new(a.text_range().start(), b.text_range().start()),
        ),
        _ => false,
    }
}

/// Whether a line comment follows the token on its line, so what comes next starts a new one.
pub fn line_comment_after(cx: &FormatCtx<'_>, t: &SyntaxToken) -> bool {
    !matches!(t.text(), ")" | ";" | "]" | "}")
        && comments::trailing_comments(cx.grammar, t)
            .iter()
            .any(|c| c.kind != comments::CommentKind::Block)
}

/// The blank after a token: a line break when a line comment follows it on its line.
pub fn gap(cx: &FormatCtx<'_>, t: &SyntaxToken) -> Doc {
    if line_comment_after(cx, t) {
        doc::hardline()
    } else {
        doc::lit(" ")
    }
}

/// The elements one after the other, `doc` laying each out, a [`gap`] where the `[space]` rows
/// of `rows` say `yes`.
pub fn plain(
    cx: &FormatCtx<'_>,
    rows: &SpaceRules,
    els: &[SyntaxElement],
    doc: impl Fn(&SyntaxElement) -> Result<Doc, FormatError>,
) -> Result<Doc, FormatError> {
    let mut parts = Vec::with_capacity(els.len() * 2);
    let mut prev: Option<SyntaxToken> = None;
    for e in els {
        let (Some(a), Some(b)) = (prev.take(), first(cx, e)) else {
            prev = last(cx, e).or(prev);
            parts.push(doc(e)?);
            continue;
        };
        if rows.spaced(&a, &b) {
            parts.push(gap(cx, &a));
        }
        parts.push(doc(e)?);
        prev = last(cx, e);
    }
    Ok(doc::concat(parts))
}
