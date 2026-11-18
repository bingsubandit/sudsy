//! Python formatting: `ruff format`'s layout (Black's style). `stmt` prints the statements and
//! the comments between them, `paren` decides where parentheses stand, the other modules print
//! the expressions.

mod block;
mod comment;
mod compound;
mod docstring;
mod expr;
mod list;
mod literal;
mod op;
mod paren;
mod stmt;
mod util;
pub(crate) use util::syntax;

use crate::generated::kind as k;
use core_config::shared;
use core_format::comments::CommentStyle;
use core_format::{Doc, FormatCtx, FormatError, NodeFormatter};
use core_syntax::{SyntaxElement, SyntaxNode};
use std::borrow::Cow;

include!("schema.generated.rs");

pub struct PythonFormatter;

impl NodeFormatter for PythonFormatter {
    /// Two spaces before a trailing comment (PEP 8).
    fn comment_style(&self, _: &core_config::ResolvedOptions) -> CommentStyle {
        CommentStyle {
            gap: 2,
            ..Default::default()
        }
    }

    /// Comments get their `# ` and `class A():` loses its parentheses before the text is formatted (a
    /// rewrite the safety check follows).
    fn rewrite_source(
        &self,
        tree: &SyntaxNode,
        text: &str,
        _: &core_syntax::Grammar,
        _: &core_config::ResolvedOptions,
    ) -> Option<String> {
        comment::rewrite(tree, text)
    }
    fn format_node(
        &self,
        node: &SyntaxNode,
        cx: &FormatCtx<'_>,
    ) -> Result<Option<Doc>, FormatError> {
        if node.parent().is_none() && node.kind() == k::MOD_MODULE {
            return block::module(cx, node).map(Some);
        }
        Ok((node.parent().is_none()).then(|| cx.verbatim_node(node)))
    }

    fn value<'a>(&self, item: &'a SyntaxElement) -> Cow<'a, str> {
        match item {
            SyntaxElement::Token(t) => literal::value(t),
            SyntaxElement::Node(n) => Cow::Owned(literal::node_value(n)),
        }
    }
}
