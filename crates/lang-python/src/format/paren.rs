//! Where parentheses appear around a statement's expression: ruff's `maybe_parenthesize`
//! (redundant ones go, missing ones come back when the expression does not fit).

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

/// An annotated assignment's target: a name loses its parentheses.
pub(crate) fn ann_target(cx: &FormatCtx<'_>, n: &SyntaxNode) -> R {
    let inner = strip(cx, n);
    if inner.kind() == k::EXPR_NAME {
        return expr::expr(cx, &inner);
    }
    expr::expr(cx, n)
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
        k::EXPR_TUPLE if list::is_bare(cx, &inner) => {
            if can_omit(cx, &inner) {
                list::bare_tuple_flat(cx, &inner)
            } else {
                expanded(|| list::bare_tuple(cx, &inner))
            }
        }
        k::EXPR_BIN_OP | k::EXPR_BOOL_OP | k::EXPR_COMPARE | k::EXPR_UNARY_OP => {
            if can_omit(cx, &inner) {
                op::binary_like(cx, &inner, false)
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
            if joins && depth() * style(cx).indent + prefix + head_width(&d) <= style(cx).width {
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
                let head = head_width(&d);
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

/// Parentheses around an implicit concatenation of strings that end the logical line: a comment
/// after the closing one is the line's comment, like one after the last string.
fn ends_line(cx: &FormatCtx<'_>, paren: &SyntaxNode) -> bool {
    let joins = paren.children().next().is_some_and(|n| {
        n.kind() != k::PARENTHESIZED
            && n.children().count() > 1
            && expr::joined_with(cx, &n, true).is_some()
    });
    joins
        && cx
            .last_token(paren)
            .is_some_and(|t| ends_logical_line(cx, &t))
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
    match flat_width(&d) {
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

/// The width of a document up to its first possible line break.
fn head_width(d: &Doc) -> usize {
    fn walk(d: &Doc, w: &mut usize) -> bool {
        match d {
            Doc::Text(s) => {
                *w += s.chars().count();
                false
            }
            Doc::Verbatim(s) => {
                *w += s.chars().count();
                false
            }
            Doc::Line | Doc::SoftLine | Doc::HardLine => true,
            Doc::Concat(ps) => ps.iter().any(|p| walk(p, w)),
            Doc::Group(g) => walk(&g.contents, w),
            Doc::Indent(d) | Doc::Align(_, d) | Doc::Label(_, d) | Doc::IndentIfBreak(_, d) => {
                walk(d, w)
            }
            Doc::IfBreak { flat, .. } => walk(flat, w),
            Doc::Shared(s) => walk(&s.doc, w),
            _ => false,
        }
    }
    let mut w = 0;
    walk(d, &mut w);
    w
}

/// The width of a document printed flat; `None` when it has a forced line break.
pub(crate) fn flat_width(d: &Doc) -> Option<usize> {
    Some(match d {
        Doc::Nil | Doc::SoftLine | Doc::LineSuffix(_) | Doc::LineSuffixBoundary => 0,
        Doc::Text(s) => {
            if s.contains('\n') {
                return None;
            }
            s.chars().count()
        }
        Doc::Verbatim(s) => s.chars().count(),
        Doc::Line => 1,
        Doc::Concat(ps) | Doc::Fill(ps) => {
            let mut n = 0;
            for p in ps {
                n += flat_width(p)?;
            }
            n
        }
        Doc::Group(g) => {
            if g.broken {
                return None;
            }
            flat_width(&g.contents)?
        }
        Doc::Indent(d) | Doc::Align(_, d) | Doc::Label(_, d) | Doc::IndentIfBreak(_, d) => {
            flat_width(d)?
        }
        Doc::IfBreak { flat, .. } => flat_width(flat)?,
        Doc::Shared(s) => flat_width(&s.doc)?,
        _ => return None,
    })
}

/// The right-hand side of an assignment; `prefix` is the width of the targets and `=` before it.
pub(crate) fn rhs(cx: &FormatCtx<'_>, value: &SyntaxNode, prefix: usize) -> R {
    with_open(false, || maybe_parenthesize_inner(cx, value, prefix, true))
}

/// An assignment target: a tuple without parentheses gets them when it does not fit.
pub(crate) fn target(cx: &FormatCtx<'_>, n: &SyntaxNode) -> R {
    if n.kind() == k::EXPR_TUPLE && list::is_bare(cx, n) {
        return with_open(false, || expanded(|| list::bare_tuple(cx, n)));
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
    expr::expr(cx, n)
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

struct Visit {
    any_paren: bool,
    /// A comment between the operands.
    comments: bool,
    /// The loosest operator seen (`u8::MAX`: none) and how many of that kind.
    loosest: u8,
    count: usize,
    first: Option<SyntaxNode>,
    last: Option<SyntaxNode>,
}

impl Visit {
    fn update(&mut self, prec: u8, n: usize) {
        if prec < self.loosest {
            self.loosest = prec;
            self.count = n;
        } else if prec == self.loosest {
            self.count += n;
        }
    }

    /// An operand: the first one visited starts the expression (a call or subscript starts with
    /// what it is applied to), the last one ends it.
    fn leaf(&mut self, n: &SyntaxNode) {
        if self.first.is_none() {
            let mut f = n.clone();
            while matches!(
                f.kind(),
                k::EXPR_CALL | k::EXPR_ATTRIBUTE | k::EXPR_SUBSCRIPT
            ) {
                match f.children().next() {
                    Some(c) => f = c,
                    None => break,
                }
            }
            self.first = Some(f);
        }
        self.last = Some(n.clone());
    }

    fn visit(&mut self, cx: &FormatCtx<'_>, n: &SyntaxNode) {
        match n.kind() {
            k::EXPR_BIN_OP | k::EXPR_BOOL_OP | k::EXPR_COMPARE
                if kids(cx, n).iter().any(|e| match e {
                    core_syntax::SyntaxElement::Token(t) => has_comments(cx, t),
                    core_syntax::SyntaxElement::Node(c) => cx.first_token(c).is_some_and(|t| {
                        !core_format::comments::leading_comments(cx.grammar, &t).is_empty()
                    }),
                }) =>
            {
                self.comments = true;
            }
            k::EXPR_BIN_OP => {
                let ks = kids(cx, n);
                if let Some(o) = ks.get(1).and_then(tok_kind) {
                    self.update(op::precedence(o), 1);
                }
                for c in n.children() {
                    self.visit(cx, &c);
                }
            }
            k::EXPR_BOOL_OP => {
                let ks = kids(cx, n);
                let ops = ks.iter().filter(|e| e.as_token().is_some()).count();
                let prec = ks
                    .iter()
                    .find_map(tok_kind)
                    .map_or(1, |o| if o == k::AND { 2 } else { 1 });
                self.update(prec, ops);
                for c in n.children() {
                    self.visit(cx, &c);
                }
            }
            k::EXPR_COMPARE => {
                let ops = kids(cx, n)
                    .iter()
                    .filter(|e| e.as_token().is_some())
                    .count();
                self.update(4, ops.max(1));
                for c in n.children() {
                    self.visit(cx, &c);
                }
            }
            k::EXPR_UNARY_OP => {
                let ks = kids(cx, n);
                if ks.iter().any(|e| match e {
                    core_syntax::SyntaxElement::Token(t) => has_comments(cx, t),
                    core_syntax::SyntaxElement::Node(c) => cx.first_token(c).is_some_and(|t| {
                        !core_format::comments::leading_comments(cx.grammar, &t).is_empty()
                    }),
                }) {
                    self.comments = true;
                    return;
                }
                let o = ks.first().and_then(tok_kind);
                self.update(if o == Some(k::NOT) { 3 } else { 11 }, 1);
                for c in n.children() {
                    self.visit(cx, &c);
                }
            }
            k::EXPR_AWAIT => {
                for c in n.children() {
                    self.visit(cx, &c);
                }
            }
            k::EXPR_TUPLE if list::is_bare(cx, n) => {
                let ops = kids(cx, n)
                    .iter()
                    .filter(|e| e.as_token().is_some())
                    .count();
                self.update(0, ops.saturating_sub(1).max(1));
                for c in n.children() {
                    self.visit(cx, &c);
                }
            }
            _ => {
                if is_parenthesized(cx, n) || matches!(n.kind(), k::EXPR_CALL | k::EXPR_SUBSCRIPT) {
                    self.any_paren = true;
                }
                self.leaf(n);
            }
        }
    }
}

/// Does the expression end in a bracket pair with something in it?
fn is_parenthesized(cx: &FormatCtx<'_>, n: &SyntaxNode) -> bool {
    match n.kind() {
        k::PARENTHESIZED => true,
        k::EXPR_LIST
        | k::EXPR_SET
        | k::EXPR_DICT
        | k::EXPR_TUPLE
        | k::EXPR_LIST_COMP
        | k::EXPR_SET_COMP
        | k::EXPR_DICT_COMP
        | k::EXPR_GENERATOR => {
            let ks = kids(cx, n);
            ks.len() > 2 && ks.first().and_then(tok_kind).is_some_and(is_open)
        }
        k::EXPR_CALL => n
            .children()
            .find(|c| c.kind() == k::ARGUMENTS)
            .is_some_and(|a| kids(cx, &a).len() > 2),
        k::EXPR_STARRED => n
            .children()
            .next()
            .is_some_and(|c| is_parenthesized(cx, &c)),
        _ => false,
    }
}

/// ruff's `can_omit_optional_parentheses`: the expression ends in a bracket that can split and has
/// one operator level, so the bracket splits instead of the operators.
fn can_omit(cx: &FormatCtx<'_>, n: &SyntaxNode) -> bool {
    let mut v = Visit {
        any_paren: false,
        comments: false,
        loosest: u8::MAX,
        count: 0,
        first: None,
        last: None,
    };
    v.visit(cx, n);
    if v.comments || !v.any_paren || v.count > 1 {
        return false;
    }
    if v.loosest == u8::MAX {
        return true;
    }
    let p = |x: &Option<SyntaxNode>| x.as_ref().is_some_and(|x| is_parenthesized(cx, x));
    p(&v.first) || p(&v.last)
}
