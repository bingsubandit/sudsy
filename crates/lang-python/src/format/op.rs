//! Operators: binary, boolean and comparison chains, unary, conditional, lambda, `:=`, `await`
//! and `yield`.
//!
//! concept: binary-chain-layout, conditional-expression-layout

use super::expr::{elem, expr};
use super::util::*;
use crate::generated::kind as k;
use core_format::FormatCtx;
use core_format::doc::{Doc, concat, group, hardline, lit};
use core_format::shapes;
use core_syntax::{SyntaxElement, SyntaxKind, SyntaxNode};

/// Binding strength of a binary operator (higher binds tighter).
pub(crate) fn precedence(op: SyntaxKind) -> u8 {
    match op {
        k::VBAR => 5,
        k::CIRCUM_FLEX => 6,
        k::AMPER => 7,
        k::LEFT_SHIFT | k::RIGHT_SHIFT => 8,
        k::PLUS | k::MINUS => 9,
        k::STAR | k::SLASH | k::DOUBLE_SLASH | k::PERCENT | k::AT => 10,
        _ => 12,
    }
}

/// Operands and operators of a left-associative chain of one precedence level.
fn flatten(cx: &FormatCtx<'_>, node: &SyntaxNode, out: &mut Vec<SyntaxElement>) {
    let ks = kids(cx, node);
    let op = ks.get(1).and_then(tok_kind).unwrap_or(k::PLUS);
    match ks.first() {
        Some(SyntaxElement::Node(l))
            if l.kind() == k::EXPR_BIN_OP
                && kids(cx, l)
                    .get(1)
                    .and_then(tok_kind)
                    .is_some_and(|o| o != k::DOUBLE_STAR && precedence(o) == precedence(op))
                && op != k::DOUBLE_STAR =>
        {
            flatten(cx, l, out);
        }
        Some(e) => out.push(e.clone()),
        None => {}
    }
    out.extend(ks.into_iter().skip(1));
}

pub(crate) fn binary_like(cx: &FormatCtx<'_>, node: &SyntaxNode, breakable: bool) -> R {
    match node.kind() {
        k::EXPR_BIN_OP => binary_with(cx, node, breakable),
        k::EXPR_BOOL_OP => chain(cx, &flatten_bool(cx, node), breakable),
        k::EXPR_COMPARE => compare_with(cx, node, breakable),
        _ => unary(cx, node),
    }
}

pub(crate) fn binary(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    binary_with(cx, node, true)
}

fn binary_with(cx: &FormatCtx<'_>, node: &SyntaxNode, breakable: bool) -> R {
    let ks = kids(cx, node);
    if ks.get(1).and_then(tok_kind) == Some(k::DOUBLE_STAR) {
        return power(cx, &ks);
    }
    let mut elems = Vec::new();
    flatten(cx, node, &mut elems);
    chain(cx, &elems, breakable)
}

/// An operator token and its trailing comment, which stays with the operand before it (ruff).
pub(crate) fn operator(cx: &FormatCtx<'_>, t: &core_syntax::SyntaxToken) -> (Doc, Doc) {
    if !core_format::comments::has_trailing_comment(cx.grammar, t) {
        return (Doc::Nil, cx.token(t));
    }
    cx.hold_trailing(t);
    let op = cx.token(t);
    (cx.held_trailing(t), op)
}

/// `a`, then per operator a break opportunity before it.
fn chain(cx: &FormatCtx<'_>, elems: &[SyntaxElement], breakable: bool) -> R {
    let mut parts = Vec::new();
    // After a multi-line string each operator and operand is a group of its own: the operator
    // stays behind the string when the operand fits there.
    let pairs = elems
        .first()
        .and_then(|e| e.as_node())
        .is_some_and(is_multiline_string);
    let mut pair: Vec<Doc> = Vec::new();
    for (i, e) in elems.iter().enumerate() {
        match e {
            SyntaxElement::Token(t) => {
                if !pair.is_empty() {
                    parts.push(group(concat(std::mem::take(&mut pair))));
                }
                let (comment, op) = operator(cx, t);
                pair.push(comment);
                pair.push(if breakable { brk() } else { sp() });
                pair.push(op);
                pair.push(sp());
            }
            SyntaxElement::Node(n) => {
                let implicit = matches!(n.kind(), k::EXPR_STRING_LITERAL | k::EXPR_BYTES_LITERAL)
                    && n.children().count() > 1;
                let held = held_operand_comment(cx, n, i + 1 < elems.len() && !implicit);
                let d = if matches!(n.kind(), k::EXPR_STRING_LITERAL | k::EXPR_BYTES_LITERAL) {
                    super::expr::strings(cx, n, false)?
                } else {
                    expr(cx, n)?
                };
                let d = concat(vec![d, held.map_or(Doc::Nil, |l| cx.held_trailing(&l))]);
                if pairs && i > 0 {
                    pair.push(d);
                } else {
                    parts.extend(std::mem::take(&mut pair));
                    parts.push(d);
                }
            }
        }
    }
    if !pair.is_empty() {
        parts.push(if pairs {
            group(concat(pair))
        } else {
            concat(pair)
        });
    }
    Ok(group(concat(parts)))
}

