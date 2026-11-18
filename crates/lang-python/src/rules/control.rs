//! Checks of ruff's rules about the shape of `if` and `try` statements and what a handler does
//! (flake8-blind-except, flake8-simplify, flake8-bandit, flake8-debugger).

use super::Check;
use super::tree::{span, statements, tokens};
use crate::generated::kind as k;
use core_rules::RuleContext;
use core_syntax::SyntaxNode;

fn has_clauses(if_: &SyntaxNode) -> bool {
    if_.children().any(|c| c.kind() == k::ELIF_ELSE_CLAUSE)
}

/// Is `n` an `if` or `elif` whose only statement is an `if` without clauses, and that has no
/// clause after it? Then the two tests can be one.
fn collapses(n: &SyntaxNode) -> bool {
    let (alone, last) = match n.kind() {
        k::STMT_IF => (!has_clauses(n), true),
        k::ELIF_ELSE_CLAUSE => (
            tokens(n).any(|t| t.kind() == k::ELIF),
            n.next_sibling().is_none(),
        ),
        _ => return false,
    };
    let mut body = statements(n);
    let (Some(s), None) = (body.next(), body.next()) else {
        return false;
    };
    alone && last && s.kind() == k::STMT_IF && !has_clauses(&s)
}

/// SIM102: an `if` or `elif` whose only statement is another `if`, neither having more clauses.
pub(super) fn collapsible_if(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    if collapses(n) {
        c.hit(ctx, span(ctx, n), "");
    }
}
