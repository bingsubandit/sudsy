//! Checks of ruff's rules about the shape of `if` and `try` statements and what a handler does
//! (flake8-blind-except, flake8-simplify, flake8-bandit, flake8-debugger).

use super::Check;
use super::lints::{call_args, collect_raises, handler_type};
use super::tree::{callee, inner, keyword, span, statements, tokens};
use crate::generated::kind as k;
use crate::semantic::{Cx, model};
use crate::text::code_text;
use core_rules::RuleContext;
use core_syntax::SyntaxNode;

fn has_clauses(if_: &SyntaxNode) -> bool {
    if_.children().any(|c| c.kind() == k::ELIF_ELSE_CLAUSE)
}

/// The names of the exception classes a handler catches.
fn caught(cx: &Cx, t: &SyntaxNode) -> Vec<String> {
    let t = inner(t);
    if t.kind() == k::EXPR_TUPLE {
        return t.children().flat_map(|e| caught(cx, &e)).collect();
    }
    cx.qualified(&t).into_iter().collect()
}

/// Does the handler's body log the exception with its traceback?
fn logs_traceback(n: &SyntaxNode) -> bool {
    n.descendants()
        .filter(|c| c.kind() == k::EXPR_CALL)
        .any(|call| {
            let Some(f) = callee(&call).filter(|f| f.kind() == k::EXPR_ATTRIBUTE) else {
                return false;
            };
            let name = f
                .children()
                .nth(1)
                .map(|m| code_text(&m))
                .unwrap_or_default();
            let (_, kws) = call_args(&call);
            name == "exception"
                || (matches!(
                    name.as_str(),
                    "error" | "critical" | "warning" | "info" | "debug" | "log"
                ) && kws.iter().any(|kw| {
                    let (key, v) = keyword(kw);
                    key.as_deref() == Some("exc_info")
                        && v.is_some_and(|v| code_text(&inner(&v)) != "False")
                }))
        })
}

/// BLE001: `except Exception` or `except BaseException` that neither re-raises nor logs.
pub(super) fn blind_except(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let Some(cx) = model(ctx.file) else { return };
    let Some(t) = handler_type(ctx, n) else {
        return;
    };
    let blind: Vec<String> = caught(&cx, &t)
        .into_iter()
        .filter(|q| matches!(q.as_str(), "Exception" | "BaseException"))
        .collect();
    let Some(name) = blind.first() else { return };
    let bound = n
        .children()
        .find(|c| c.kind() == k::IDENTIFIER)
        .map(|i| code_text(&i));
    let mut raises = Vec::new();
    collect_raises(statements(n), &mut raises);
    let names_bound = |e: &SyntaxNode| e.kind() == k::EXPR_NAME && Some(code_text(e)) == bound;
    let reraised = raises.iter().any(|r| {
        let mut parts = r.children();
        match (parts.next(), parts.last()) {
            (None, _) => true,
            (Some(exc), cause) => names_bound(&exc) || cause.is_some_and(|c| names_bound(&c)),
        }
    });
    if !reraised && !logs_traceback(n) {
        c.hit(ctx, span(ctx, &t), name);
    }
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

/// S110: a handler for everything (`except:`, `except Exception`) whose body is `pass`.
pub(super) fn try_except_pass(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let Some(cx) = model(ctx.file) else { return };
    let mut body = statements(n);
    let (Some(s), None) = (body.next(), body.next()) else {
        return;
    };
    if s.kind() != k::STMT_PASS {
        return;
    }
    let broad = match handler_type(ctx, n) {
        None => true,
        Some(t) => caught(&cx, &t)
            .iter()
            .any(|q| matches!(q.as_str(), "Exception" | "BaseException")),
    };
    if broad {
        c.hit(ctx, span(ctx, n), "");
    }
}

const DEBUGGERS: &[&str] = &["pdb", "ipdb", "pudb", "wdb", "rpdb", "bpdb", "pdbr"];

/// T100: an import of a debugger, or a call of `breakpoint()`.
pub(super) fn debugger(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let hit = match n.kind() {
        k::EXPR_CALL => {
            let Some(cx) = model(ctx.file) else { return };
            callee(n).and_then(|f| cx.qualified(&f)).is_some_and(|q| {
                q == "breakpoint"
                    || q.strip_suffix(".set_trace")
                        .is_some_and(|m| DEBUGGERS.contains(&m))
            })
        }
        k::STMT_IMPORT => n
            .children()
            .filter_map(|a| a.children().next())
            .any(|m| DEBUGGERS.contains(&code_text(&m).as_str())),
        _ => n
            .children()
            .find(|c| c.kind() == k::IDENTIFIER)
            .is_some_and(|m| DEBUGGERS.contains(&code_text(&m).as_str())),
    };
    if hit {
        c.hit(ctx, span(ctx, n), "");
    }
}