/// The comment after an operand that ends where its flattened parent does (`(a) | (b)  # c` before
/// the next operator): printed after the operand, outside its own parentheses, which would break.
fn held_operand_comment(
    cx: &FormatCtx<'_>,
    n: &SyntaxNode,
    inner: bool,
) -> Option<core_syntax::SyntaxToken> {
    let last = cx.last_token(n).filter(|l| {
        inner
            && !cx.is_held(l)
            && core_format::comments::comment_at_end(cx.grammar, n)
            && core_format::comments::has_trailing_comment(cx.grammar, l)
            && n.parent()
                .is_some_and(|p| cx.last_token(&p).is_some_and(|pl| pl == *l))
    })?;
    cx.hold_trailing(&last);
    Some(last)
}

/// An implicit concatenation or a string with line breaks: no break before the operator after it.
fn is_multiline_string(n: &SyntaxNode) -> bool {
    matches!(
        n.kind(),
        k::EXPR_STRING_LITERAL | k::EXPR_BYTES_LITERAL | k::EXPR_F_STRING | k::EXPR_T_STRING
    ) && (n.children().count() > 1
        || n.descendants_with_tokens()
            .filter_map(|e| e.into_token())
            .any(|t| matches!(t.kind(), k::STRING | k::F_STRING_MIDDLE) && t.text().contains('\n')))
}

fn power(cx: &FormatCtx<'_>, ks: &[SyntaxElement]) -> R {
    let simple = |e: &SyntaxElement| e.as_node().is_some_and(is_simple_power_operand);
    let tight = simple(&ks[0]) && ks.get(2).is_some_and(simple);
    let (Some(l), Some(SyntaxElement::Token(op)), Some(r)) = (ks.first(), ks.get(1), ks.get(2))
    else {
        return Err(shape_error("BinOp"));
    };
    if tight {
        Ok(concat(vec![elem(cx, l)?, cx.token(op), elem(cx, r)?]))
    } else {
        Ok(group(concat(vec![
            elem(cx, l)?,
            brk(),
            cx.token(op),
            sp(),
            elem(cx, r)?,
        ])))
    }
}

fn is_simple_power_operand(n: &SyntaxNode) -> bool {
    match n.kind() {
        k::EXPR_NAME | k::EXPR_NUMBER_LITERAL | k::EXPR_BOOLEAN_LITERAL | k::EXPR_NONE_LITERAL => {
            true
        }
        k::EXPR_ATTRIBUTE => n
            .children()
            .next()
            .is_some_and(|c| is_simple_power_operand(&c)),
        k::EXPR_UNARY_OP => n
            .children()
            .all(|c| matches!(c.kind(), k::EXPR_NUMBER_LITERAL | k::EXPR_NAME)),
        // `a ** b ** c`: the whole chain is simple.
        k::EXPR_BIN_OP => {
            let ks: Vec<_> = n
                .children_with_tokens()
                .filter(|e| e.kind() != k::WHITESPACE && e.kind() != k::NEWLINE)
                .collect();
            ks.iter().any(|e| e.kind() == k::DOUBLE_STAR)
                && n.children().all(|c| is_simple_power_operand(&c))
        }
        _ => false,
    }
}

pub(crate) fn bool_op(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    chain(cx, &flatten_bool(cx, node), true)
}

