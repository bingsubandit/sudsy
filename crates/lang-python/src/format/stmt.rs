//! Simple statements: `return`, assignments, `raise`, `assert`, `del`, `global`, imports.
//!
//! concept: assignment-layout

use super::expr::{self, expr as expr_doc};
use super::list;
use super::paren;
use super::util::*;
use crate::format::keep_statement_parentheses;
use crate::generated::kind as k;
use core_format::FormatCtx;
use core_format::doc::{Doc, concat, lit};
use core_format::shapes;
use core_syntax::{SyntaxElement, SyntaxKind, SyntaxNode};

pub(crate) fn statement(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    match node.kind() {
        k::STMT_PASS | k::STMT_BREAK | k::STMT_CONTINUE | k::STMT_IPY_ESCAPE_COMMAND => Ok(concat(
            kids(cx, node)
                .iter()
                .map(|e| expr::elem(cx, e))
                .collect::<Result<_, _>>()?,
        )),
        k::STMT_EXPR => {
            let e = node
                .children()
                .next()
                .ok_or_else(|| shape_error("StmtExpr"))?;
            let inner = paren::strip(cx, &e);
            if e.kind() == k::PARENTHESIZED
                && inner.kind() != k::PARENTHESIZED
                && keep_statement_parentheses(cx.options)
            {
                // (a notebook cell shows the value of its last expression: the parentheses stay)
                let d = with_fluent(false, || with_open(true, || expr_doc(cx, &inner)))?;
                return Ok(shapes::enclosed(lit("("), d, lit(")")));
            }
            with_fluent(false, || paren::maybe_parenthesize(cx, &e))
        }
        k::STMT_RETURN => keyword_value(cx, node, k::RETURN),
        k::STMT_ASSIGN => assign(cx, node),
        k::STMT_AUG_ASSIGN => assign(cx, node),
        k::STMT_ANN_ASSIGN => ann_assign(cx, node),
        k::STMT_RAISE => raise(cx, node),
        k::STMT_ASSERT => assert(cx, node),
        k::STMT_DELETE => delete(cx, node),
        k::STMT_GLOBAL | k::STMT_NONLOCAL => global(cx, node),
        k::STMT_IMPORT => import(cx, node),
        k::STMT_IMPORT_FROM => import_from(cx, node),
        k::STMT_TYPE_ALIAS => type_alias(cx, node),
        _ => super::compound::compound(cx, node),
    }
}

/// `return value`, a keyword and an optional expression.
fn keyword_value(cx: &FormatCtx<'_>, node: &SyntaxNode, kw: SyntaxKind) -> R {
    let mut parts = Vec::new();
    for e in kids(cx, node) {
        match &e {
            SyntaxElement::Token(t) if t.kind() == kw => parts.push(cx.token(t)),
            SyntaxElement::Node(n) => {
                parts.push(sp());
                parts.push(paren::maybe_parenthesize(cx, n)?);
            }
            SyntaxElement::Token(t) => parts.push(cx.token(t)),
        }
    }
    Ok(concat(parts))
}

/// `a = b = value` and `a += value`: targets, operators, then the value.
fn assign(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let ks = kids(cx, node);
    let mut parts = Vec::new();
    for (i, e) in ks.iter().enumerate() {
        match e {
            SyntaxElement::Token(t) => {
                parts.push(sp());
                parts.push(cx.token(t));
                parts.push(sp());
            }
            SyntaxElement::Node(n) if i + 1 == ks.len() => {
                parts.push(paren::rhs(cx, n, prefix_width(&parts))?);
            }
            SyntaxElement::Node(n) => parts.push(paren::target(cx, n)?),
        }
    }
    Ok(concat(parts))
}

/// The flat width of what stands before an assignment's value (a forced break: no room).
fn prefix_width(parts: &[Doc]) -> usize {
    parts
        .iter()
        .try_fold(0, |n, d| d.flat_width().map(|w| n + w))
        .unwrap_or(1000)
}

/// `target: annotation = value`.
fn ann_assign(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let ks = kids(cx, node);
    let mut parts = Vec::new();
    let mut nodes = 0;
    for e in &ks {
        match e {
            SyntaxElement::Token(t) if t.kind() == k::COLON => {
                parts.push(cx.token(t));
                parts.push(sp());
            }
            SyntaxElement::Token(t) => {
                parts.push(sp());
                parts.push(cx.token(t));
                parts.push(sp());
            }
            SyntaxElement::Node(n) => {
                nodes += 1;
                parts.push(match nodes {
                    1 => expr_doc(cx, n)?,
                    // (an `await` in an annotation keeps its parentheses)
                    2 if paren::strip(cx, n).kind() == k::EXPR_AWAIT => expr_doc(cx, n)?,
                    2 => paren::maybe_parenthesize(cx, n)?,
                    _ => paren::rhs(cx, n, prefix_width(&parts))?,
                });
            }
        }
    }
    Ok(concat(parts))
}

