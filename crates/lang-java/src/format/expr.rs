//! Expressions: operator chains, method chains, lambdas, assignments and initializers.

use super::*;
use core_format::FormatError;
use core_format::shapes::{self, ChainPolicy};

pub(super) fn rule(
    cx: &FormatCtx<'_>,
    node: &SyntaxNode,
    kind: core_syntax::SyntaxKind,
) -> Option<R> {
    Some(match kind {
        k::BINARY_EXPRESSION => binary(cx, node),
        k::TERNARY_EXPRESSION => ternary(cx, node),
        k::METHOD_INVOCATION => chain(cx, node),
        k::LAMBDA_EXPRESSION => lambda(cx, node),
        k::ASSIGNMENT_EXPRESSION | k::ELEMENT_VALUE_PAIR => assignment(cx, node),
        k::UNARY_EXPRESSION | k::UPDATE_EXPRESSION => tight(cx, node),
        k::CAST_EXPRESSION => cast(cx, node),
        k::ARRAY_INITIALIZER | k::ELEMENT_VALUE_ARRAY_INITIALIZER => initializer(cx, node),
        _ => return None,
    })
}

/// Operands that are tokens print with the comment rules of the language.
struct BinaryHooks;

impl shapes::BinaryHooks for BinaryHooks {
    fn token(&self, cx: &FormatCtx<'_>, t: &SyntaxToken) -> Doc {
        tok(cx, t)
    }

    fn comment_ends_line(&self, cx: &FormatCtx<'_>, t: &SyntaxToken) -> bool {
        line_comment_after(cx, t)
    }
}

fn binary(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    shapes::binary_chain(
        cx,
        node,
        &shapes::BinaryPolicy::of(cx.options),
        &BinaryHooks,
    )
}

/// Tokens and comments print with the rules of the language.
struct TernaryHooks;

impl shapes::ConditionalHooks for TernaryHooks {
    fn token(&self, cx: &FormatCtx<'_>, t: &SyntaxToken) -> Doc {
        tok(cx, t)
    }

    fn comment_ends_line(&self, cx: &FormatCtx<'_>, t: &SyntaxToken) -> bool {
        line_comment_after(cx, t)
    }
}

fn ternary(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    match shapes::conditional(
        cx,
        node,
        &shapes::ConditionalPolicy::of(cx.options),
        &TernaryHooks,
    ) {
        Err(FormatError::Shape(_)) => generic(cx, node),
        r => r,
    }
}

fn assign_op(e: &SyntaxElement) -> bool {
    matches!(e, SyntaxElement::Token(t)
        if t.text().ends_with('=') && !matches!(t.text(), "==" | "!=" | "<=" | ">="))
}

/// An array initializer or the value of an assignment that is a lambda stays on the operator's
/// line.
struct AssignHooks;

impl shapes::AssignHooks for AssignHooks {
    fn layout(
        &self,
        _cx: &FormatCtx<'_>,
        a: &shapes::Assign<'_>,
        right: &SyntaxElement,
        _policy: &shapes::AssignPolicy,
    ) -> Option<shapes::AssignLayout> {
        // (the lambda that is the value of a declarator moves below the `=`)
        (right.kind() == k::ARRAY_INITIALIZER
            || (right.kind() == k::LAMBDA_EXPRESSION && a.node.kind() != k::VARIABLE_DECLARATOR))
            .then_some(shapes::AssignLayout::NeverBreakAfterOperator)
    }

    fn right_doc(
        &self,
        cx: &FormatCtx<'_>,
        right: &SyntaxElement,
        _layout: shapes::AssignLayout,
    ) -> R {
        el(cx, right)
    }
}

/// `lhs = rhs` (`els`: the elements of the node, `eq` the operator's position): the right side
/// moves to the next line when it does not fit.
pub(super) fn assign_at(
    cx: &FormatCtx<'_>,
    node: &SyntaxNode,
    els: &[SyntaxElement],
    eq: usize,
) -> R {
    let [right] = &els[eq + 1..] else {
        return generic(cx, node);
    };
    shapes::assignment(
        cx,
        shapes::Assign {
            node,
            left: seq(cx, &els[..=eq])?,
            op: Doc::Nil,
            op_token: None,
            right: Some(right),
            short_key: false,
        },
        &shapes::AssignPolicy::of(cx.options),
        &AssignHooks,
    )
}

fn assignment(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let els = kids(cx, node);
    match els.iter().position(assign_op) {
        Some(eq) => assign_at(cx, node, &els, eq),
        None => generic(cx, node),
    }
}

