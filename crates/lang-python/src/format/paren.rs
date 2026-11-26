//! Where parentheses appear around a statement's expression: ruff's `maybe_parenthesize`
//! (redundant ones go, missing ones come back when the expression does not fit).
//!
//! concept: maybe-parenthesize, parenthesized-expression

use super::block::depth;
use super::util::*;
use super::{expr, list, op};
use crate::generated::kind as k;
use core_format::FormatCtx;
use core_format::doc::{Doc, group, if_break, lit};
use core_format::shapes;
use core_syntax::SyntaxNode;

/// `(`, the content one level deeper, `)`: the parentheses show only when the content does not
/// fit on the line.
pub(crate) fn parenthesize_if_expands(d: Doc) -> Doc {
    shapes::maybe_parenthesize(d)
}

/// The expression below the redundant parentheses of `e` (those without comments).
pub(crate) fn strip(cx: &FormatCtx<'_>, e: &SyntaxNode) -> SyntaxNode {
    strip_with(cx, e, false)
}

/// `named`: the parentheses of an assignment expression go too (an `if` or `while` test).
fn strip_with(cx: &FormatCtx<'_>, e: &SyntaxNode, named: bool) -> SyntaxNode {
    strip_all(cx, e, named, false)
}

fn strip_all(cx: &FormatCtx<'_>, e: &SyntaxNode, named: bool, yields: bool) -> SyntaxNode {
    let mut cur = e.clone();
    while cur.kind() == k::PARENTHESIZED {
        let ks = kids(cx, &cur);
        let eol = ends_line(cx, &cur);
        let plain = ks.iter().all(|x| match x {
            core_syntax::SyntaxElement::Token(t) => {
                !has_comments(cx, t)
                    || eol
                        && Some(t) == ks.last().and_then(|l| l.as_token())
                        && core_format::comments::leading_comments(cx.grammar, t).is_empty()
            }
            _ => true,
        });
        let inner = cur.children().next();
        let edge_comments = inner.as_ref().is_some_and(|n| {
            cx.first_token(n).is_some_and(|t| {
                !core_format::comments::leading_comments(cx.grammar, &t).is_empty()
            }) || cx.last_token(n).is_some_and(|t| {
                !core_format::comments::trailing_comments(cx.grammar, &t).is_empty()
            })
        });
        match inner {
            Some(n)
                if plain
                    && !edge_comments
                    && !(n.kind() == k::EXPR_NAMED && !named)
                    && !(n.kind() == k::EXPR_LAMBDA
                        && n.descendants_with_tokens().any(|e| e.kind() == k::COMMENT))
                    && !(n.kind() == k::EXPR_STARRED
                        || !yields && matches!(n.kind(), k::EXPR_YIELD | k::EXPR_YIELD_FROM)) =>
            {
                cur = n
            }
            _ => break,
        }
    }
    cur
}

pub(crate) fn maybe_parenthesize(cx: &FormatCtx<'_>, e: &SyntaxNode) -> R {
    with_open(false, || maybe_parenthesize_inner(cx, e, 0, false))
}

/// [`maybe_parenthesize`] for the test of an `if`, `elif`, `while` or `case` guard.
pub(crate) fn maybe_parenthesize_test(cx: &FormatCtx<'_>, e: &SyntaxNode) -> R {
    let inner = strip_with(cx, e, true);
    if inner.kind() == k::EXPR_NAMED {
        return expr::expr(cx, &inner);
    }
    maybe_parenthesize(cx, e)
}

/// The expression inside parentheses that show when it does not fit: operators may break.
fn expanded(f: impl FnOnce() -> R) -> R {
    Ok(parenthesize_if_expands(with_open(true, f)?))
}

