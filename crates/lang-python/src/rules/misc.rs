//! Checks of ruff's rules about single calls, decorators and signatures (flake8-bugbear,
//! tryceratops, pylint, flynt, flake8-pyi).

use super::Check;
use super::lints::{call_args, collect_raises};
use super::tree::{callee, inner, keyword, span, statements};
use super::types::is_type_variable;
use crate::generated::kind as k;
use crate::semantic::model;
use crate::text::code_text;
use core_rules::RuleContext;
use core_syntax::SyntaxNode;

fn name_of(n: &SyntaxNode) -> String {
    n.children()
        .find(|i| i.kind() == k::IDENTIFIER)
        .map(|i| code_text(&i))
        .unwrap_or_default()
}

fn in_class(f: &SyntaxNode) -> bool {
    f.ancestors()
        .skip(1)
        .find(|a| {
            matches!(
                a.kind(),
                k::STMT_FUNCTION_DEF | k::STMT_CLASS_DEF | k::MOD_MODULE
            )
        })
        .is_some_and(|a| a.kind() == k::STMT_CLASS_DEF)
}

/// The `i`th parameter of a function.
fn parameter(f: &SyntaxNode, i: usize) -> Option<SyntaxNode> {
    f.children()
        .find(|p| p.kind() == k::PARAMETERS)?
        .descendants()
        .filter(|p| p.kind() == k::PARAMETER)
        .nth(i)
}

/// The decorators of a function, as the names they stand for.
fn decorators(f: &SyntaxNode, cx: &crate::semantic::Cx) -> Vec<(SyntaxNode, String)> {
    f.children()
        .filter(|d| d.kind() == k::DECORATOR)
        .filter_map(|d| {
            let e = d.children().next()?;
            let e = if e.kind() == k::EXPR_CALL {
                callee(&e)?
            } else {
                e
            };
            let q = cx.qualified(&e)?;
            Some((d, q))
        })
        .collect()
}

/// B019: `lru_cache` or `cache` on a method keeps every instance alive.
pub(super) fn cached_method(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    if !in_class(n) {
        return;
    }
    let Some(cx) = model(ctx.file) else { return };
    let decorators = decorators(n, &cx);
    if decorators
        .iter()
        .any(|(_, q)| matches!(q.as_str(), "staticmethod" | "classmethod"))
    {
        return;
    }
    for (d, _) in decorators
        .iter()
        .filter(|(_, q)| matches!(q.as_str(), "functools.lru_cache" | "functools.cache"))
    {
        c.hit(ctx, span(ctx, d), "");
    }
}

/// Calls that return a value that cannot change.
const IMMUTABLE_CALLS: &[&str] = &[
    "tuple",
    "frozenset",
    "bool",
    "int",
    "float",
    "str",
    "bytes",
    "complex",
    "range",
    "type",
];

/// B008: a call that runs once, when the function is defined, to make a default.
pub(super) fn function_call_in_default(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let Some(default) = n.children().nth(1) else {
        return;
    };
    let Some(cx) = model(ctx.file) else { return };
    for call in default.descendants().filter(|d| d.kind() == k::EXPR_CALL) {
        let Some(f) = callee(&call) else { continue };
        let in_lambda = default.kind() == k::EXPR_LAMBDA
            || call
                .ancestors()
                .take_while(|a| *a != default)
                .any(|a| a.kind() == k::EXPR_LAMBDA);
        if !in_lambda
            && !cx
                .qualified(&f)
                .is_some_and(|q| IMMUTABLE_CALLS.contains(&q.as_str()))
        {
            c.hit(ctx, span(ctx, &call), &code_text(&f));
        }
    }
}

/// PYI019: a method whose `self` is annotated with a type variable that its return type repeats.
pub(super) fn custom_type_var_return(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    if !in_class(n) {
        return;
    }
    let Some(cx) = model(ctx.file) else { return };
    let first = parameter(n, 0);
    let (Some(first), Some(returns)) = (first, cx.slot(n, "returns")) else {
        return;
    };
    let Some(annotation) = cx.slot(&first, "annotation").map(|a| inner(&a)) else {
        return;
    };
    if annotation.kind() == k::EXPR_NAME
        && code_text(&annotation) == code_text(&inner(&returns))
        && is_type_variable(&cx, &annotation)
        && !decorators(n, &cx).iter().any(|(_, q)| q == "staticmethod")
    {
        c.hit(ctx, span(ctx, &annotation), &code_text(&annotation));
    }
}

