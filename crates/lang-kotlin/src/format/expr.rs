//! Expressions and statements: control flow, lambdas, operators.

use super::*;
use core_format::shapes::{self, BlockStyle};

pub(super) fn rule(cx: &FormatCtx<'_>, node: &SyntaxNode, kind: SyntaxKind) -> Option<R> {
    Some(match kind {
        k::IF | k::WHILE | k::FOR => control(cx, node),
        k::DO_WHILE | k::TRY => joined(cx, node),
        k::WHEN => when(cx, node),
        k::FUNCTION_LITERAL => lambda(cx, node),
        k::BINARY_EXPRESSION => binary(cx, node),
        k::PARENTHESIZED => parenthesized(cx, node),
        _ => return None,
    })
}

/// A branch or body after the head of a statement: on the line of the head, or below it when
/// the source has it there (a block always stays on the line).
fn branch(cx: &FormatCtx<'_>, prev: &SyntaxElement, e: &SyntaxElement) -> R {
    let block = e.as_node().and_then(|n| n.first_child()).is_some_and(|c| {
        c.kind() == k::BLOCK && cx.first_token(&c).is_some_and(|t| t.kind() == k::LBRACE)
    });
    if !block && break_after(cx, prev) {
        Ok(doc::indent(doc::concat(vec![doc::hardline(), el(cx, e)?])))
    } else {
        Ok(doc::concat(vec![doc::lit(" "), el(cx, e)?]))
    }
}

/// `if (c) a else b`, `while (c) a`, `for (x in y) a`.
fn control(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let els = kids(cx, node);
    let Some(close) = els.iter().position(|e| e.kind() == k::RPAR) else {
        return generic(cx, node);
    };
    let mut parts = vec![seq(cx, &els[..=close])?];
    let mut prev = &els[close];
    let mut then_block = false;
    for e in &els[close + 1..] {
        if kw(e, "else") {
            parts.push(if then_block || !break_after(cx, prev) {
                doc::lit(" ")
            } else {
                doc::hardline()
            });
            parts.push(el(cx, e)?);
        } else {
            then_block = e
                .as_node()
                .and_then(|n| n.first_child())
                .is_some_and(|c| c.kind() == k::BLOCK);
            parts.push(branch(cx, prev, e)?);
        }
        prev = e;
    }
    Ok(doc::concat(parts))
}

/// `try {} catch () {} finally {}`, `do {} while ()`: the parts on one line.
fn joined(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let els = kids(cx, node);
    let mut parts = Vec::with_capacity(els.len() * 2);
    for (i, e) in els.iter().enumerate() {
        if i > 0 {
            parts.push(doc::lit(" "));
        }
        parts.push(el(cx, e)?);
    }
    Ok(doc::concat(parts))
}

fn when(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let els = kids(cx, node);
    let (Some(open), Some(close)) = (
        els.iter().find(|e| e.kind() == k::LBRACE),
        els.iter().rfind(|e| e.kind() == k::RBRACE),
    ) else {
        return generic(cx, node);
    };
    let (SyntaxElement::Token(open), SyntaxElement::Token(close)) = (open, close) else {
        return generic(cx, node);
    };
    let at = els.iter().position(|e| e.kind() == k::LBRACE).unwrap_or(0);
    let body = shapes::block_between(cx, node, open, close, &BlockStyle::default())?;
    Ok(doc::concat(vec![seq(cx, &els[..at])?, doc::lit(" "), body]))
}

/// `{ a, b -> body }`: the body below the head when the source spans lines.
fn lambda(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let els = kids(cx, node);
    let (Some(SyntaxElement::Token(open)), Some(SyntaxElement::Token(close))) =
        (els.first(), els.last())
    else {
        return generic(cx, node);
    };
    if open.kind() != k::LBRACE || close.kind() != k::RBRACE {
        return generic(cx, node);
    }
    let inner = &els[1..els.len() - 1];
    let Some(body) = inner.last().filter(|e| e.kind() == k::BLOCK) else {
        if inner.is_empty() {
            return super::decl::empty_braces(cx, &els).unwrap_or_else(|| generic(cx, node));
        }
        return generic(cx, node);
    };
    let head = &inner[..inner.len() - 1];
    let mut parts = vec![cx.token(open)];
    if !head.is_empty() {
        parts.push(doc::lit(" "));
        parts.push(seq(cx, head)?);
    }
    let body_doc = el(cx, body)?;
    let broken = multiline(cx, node) || body_doc.will_break();
    parts.push(doc::indent(doc::concat(vec![doc::line(), body_doc])));
    parts.push(doc::line());
    parts.push(cx.token(close));
    let lambda = doc::concat(parts);
    Ok(if broken {
        doc::group_broken(lambda)
    } else {
        doc::group(lambda)
    })
}

/// `a op b`: an operator that wraps stays at the end of its line, except `?:`, which starts the
/// next one.
fn binary(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let els = kids(cx, node);
    let [left, op, right] = &els[..] else {
        return generic(cx, node);
    };
    let elvis = first(cx, op).is_some_and(|t| t.kind() == k::ELVIS);
    let wraps = break_after(cx, left) || break_after(cx, op);
    if !wraps {
        return generic(cx, node);
    }
    let (op_doc, right_doc) = (el(cx, op)?, el(cx, right)?);
    let tail = if elvis {
        vec![doc::hardline(), op_doc, doc::lit(" "), right_doc]
    } else {
        vec![doc::lit(" "), op_doc, doc::hardline(), right_doc]
    };
    Ok(doc::concat(vec![
        el(cx, left)?,
        doc::indent(doc::concat(tail)),
    ]))
}

/// `(a)`: a break after the parenthesis puts the content one level below.
fn parenthesized(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let els = kids(cx, node);
    let [open, inner, close] = &els[..] else {
        return generic(cx, node);
    };
    if !break_after(cx, open) {
        return generic(cx, node);
    }
    Ok(doc::concat(vec![
        el(cx, open)?,
        doc::indent(doc::concat(vec![doc::hardline(), el(cx, inner)?])),
        doc::hardline(),
        el(cx, close)?,
    ]))
}
