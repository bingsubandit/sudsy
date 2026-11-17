//! Rules about single statements and expressions: `no-unused-expressions`,
//! `no-unexpected-multiline`, `no-duplicate-enum-values`, `no-unsafe-finally` and `require-yield`.

use super::{Check, code_text, has_own, span};
use crate::generated::kind as k;
use core_rules::RuleContext;
use core_syntax::{Grammar, SyntaxKind, SyntaxNode, SyntaxToken, TextRange};
use std::collections::HashSet;

/// Does the expression do nothing (ESLint's defaults: short circuits, ternaries and tagged
/// templates count as nothing)? A call, `new`, assignment, update, `await`, `yield`, `import()`,
/// `void` and `delete` do something.
fn does_nothing(n: &SyntaxNode, g: &Grammar) -> bool {
    match n.kind() {
        k::PARENTHESIZED_EXPRESSION
        | k::CHAIN_EXPRESSION
        | k::TS_AS_EXPRESSION
        | k::TS_TYPE_ASSERTION
        | k::TS_NON_NULL_EXPRESSION
        | k::TS_INSTANTIATION_EXPRESSION => {
            n.children().next().is_some_and(|e| does_nothing(&e, g))
        }
        k::UNARY_EXPRESSION => !has_own(n, g, "void") && !has_own(n, g, "delete"),
        k::ARRAY_EXPRESSION
        | k::ARROW_FUNCTION_EXPRESSION
        | k::BINARY_EXPRESSION
        | k::PRIVATE_IN_EXPRESSION
        | k::CLASS
        | k::CONDITIONAL_EXPRESSION
        | k::FUNCTION
        | k::IDENTIFIER_REFERENCE
        | k::BOOLEAN_LITERAL
        | k::NULL_LITERAL
        | k::NUMERIC_LITERAL
        | k::STRING_LITERAL
        | k::BIG_INT_LITERAL
        | k::REG_EXP_LITERAL
        | k::LOGICAL_EXPRESSION
        | k::STATIC_MEMBER_EXPRESSION
        | k::COMPUTED_MEMBER_EXPRESSION
        | k::PRIVATE_FIELD_EXPRESSION
        | k::IMPORT_META
        | k::NEW_TARGET
        | k::OBJECT_EXPRESSION
        | k::SEQUENCE_EXPRESSION
        | k::TAGGED_TEMPLATE_EXPRESSION
        | k::TEMPLATE_LITERAL
        | k::THIS_EXPRESSION => true,
        _ => false,
    }
}

/// `no-unused-expressions`
pub(super) fn unused_expressions(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let g = ctx.file.grammar;
    if n.children().next().is_some_and(|e| does_nothing(&e, g)) {
        c.hit(ctx, span(ctx, n), "");
    }
}

/// The last code token before `t`.
fn code_before(t: &SyntaxToken, g: &Grammar) -> Option<SyntaxToken> {
    std::iter::successors(t.prev_token(), |p| p.prev_token()).find(|p| !g.is_trivia(p.kind()))
}

/// Reports a line break between the code before `open` and `open`.
fn break_before(c: &Check, open: &SyntaxToken, ctx: &mut RuleContext<'_>, what: &str) {
    let Some(prev) = code_before(open, ctx.file.grammar) else {
        return;
    };
    let gap = TextRange::new(prev.text_range().end(), open.text_range().start());
    if ctx.file.text[gap].contains('\n') {
        c.hit(ctx, gap, what);
    }
}

/// `no-unexpected-multiline`: `a\n[b]`, `a\n(b)` and a template on a line of its own after its tag.
pub(super) fn unexpected_multiline(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let g = ctx.file.grammar;
    let own = |text: &str| {
        n.children_with_tokens()
            .filter_map(|e| e.into_token())
            .find(|t| t.text() == text)
    };
    if has_own(n, g, "?.") {
        return;
    }
    match n.kind() {
        k::COMPUTED_MEMBER_EXPRESSION => {
            if let Some(open) = own("[") {
                break_before(c, &open, ctx, "object and [ of property access");
            }
        }
        k::CALL_EXPRESSION => {
            let arguments = n
                .children()
                .skip(1)
                .any(|a| a.kind() != k::TS_TYPE_PARAMETER_INSTANTIATION);
            if let (true, Some(open)) = (arguments, own("(")) {
                break_before(c, &open, ctx, "function and ( of function call");
            }
        }
        _ => {
            let first = n.children().last().and_then(|q| {
                q.descendants_with_tokens()
                    .filter_map(|e| e.into_token())
                    .find(|t| !g.is_trivia(t.kind()))
            });
            if let Some(open) = first {
                break_before(c, &open, ctx, "template tag and template literal");
            }
        }
    }
}