fn lambda(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let els = kids(cx, node);
    let Some(arrow) = els.iter().position(|e| e.kind() == k::ARROW) else {
        return generic(cx, node);
    };
    let head = seq(cx, &els[..=arrow])?;
    let body = &els[arrow + 1..];
    if body.first().is_some_and(|b| b.kind() == k::BLOCK) {
        return Ok(doc::concat(vec![head, doc::lit(" "), seq(cx, body)?]));
    }
    let sep = if matches!(gap_after(cx, &els[arrow]), Doc::Nil | Doc::Text(_)) {
        doc::line()
    } else {
        doc::hardline()
    };
    Ok(doc::group(doc::concat(vec![
        head,
        cont(doc::group(doc::concat(vec![sep, seq(cx, body)?]))),
    ])))
}

/// Prefix and postfix operators stick to their operand.
fn tight(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let els = kids(cx, node);
    let mut parts = Vec::new();
    let mut prev: Option<SyntaxToken> = None;
    for e in &els {
        if let (Some(a), Some(b)) = (&prev, first(cx, e)) {
            let (x, y) = (a.text(), b.text());
            if (x.ends_with('-') && y.starts_with('-')) || (x.ends_with('+') && y.starts_with('+'))
            {
                parts.push(doc::lit(" "));
            }
        }
        parts.push(el(cx, e)?);
        prev = last(cx, e);
    }
    Ok(doc::concat(parts))
}

/// `(Type) expr`: a long operand moves below the cast.
fn cast(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let els = kids(cx, node);
    let Some(close) = els.iter().position(|e| is_tok(e, ")")) else {
        return generic(cx, node);
    };
    if close + 2 != els.len() {
        return generic(cx, node);
    }
    Ok(doc::group(doc::concat(vec![
        seq(cx, &els[..=close])?,
        cont(doc::concat(vec![doc::line(), el(cx, &els[close + 1])?])),
    ])))
}

/// `{a, b}`: on one line, else after the `{` one per line, or filled when the elements are
/// literals or arrays. A trailing comma of the source stays and breaks the braces.
fn initializer(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let els = kids(cx, node);
    let (Some(open), Some(close)) = (els.first(), els.last()) else {
        return generic(cx, node);
    };
    let inner = &els[1..els.len() - 1];
    let items: Vec<_> = inner.iter().filter(|e| !is_tok(e, ",")).collect();
    if items.is_empty() {
        return seq(cx, &els);
    }
    let trailing = inner.last().is_some_and(|e| is_tok(e, ","));
    let filled = items
        .iter()
        .all(|e| matches!(e.kind(), k::CHAR | k::NUMBER | k::ARRAY_INITIALIZER));
    let commas: Vec<_> = inner.iter().filter(|e| is_tok(e, ",")).collect();
    let mut parts = Vec::new();
    for (i, e) in items.iter().enumerate() {
        let comma = match commas.get(i) {
            Some(c) => el(cx, c)?,
            None => doc::nil(),
        };
        parts.push(doc::concat(vec![el(cx, e)?, comma]));
        if i + 1 < items.len() {
            let broken = commas
                .get(i)
                .and_then(|c| c.as_token())
                .is_some_and(|c| line_comment_after(cx, c));
            parts.push(if broken { doc::hardline() } else { doc::line() });
        }
    }
    let body = if filled {
        doc::fill(parts)
    } else {
        doc::concat(parts)
    };
    let mut out = vec![
        el(cx, open)?,
        doc::indent(doc::concat(vec![doc::softline(), body])),
        doc::softline(),
        el(cx, close)?,
    ];
    if trailing {
        out.push(doc::break_parent());
    }
    Ok(doc::group(doc::concat(out)))
}

/// `a.b().c()`: the shared member chain (`core_format::shapes::call_chain`) under the
/// `member_chain_*` options; the arguments in front of the first break go one level deeper.
fn chain(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    struct Hooks<'a, 'b>(&'a FormatCtx<'b>);
    impl shapes::ChainHooks for Hooks<'_, '_> {
        fn element(&self, e: &SyntaxElement, in_head: bool) -> Option<R> {
            Some(match e.as_node() {
                Some(n) if in_head && n.kind() == k::ARGUMENT_LIST => {
                    decl::paren_list_extra(self.0, n)
                }
                _ => el(self.0, e),
            })
        }
    }
    shapes::call_chain(cx, node, &ChainPolicy::of(cx.options), &Hooks(cx))
}
