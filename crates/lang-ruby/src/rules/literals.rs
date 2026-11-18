//! Checks on literals: floats compared for equality, symbols that are booleans, single-quoted
//! strings that contain `#{`.
//!
//! concept: new literal-checks
//! what: Checks on literals: float comparison, boolean symbols, interpolation in strings.
//! tools: RuboCop Lint

use super::tree::{method_name, span};
use crate::generated::kind as k;
use core_rules::RuleContext;
use core_rules::checks::NodeCheck;
use core_syntax::SyntaxNode;
use core_syntax::SyntaxNodeExt;
use core_syntax::roles::code_tokens;

/// Whether `n` is a number that is zero, or `nil`, however many parentheses are around it.
fn literal_safe(n: &SyntaxNode) -> bool {
    match n.kind() {
        k::PARENTHESES => n
            .descendants()
            .find(|d| d.kind() != k::PARENTHESES && d.kind() != k::STATEMENTS)
            .is_some_and(|d| literal_safe(&d)),
        k::NIL => true,
        k::INTEGER | k::FLOAT => code_tokens(n)
            .find(|t| t.kind() == k::NUMBER)
            .is_some_and(|t| {
                t.text()
                    .replace('_', "")
                    .parse::<f64>()
                    .is_ok_and(|v| v == 0.0)
            }),
        _ => false,
    }
}

/// Whether the expression is a float literal or a call that returns a float.
fn float(n: &SyntaxNode) -> bool {
    match n.kind() {
        k::FLOAT => true,
        k::PARENTHESES => n
            .descendants()
            .find(|d| d.kind() != k::PARENTHESES && d.kind() != k::STATEMENTS)
            .is_some_and(|d| float(&d)),
        k::CALL => float_call(n),
        _ => false,
    }
}

fn float_call(call: &SyntaxNode) -> bool {
    let Some(name) = method_name(call) else {
        return false;
    };
    let receiver = call.children().next().filter(|c| c.kind() != k::ARGUMENTS);
    let argument = call.child(k::ARGUMENTS).and_then(|a| a.children().next());
    match name.text() {
        "+" | "-" | "*" | "/" | "%" | "**" if name.kind() == k::OPERATOR => {
            receiver.as_ref().is_some_and(float) || argument.as_ref().is_some_and(float)
        }
        "to_f" | "Float" | "fdiv" => true,
        m => receiver
            .filter(|r| r.kind() == k::FLOAT)
            .is_some_and(|r| match m {
                "abs" | "magnitude" | "modulo" | "next_float" | "prev_float" | "quo" => true,
                "angle" | "arg" | "phase" => r.text().to_string().trim().starts_with('-'),
                "ceil" | "floor" | "round" | "truncate" => {
                    argument.is_some_and(|a| a.kind() == k::INTEGER && !literal_safe(&a))
                }
                _ => false,
            }),
    }
}

/// `x == 1.0` and its kin, and `when 1.0`: floats are rarely equal when they should be.
pub fn float_comparison(check: &NodeCheck, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    if node.kind() == k::WHEN {
        for condition in node.children().take_while(|c| c.kind() != k::STATEMENTS) {
            if float(&condition) && !literal_safe(&condition) {
                let range = span(ctx, &condition);
                check.hit(ctx, range, "when");
            }
        }
        return;
    }
    let Some(name) = method_name(node) else {
        return;
    };
    if !matches!(name.text(), "==" | "!=" | "eql?" | "equal?") {
        return;
    }
    let Some(args) = node.child(k::ARGUMENTS) else {
        return;
    };
    let mut arguments = args.children();
    let (Some(rhs), None) = (arguments.next(), arguments.next()) else {
        return;
    };
    let Some(lhs) = node.children().next().filter(|c| c.kind() != k::ARGUMENTS) else {
        return;
    };
    if literal_safe(&lhs) || literal_safe(&rhs) || !(float(&lhs) || float(&rhs)) {
        return;
    }
    let range = span(ctx, node);
    check.hit(ctx, range, name.text());
}

/// `:true` and `:false`.
pub fn boolean_symbol(check: &NodeCheck, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let range = span(ctx, node);
    let text = &ctx.file.text[range];
    if matches!(text, ":true" | ":false" | "true:" | "false:") {
        check.hit(ctx, range, text);
    }
}

