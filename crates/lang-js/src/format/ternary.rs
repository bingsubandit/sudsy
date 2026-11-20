//! Conditional expressions on the shared layout (`core_format::shapes::conditional`, the
//! `chained` layout: Prettier's `printTernary`); what stays here is the JSX mode that wraps
//! branches in parentheses and the places that indent the whole conditional.
//!
//! concept: conditional-expression-layout

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
                if gp.as_ref().is_some_and(|g| {
                    matches!(g.kind(), k::TS_AS_EXPRESSION | k::TS_SATISFIES_EXPRESSION)
                }) {
                    return false;
                }
                break gp;
            }
            _ => break Some(p),
        }
    };
    // Only where the chain is the value of a declaration, assignment, `return` or `throw`.
    child != *node
        && chain_parent.is_some_and(|p| {
            matches!(
                p.kind(),
                k::VARIABLE_DECLARATOR
                    | k::ASSIGNMENT_EXPRESSION
                    | k::RETURN_STATEMENT
                    | k::THROW_STATEMENT
            ) && p.last_child().is_some_and(|l| l == outer(&child))
        })
}

struct Hooks;

impl shapes::ConditionalHooks for Hooks {
    fn wrap_branches(&self, _cx: &FormatCtx<'_>, node: &SyntaxNode, top: &SyntaxNode) -> bool {
        node.children().any(|c| is_jsx(un(&c).kind())) || chain_contains_jsx(top)
    }

    fn bare_branch(&self, cx: &FormatCtx<'_>, n: &SyntaxNode) -> bool {
        n.kind() == k::NULL_LITERAL
            || (n.kind() == k::IDENTIFIER_REFERENCE
                && super::ast::name_of(cx, n).is_some_and(|x| x == "undefined"))
    }

    fn around(&self, _cx: &FormatCtx<'_>, node: &SyntaxNode) -> Around {
        let p = parent(node);
        Around {
            indent: should_extra_indent(node),
            member_operand: p.as_ref().is_some_and(|p| {
                matches!(
                    p.kind(),
                    k::STATIC_MEMBER_EXPRESSION | k::PRIVATE_FIELD_EXPRESSION
                )
            }),
        }
    }
}