/// `raise exc from cause`.
fn raise(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let mut parts = Vec::new();
    for e in kids(cx, node) {
        match &e {
            SyntaxElement::Token(t) => {
                if t.kind() != k::RAISE {
                    parts.push(sp());
                }
                parts.push(cx.token(t));
            }
            SyntaxElement::Node(n) => {
                parts.push(sp());
                parts.push(paren::maybe_parenthesize(cx, n)?);
            }
        }
    }
    Ok(concat(parts))
}

/// `assert test, msg`.
fn assert(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let mut parts = Vec::new();
    for e in kids(cx, node) {
        match &e {
            SyntaxElement::Token(t) if t.kind() == k::COMMA => {
                parts.push(cx.token(t));
            }
            SyntaxElement::Token(t) => parts.push(cx.token(t)),
            SyntaxElement::Node(n) => {
                parts.push(sp());
                // The message follows the test on its line, like the value of an assignment.
                let d = if parts.len() > 3 {
                    paren::rhs(cx, n, prefix_width(&parts))?
                } else {
                    paren::maybe_parenthesize(cx, n)?
                };
                parts.push(d);
            }
        }
    }
    Ok(concat(parts))
}

/// `del a, b`: the targets are one tuple (the adapter makes it), parenthesized when it does not fit.
fn delete(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let mut parts = Vec::new();
    for e in kids(cx, node) {
        match e {
            SyntaxElement::Token(t) if t.kind() == k::DEL => parts.push(cx.token(&t)),
            SyntaxElement::Node(n) => {
                parts.push(sp());
                parts.push(paren::maybe_parenthesize(cx, &n)?);
            }
            SyntaxElement::Token(_) => {}
        }
    }
    Ok(concat(parts))
}

/// `global a, b`.
fn global(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let mut parts = Vec::new();
    for (i, e) in kids(cx, node).iter().enumerate() {
        if i > 0 && tok_kind(e) != Some(k::COMMA) {
            parts.push(sp());
        }
        parts.push(expr::elem(cx, e)?);
    }
    Ok(concat(parts))
}

/// `import a, b as c`.
fn import(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    global(cx, node)
}

/// `from .x import a, b`, the names in parentheses when they do not fit.
fn import_from(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let ks = kids(cx, node);
    let mut head = Vec::new();
    let mut names: Vec<SyntaxElement> = Vec::new();
    let mut seen_import = false;
    let (mut open, mut close) = (None, None);
    for e in &ks {
        if seen_import {
            match tok_kind(e) {
                Some(k::LPAR) => open = e.as_token().cloned(),
                Some(k::RPAR) => close = e.as_token().cloned(),
                _ => names.push(e.clone()),
            }
            continue;
        }
        match e {
            SyntaxElement::Token(t) if t.kind() == k::IMPORT => {
                seen_import = true;
                head.push(sp());
                head.push(cx.token(t));
                head.push(sp());
            }
            SyntaxElement::Token(t) if t.kind() == k::FROM => {
                head.push(cx.token(t));
                head.push(sp());
            }
            _ => head.push(expr::elem(cx, e)?),
        }
    }
    let p = list::from_elements(open, close, names);
    let star = p.runs.len() == 1
        && p.runs[0].len() == 1
        && match &p.runs[0][0] {
            SyntaxElement::Node(n) => n.text().to_string().trim() == "*",
            SyntaxElement::Token(t) => t.kind() == k::STAR,
        };
    let list = if star {
        concat(vec![expr::elem(cx, &p.runs[0][0])?])
    } else {
        list::import_names(cx, &p)?
    };
    head.push(list);
    Ok(concat(head))
}

/// `type X[T] = value`.
fn type_alias(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let ks = kids(cx, node);
    let mut parts = Vec::new();
    let last = ks.len().saturating_sub(1);
    for (i, e) in ks.iter().enumerate() {
        match e {
            SyntaxElement::Token(t) if t.kind() == k::TYPE => {
                parts.push(cx.token(t));
                parts.push(sp());
            }
            SyntaxElement::Token(t) => {
                parts.push(sp());
                parts.push(cx.token(t));
                parts.push(sp());
            }
            SyntaxElement::Node(n) if i == last => {
                parts.push(paren::rhs(cx, n, prefix_width(&parts))?)
            }
            SyntaxElement::Node(n) => parts.push(expr_doc(cx, n)?),
        }
    }
    Ok(concat(parts))
}
