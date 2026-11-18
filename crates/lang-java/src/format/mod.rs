//! Java formatting: google-java-format's layout on our tree.
//!
//! Every node becomes a `Doc` by `node`; a node without a rule of its own prints its tokens one
//! after the other with the spacing of the `[space]` rows. A rule wraps the parts that may break in
//! groups: `list` (the parenthesised lists), `body` (braces with one member per line), `binary`
//! and the rest of `expr.rs`.

mod decl;
mod expr;
mod stmt;

use crate::generated::kind as k;
use core_config::shared;
use core_format::comments::{self, CommentKind};
use core_format::doc::{self, Doc};
use core_format::rules::LazyRules;
use core_format::{FormatCtx, FormatError, NodeFormatter};
use core_syntax::grammars;
use core_syntax::{SyntaxElement, SyntaxNode, SyntaxToken};

include!("../schema.generated.rs");

pub struct JavaFormatter;

static ROWS: LazyRules =
    LazyRules::new(|| grammars::JAVA.get(), include_str!("../../format.rules"));

/// Does a blank stand between the two tokens?
pub(super) fn spaced(a: &SyntaxToken, b: &SyntaxToken) -> bool {
    ROWS.get().space().spaced(a, b)
}

pub(super) type R = Result<Doc, FormatError>;

impl NodeFormatter for JavaFormatter {
    fn rewrite_source(
        &self,
        tree: &SyntaxNode,
        text: &str,
        _: &core_syntax::Grammar,
        _: &core_config::ResolvedOptions,
    ) -> Option<String> {
        crate::imports::sort_imports(tree, text)
    }

    fn format_node(
        &self,
        node: &SyntaxNode,
        cx: &FormatCtx<'_>,
    ) -> Result<Option<Doc>, FormatError> {
        node_doc(cx, node).map(Some)
    }
}

fn node_doc(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let kind = node.kind();
    if kind == k::PROGRAM {
        return decl::program(cx, node);
    }
    // The comments in front of a node that starts with a group go in front of the group: their
    // line breaks must not break it.
    let held = if starts_group(kind) {
        cx.first_token(node).filter(|t| !cx.is_held_leading(t))
    } else {
        None
    };
    if let Some(t) = &held {
        cx.hold_leading(t);
    }
    let doc = decl::rule(cx, node, kind)
        .or_else(|| stmt::rule(cx, node, kind))
        .or_else(|| expr::rule(cx, node, kind))
        .unwrap_or_else(|| generic(cx, node))?;
    Ok(match held {
        Some(t) => doc::concat(vec![cx.held_leading(&t), doc]),
        None => doc,
    })
}

fn starts_group(kind: core_syntax::SyntaxKind) -> bool {
    matches!(
        kind,
        k::BINARY_EXPRESSION
            | k::TERNARY_EXPRESSION
            | k::METHOD_INVOCATION
            | k::LAMBDA_EXPRESSION
            | k::ASSIGNMENT_EXPRESSION
            | k::ARRAY_INITIALIZER
            | k::CLASS_DECLARATION
            | k::INTERFACE_DECLARATION
            | k::ENUM_DECLARATION
            | k::RECORD_DECLARATION
            | k::ANNOTATION_TYPE_DECLARATION
            | k::METHOD_DECLARATION
            | k::CONSTRUCTOR_DECLARATION
            | k::VARIABLE_DECLARATOR
            | k::FIELD_DECLARATION
            | k::LOCAL_VARIABLE_DECLARATION
            | k::CONSTANT_DECLARATION
            | k::ARGUMENT_LIST
            | k::FORMAL_PARAMETERS
    )
}

pub(super) use core_format::seq::{first, flat, gap, kids, last, line_comment_after};

pub(super) fn el(cx: &FormatCtx<'_>, e: &SyntaxElement) -> R {
    match e {
        SyntaxElement::Node(n) => cx.format(n),
        SyntaxElement::Token(t) => Ok(tok(cx, t)),
    }
}

/// A token with its comments. A line comment after it moves to the end of the line without
/// breaking the group around it, except after a token that has more of the list to come.
fn tok(cx: &FormatCtx<'_>, t: &SyntaxToken) -> Doc {
    let trailing = comments::trailing_comments(cx.grammar, t);
    if trailing.is_empty() {
        return cx.token(t);
    }
    let lead = if cx.is_held_leading(t) {
        Doc::Nil
    } else {
        cx.leading_comments_doc(t)
    };
    let mut parts = vec![lead, cx.token_text(t)];
    for c in &trailing {
        let text = doc::text(c.text().to_string());
        if c.kind == CommentKind::Block {
            parts.push(doc::lit(" "));
            parts.push(text);
        } else {
            parts.push(doc::line_suffix(doc::concat(vec![doc::lit(" "), text])));
            if !matches!(t.text(), ")" | ";" | "]" | "}") && !closes_next(t) {
                parts.push(doc::break_parent());
            }
        }
    }
    doc::concat(parts)
}

/// [`gap`] after the last token of an element.
pub(super) fn gap_after(cx: &FormatCtx<'_>, e: &SyntaxElement) -> Doc {
    last(cx, e).map_or_else(|| doc::lit(" "), |t| gap(cx, &t))
}

/// Whether the next token closes the construct the token is in (`false // c` before `)`).
fn closes_next(t: &SyntaxToken) -> bool {
    let mut next = t.next_token();
    while let Some(n) = next {
        if !n.text().trim().is_empty() && !n.text().starts_with("//") && !n.text().starts_with("/*")
        {
            return matches!(n.text(), ")" | "]" | "}");
        }
        next = n.next_token();
    }
    false
}

pub(super) fn is_tok(e: &SyntaxElement, text: &str) -> bool {
    matches!(e, SyntaxElement::Token(t) if t.text() == text)
}

/// The elements one after the other with the spacing of the `[space]` rows.
pub(super) fn seq(cx: &FormatCtx<'_>, els: &[SyntaxElement]) -> R {
    seq_with(cx, els, |e| el(cx, e))
}

/// [`seq`] with `doc` printing the elements.
pub(super) fn seq_with(
    cx: &FormatCtx<'_>,
    els: &[SyntaxElement],
    doc: impl Fn(&SyntaxElement) -> R,
) -> R {
    core_format::seq::plain(cx, ROWS.get().space(), els, doc)
}

pub(super) fn generic(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    seq(cx, &flat(cx, node))
}

/// A blank line stands in front of the element in the source (comments in front of it excluded).
pub(super) fn blank_before(cx: &FormatCtx<'_>, e: &SyntaxElement) -> bool {
    match e {
        SyntaxElement::Node(n) => core_format::shapes::blank_before(cx, n),
        SyntaxElement::Token(t) => comments::blank_lines_before(cx.grammar, t) > 0,
    }
}

/// Continuation indent: two levels.
pub(super) fn cont(d: Doc) -> Doc {
    doc::indent(doc::indent(d))
}
