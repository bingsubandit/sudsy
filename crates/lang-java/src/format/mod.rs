//! Java formatting: google-java-format's layout on our tree.
//!
//! Every node becomes a `Doc` by `node`; a node without a rule of its own prints its tokens one
//! after the other with the spacing of `space::between`. A rule wraps the parts that may break in
//! groups: `list` (the parenthesised lists), `body` (braces with one member per line), `binary`
//! and the rest of `expr.rs`.

mod decl;
mod expr;
mod space;
mod stmt;

use crate::generated::kind as k;
use core_config::shared;
use core_format::comments::{self, CommentKind};
use core_format::doc::{self, Doc};
use core_format::{FormatCtx, FormatError, NodeFormatter};
use core_syntax::{SyntaxElement, SyntaxNode, SyntaxToken};

include!("../schema.generated.rs");

pub struct JavaFormatter;

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

/// The children that are not trivia.
pub(super) fn kids(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Vec<SyntaxElement> {
    node.children_with_tokens()
        .filter(|e| !cx.grammar.is_trivia(e.kind()) && !e.text_range().is_empty())
        .collect()
}

/// The children, with the wrapper nodes of rezel's grammar (`...Core`, the entry points) replaced
/// by their own children.
pub(super) fn flat(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Vec<SyntaxElement> {
    let mut out = Vec::new();
    for e in kids(cx, node) {
        match &e {
            SyntaxElement::Node(n) if is_wrapper(n.kind()) => out.extend(flat(cx, n)),
            _ => out.push(e),
        }
    }
    out
}

fn is_wrapper(kind: core_syntax::SyntaxKind) -> bool {
    matches!(
        kind,
        k::CLASS_DECLARATION_CORE
            | k::INTERFACE_DECLARATION_CORE
            | k::ENUM_DECLARATION_CORE
            | k::RECORD_DECLARATION_CORE
            | k::ANNOTATION_TYPE_DECLARATION_CORE
            | k::LOCAL_VARIABLE_DECLARATION_CORE
            | k::TYPE_OR_STATIC_IMPORT_DECLARATION
            | k::STATEMENT_EXPRESSION
            | k::TOP_LEVEL_DECLARATION
            | k::COMPILATION_UNIT
            | k::UNNAMED_COMPILATION_UNIT
            | k::UNIT_BEFORE_FIRST_METHOD
            | k::SWITCH_BLOCK_STATEMENT_GROUP_BODY
            | k::SWITCH_GROUP_STATEMENTS
    )
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

/// Whether a line comment follows the token on its line, so what comes next starts a new one.
pub(super) fn line_comment_after(cx: &FormatCtx<'_>, t: &SyntaxToken) -> bool {
    !matches!(t.text(), ")" | ";" | "]" | "}")
        && comments::trailing_comments(cx.grammar, t)
            .iter()
            .any(|c| c.kind != CommentKind::Block)
}

/// The space after a token: a line break when a line comment follows it on its line.
pub(super) fn gap(cx: &FormatCtx<'_>, t: &SyntaxToken) -> Doc {
    if line_comment_after(cx, t) {
        doc::hardline()
    } else {
        doc::lit(" ")
    }
}

/// A line break before an operator when a line comment follows the operand before it, else a
/// `line`.
pub(super) fn line_after(cx: &FormatCtx<'_>, operand: &SyntaxElement) -> Doc {
    let comment = last(cx, operand).is_some_and(|t| {
        comments::trailing_comments(cx.grammar, &t)
            .iter()
            .any(|c| c.kind != CommentKind::Block)
    });
    if comment {
        doc::hardline()
    } else {
        doc::line()
    }
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

/// The elements one after the other with the spacing of `space::between`.
pub(super) fn seq(cx: &FormatCtx<'_>, els: &[SyntaxElement]) -> R {
    seq_with(cx, els, |e| el(cx, e))
}

/// [`seq`] with `doc` printing the elements.
pub(super) fn seq_with(
    cx: &FormatCtx<'_>,
    els: &[SyntaxElement],
    doc: impl Fn(&SyntaxElement) -> R,
) -> R {
    let mut parts = Vec::with_capacity(els.len() * 2);
    let mut prev: Option<SyntaxToken> = None;
    for e in els {
        let (Some(a), Some(b)) = (prev.take(), first(cx, e)) else {
            prev = last(cx, e).or(prev);
            parts.push(doc(e)?);
            continue;
        };
        if space::between(&a, &b) {
            parts.push(gap(cx, &a));
        }
        parts.push(doc(e)?);
        prev = last(cx, e);
    }
    Ok(doc::concat(parts))
}

pub(super) fn generic(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    seq(cx, &flat(cx, node))
}

/// Blank lines the source has in front of the element (comments in front of it excluded).
pub(super) fn blank_before(cx: &FormatCtx<'_>, e: &SyntaxElement) -> bool {
    let Some(t) = first(cx, e) else {
        return false;
    };
    let mut breaks = 0;
    let mut prev = t.prev_token();
    while let Some(p) = prev.filter(|p| cx.grammar.is_trivia(p.kind())) {
        if cx.grammar.has_tag(p.kind(), "comment") {
            return comments::blank_lines_before(cx.grammar, &t) > 0;
        }
        breaks += p.text().matches('\n').count();
        prev = p.prev_token();
    }
    breaks > 1
}

/// Elements on lines of their own, with the blank lines of the source (at most one) between them
/// and `force_blank(prev, next)` where the layout wants one.
pub(super) fn lines(
    cx: &FormatCtx<'_>,
    items: &[SyntaxElement],
    force_blank: impl Fn(&SyntaxElement, &SyntaxElement) -> bool,
) -> R {
    let mut parts = Vec::with_capacity(items.len() * 2);
    for (i, e) in items.iter().enumerate() {
        if i > 0 {
            parts.push(doc::hardline());
            if blank_before(cx, e) || force_blank(&items[i - 1], e) {
                parts.push(doc::hardline());
            }
        }
        parts.push(el(cx, e)?);
    }
    Ok(doc::concat(parts))
}

/// `{`, the items indented one per line, `}`; `{}` when there are none. Comments in front of the
/// `}` stay inside, after the items.
pub(super) fn braces(
    cx: &FormatCtx<'_>,
    open: &SyntaxToken,
    close: &SyntaxToken,
    body: Option<Doc>,
) -> R {
    let lead = comments::leading_comments(cx.grammar, close);
    let mut inner = Vec::new();
    if !lead.is_empty() {
        cx.hold_leading(close);
        if body.is_some() && comments::blank_lines_before(cx.grammar, close) > 0 {
            inner.push(doc::hardline());
        }
    }
    let open_doc = cx.token(open);
    let close_doc = cx.token(close);
    if body.is_none() && lead.is_empty() {
        return Ok(doc::concat(vec![open_doc, close_doc]));
    }
    if let Some(b) = body {
        inner.insert(0, b);
        if !lead.is_empty() {
            inner.insert(1.min(inner.len()), doc::hardline());
        }
    }
    for (i, c) in lead.iter().enumerate() {
        if i > 0 {
            inner.push(doc::hardline());
            if lead[i - 1].breaks_after >= 2 {
                inner.push(doc::hardline());
            }
        }
        inner.push(core_format::context::comment_doc(c));
    }
    Ok(doc::concat(vec![
        open_doc,
        doc::indent(doc::concat(vec![doc::hardline(), doc::concat(inner)])),
        doc::hardline(),
        close_doc,
    ]))
}

/// Continuation indent: two levels.
pub(super) fn cont(d: Doc) -> Doc {
    doc::indent(doc::indent(d))
}
