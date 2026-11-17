//! Checks of ruff's rules about what a statement does with its branches and its values
//! (flake8-simplify, tryceratops, flake8-return, flake8-bugbear).

use super::Check;
use super::lints::boolean;
use super::tree::{callee, inner, span, statements, tokens};
use crate::generated::kind as k;
use crate::semantic::{Cx, model};
use crate::text::code_text;
use core_rules::RuleContext;
use core_syntax::SyntaxNode;

fn has_elif(clause: &SyntaxNode) -> bool {
    tokens(clause).any(|t| t.kind() == k::ELIF)
}

fn clauses(n: &SyntaxNode) -> impl Iterator<Item = SyntaxNode> {
    n.children().filter(|c| c.kind() == k::ELIF_ELSE_CLAUSE)
}

fn bool_return(s: &SyntaxNode) -> Option<bool> {
    if s.kind() != k::STMT_RETURN {
        return None;
    }
    boolean(&s.children().next()?)
}

/// The only statement of a body, when it is a `return True` or `return False`.
fn only_bool_return(mut body: impl Iterator<Item = SyntaxNode>) -> Option<bool> {
    let (s, rest) = (body.next()?, body.next());
    rest.is_none().then(|| bool_return(&s)).flatten()
}

/// SIM103: `if x: return True` followed (or continued by `else`) by `return False`.
pub(super) fn needless_bool(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let Some(a) = only_bool_return(statements(n)) else {
        return;
    };
    let other = if n.kind() == k::ELIF_ELSE_CLAUSE {
        // The last `elif` of a chain that ends in `else`.
        let last = n
            .next_sibling()
            .filter(|e| !has_elif(e) && e.next_sibling().is_none());
        if !has_elif(n) {
            return;
        }
        last.and_then(|e| only_bool_return(statements(&e)))
    } else {
        let mut rest = clauses(n);
        match (rest.next(), rest.next()) {
            (None, _) => n.next_sibling().and_then(|s| bool_return(&s)),
            (Some(e), None) if !has_elif(&e) => only_bool_return(statements(&e)),
            _ => None,
        }
    };
    if other.is_some_and(|b| b != a) {
        c.hit(ctx, span(ctx, n), "");
    }
}

/// The statements of a body as one text, to compare bodies.
fn body_key(n: &SyntaxNode) -> String {
    statements(n)
        .map(|s| code_text(&s))
        .collect::<Vec<_>>()
        .join("\n")
}

/// SIM114: two neighbouring branches of an `if` chain with the same body.
pub(super) fn if_with_same_arms(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    if clauses(n).next().is_none() {
        return;
    }
    let mut previous = body_key(n);
    for clause in clauses(n) {
        if !has_elif(&clause) {
            break;
        }
        let key = body_key(&clause);
        if key == previous && !key.is_empty() {
            c.hit(ctx, span(ctx, &clause), "");
        }
        previous = key;
    }
}

const OPENERS: &[&str] = &[
    "open",
    "io.open",
    "builtins.open",
    "tarfile.open",
    "tempfile.NamedTemporaryFile",
    "tempfile.TemporaryFile",
    "tempfile.SpooledTemporaryFile",
];

/// SIM115: a file opened where nothing closes it: not the context of a `with`, not returned.
pub(super) fn open_file_without_with(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let Some(f) = callee(n) else { return };
    if !f.last_token().is_some_and(|t| {
        matches!(
            t.text(),
            "open" | "NamedTemporaryFile" | "TemporaryFile" | "SpooledTemporaryFile"
        )
    }) {
        return;
    }
    let Some(cx) = model(ctx.file) else { return };
    if !cx
        .qualified(&f)
        .is_some_and(|q| OPENERS.contains(&q.as_str()))
    {
        return;
    }
    let Some(parent) = n.ancestors().skip(1).find(|a| a.kind() != k::PARENTHESIZED) else {
        return;
    };
    let returned = n
        .ancestors()
        .skip(1)
        .take_while(|a| !matches!(a.kind(), k::STMT_FUNCTION_DEF | k::MOD_MODULE))
        .any(|a| a.kind() == k::STMT_RETURN);
    let managed = returned
        || match parent.kind() {
            k::WITH_ITEM | k::STMT_RETURN | k::EXPR_YIELD => true,
            k::EXPR_ATTRIBUTE => parent
                .children()
                .nth(1)
                .is_some_and(|m| code_text(&m) == "close"),
            k::ARGUMENTS => parent
                .parent()
                .and_then(|call| callee(&call))
                .and_then(|f| f.last_token())
                .is_some_and(|t| {
                    matches!(t.text(), "enter_context" | "closing" | "push" | "callback")
                }),
            _ => false,
        };
    if !managed {
        c.hit(ctx, span(ctx, n), "");
    }
}

/// SIM117: a `with` whose only statement is another `with`.
pub(super) fn nested_with(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let mut body = statements(n);
    let (Some(s), None) = (body.next(), body.next()) else {
        return;
    };
    let is_async = |w: &SyntaxNode| tokens(w).any(|t| t.kind() == k::ASYNC);
    if s.kind() == k::STMT_WITH && is_async(n) == is_async(&s) {
        c.hit(ctx, span(ctx, n), "");
    }
}

