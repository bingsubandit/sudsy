//! Java formatting: google-java-format's layout on our tree.
//!
//! Every node becomes a `Doc` by `node`; a node without a rule of its own prints its tokens one
//! after the other with the spacing of the `[space]` rows. A rule wraps the parts that may break in
//! groups: `list` (the parenthesised lists), `body` (braces with one member per line), `binary`
//! and the rest of `expr.rs`.
//!
//! concept: none (glue: formatter dispatch)

mod decl;
mod expr;
mod stmt;

use crate::generated::kind as k;
use core_config::shared;
use core_format::comments::{self, CommentStyle};
use core_format::doc::{self, Doc};
use core_format::rules::LazyRules;
use core_format::{FormatCtx, FormatError, NodeFormatter};
use core_rules::imports::{Compare, Order, Separate};
use core_syntax::grammars;
use core_syntax::{SyntaxElement, SyntaxNode, SyntaxToken};

/// The body of a lambda or a switch rule: two levels in, a group of its own.
const ARROW_BODY: core_format::shapes::ArrowStyle = core_format::shapes::ArrowStyle {
    arrow: k::ARROW,
    levels: 2,
    own_group: true,
};

include!("../schema.generated.rs");

pub struct JavaFormatter;

/// google-java-format: `import static` first, then the others, each in ASCII order without
/// duplicates, one blank line between the two; the list is left alone when it holds a comment.
static IMPORT_ORDER: std::sync::LazyLock<Order> = std::sync::LazyLock::new(|| Order {
    dedup: true,
    separate: Separate::Category,
    ..Order::new(Compare::Ordinal, &["static", "*"])
});

static ROWS: LazyRules =
    LazyRules::new(|| grammars::JAVA.get(), include_str!("../../format.rules"));

/// Does a blank stand between the two tokens?
pub(super) fn spaced(a: &SyntaxToken, b: &SyntaxToken) -> bool {
    ROWS.get().space().spaced(a, b)
}

pub(super) type R = Result<Doc, FormatError>;

impl NodeFormatter for JavaFormatter {
    fn rows(&self) -> &'static str {
        ROWS.rows()
    }

    fn rewrite_rows(&self) -> Option<&core_format::rewrite::RewriteRules> {
        ROWS.get().rewrite_rules()
    }

    fn rewrite_scanned(
        &self,
        tree: &SyntaxNode,
        text: &str,
        grammar: &core_syntax::Grammar,
        _: &core_config::ResolvedOptions,
        scan: Option<core_format::rewrite::RewriteScan<'_>>,
    ) -> Option<String> {
        core_rules::imports::sort_source(tree, text, grammar, &IMPORT_ORDER)
            .or_else(|| scan.and_then(|s| s.apply(grammar, tree, text)))
    }

    fn rewrite_source(
        &self,
        tree: &SyntaxNode,
        text: &str,
        grammar: &core_syntax::Grammar,
        _: &core_config::ResolvedOptions,
    ) -> Option<String> {
        core_rules::imports::sort_source(tree, text, grammar, &IMPORT_ORDER)
            .or_else(|| ROWS.get().rewrite_source(grammar, tree, text))
    }

    /// The import order, then the `[rewrite]` rows (the modifier order is a rule's round).
    fn rewrite_rounds(&self) -> usize {
        2
    }

    fn blank_between(&self, cx: &FormatCtx<'_>, prev: &SyntaxNode, next: &SyntaxNode) -> bool {
        ROWS.get().blank_between(cx, prev, next)
    }

    fn comment_style(&self, options: &core_config::ResolvedOptions) -> CommentStyle {
        CommentStyle {
            reflow_width: Some(core_format::print_options(options).line_width),
            trailing_counts_after: &[";", ")", "]", "}"],
            attach_docs: true,
            ..CommentStyle::default()
        }
    }

    fn reflows_comments(&self) -> bool {
        true
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

/// A token with its comments (line comments after a closer stay at the end of the line).
fn tok(cx: &FormatCtx<'_>, t: &SyntaxToken) -> Doc {
    core_format::seq::token_deferring_comments(cx, t, cx.comment_style.trailing_counts_after)
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
