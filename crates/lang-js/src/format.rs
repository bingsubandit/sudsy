//! JavaScript and TypeScript formatting, Prettier's layout. `format_node` asks each module in
//! [`MODULES`] for the node (a module matches the kinds it owns); a node no module owns is
//! printed as written, with its strings and numbers restyled (`literal::verbatim`). A later step
//! adds its own module and one line in `MODULES`.

mod arrow;
mod assign;
mod ast;
mod binary;
mod call;
mod chain;
mod class;
mod decl;
mod embed;
mod jsx;
mod literal;
mod object;
mod parens;
mod rewrite;
mod stmt;
mod template;
mod ternary;
mod ts;
mod unary;
mod util;

use core_config::shared;
use core_format::{Doc, FormatCtx, FormatError, NodeFormatter, comments};
use core_syntax::{SyntaxElement, SyntaxNode};
use std::borrow::Cow;

pub(crate) use literal::value as literal_value;

include!("schema.generated.rs");

pub struct JsFormatter;

/// The last token of `node` that is not trivia (JS has no structured trivia, so this is
/// `FormatCtx::last_token` without its ancestor checks, which cost on every node).
fn last_significant(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Option<core_syntax::SyntaxToken> {
    let start = node.text_range().start();
    let mut t = node.last_token();
    while let Some(x) = t {
        if !cx.grammar.is_trivia(x.kind()) {
            return Some(x);
        }
        t = x.prev_token().filter(|p| p.text_range().start() >= start);
    }
    None
}

/// The token in front of the `;` that ends a statement or member: its trailing comments go after
/// the `;` (a type alias and a `for` head keep them in front).
fn before_semicolon(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Option<core_syntax::SyntaxToken> {
    use crate::generated::kind as k;
    if matches!(node.kind(), k::FOR_STATEMENT | k::TS_TYPE_ALIAS_DECLARATION) {
        return None;
    }
    let written = comments::ends_with_token(cx.grammar, node, k::SEMICOLON);
    let mut t = if written {
        let mut last = node.last_child_or_token();
        let semi = loop {
            let e = last?;
            match e {
                core_syntax::SyntaxElement::Token(t) if t.kind() == k::SEMICOLON => break t,
                _ if cx.grammar.is_trivia(e.kind()) => last = e.prev_sibling_or_token(),
                _ => return None,
            }
        };
        semi.prev_token()
    } else if node.kind() == k::EXPRESSION_STATEMENT {
        // (the `;` is written by the formatter)
        node.last_token()
    } else {
        return None;
    };
    while let Some(x) = t {
        if cx.grammar.is_trivia(x.kind()) {
            t = x.prev_token();
        } else if node.kind() == k::EXPRESSION_STATEMENT
            && parens::is_dropped_close(cx, &x)
            && !comments::has_trailing_comment(cx.grammar, &x)
            // (a `;` the formatter writes: the parentheses an arrow body leaves out; others, like
            // the argument of `void`, come back around the argument)
            && (written
                || x.parent()
                    .and_then(|p| p.parent())
                    .is_some_and(|p| p.kind() == k::ARROW_FUNCTION_EXPRESSION))
        {
            // `(a = c /* c */);` is `a = c;` and the comment after it.
            t = x.prev_token();
        } else {
            return comments::has_trailing_comment(cx.grammar, &x).then_some(x);
        }
    }
    None
}

/// Does a comment come before the first token of `node` (inside the node or right before it)?
fn comment_before(cx: &FormatCtx<'_>, node: &SyntaxNode) -> bool {
    let spans = util::comment_spans(cx, node);
    let start = u32::from(node.text_range().start());
    let i = spans.partition_point(|s| s.1 < start);
    spans.get(i).is_some_and(|s| s.0 <= start)
}

type Module = fn(&SyntaxNode, &FormatCtx<'_>) -> Result<Option<Doc>, FormatError>;

const MODULES: &[Module] = &[
    embed::format,
    stmt::format,
    decl::format,
    class::format,
    parens::format,
    binary::format,
    call::format,
    chain::format,
    assign::format,
    ternary::format,
    arrow::format,
    unary::format,
    jsx::format,
    template::format,
    ts::format,
    object::format,
    literal::format,
];

impl NodeFormatter for JsFormatter {
    fn format_node(
        &self,
        node: &SyntaxNode,
        cx: &FormatCtx<'_>,
    ) -> Result<Option<Doc>, FormatError> {
        // A trailing comment belongs to the largest node that ends at its token (Prettier's
        // attachment), so it goes after that node's layout and its line break reaches only the
        // groups around the node, not those inside it.
        let hoisted = comments::comment_at_end(cx.grammar, node)
            .then(|| last_significant(cx, node))
            .flatten()
            .filter(|last| {
                comments::has_trailing_comment(cx.grammar, last)
                    && !node.ancestors().skip(1).any(|p| {
                        // (the statement of a dropped pair of parentheses places it itself)
                        (p.kind() == crate::generated::kind::EXPRESSION_STATEMENT
                            || Some(&p) == node.parent().as_ref())
                            && before_semicolon(cx, &p).is_some_and(|t| t == *last)
                    })
                    && node
                        .parent()
                        .is_some_and(|p| cx.last_token(&p).is_some_and(|l| l != *last))
            });
        if let Some(last) = &hoisted {
            cx.hold_trailing(last);
        }
        let embedded = util::style(cx).embedded;
        let before_semi = if embedded {
            None
        } else {
            before_semicolon(cx, node)
        };
        if let Some(t) = &before_semi {
            cx.hold_trailing(t);
        }
        // Likewise leading comments on lines of their own go before the largest node that starts
        // at their token, so their line breaks do not break the groups inside it.
        let lead = (!embedded
            && comment_before(cx, node)
            && node.first_child().is_some()
            && node.parent().is_some_and(|p| {
                p.parent().is_none() || p.text_range().start() != node.text_range().start()
            }))
        .then(|| cx.first_token(node))
        .flatten()
        .filter(|t| {
            // (a union lays out the comments in front of its first member itself)
            !matches!(t.text(), "|" | "&")
                && node.kind() != crate::generated::kind::TS_UNION_TYPE
                && comments::leading_comments(cx.grammar, t)
                    .iter()
                    .any(|c| c.kind != comments::CommentKind::Block || c.breaks_after > 0)
                && node.parent().is_some_and(|p| {
                    p.kind() != crate::generated::kind::PARENTHESIZED_EXPRESSION
                        && (p.parent().is_none() || cx.first_token(&p).is_some_and(|f| f != *t))
                })
        });
        if let Some(t) = &lead {
            cx.hold_leading(t);
        }
        let mut doc = None;
        for module in MODULES {
            if let Some(d) = module(node, cx)? {
                doc = Some(d);
                break;
            }
        }
        let doc = parens::wrap(node, cx, doc.unwrap_or_else(|| literal::verbatim(cx, node)));
        let doc = match &hoisted {
            Some(last) => core_format::doc::concat(vec![doc, cx.held_trailing(last)]),
            None => doc,
        };
        let doc = match &before_semi {
            Some(t) => core_format::doc::concat(vec![doc, cx.held_trailing(t)]),
            None => doc,
        };
        Ok(Some(match &lead {
            Some(t) => core_format::doc::concat(vec![cx.held_leading(t), doc]),
            None => doc,
        }))
    }

    fn rewrite_rounds(&self) -> usize {
        4
    }

    fn rewrite_source(
        &self,
        tree: &SyntaxNode,
        text: &str,
        grammar: &core_syntax::Grammar,
        options: &core_config::ResolvedOptions,
    ) -> Option<String> {
        rewrite::rewrite(tree, text, grammar, options)
    }

    fn value<'a>(&self, item: &'a SyntaxElement) -> Cow<'a, str> {
        crate::safety::value(item)
    }

    fn extra_check(&self, before: &SyntaxNode, after: &SyntaxNode) -> Result<(), String> {
        crate::safety::check(before, after)
    }
}