/// `prefix` is the width of what stands before the expression on its line; `assignment`: the
/// expression is the value of an assignment, where a call or subscript that fits in parentheses
/// is parenthesized rather than split at its brackets.
fn maybe_parenthesize_inner(
    cx: &FormatCtx<'_>,
    e: &SyntaxNode,
    prefix: usize,
    assignment: bool,
) -> R {
    let inner = strip_all(cx, e, false, assignment);
    if inner.kind() == k::PARENTHESIZED {
        return expr::expr(cx, &inner);
    }
    match inner.kind() {
        k::EXPR_TUPLE if list::is_bare(cx, &inner) && list::is_one_item_tuple(cx, &inner) => {
            list::bare_tuple(cx, &inner)
        }
        k::EXPR_TUPLE if list::is_bare(cx, &inner) => {
            if can_omit(cx, &inner) {
                list::bare_tuple_flat(cx, &inner)
            } else {
                expanded(|| list::bare_tuple_ungrouped(cx, &inner))
            }
        }
        k::EXPR_ATTRIBUTE | k::EXPR_UNARY_OP if is_plain(&inner) => best_fit(cx, &inner, prefix, e),
        k::EXPR_BIN_OP | k::EXPR_BOOL_OP | k::EXPR_COMPARE | k::EXPR_UNARY_OP => {
            if can_omit(cx, &inner) {
                // The brackets split instead of the operators when the line up to the first one fits
                //.
                let d = op::binary_like(cx, &inner, false)?;
                if shapes::head_fits(
                    cx,
                    e,
                    prefix,
                    &d,
                    depth() * style(cx).indent,
                    style(cx).width,
                ) {
                    Ok(d)
                } else {
                    expanded(|| expr::expr(cx, &inner))
                }
            } else {
                expanded(|| expr::expr(cx, &inner))
            }
        }
        k::EXPR_AWAIT if can_omit(cx, &inner) => expr::expr(cx, &inner),
        k::EXPR_IF | k::EXPR_AWAIT => expanded(|| expr::expr(cx, &inner)),
        k::EXPR_ATTRIBUTE | k::EXPR_CALL | k::EXPR_SUBSCRIPT
            if expr::is_fluent(cx, &inner) || expr::chain_has_comments(cx, &inner) =>
        {
            expanded(|| expr::expr(cx, &inner))
        }
        k::EXPR_ATTRIBUTE | k::EXPR_CALL | k::EXPR_SUBSCRIPT
            if implicit_concat_head(&inner).is_some() =>
        {
            let d = with_open(true, || expr::expr(cx, &inner))?;
            // The parts join when the line up to the first bracket holds them.
            let joins =
                implicit_concat_head(&inner).is_some_and(|h| expr::joined(cx, &h).is_some());
            if joins && depth() * style(cx).indent + prefix + d.head_width() <= style(cx).width {
                Ok(d)
            } else {
                Ok(parenthesize_if_expands(d))
            }
        }
        k::EXPR_ATTRIBUTE | k::EXPR_CALL | k::EXPR_SUBSCRIPT => {
            if assignment && starts_with_atom(&inner) {
                // Split at the brackets when the line up to the first one fits.
                let d = with_open(true, || expr::expr(cx, &inner))?;
                let s = style(cx);
                let head = d.head_width();
                if depth() * s.indent + prefix + head <= s.width {
                    Ok(d)
                } else if depth() * s.indent + prefix < s.width
                    && (depth() + 1) * s.indent + head <= s.width
                {
                    // `target = (`, the value split inside the parentheses.
                    Ok(parenthesize_if_expands(d))
                } else {
                    Ok(d)
                }
            } else {
                expr::expr(cx, &inner)
            }
        }
        k::EXPR_NAME
        | k::EXPR_NUMBER_LITERAL
        | k::EXPR_STRING_LITERAL
        | k::EXPR_BYTES_LITERAL
        | k::EXPR_F_STRING
        | k::EXPR_T_STRING
        | k::EXPR_BOOLEAN_LITERAL
        | k::EXPR_NONE_LITERAL => best_fit(cx, &inner, prefix, e),
        _ => expr::expr(cx, &inner),
    }
}

/// Parentheses that end the logical line: a comment after the closing one is the line's comment.
fn ends_line(cx: &FormatCtx<'_>, paren: &SyntaxNode) -> bool {
    let joins = paren.children().next().is_some_and(|n| {
        n.kind() != k::PARENTHESIZED
            && n.children().count() > 1
            && expr::joined_with(cx, &n, true).is_some()
    });
    (joins
        && cx
            .last_token(paren)
            .is_some_and(|t| ends_logical_line(cx, &t)))
        || shapes::ends_statement_line(cx, paren)
}

/// Is `tok` the last token of its (innermost) statement? A comment after it is the statement's.
pub(crate) fn ends_logical_line(cx: &FormatCtx<'_>, tok: &core_syntax::SyntaxToken) -> bool {
    tok.parent_ancestors()
        .find(|a| cx.grammar.name(a.kind()).starts_with("Stmt"))
        .is_some_and(|stmt| cx.last_token(&stmt).as_ref() == Some(tok))
}

/// The head of a chain of attributes, calls and subscripts when it is several strings in a row.
fn implicit_concat_head(n: &SyntaxNode) -> Option<SyntaxNode> {
    let mut cur = n.clone();
    while matches!(
        cur.kind(),
        k::EXPR_CALL | k::EXPR_ATTRIBUTE | k::EXPR_SUBSCRIPT
    ) {
        cur = cur.children().next()?;
    }
    (matches!(
        cur.kind(),
        k::EXPR_STRING_LITERAL | k::EXPR_BYTES_LITERAL | k::EXPR_F_STRING | k::EXPR_T_STRING
    ) && cur.children().count() > 1)
        .then_some(cur)
}

