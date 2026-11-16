//! Statements: blocks, the control statements and `switch`.

use super::*;

pub(super) fn rule(
    cx: &FormatCtx<'_>,
    node: &SyntaxNode,
    kind: core_syntax::SyntaxKind,
) -> Option<R> {
    Some(match kind {
        k::BLOCK | k::CONSTRUCTOR_BODY | k::SWITCH_STATEMENT_BLOCK | k::SWITCH_EXPRESSION_BLOCK => {
            block(cx, node)
        }
        k::IF_STATEMENT
        | k::WHILE_STATEMENT
        | k::FOR_STATEMENT
        | k::ENHANCED_FOR_STATEMENT
        | k::DO_STATEMENT
        | k::LABELED_STATEMENT
        | k::SYNCHRONIZED_STATEMENT => control(cx, node),
        k::SWITCH_BLOCK_STATEMENT_GROUP => switch_group(cx, node),
        k::FOR_SPEC => for_spec(cx, node),
        k::SWITCH_STATEMENT_RULE | k::SWITCH_EXPRESSION_RULE => switch_rule(cx, node),
        _ => return None,
    })
}

fn is_statement(kind: core_syntax::SyntaxKind) -> bool {
    matches!(
        kind,
        k::BLOCK
            | k::IF_STATEMENT
            | k::WHILE_STATEMENT
            | k::FOR_STATEMENT
            | k::ENHANCED_FOR_STATEMENT
            | k::DO_STATEMENT
            | k::LABELED_STATEMENT
            | k::SYNCHRONIZED_STATEMENT
            | k::EXPRESSION_STATEMENT
            | k::RETURN_STATEMENT
            | k::THROW_STATEMENT
            | k::BREAK_STATEMENT
            | k::CONTINUE_STATEMENT
            | k::YIELD_STATEMENT
            | k::ASSERT_STATEMENT
            | k::SWITCH_STATEMENT
            | k::TRY_STATEMENT
            | k::TRY_WITH_RESOURCES_STATEMENT
            | k::LOCAL_VARIABLE_DECLARATION
            | k::SEMI
    )
}

fn block(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let els = kids(cx, node);
    let (Some(SyntaxElement::Token(open)), Some(SyntaxElement::Token(close))) =
        (els.first().cloned(), els.last().cloned())
    else {
        return generic(cx, node);
    };
    let items: Vec<_> = els[1..els.len() - 1]
        .iter()
        .filter(|e| !is_tok(e, ";"))
        .cloned()
        .collect();
    if items.is_empty() {
        if keeps_lines(node) && comments::leading_comments(cx.grammar, &close).is_empty() {
            return Ok(doc::concat(vec![
                cx.token(&open),
                doc::hardline(),
                cx.token(&close),
            ]));
        }
        return braces(cx, &open, &close, None);
    }
    let body = lines(cx, &items, |_, _| false)?;
    braces(
        cx,
        &open,
        &close,
        Some(decl::lead_blank(cx, &items[0], body)),
    )
}

/// An empty block that is part of a longer statement (`catch`, `finally`, `if` with an `else`,
/// `try` with a `catch`) keeps its `}` on a line of its own.
fn keeps_lines(block: &SyntaxNode) -> bool {
    let Some(parent) = block.parent() else {
        return false;
    };
    let has = |kind| parent.children().any(|c| c.kind() == kind);
    match parent.kind() {
        k::CATCH_CLAUSE | k::FINALLY_CLAUSE => true,
        k::TRY_STATEMENT | k::TRY_WITH_RESOURCES_STATEMENT => {
            has(k::CATCH_CLAUSE) || has(k::FINALLY_CLAUSE)
        }
        k::IF_STATEMENT => parent
            .children_with_tokens()
            .any(|c| c.as_token().is_some_and(|t| t.text() == "else")),
        _ => false,
    }
}