/// The class a `raise` names: `X` of `raise X` and `raise X(...)`.
fn raised_class(r: &SyntaxNode) -> Option<SyntaxNode> {
    let exc = r.children().next()?;
    let class = if exc.kind() == k::EXPR_CALL {
        callee(&exc)?
    } else {
        exc
    };
    (class.kind() == k::EXPR_NAME).then_some(class)
}

/// TRY002: `raise Exception(...)`.
pub(super) fn raise_vanilla_class(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let Some(class) = raised_class(n).filter(|e| code_text(e) == "Exception") else {
        return;
    };
    if model(ctx.file).is_some_and(|cx| cx.qualified(&class).as_deref() == Some("Exception")) {
        c.hit(ctx, span(ctx, n), "");
    }
}

/// Is `e` a combination of `isinstance`, `issubclass` and `callable` calls?
fn is_type_check(cx: &Cx, e: &SyntaxNode) -> bool {
    let e = inner(e);
    match e.kind() {
        k::EXPR_CALL => callee(&e)
            .and_then(|f| cx.qualified(&f))
            .is_some_and(|q| matches!(q.as_str(), "isinstance" | "issubclass" | "callable")),
        k::EXPR_BOOL_OP | k::EXPR_UNARY_OP => e.children().all(|x| is_type_check(cx, &x)),
        _ => false,
    }
}

/// TRY004: a chain of type checks that raises something other than `TypeError`.
pub(super) fn type_check_without_type_error(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let Some(cx) = model(ctx.file) else { return };
    let checks = |b: &SyntaxNode| {
        cx.slot(b, "condition")
            .is_some_and(|t| is_type_check(&cx, &t))
    };
    if !checks(n) || !clauses(n).filter(has_elif).all(|e| checks(&e)) {
        return;
    }
    // Inside another `if`, only when that one tests a type too.
    let owner = n.parent().and_then(|p| match p.kind() {
        k::STMT_IF => Some(p),
        k::ELIF_ELSE_CLAUSE => p.parent(),
        _ => None,
    });
    if owner.is_some_and(|o| !checks(&o)) {
        return;
    }
    for body in std::iter::once(n.clone()).chain(clauses(n)) {
        for r in statements(&body).filter(|s| s.kind() == k::STMT_RAISE) {
            let Some(q) = raised_class(&r).and_then(|e| cx.qualified(&e)) else {
                continue;
            };
            if matches!(
                q.as_str(),
                "Exception"
                    | "ValueError"
                    | "RuntimeError"
                    | "AssertionError"
                    | "AttributeError"
                    | "ArithmeticError"
                    | "ImportError"
                    | "LookupError"
                    | "NameError"
            ) {
                c.hit(ctx, span(ctx, &r), "");
            }
        }
    }
}

/// RET501: `return None` in a function that returns nothing else.
pub(super) fn explicit_return_none(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    if !n
        .children()
        .next()
        .is_some_and(|v| v.kind() == k::EXPR_NONE_LITERAL)
    {
        return;
    }
    let scope = |r: &SyntaxNode| {
        r.ancestors().skip(1).find(|a| {
            matches!(
                a.kind(),
                k::STMT_FUNCTION_DEF | k::STMT_CLASS_DEF | k::EXPR_LAMBDA
            )
        })
    };
    let Some(f) = scope(n).filter(|a| a.kind() == k::STMT_FUNCTION_DEF) else {
        return;
    };
    // Left alone: a function that is nothing but `return None`, a property, and a function
    // whose annotation allows a value.
    let property = f
        .children()
        .filter(|d| d.kind() == k::DECORATOR)
        .any(|d| code_text(&d).ends_with("property"));
    let annotated = model(ctx.file)
        .and_then(|cx| cx.slot(&f, "returns"))
        .is_some_and(|r| code_text(&r) != "None");
    if property || annotated || (n.parent().as_ref() == Some(&f) && statements(&f).count() == 1) {
        return;
    }
    let only_none = f
        .descendants()
        .filter(|r| r.kind() == k::STMT_RETURN && scope(r).as_ref() == Some(&f))
        .all(|r| {
            r.children()
                .next()
                .is_none_or(|v| v.kind() == k::EXPR_NONE_LITERAL)
        });
    let yields = f
        .descendants()
        .any(|y| matches!(y.kind(), k::EXPR_YIELD | k::EXPR_YIELD_FROM));
    if only_none && !yields {
        c.hit(ctx, span(ctx, n), "");
    }
}

/// B018: an expression statement that does nothing.
pub(super) fn useless_expression(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let Some(e) = n.children().next() else { return };
    if matches!(
        e.kind(),
        k::EXPR_NAME
            | k::EXPR_ATTRIBUTE
            | k::EXPR_NUMBER_LITERAL
            | k::EXPR_BOOLEAN_LITERAL
            | k::EXPR_NONE_LITERAL
            | k::EXPR_LIST
            | k::EXPR_SET
            | k::EXPR_DICT
            | k::EXPR_TUPLE
    ) {
        c.hit(ctx, span(ctx, n), "");
    }
}
