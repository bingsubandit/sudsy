//! Conditional expressions on the shared layout (`core_format::shapes::conditional`, the
//! `chained` layout: Prettier's `printTernary`); what stays here is the JSX mode that wraps
//! branches in parentheses and the places that indent the whole conditional.

use super::ast::{is_jsx, is_member, outer, parent, un};
use crate::generated::kind as k;
use core_format::doc::Doc;
use core_format::shapes::{self, Around};
use core_format::{FormatCtx, FormatError};
use core_syntax::SyntaxNode;

pub(crate) fn format(node: &SyntaxNode, cx: &FormatCtx<'_>) -> Result<Option<Doc>, FormatError> {
    if node.kind() != k::CONDITIONAL_EXPRESSION {
        return Ok(None);
    }
    shapes::conditional(cx, node, &shapes::ConditionalPolicy::of(cx.options), &Hooks).map(Some)
}

fn is_cond(n: &SyntaxNode) -> bool {
    n.kind() == k::CONDITIONAL_EXPRESSION
}

fn chain_contains_jsx(top: &SyntaxNode) -> bool {
    let mut todo = vec![top.clone()];
    while let Some(c) = todo.pop() {
        for part in c.children() {
            let part = un(&part);
            if is_jsx(part.kind()) {
                return true;
            }
            if is_cond(&part) {
                todo.push(part);
            }
        }
    }
    false
}

/// The part of `parent` its text starts with (what Prettier calls its left side).
fn left_side(parent: &SyntaxNode) -> Option<SyntaxNode> {
    match parent.kind() {
        k::EXPRESSION_STATEMENT
        | k::SEQUENCE_EXPRESSION
        | k::BINARY_EXPRESSION
        | k::LOGICAL_EXPRESSION
        | k::ASSIGNMENT_EXPRESSION
        | k::CONDITIONAL_EXPRESSION
        | k::TAGGED_TEMPLATE_EXPRESSION
        | k::CALL_EXPRESSION
        | k::NEW_EXPRESSION
        | k::AWAIT_EXPRESSION
        | k::TS_AS_EXPRESSION
        | k::TS_SATISFIES_EXPRESSION
        | k::TS_NON_NULL_EXPRESSION
        | k::CHAIN_EXPRESSION => parent.first_child(),
        kind if is_member(kind) => parent.first_child(),
        k::UNARY_EXPRESSION
        | k::SPREAD_ELEMENT
        | k::YIELD_EXPRESSION
        | k::RETURN_STATEMENT
        | k::THROW_STATEMENT => parent.last_child(),
        _ => None,
    }
}

/// `(a ? b : c).d()`: the parenthesized chain breaks inside its parentheses, indented.
fn should_extra_indent(node: &SyntaxNode) -> bool {
    let mut child = node.clone();
    let chain_parent = loop {
        let Some(p) = parent(&child) else {
            return false;
        };
        let first = p.first_child().as_ref() == Some(&outer(&child));
        match p.kind() {
            k::CHAIN_EXPRESSION | k::TS_NON_NULL_EXPRESSION => child = p,
            k::CALL_EXPRESSION if first => child = p,
            kind if is_member(kind) && first => child = p,
            k::NEW_EXPRESSION | k::TS_AS_EXPRESSION | k::TS_SATISFIES_EXPRESSION if first => {
                let gp = parent(&p);
                child = p;
                break gp;
            }
            _ => break Some(p),
        }
    };
    child != *node
        && chain_parent
            .and_then(|p| left_side(&p))
            .is_some_and(|l| l == outer(&child))
}

struct Hooks;

impl shapes::ConditionalHooks for Hooks {
    fn outer(&self, n: &SyntaxNode) -> SyntaxNode {
        outer(n)
    }

    fn inner(&self, n: &SyntaxNode) -> SyntaxNode {
        un(n)
    }

    fn wrap_branches(&self, _cx: &FormatCtx<'_>, node: &SyntaxNode, top: &SyntaxNode) -> bool {
        node.children().any(|c| is_jsx(un(&c).kind())) || chain_contains_jsx(top)
    }

    fn bare_branch(&self, cx: &FormatCtx<'_>, n: &SyntaxNode) -> bool {
        n.kind() == k::NULL_LITERAL
            || (n.kind() == k::IDENTIFIER_REFERENCE
                && super::ast::name_of(cx, n).is_some_and(|x| x == "undefined"))
    }

    fn around(&self, _cx: &FormatCtx<'_>, node: &SyntaxNode) -> Around {
        let held = outer(node);
        let p = parent(node);
        // `(a ? b : c) as T`: the parentheses break around the conditional.
        let cast = p.as_ref().is_some_and(|p| {
            matches!(p.kind(), k::TS_AS_EXPRESSION | k::TS_SATISFIES_EXPRESSION)
                && p.first_child().as_ref() == Some(&held)
        });
        Around {
            indent: cast || should_extra_indent(node),
            member_operand: p.as_ref().is_some_and(|p| {
                matches!(
                    p.kind(),
                    k::STATIC_MEMBER_EXPRESSION | k::PRIVATE_FIELD_EXPRESSION
                )
            }),
        }
    }
}