/// `if (...) body else body`, loops and the like: a block body follows on the line, any other
/// statement on the next line when it does not fit.
fn control(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let els = flat(cx, node);
    let mut parts = Vec::new();
    let mut prev: Option<SyntaxToken> = None;
    let mut after_block = false;
    let mut head = true;
    for (i, e) in els.iter().enumerate() {
        let before = i.checked_sub(1).map(|j| &els[j]);
        let is_body = is_statement(e.kind())
            && !(node.kind() == k::DO_STATEMENT && is_tok(e, ";"))
            && before.is_some_and(|b| {
                matches!(b.kind(), k::PARENTHESIZED_EXPRESSION | k::FOR_SPEC)
                    || is_tok(b, "else")
                    || is_tok(b, "do")
                    || is_tok(b, ":")
            });
        let trailing_kw =
            (is_tok(e, "else") || (is_tok(e, "while") && node.kind() == k::DO_STATEMENT)) && !head;
        if is_body {
            if e.kind() == k::BLOCK
                || (e.kind() == k::IF_STATEMENT && before.is_some_and(|b| is_tok(b, "else")))
            {
                parts.push(before.map_or_else(|| doc::lit(" "), |b| gap_after(cx, b)));
                parts.push(el(cx, e)?);
            } else {
                parts.push(doc::group(doc::indent(doc::concat(vec![
                    doc::line(),
                    el(cx, e)?,
                ]))));
            }
            after_block = e.kind() == k::BLOCK;
        } else if trailing_kw {
            parts.push(if after_block {
                doc::lit(" ")
            } else {
                doc::hardline()
            });
            parts.push(el(cx, e)?);
        } else {
            if let (Some(a), Some(b)) = (&prev, first(cx, e))
                && space::between(a, &b)
            {
                parts.push(gap(cx, a));
            }
            parts.push(el(cx, e)?);
        }
        if i == 0 {
            head = false;
        }
        prev = last(cx, e);
    }
    Ok(doc::concat(parts))
}

/// `case A:` labels on lines of their own, the statements indented below.
fn switch_group(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let els = flat(cx, node);
    let split = els
        .iter()
        .rposition(|e| is_tok(e, ":"))
        .map_or(0, |i| i + 1);
    let mut parts = Vec::new();
    let mut i = 0;
    while i < split {
        if i > 0 {
            parts.push(doc::hardline());
        }
        let mut label = vec![els[i].clone()];
        if els.get(i + 1).is_some_and(|c| is_tok(c, ":")) {
            label.push(els[i + 1].clone());
            i += 1;
        }
        parts.push(seq(cx, &label)?);
        i += 1;
    }
    if split < els.len() {
        parts.push(doc::indent(doc::concat(vec![
            doc::hardline(),
            lines(cx, &els[split..], |_, _| false)?,
        ])));
    }
    let d = doc::concat(parts);
    Ok(d)
}

/// `for (init; cond; update)` breaks before the condition and the update; `for (x : xs)` after
/// the colon.
fn for_spec(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let els = kids(cx, node);
    let (Some(open), Some(close)) = (els.first(), els.last()) else {
        return generic(cx, node);
    };
    let inner = &els[1..els.len() - 1];
    if let Some(c) = inner.iter().position(|e| is_tok(e, ":")) {
        return Ok(doc::group(doc::concat(vec![
            el(cx, open)?,
            seq(cx, &inner[..c])?,
            doc::lit(" :"),
            cont(doc::concat(vec![doc::line(), seq(cx, &inner[c + 1..])?])),
            el(cx, close)?,
        ])));
    }
    let [init, s1, cond, s2, update] = inner else {
        return generic(cx, node);
    };
    Ok(doc::group(doc::concat(vec![
        el(cx, open)?,
        el(cx, init)?,
        el(cx, s1)?,
        cont(doc::concat(vec![
            doc::line(),
            el(cx, cond)?,
            el(cx, s2)?,
            doc::line(),
            el(cx, update)?,
        ])),
        el(cx, close)?,
    ])))
}

/// `case X -> body`: an expression body moves to the next line when it does not fit.
fn switch_rule(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let els = kids(cx, node);
    let Some(arrow) = els.iter().position(|e| e.kind() == k::ARROW) else {
        return generic(cx, node);
    };
    let body = &els[arrow + 1..];
    if body.len() != 1 || body[0].kind() == k::BLOCK {
        return generic(cx, node);
    }
    Ok(doc::group(doc::concat(vec![
        seq(cx, &els[..=arrow])?,
        cont(doc::concat(vec![doc::line(), el(cx, &body[0])?])),
    ])))
}
