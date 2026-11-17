//! Kotlin formatting: ktlint's `ktlint_official` layout on the shared kit (docs/formatting/kotlin.md).
//!
//! Every node becomes a `Doc` by `node_doc`. The kit lays out what has a shape (bodies, argument
//! and parameter lists, assignments); the rest is `seq`: the children one after the other with
//! the spacing of `space::want`, a line break of the source kept as a line break, and the lines
//! after the first one hung one level below.

mod decl;
mod expr;
mod space;

use crate::generated::kind as k;
use core_config::shared;
use core_format::{Doc, FormatCtx, FormatError, NodeFormatter, comments, doc};
use core_syntax::{SyntaxElement, SyntaxKind, SyntaxNode, SyntaxToken};

include!("../schema.generated.rs");

pub struct KotlinFormatter;

pub(super) type R = Result<Doc, FormatError>;

impl NodeFormatter for KotlinFormatter {
    /// Braces around a branch can make the branch before it need them too.
    fn rewrite_rounds(&self) -> usize {
        3
    }

    fn rewrite_source(
        &self,
        tree: &SyntaxNode,
        text: &str,
        _: &core_syntax::Grammar,
        options: &core_config::ResolvedOptions,
    ) -> Option<String> {
        crate::rewrite::rewrite(tree, text, options.int("line_width") as usize)
    }

    fn format_node(
        &self,
        node: &SyntaxNode,
        cx: &FormatCtx<'_>,
    ) -> Result<Option<Doc>, FormatError> {
        let kind = node.kind();
        decl::rule(cx, node, kind)
            .or_else(|| expr::rule(cx, node, kind))
            .unwrap_or_else(|| generic(cx, node))
            .map(Some)
    }
}

/// The children that are not trivia (and not empty: the parser leaves zero-width nodes).
pub(super) fn kids(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Vec<SyntaxElement> {
    node.children_with_tokens()
        .filter(|e| !cx.grammar.is_trivia(e.kind()) && !e.text_range().is_empty())
        .collect()
}

pub(super) fn first(cx: &FormatCtx<'_>, e: &SyntaxElement) -> Option<SyntaxToken> {
    match e {
        SyntaxElement::Token(t) => Some(t.clone()),
        SyntaxElement::Node(n) => cx.first_token(n),
    }
}

pub(super) fn last(cx: &FormatCtx<'_>, e: &SyntaxElement) -> Option<SyntaxToken> {
    match e {
        SyntaxElement::Token(t) => Some(t.clone()),
        SyntaxElement::Node(n) => cx.last_token(n),
    }
}

pub(super) fn el(cx: &FormatCtx<'_>, e: &SyntaxElement) -> R {
    match e {
        SyntaxElement::Node(n) => cx.format(n),
        SyntaxElement::Token(t) => Ok(cx.token(t)),
    }
}

pub(super) fn kw(e: &SyntaxElement, text: &str) -> bool {
    matches!(e, SyntaxElement::Token(t) if t.kind() == k::KEYWORD && t.text() == text)
}

/// Does the source have a line break after the element?
pub(super) fn break_after(cx: &FormatCtx<'_>, e: &SyntaxElement) -> bool {
    last(cx, e).is_some_and(|t| comments::break_after(cx.grammar, &t))
}

/// Does the source have a line break between the first and the last token of the node?
pub(super) fn multiline(cx: &FormatCtx<'_>, node: &SyntaxNode) -> bool {
    match (cx.first_token(node), cx.last_token(node)) {
        (Some(a), Some(b)) if a != b => cx.has_line_break(
            node,
            core_syntax::TextRange::new(a.text_range().start(), b.text_range().start()),
        ),
        _ => false,
    }
}

/// What stands between two neighbouring elements: a line break the source has, else a blank
/// where the spacing rules or the source want one.
pub(super) fn between(cx: &FormatCtx<'_>, a: &SyntaxToken, b: &SyntaxToken) -> Option<Doc> {
    if comments::break_after(cx.grammar, a) {
        return Some(doc::hardline());
    }
    let gap = a.text_range().end() != b.text_range().start();
    space::want(a, b).unwrap_or(gap).then(|| doc::lit(" "))
}

/// The elements one after the other (see the module comment).
pub(super) fn seq(cx: &FormatCtx<'_>, els: &[SyntaxElement]) -> R {
    seq_with(cx, els, |e| el(cx, e))
}

/// [`seq`] with `doc` printing the elements.
pub(super) fn seq_with(
    cx: &FormatCtx<'_>,
    els: &[SyntaxElement],
    doc: impl Fn(&SyntaxElement) -> R,
) -> R {
    let mut parts: Vec<Doc> = Vec::with_capacity(els.len() * 2);
    // The part after a line break of the source, one level deeper than what stands before it;
    // a closing parenthesis on a line of its own ends it.
    let mut hung: Vec<Doc> = Vec::new();
    let mut prev: Option<(&SyntaxElement, SyntaxToken)> = None;
    for e in els {
        let d = doc(e)?;
        if let (Some((p, a)), Some(b)) = (prev.take(), first(cx, e)) {
            let sep = between(cx, &a, &b);
            let broken = matches!(sep, Some(Doc::HardLine));
            if broken && matches!(b.kind(), k::RPAR | k::RBRACKET) {
                if !hung.is_empty() {
                    parts.push(doc::indent(doc::concat(std::mem::take(&mut hung))));
                }
                parts.extend(sep);
                parts.push(d);
            } else {
                let out = if (broken && !matches!(p.kind(), k::MODIFIER_LIST | k::ANNOTATION_ENTRY))
                    || !hung.is_empty()
                {
                    &mut hung
                } else {
                    &mut parts
                };
                out.extend(sep);
                out.push(d);
            }
        } else if hung.is_empty() {
            parts.push(d);
        } else {
            hung.push(d);
        }
        prev = last(cx, e).map(|t| (e, t));
    }
    if !hung.is_empty() {
        parts.push(doc::indent(doc::concat(hung)));
    }
    Ok(doc::concat(parts))
}

pub(super) fn generic(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    seq(cx, &kids(cx, node))
}

/// The kind of the node's parent, `KT_FILE` for the root.
pub(super) fn parent_kind(node: &SyntaxNode) -> SyntaxKind {
    node.parent().map_or(k::KT_FILE, |p| p.kind())
}