/// The value of a constant enum initializer, written so equal values read equal.
fn enum_value(init: &SyntaxNode, g: &Grammar) -> Option<String> {
    let text = code_text(init, g);
    match init.kind() {
        k::STRING_LITERAL => Some(format!("s{}", &text[1..text.len() - 1])),
        k::TEMPLATE_LITERAL if !text.contains("${") => {
            Some(format!("s{}", &text[1..text.len() - 1]))
        }
        k::NUMERIC_LITERAL => {
            let t = text.replace('_', "").to_ascii_lowercase();
            let radix = |p: &str, r| {
                t.strip_prefix(p)
                    .and_then(|d| u64::from_str_radix(d, r).ok())
                    .map(|v| v as f64)
            };
            let v = radix("0x", 16)
                .or_else(|| radix("0o", 8))
                .or_else(|| radix("0b", 2))
                .or_else(|| t.parse::<f64>().ok())?;
            Some(format!("n{v}"))
        }
        _ => None,
    }
}

/// `no-duplicate-enum-values`
pub(super) fn duplicate_enum_values(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let g = ctx.file.grammar;
    let mut seen = HashSet::new();
    for member in n.descendants().filter(|m| m.kind() == k::TS_ENUM_MEMBER) {
        if !has_own(&member, g, "=") {
            continue;
        }
        let Some(value) = member.children().last().and_then(|i| enum_value(&i, g)) else {
            continue;
        };
        if !seen.insert(value.clone()) {
            c.hit(ctx, span(ctx, &member), &value[1..]);
        }
    }
}

fn is_loop(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        k::WHILE_STATEMENT
            | k::DO_WHILE_STATEMENT
            | k::FOR_STATEMENT
            | k::FOR_IN_STATEMENT
            | k::FOR_OF_STATEMENT
    )
}

fn is_function(kind: SyntaxKind) -> bool {
    matches!(kind, k::FUNCTION | k::ARROW_FUNCTION_EXPRESSION | k::CLASS)
}

fn label_of(n: &SyntaxNode, g: &Grammar) -> Option<String> {
    n.children()
        .find(|l| l.kind() == k::LABEL_IDENTIFIER)
        .map(|l| code_text(&l, g))
}

/// Is `block` the `finally` block of a `try`?
fn is_finalizer(block: &SyntaxNode, g: &Grammar) -> bool {
    let after_finally =
        std::iter::successors(block.prev_sibling_or_token(), |e| e.prev_sibling_or_token())
            .find(|e| !e.as_token().is_some_and(|t| g.is_trivia(t.kind())))
            .is_some_and(|e| e.as_token().is_some_and(|t| t.text() == "finally"));
    after_finally && block.parent().is_some_and(|p| p.kind() == k::TRY_STATEMENT)
}

/// `return`, `throw`, and a `break` or `continue` that leaves the `finally` block.
pub(super) fn unsafe_finally(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let g = ctx.file.grammar;
    let label = label_of(n, g);
    let jump = n.kind();
    let mut current = n.clone();
    while let Some(p) = current.parent() {
        match p.kind() {
            kind if is_function(kind) => return,
            kind if is_loop(kind)
                && label.is_none()
                && matches!(jump, k::BREAK_STATEMENT | k::CONTINUE_STATEMENT) =>
            {
                return;
            }
            k::SWITCH_STATEMENT if label.is_none() && jump == k::BREAK_STATEMENT => return,
            k::LABELED_STATEMENT if label.is_some() && label_of(&p, g) == label => return,
            k::BLOCK_STATEMENT if is_finalizer(&p, g) => {
                c.hit(ctx, span(ctx, n), g.name(jump));
                return;
            }
            _ => {}
        }
        current = p;
    }
}

fn has_yield(n: &SyntaxNode) -> bool {
    n.children()
        .any(|c| c.kind() == k::YIELD_EXPRESSION || (c.kind() != k::FUNCTION && has_yield(&c)))
}

/// A generator with a body and no `yield`.
pub(super) fn require_yield(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let g = ctx.file.grammar;
    let method = n
        .parent()
        .filter(|p| matches!(p.kind(), k::METHOD_DEFINITION | k::OBJECT_PROPERTY));
    let generator = has_own(n, g, "*") || method.is_some_and(|m| has_own(&m, g, "*"));
    let Some(body) = n.children().find(|b| b.kind() == k::FUNCTION_BODY) else {
        return;
    };
    if generator && body.children().next().is_some() && !has_yield(&body) {
        c.hit(ctx, span(ctx, n), "");
    }
}