/// Is the head of a chain of attributes, calls and subscripts a name or a literal?
fn starts_with_atom(n: &SyntaxNode) -> bool {
    let mut cur = n.clone();
    while matches!(
        cur.kind(),
        k::EXPR_CALL | k::EXPR_ATTRIBUTE | k::EXPR_SUBSCRIPT
    ) {
        match cur.children().next() {
            Some(c) => cur = c,
            None => return false,
        }
    }
    matches!(
        cur.kind(),
        k::EXPR_NAME | k::EXPR_STRING_LITERAL | k::EXPR_NUMBER_LITERAL | k::EXPR_F_STRING
    )
}

/// Names, numbers and constants joined by `.` and unary operators: no place to break inside.
fn is_plain(n: &SyntaxNode) -> bool {
    let comment = n
        .descendants_with_tokens()
        .any(|e| e.as_token().is_some_and(|t| t.kind() == k::COMMENT));
    !comment
        && n.descendants().all(|d| {
            matches!(
                d.kind(),
                k::EXPR_ATTRIBUTE
                    | k::EXPR_UNARY_OP
                    | k::EXPR_NAME
                    | k::IDENTIFIER
                    | k::EXPR_NUMBER_LITERAL
                    | k::EXPR_BOOLEAN_LITERAL
                    | k::EXPR_NONE_LITERAL
            )
        })
}

/// Does a comment (not a tool's pragma) follow the expression at the end of its line?
fn has_line_comment(cx: &FormatCtx<'_>, e: &SyntaxNode) -> bool {
    cx.last_token(e).is_some_and(|last| {
        // (the colon of a header carries the comment after it)
        let colon = std::iter::successors(last.next_token(), |n| n.next_token())
            .find(|n| !cx.grammar.is_trivia(n.kind()))
            .filter(|n| n.kind() == k::COLON);
        (colon.is_some() || ends_logical_line(cx, &last))
            && core_format::comments::trailing_comments(cx.grammar, &colon.unwrap_or(last))
                .iter()
                .any(|c| !is_pragma(&c.text()))
    })
}

/// Parentheses only when the expression fits on a line of its own inside them and the line
/// before it fits with the opening one.
fn best_fit(cx: &FormatCtx<'_>, inner: &SyntaxNode, prefix: usize, outer: &SyntaxNode) -> R {
    let d = with_open(true, || expr::expr(cx, inner))?;
    let s = style(cx);
    let strings = matches!(
        inner.kind(),
        k::EXPR_STRING_LITERAL | k::EXPR_BYTES_LITERAL | k::EXPR_F_STRING | k::EXPR_T_STRING
    );
    if strings && inner.children().count() > 1 {
        return Ok(
            joined_with_comment(cx, inner, outer).unwrap_or_else(|| parenthesize_if_expands(d))
        );
    }
    match d.flat_width() {
        // (a comment after the line that is too wide alone does not make the atom break)
        Some(_)
            if has_line_comment(cx, outer)
                && shapes::head_fits(cx, outer, prefix, &d, depth() * s.indent, s.width) =>
        {
            Ok(d)
        }
        Some(w)
            if (depth() + 1) * s.indent + w <= s.width && depth() * s.indent + prefix < s.width =>
        {
            Ok(parenthesize_if_expands(d))
        }
        _ => Ok(d),
    }
}

/// An implicit concatenation followed by a comment that counts for the width (ruff): it is joined
/// inside the parentheses only when the comment fits after the joined string too (the comment
/// follows the closing parenthesis there, where the string's own group does not see it).
fn joined_with_comment(cx: &FormatCtx<'_>, inner: &SyntaxNode, outer: &SyntaxNode) -> Option<Doc> {
    let last = cx.last_token(outer)?;
    let width: usize = core_format::comments::trailing_comments(cx.grammar, &last)
        .iter()
        .map(|c| c.text().to_string())
        .filter(|t| !is_pragma(t))
        .map(|t| 2 + t.chars().count())
        .sum();
    if width == 0 || !ends_logical_line(cx, &last) {
        return None;
    }
    let parts = expr::string_parts(cx, inner);
    let flat = expr::joined_with(cx, inner, true)?;
    let s = style(cx);
    let id = core_format::doc::new_group_id();
    let expanded = group(core_format::doc::join(&brk(), parts));
    let inside = if (depth() + 1) * s.indent + flat.chars().count() + width <= s.width {
        core_format::doc::text(flat.clone())
    } else {
        expanded
    };
    Some(core_format::doc::group_with_id(
        id,
        shapes::between(
            if_break(lit("("), Doc::Nil),
            core_format::doc::if_break_in(id, inside, core_format::doc::text(flat)),
            if_break(lit(")"), Doc::Nil),
        ),
    ))
}

