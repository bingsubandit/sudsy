//! Checks on one statement or expression of its own.

use super::{Check, code_text, has_own, operator, span, test_of, un};
use crate::generated::kind as k;
use core_rules::RuleContext;
use core_syntax::SyntaxNode;

fn first_and_last(n: &SyntaxNode) -> Option<(SyntaxNode, SyntaxNode)> {
    Some((n.children().next()?, n.children().last()?))
}

/// `catch (e) { throw e; }`
pub(super) fn useless_catch(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let Some(handler) = n.children().find(|c| c.kind() == k::CATCH_CLAUSE) else {
        return;
    };
    let param = handler.children().find(|c| c.kind() == k::CATCH_PARAMETER);
    let Some(body) = handler.children().find(|c| c.kind() == k::BLOCK_STATEMENT) else {
        return;
    };
    let mut statements = body.children();
    let (Some(only), None) = (statements.next(), statements.next()) else {
        return;
    };
    let thrown = only.children().next().map(|n| un(&n));
    let (Some(param), Some(thrown)) = (param, thrown) else {
        return;
    };
    let named = param
        .children()
        .next()
        .is_some_and(|p| p.kind() == k::BINDING_IDENTIFIER);
    if only.kind() == k::THROW_STATEMENT
        && named
        && thrown.kind() == k::IDENTIFIER_REFERENCE
        && code_text(&thrown) == code_text(&param)
    {
        if has_own(n, "finally") {
            c.hit(ctx, span(ctx, &handler), "catch clause");
        } else {
            c.hit(ctx, span(ctx, n), "try/catch wrapper");
        }
    }
}

fn is_nan(n: &SyntaxNode) -> bool {
    matches!(code_text(&un(n)).as_str(), "NaN" | "Number.NaN")
}

pub(super) fn use_isnan(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    match n.kind() {
        k::BINARY_EXPRESSION => {
            let compares = matches!(
                operator(n).as_str(),
                "==" | "===" | "!=" | "!==" | "<" | ">" | "<=" | ">="
            );
            if compares && first_and_last(n).is_some_and(|(l, r)| is_nan(&l) || is_nan(&r)) {
                c.hit(
                    ctx,
                    span(ctx, n),
                    "Use the isNaN function to compare with NaN.",
                );
            }
        }
        k::SWITCH_STATEMENT => {
            if n.children().next().is_some_and(|d| is_nan(&d)) {
                c.hit(ctx, span(ctx, n), "'switch(NaN)' can never match a case clause. Use Number.isNaN instead of the switch.");
            }
        }
        _ => {
            if has_own(n, "case") && n.children().next().is_some_and(|t| is_nan(&t)) {
                c.hit(
                    ctx,
                    span(ctx, n),
                    "'case NaN' can never match. Use Number.isNaN before the switch.",
                );
            }
        }
    }
}

const TYPEOF_VALUES: [&str; 8] = [
    "symbol",
    "undefined",
    "object",
    "boolean",
    "number",
    "string",
    "function",
    "bigint",
];

/// `typeof x === "strnig"`
pub(super) fn valid_typeof(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    if !matches!(operator(n).as_str(), "==" | "===" | "!=" | "!==") {
        return;
    }
    let Some((l, r)) = first_and_last(n).map(|(l, r)| (un(&l), un(&r))) else {
        return;
    };
    let typeof_of = |x: &SyntaxNode| x.kind() == k::UNARY_EXPRESSION && operator(x) == "typeof";
    let other = if typeof_of(&l) {
        r
    } else if typeof_of(&r) {
        l
    } else {
        return;
    };
    let text = code_text(&other);
    let literal = match other.kind() {
        k::STRING_LITERAL => Some(&text[1..text.len() - 1]),
        k::TEMPLATE_LITERAL if !text.contains("${") => Some(&text[1..text.len() - 1]),
        _ => None,
    };
    let invalid = match literal {
        Some(value) => !TYPEOF_VALUES.contains(&value),
        None => other.kind() == k::IDENTIFIER_REFERENCE && text == "undefined",
    };
    if invalid {
        c.hit(ctx, span(ctx, &other), "");
    }
}

/// Is `node` where a boolean is expected anyway?
fn in_boolean_context(node: &SyntaxNode) -> bool {
    let mut outer = node.clone();
    while let Some(p) = outer
        .parent()
        .filter(|p| p.kind() == k::PARENTHESIZED_EXPRESSION)
    {
        outer = p;
    }
    let Some(parent) = outer.parent() else {
        return false;
    };
    match parent.kind() {
        k::IF_STATEMENT
        | k::WHILE_STATEMENT
        | k::DO_WHILE_STATEMENT
        | k::FOR_STATEMENT
        | k::CONDITIONAL_EXPRESSION => test_of(&parent).as_ref() == Some(&outer),
        k::UNARY_EXPRESSION => operator(&parent) == "!",
        k::CALL_EXPRESSION | k::NEW_EXPRESSION => {
            is_boolean_callee(&parent) && parent.children().nth(1).as_ref() == Some(&outer)
        }
        _ => false,
    }
}

fn is_boolean_callee(call: &SyntaxNode) -> bool {
    call.children()
        .next()
        .is_some_and(|c| c.kind() == k::IDENTIFIER_REFERENCE && code_text(&c) == "Boolean")
}

/// `!!x` or `Boolean(x)` where a boolean is expected anyway.
pub(super) fn extra_boolean_cast(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let cast = if n.kind() == k::UNARY_EXPRESSION {
        let inner = n.children().next().map(|n| un(&n));
        let double = operator(n) == "!"
            && inner.is_some_and(|i| i.kind() == k::UNARY_EXPRESSION && operator(&i) == "!");
        double.then_some("Redundant double negation.")
    } else {
        is_boolean_callee(n).then_some("Redundant Boolean call.")
    };
    if let Some(message) = cast.filter(|_| in_boolean_context(n)) {
        c.hit(ctx, span(ctx, n), message);
    }
}
