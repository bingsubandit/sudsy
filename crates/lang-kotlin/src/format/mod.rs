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
use core_format::rules::LazyRules;
use core_format::shapes::flow::Flow;
use core_format::{Doc, FormatCtx, FormatError, NodeFormatter, comments, doc};
use core_syntax::grammars;
use core_syntax::{SyntaxElement, SyntaxKind, SyntaxNode, SyntaxToken};

include!("../schema.generated.rs");

pub struct KotlinFormatter;

static ROWS: LazyRules = LazyRules::new(
    || grammars::KOTLIN.get(),
    include_str!("../../format.rules"),
);

pub(super) use core_format::shapes::flow::{break_after, el, first, kids, last, multiline};

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
        ROWS.get()
            .layout_node(node, cx, &FLOW)
            .or_else(|| decl::rule(cx, node, kind))
            .or_else(|| expr::rule(cx, node, kind))
            .unwrap_or_else(|| generic(cx, node))
            .map(Some)
    }
}

pub(super) fn kw(e: &SyntaxElement, text: &str) -> bool {
    matches!(e, SyntaxElement::Token(t) if t.kind() == k::KEYWORD && t.text() == text)
}

const FLOW: Flow<'static> = Flow {
    want: space::want,
    flat_after: |e| matches!(e.kind(), k::MODIFIER_LIST | k::ANNOTATION_ENTRY),
    closers: &[k::RPAR, k::RBRACKET],
    unhang: &[],
    joined: |_, _| false,
    at_least_one: |_, _| false,
    op_first: |_| false,
    exact: false,
    source_hang: false,
    keep_blank_lines: false,
    rules: None,
};

/// The elements one after the other (see the module comment).
pub(super) fn seq(cx: &FormatCtx<'_>, els: &[SyntaxElement]) -> R {
    FLOW.seq(cx, els)
}

pub(super) fn generic(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    seq(cx, &kids(cx, node))
}

/// The kind of the node's parent, `KT_FILE` for the root.
pub(super) fn parent_kind(node: &SyntaxNode) -> SyntaxKind {
    node.parent().map_or(k::KT_FILE, |p| p.kind())
}
