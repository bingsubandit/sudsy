//! Binary and logical expressions on the shared operator chain
//! (`core_format::shapes::binary_chain`); what stays here is where a chain stands in JS (inside
//! the parentheses of `if`, in a JSX attribute, ...), which right sides stay on the operator's
//! line, and the comments in front of a chain.

use super::ast::{outer, parent, token_comments, un};
use crate::generated::kind as k;
use core_format::doc::{Doc, concat};
use core_format::shapes::{self, Place};
use core_format::{FormatCtx, FormatError};
use core_syntax::SyntaxNode;

/// `a || { ... }`, `a && [...]`: the right side stays on the operator's line.
pub(crate) fn should_inline_logical(node: &SyntaxNode) -> bool {
    if node.kind() != k::LOGICAL_EXPRESSION {
        return false;
    }
    let Some(right) = node.last_child() else {
        return false;
    };
    let right = un(&right);
    match right.kind() {
        k::OBJECT_EXPRESSION | k::ARRAY_EXPRESSION => {
            right.children_with_tokens().any(|e| e.as_node().is_some())
        }
        k::JSX_ELEMENT | k::JSX_FRAGMENT => true,
        _ => false,
    }
}

pub(crate) fn format(node: &SyntaxNode, cx: &FormatCtx<'_>) -> Result<Option<Doc>, FormatError> {
    if !matches!(
        node.kind(),
        k::BINARY_EXPRESSION | k::LOGICAL_EXPRESSION | k::PRIVATE_IN_EXPRESSION
    ) {
        return Ok(None);
    }
    // Comments on lines of their own in front of the chain go in front of its group, not inside
    // the group of its first operand.
    let lead = leading_token(node, cx);
    if let Some(t) = &lead {
        cx.hold_leading(t);
    }
    let doc = shapes::binary_chain(cx, node, &shapes::BinaryPolicy::of(cx.options), &Hooks)?;
    Ok(Some(match &lead {
        Some(t) => concat(vec![cx.held_leading(t), doc]),
        None => doc,
    }))
}

/// The first token of `node` past the parentheses (there are some) that are left out, when comments on lines of
/// their own are in front of it and nothing holds them yet.
fn leading_token(node: &SyntaxNode, cx: &FormatCtx<'_>) -> Option<core_syntax::SyntaxToken> {
    let mut t = cx.first_token(node)?;
    if t.kind() != k::L_PAREN {
        return None;
    }
    while t.kind() == k::L_PAREN {
        let close = t.parent()?.last_token()?;
        if !super::parens::is_dropped_close(cx, &close) {
            return None;
        }
        t = t.next_token()?;
        while cx.grammar.is_trivia(t.kind()) {
            t = t.next_token()?;
        }
    }
    (!cx.is_held_leading(&t)
        && core_format::comments::leading_comments(cx.grammar, &t)
            .iter()
            .any(|c| c.kind != core_format::comments::CommentKind::Block || c.breaks_after > 0))
    .then_some(t)
}

struct Hooks;

impl shapes::BinaryHooks for Hooks {
    fn outer(&self, n: &SyntaxNode) -> SyntaxNode {
        outer(n)
    }

    fn inner(&self, n: &SyntaxNode) -> SyntaxNode {
        un(n)
    }

    /// Prettier's tree has no parentheses; only ones with a comment in the way (a comment after
    /// the `)` or inside, not in front of the content) are in the way of flattening.
    fn flattens(&self, cx: &FormatCtx<'_>, n: &SyntaxNode) -> bool {
        std::iter::successors(Some(n.clone()), |x| {
            (x.kind() == k::PARENTHESIZED_EXPRESSION)
                .then(|| x.first_child())
                .flatten()
        })
        .all(|x| {
            x.kind() != k::PARENTHESIZED_EXPRESSION
                || !x
                    .children_with_tokens()
                    .filter_map(|e| e.into_token())
                    .any(|t| t.kind() == k::R_PAREN && token_comments(cx, &t))
        })
    }

    fn reattach_comments(&self) -> bool {
        true
    }

    fn inline_right(&self, _cx: &FormatCtx<'_>, node: &SyntaxNode) -> bool {
        should_inline_logical(node)
    }

    fn tail_outside(&self, _cx: &FormatCtx<'_>, node: &SyntaxNode) -> bool {
        node.last_child()
            .is_some_and(|r| super::ast::is_jsx(un(&r).kind()))
    }

    fn place(&self, cx: &FormatCtx<'_>, node: &SyntaxNode) -> Option<Place> {
        let p = parent(node);
        let pk = p.as_ref().map(|p| p.kind());
        let c = outer(node);
        let first_of_parent = p
            .as_ref()
            .is_some_and(|p| p.first_child().as_ref() == Some(&c));
        let last_of_parent = p
            .as_ref()
            .is_some_and(|p| p.last_child().as_ref() == Some(&c));
        let inside_parens = match pk {
            Some(
                k::IF_STATEMENT | k::WHILE_STATEMENT | k::SWITCH_STATEMENT | k::WITH_STATEMENT,
            ) => first_of_parent,
            Some(k::DO_WHILE_STATEMENT) => last_of_parent,
            Some(k::UNARY_EXPRESSION) => p
                .as_ref()
                .is_some_and(|p| super::unary::has_commented_parentheses(cx, p)),
            Some(k::ARRAY_EXPRESSION) => super::embed::is_unindented_root(cx, node),
            _ => false,
        };
        if inside_parens {
            return Some(Place::Bare);
        }
        let Some(p) = p else {
            return Some(Place::Flush);
        };
        let pk = p.kind();
        if (matches!(pk, k::CALL_EXPRESSION | k::NEW_EXPRESSION) && first_of_parent)
            || pk == k::UNARY_EXPRESSION
            || matches!(
                pk,
                k::STATIC_MEMBER_EXPRESSION | k::PRIVATE_FIELD_EXPRESSION
            )
        {
            return Some(Place::Wrapped);
        }
        let gk = parent(&p).map(|g| g.kind());
        let flush = (pk == k::JSX_EXPRESSION_CONTAINER && gk == Some(k::JSX_ATTRIBUTE))
            || (pk == k::ARROW_FUNCTION_EXPRESSION && last_of_parent)
            || (pk == k::FOR_STATEMENT && !last_of_parent)
            || (pk == k::CONDITIONAL_EXPRESSION
                && !matches!(
                    gk,
                    Some(k::RETURN_STATEMENT | k::THROW_STATEMENT | k::CALL_EXPRESSION)
                ))
            || pk == k::TEMPLATE_LITERAL;
        flush.then_some(Place::Flush)
    }
}