/// A single-quoted string with `#{...}` in it, which would interpolate in double quotes.
pub fn interpolation_check(check: &NodeCheck, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    if !code_tokens(node)
        .next()
        .is_some_and(|t| t.kind() == k::QUOTE && t.text() == "'")
    {
        return;
    }
    let text = node.text().to_string();
    let content = text.trim().trim_start_matches('\'').trim_end_matches('\'');
    let hit = content.match_indices("#{").any(|(i, _)| {
        !content[..i].ends_with('\\')
            && content[i..]
                .split('\n')
                .next()
                .is_some_and(|l| l.contains('}'))
    });
    if !hit {
        return;
    }
    let quoted = if content.contains('"') {
        format!("%{{{content}}}")
    } else {
        format!("\"{content}\"")
    };
    let parsed = crate::parse(&quoted);
    if parsed.errors.is_empty()
        && parsed
            .tree
            .descendants()
            .any(|d| d.kind() == k::EMBEDDED_STATEMENTS)
    {
        let range = span(ctx, node);
        check.hit(ctx, range, "");
    }
}

/// `"#{1}"`: a literal in an interpolation.
pub fn literal_in_interpolation(check: &NodeCheck, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let Some(statements) = node.child(k::STATEMENTS) else {
        return;
    };
    let mut all = statements.children();
    let (Some(only), None) = (all.next(), all.next()) else {
        return;
    };
    let literal = matches!(only.kind(), k::ARRAY | k::HASH | k::ASSOC)
        || !ctx.file.text[span(ctx, &only)]
            .trim_matches(['\'', '"'])
            .trim()
            .is_empty();
    let all = only.descendants().all(|d| {
        matches!(
            d.kind(),
            k::INTEGER
                | k::FLOAT
                | k::SYMBOL
                | k::TRUE
                | k::FALSE
                | k::NIL
                | k::STRING
                | k::ARRAY
                | k::HASH
                | k::ASSOC
                | k::KEYWORD_HASH
        )
    });
    if literal && all {
        let range = span(ctx, &only);
        check.hit(ctx, range, "");
    }
}

/// A regexp that has named and numbered captures: the numbered ones are not captured.
pub fn mixed_regexp_capture_types(check: &NodeCheck, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let range = span(ctx, node);
    let text = &ctx.file.text[range];
    let bytes = text.as_bytes();
    let (mut named, mut numbered, mut class) = (false, false, false);
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => i += 1,
            b'[' => class = true,
            b']' => class = false,
            b'(' if !class => match (bytes.get(i + 1), bytes.get(i + 2)) {
                (Some(b'?'), Some(b'<')) if !matches!(bytes.get(i + 3), Some(b'=' | b'!')) => {
                    named = true
                }
                (Some(b'?'), _) => {}
                _ => numbered = true,
            },
            _ => {}
        }
        i += 1;
    }
    if named && numbered {
        check.hit(ctx, range, "");
    }
}

/// `x == x`, `a && a`: both sides are the same code.
pub fn identical_operands(check: &NodeCheck, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let (left, right, op) = if matches!(node.kind(), k::AND | k::OR) {
        let mut parts = node.children();
        let (Some(l), Some(r)) = (parts.next(), parts.next()) else {
            return;
        };
        let Some(op) = code_tokens(node).find(|t| t.kind() == k::OPERATOR) else {
            return;
        };
        (l, r, op)
    } else {
        let Some(op) = method_name(node).filter(|n| {
            n.kind() == k::OPERATOR
                && matches!(
                    n.text(),
                    "==" | "!=" | "===" | "<=>" | "=~" | ">" | ">=" | "<" | "<=" | "|" | "^"
                )
        }) else {
            return;
        };
        let Some(l) = node.children().next().filter(|c| c.kind() != k::ARGUMENTS) else {
            return;
        };
        let Some(args) = node.child(k::ARGUMENTS) else {
            return;
        };
        let mut all = args.children();
        let (Some(r), None) = (all.next(), all.next()) else {
            return;
        };
        (l, r, op)
    };
    if ctx.file.text[span(ctx, &left)] == ctx.file.text[span(ctx, &right)] {
        let range = span(ctx, node);
        check.hit(ctx, range, op.text());
    }
}