/// The operands and operators of a boolean chain; nested chains of the other operator join it.
fn flatten_bool(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Vec<SyntaxElement> {
    let mut out = Vec::new();
    for e in kids(cx, node) {
        match &e {
            SyntaxElement::Node(n) if n.kind() == k::EXPR_BOOL_OP => {
                out.extend(flatten_bool(cx, n))
            }
            _ => out.push(e),
        }
    }
    out
}

/// `a < b`, `a not in b`, `a is not b`.
pub(crate) fn compare(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    compare_with(cx, node, true)
}

fn compare_with(cx: &FormatCtx<'_>, node: &SyntaxNode, breakable: bool) -> R {
    let mut parts = Vec::new();
    let mut in_op = false;
    for (i, e) in kids(cx, node).iter().enumerate() {
        match e {
            SyntaxElement::Node(n) => {
                parts.push(expr(cx, n)?);
                in_op = false;
            }
            SyntaxElement::Token(t) => {
                let (comment, op) = operator(cx, t);
                parts.push(comment);
                if in_op {
                    parts.push(sp());
                } else if i > 0 {
                    parts.push(if breakable { brk() } else { sp() });
                }
                parts.push(op);
                in_op = true;
            }
        }
        // A space after the last token of an operator, before the operand.
        if let SyntaxElement::Token(_) = e
            && kids(cx, node)
                .get(i + 1)
                .is_some_and(|n| n.as_node().is_some())
        {
            parts.push(sp());
        }
    }
    Ok(group(concat(parts)))
}

pub(crate) fn unary(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let ks = kids(cx, node);
    let mut parts = Vec::new();
    for e in &ks {
        // The operand that is a power is written in parentheses: `-(x**2)`.
        let power = e.as_node().map(|n| super::paren::strip(cx, n)).filter(|n| {
            n.kind() == k::EXPR_BIN_OP
                && n.children_with_tokens()
                    .any(|c| c.as_token().is_some_and(|t| t.kind() == k::DOUBLE_STAR))
                && !n
                    .descendants_with_tokens()
                    .any(|c| c.as_token().is_some_and(|t| t.kind() == k::COMMENT))
        });
        match power {
            Some(n) => parts.push(shapes::enclosed(lit("("), expr(cx, &n)?, lit(")"))),
            None => parts.push(elem(cx, e)?),
        }
        if tok_kind(e) == Some(k::NOT) {
            parts.push(sp());
        }
    }
    Ok(concat(parts))
}

/// `body if test else orelse`.
pub(crate) fn ternary(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let mut parts = Vec::new();
    ternary_parts(cx, node, &mut parts)?;
    Ok(group(concat(parts)))
}

/// The parts of a conditional; a conditional in its `else` joins the chain.
fn ternary_parts(
    cx: &FormatCtx<'_>,
    node: &SyntaxNode,
    parts: &mut Vec<Doc>,
) -> Result<(), core_format::FormatError> {
    for e in &kids(cx, node) {
        match e {
            SyntaxElement::Token(t) if matches!(t.kind(), k::IF | k::ELSE) => {
                parts.push(brk());
                parts.push(cx.token(t));
                parts.push(sp());
            }
            SyntaxElement::Node(n) if n.kind() == k::EXPR_IF => ternary_parts(cx, n, parts)?,
            _ => parts.push(elem(cx, e)?),
        }
    }
    Ok(())
}

pub(crate) fn lambda(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let mut parts = Vec::new();
    let mut broke = false;
    for e in kids(cx, node) {
        match &e {
            SyntaxElement::Token(t) if t.kind() == k::LAMBDA => {
                parts.push(cx.token(t));
                // A comment ends its line: what follows starts the next one.
                broke = has_comment(cx, t);
                if broke {
                    parts.push(hardline());
                }
            }
            SyntaxElement::Node(n) if n.kind() == k::PARAMETERS => {
                let items = super::list::lambda_parameters(cx, n)?;
                if !items.is_nil() {
                    if !broke {
                        parts.push(sp());
                    }
                    parts.push(items);
                }
            }
            SyntaxElement::Token(t) if t.kind() == k::COLON => {
                parts.push(cx.token(t));
                parts.push(if has_comment(cx, t) { hardline() } else { sp() });
            }
            _ => parts.push(elem(cx, &e)?),
        }
    }
    Ok(concat(parts))
}

/// A trailing comment ends its line: what follows starts the next one.
fn has_comment(cx: &FormatCtx<'_>, t: &core_syntax::SyntaxToken) -> bool {
    core_format::comments::has_trailing_comment(cx.grammar, t)
}

/// `target := value`.
pub(crate) fn named(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let mut parts = Vec::new();
    for e in kids(cx, node) {
        if tok_kind(&e) == Some(k::COLON_EQUAL) {
            parts.push(sp());
            parts.push(elem(cx, &e)?);
            parts.push(sp());
        } else {
            parts.push(elem(cx, &e)?);
        }
    }
    Ok(concat(parts))
}

/// `await x`, `yield`, `yield x`, `yield from x`.
pub(crate) fn prefix(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let mut parts: Vec<Doc> = Vec::new();
    for (i, e) in kids(cx, node).iter().enumerate() {
        if i > 0 {
            parts.push(lit(" "));
        }
        // `await (x)`: the parentheses go unless the operand needs them.
        if node.kind() == k::EXPR_AWAIT
            && let SyntaxElement::Node(n) = e
        {
            let inner = super::paren::strip(cx, n);
            if is_primary(inner.kind()) {
                parts.push(elem(cx, &SyntaxElement::Node(inner))?);
                continue;
            }
        }
        parts.push(elem(cx, e)?);
    }
    Ok(concat(parts))
}

/// An operand that binds tighter than `await` needs no parentheses after it.
fn is_primary(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        k::EXPR_NAME
            | k::EXPR_ATTRIBUTE
            | k::EXPR_CALL
            | k::EXPR_SUBSCRIPT
            | k::EXPR_NUMBER_LITERAL
            | k::EXPR_STRING_LITERAL
            | k::EXPR_BYTES_LITERAL
            | k::EXPR_F_STRING
            | k::EXPR_LIST
            | k::EXPR_DICT
            | k::EXPR_SET
    )
}