/// The right-hand side of an assignment; `prefix` is the width of the targets and `=` before it.
pub(crate) fn rhs(cx: &FormatCtx<'_>, value: &SyntaxNode, prefix: usize) -> R {
    with_open(false, || maybe_parenthesize_inner(cx, value, prefix, true))
}

/// An assignment target: a tuple without parentheses gets them when it does not fit.
pub(crate) fn target(cx: &FormatCtx<'_>, n: &SyntaxNode) -> R {
    if n.kind() == k::EXPR_TUPLE && list::is_bare(cx, n) && list::is_one_item_tuple(cx, n) {
        return list::bare_tuple(cx, n);
    }
    if n.kind() == k::EXPR_TUPLE && list::is_bare(cx, n) {
        return with_open(false, || expanded(|| list::bare_tuple_ungrouped(cx, n)));
    }
    expr::expr(cx, n)
}

/// The pattern of a `case`: several alternatives get parentheses when they do not fit.
pub(crate) fn pattern(cx: &FormatCtx<'_>, n: &SyntaxNode) -> R {
    let inner = strip(cx, n);
    if matches!(inner.kind(), k::PATTERN_MATCH_OR | k::PATTERN_MATCH_AS) {
        return with_open(false, || expanded(|| expr::expr(cx, &inner)));
    }
    expr::expr(cx, n)
}

/// A `for` target: a tuple loses its parentheses.
pub(crate) fn for_target(cx: &FormatCtx<'_>, n: &SyntaxNode) -> R {
    let inner = strip(cx, n);
    let ks = kids(cx, &inner);
    let magic = style(cx).magic_comma
        && ks.len() > 2
        && tok_kind(&ks[ks.len() - 1]) == Some(k::RPAR)
        && tok_kind(&ks[ks.len() - 2]) == Some(k::COMMA);
    if inner.kind() == k::EXPR_TUPLE && !magic && !has_own_comments(cx, &inner) {
        return with_open(false, || {
            if list::is_bare(cx, &inner) && can_omit(cx, &inner) {
                list::bare_tuple_flat(cx, &inner)
            } else {
                expanded(|| list::bare_tuple_ungrouped(cx, &inner))
            }
        });
    }
    if inner.kind() == k::EXPR_TUPLE {
        return expr::expr(cx, n);
    }
    expr::expr(cx, &inner)
}

/// Comments on the tokens of `n` itself or at the edges of its first and last element.
fn has_own_comments(cx: &FormatCtx<'_>, n: &SyntaxNode) -> bool {
    let edges = n.children().next().is_some_and(|f| {
        cx.first_token(&f)
            .is_some_and(|t| !core_format::comments::leading_comments(cx.grammar, &t).is_empty())
    }) || n.children().last().is_some_and(|l| {
        cx.last_token(&l)
            .is_some_and(|t| !core_format::comments::trailing_comments(cx.grammar, &t).is_empty())
    });
    edges
        || kids(cx, n).iter().any(|e| match e {
            core_syntax::SyntaxElement::Token(t) => has_comments(cx, t),
            _ => false,
        })
}

/// The operator levels of ruff's visitor: `~` binds tightest, a bare tuple loosest.
struct Levels;

impl shapes::OmitHooks for Levels {
    fn level(&self, cx: &FormatCtx<'_>, n: &SyntaxNode) -> Option<u8> {
        match n.kind() {
            k::EXPR_BIN_OP => kids(cx, n).get(1).and_then(tok_kind).map(op::precedence),
            k::EXPR_BOOL_OP => Some(2),
            k::EXPR_COMPARE => Some(4),
            k::EXPR_IF => Some(1),
            k::EXPR_UNARY_OP => {
                (kids(cx, n).first().and_then(tok_kind) == Some(k::TILDE)).then_some(11)
            }
            k::EXPR_TUPLE if list::is_bare(cx, n) => Some(0),
            _ => None,
        }
    }

    fn is_paren(&self, n: &SyntaxNode) -> bool {
        n.kind() == k::PARENTHESIZED
    }
}

fn can_omit(cx: &FormatCtx<'_>, n: &SyntaxNode) -> bool {
    shapes::can_omit(cx, n, &Levels)
}