/// PYI032: `other: Any` in `__eq__` or `__ne__`.
pub(super) fn any_eq_parameter(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let name = name_of(n);
    if !in_class(n) || !matches!(name.as_str(), "__eq__" | "__ne__") {
        return;
    }
    let Some(cx) = model(ctx.file) else { return };
    let other = parameter(n, 1);
    if let Some(a) = other.and_then(|p| cx.slot(&p, "annotation"))
        && cx.qualified(&a).is_some_and(|q| q == "typing.Any")
    {
        c.hit(ctx, span(ctx, &a), &name);
    }
}

/// PLW1510: `subprocess.run` without `check`.
pub(super) fn subprocess_run_without_check(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    if !callee(n)
        .and_then(|f| f.last_token())
        .is_some_and(|t| t.text() == "run")
    {
        return;
    }
    let Some(cx) = model(ctx.file) else { return };
    if callee(n).and_then(|f| cx.qualified(&f)).as_deref() != Some("subprocess.run") {
        return;
    }
    let (_, keywords) = call_args(n);
    if keywords
        .iter()
        .all(|kw| keyword(kw).0.is_some_and(|name| name != "check"))
    {
        c.hit(ctx, span(ctx, n), "");
    }
}

/// PLC0105: a covariant (or contravariant) type variable whose name does not say so.
pub(super) fn type_var_name_variance(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let (Some(target), Some(call)) = (n.children().next(), n.children().last()) else {
        return;
    };
    if call.kind() != k::EXPR_CALL || target.kind() != k::EXPR_NAME {
        return;
    }
    let Some(cx) = model(ctx.file) else { return };
    if !callee(&call)
        .and_then(|f| cx.qualified(&f))
        .is_some_and(|q| matches!(q.as_str(), "typing.TypeVar" | "typing_extensions.TypeVar"))
    {
        return;
    }
    let name = code_text(&target);
    let (_, keywords) = call_args(&call);
    let set = |flag: &str| {
        keywords.iter().any(|kw| {
            let (key, v) = keyword(kw);
            key.as_deref() == Some(flag) && v.is_some_and(|v| code_text(&v) == "True")
        })
    };
    if (set("covariant") && !name.ends_with("_co"))
        || (set("contravariant") && !name.ends_with("_contra"))
    {
        c.hit(ctx, span(ctx, &target), &name);
    }
}

/// Items that read the same inside `{}` of an f-string: names, attributes, literals, calls without arguments.
fn fits_in_f_string(e: &SyntaxNode) -> bool {
    match e.kind() {
        k::EXPR_NAME | k::EXPR_ATTRIBUTE | k::EXPR_STRING_LITERAL | k::EXPR_NUMBER_LITERAL => true,
        k::EXPR_CALL => {
            call_args(e) == (Vec::new(), Vec::new())
                && callee(e).is_some_and(|f| f.descendants().all(|d| d.kind() != k::EXPR_CALL))
        }
        _ => false,
    }
}

/// FLY002: `", ".join([a, b])`, which an f-string says better.
pub(super) fn static_join(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let Some(f) = callee(n).filter(|f| f.kind() == k::EXPR_ATTRIBUTE) else {
        return;
    };
    let mut parts = f.children();
    let (Some(object), Some(member)) = (parts.next(), parts.next()) else {
        return;
    };
    if inner(&object).kind() != k::EXPR_STRING_LITERAL || code_text(&member) != "join" {
        return;
    }
    let (args, keywords) = call_args(n);
    let [only] = &args[..] else { return };
    let items = inner(only);
    if keywords.is_empty()
        && matches!(items.kind(), k::EXPR_LIST | k::EXPR_TUPLE)
        && items.children().all(|e| fits_in_f_string(&e))
    {
        c.hit(ctx, span(ctx, n), "");
    }
}

/// TRY201: `raise e` in the handler that bound `e`.
pub(super) fn verbose_raise(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let Some(bound) = n
        .children()
        .find(|i| i.kind() == k::IDENTIFIER)
        .map(|i| code_text(&i))
    else {
        return;
    };
    let mut raises = Vec::new();
    collect_raises(statements(n), &mut raises);
    for r in raises {
        let mut parts = r.children();
        let (Some(exc), None) = (parts.next(), parts.next()) else {
            continue;
        };
        if exc.kind() == k::EXPR_NAME && code_text(&exc) == bound {
            c.hit(ctx, span(ctx, &r), "");
        }
    }
}

/// PLE0704: a bare `raise` that no `except` clause of its function contains.
pub(super) fn bare_raise_outside_handler(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    if n.children().next().is_some() {
        return;
    }
    let inside = n
        .ancestors()
        .skip(1)
        .take_while(|a| {
            !matches!(
                a.kind(),
                k::STMT_FUNCTION_DEF | k::STMT_CLASS_DEF | k::MOD_MODULE
            )
        })
        .any(|a| a.kind() == k::EXCEPT_HANDLER_EXCEPT_HANDLER);
    if !inside {
        c.hit(ctx, span(ctx, n), "");
    }
}
