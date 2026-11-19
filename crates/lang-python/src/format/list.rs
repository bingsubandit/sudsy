//! Comma-separated lists: collections, call arguments, parameters, imports, comprehensions.
//!
//! concept: delimited-list

use super::expr::elem;
use super::util::*;
use crate::generated::kind as k;
use core_format::doc::{Doc, concat, group, group_broken, hardline, if_break, lit};
use core_format::shapes::{self, ListFacts, ListLayout, ListStyle, TrailingComma};
use core_format::{FormatCtx, FormatError};
use core_syntax::{SyntaxElement, SyntaxNode, SyntaxToken};

/// A list node cut into its delimiters and the runs between commas.
pub(crate) struct Parts {
    pub open: Option<SyntaxToken>,
    pub close: Option<SyntaxToken>,
    pub runs: Vec<Vec<SyntaxElement>>,
    /// The comma after each run.
    pub commas: Vec<Option<SyntaxToken>>,
}

pub(crate) fn from_elements(
    open: Option<SyntaxToken>,
    close: Option<SyntaxToken>,
    elems: Vec<SyntaxElement>,
) -> Parts {
    let mut runs: Vec<Vec<SyntaxElement>> = Vec::new();
    let mut commas = Vec::new();
    let mut cur = Vec::new();
    for e in elems {
        if tok_kind(&e) == Some(k::COMMA) {
            runs.push(std::mem::take(&mut cur));
            commas.push(e.into_token());
        } else {
            cur.push(e);
        }
    }
    if !cur.is_empty() {
        runs.push(cur);
        commas.push(None);
    }
    Parts {
        open,
        close,
        runs,
        commas,
    }
}

pub(crate) fn split(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Parts {
    let mut ks = kids(cx, node);
    let (mut open, mut close) = (None, None);
    if let Some(SyntaxElement::Token(t)) = ks.first()
        && is_open(t.kind())
    {
        open = Some(t.clone());
        ks.remove(0);
        if let Some(SyntaxElement::Token(c)) = ks.last()
            && matches!(c.kind(), k::RPAR | k::RSQB | k::RBRACE)
        {
            close = Some(c.clone());
            ks.pop();
        }
    }
    from_elements(open, close, ks)
}

/// One item: nodes as expressions, `:` followed by a space.
fn run_doc(cx: &FormatCtx<'_>, run: &[SyntaxElement]) -> R {
    let mut parts = Vec::new();
    for e in run {
        parts.push(elem(cx, e)?);
        if tok_kind(e) == Some(k::COLON) {
            parts.push(sp());
        }
    }
    Ok(concat(parts))
}

/// Items with their commas; the last comma is the trailing one.
fn items(
    cx: &FormatCtx<'_>,
    p: &Parts,
    one_tuple: bool,
    no_trailing: bool,
    single_comma: bool,
) -> Result<Doc, FormatError> {
    let n = p.runs.len();
    let mut parts = Vec::new();
    for (i, run) in p.runs.iter().enumerate() {
        if i > 0 {
            parts.push(ln());
        }
        parts.push(run_doc(cx, run)?);
        match (&p.commas[i], i + 1 == n) {
            (Some(c), false) => parts.push(cx.token(c)),
            (Some(c), true) if has_comments(cx, c) => parts.push(cx.token(c)),
            (_, true) if one_tuple => parts.push(lit(",")),
            (Some(_), true) if !no_trailing && style(cx).magic_comma => parts.push(lit(",")),
            (_, true) if !no_trailing && (n > 1 || single_comma) => parts.push(trailing_comma()),
            _ => {}
        }
    }
    Ok(concat(parts))
}

fn has_magic(cx: &FormatCtx<'_>, p: &Parts) -> bool {
    style(cx).magic_comma && p.commas.last().is_some_and(|c| c.is_some())
}

/// The style of a bracketed list from the facts of its source: a trailing comma when the list
/// breaks (not for one item, unless `single_comma`; always for a one-item tuple), the magic comma
/// of the source keeps it expanded.
fn list_style(
    cx: &FormatCtx<'_>,
    f: &ListFacts<'_>,
    one_tuple: bool,
    no_trailing: bool,
    single_comma: bool,
) -> ListStyle {
    let (last, items) = (f.last_sep, f.items);
    let commented = last.is_some_and(|c| has_comments(cx, c));
    let magic = style(cx).magic_comma && last.is_some() && !one_tuple && !no_trailing;
    let trailing_comma = if one_tuple {
        TrailingComma::Always
    } else if no_trailing && !commented {
        TrailingComma::Never
    } else if items > 1 || single_comma || magic || commented {
        TrailingComma::WhenBroken
    } else {
        TrailingComma::Never
    };
    ListStyle {
        trailing_comma,
        layout: if magic {
            ListLayout::MagicTrailingComma
        } else {
            ListLayout::Fit
        },
        max_blank_lines: 0,
        hang: true,
        keep_blank_before_end_comments: false,
        ignore_marks: false,
        ..ListStyle::default()
    }
}

/// Does the list have exactly one item and a comma after it (a one-item tuple)?
fn is_one_item(f: &ListFacts<'_>) -> bool {
    f.only.is_some() && f.last_sep.is_some()
}

/// A node that is its delimiters and the items between them: broken one item per line when it
/// does not fit or has a magic comma.
fn bracketed(
    cx: &FormatCtx<'_>,
    node: &SyntaxNode,
    one_tuple: bool,
    no_trailing: bool,
    single_comma: bool,
) -> R {
    bracketed_with(cx, node, |f| {
        list_style(cx, f, one_tuple, no_trailing, single_comma)
    })
}

/// [`bracketed`] with the style chosen from the facts of the list.
fn bracketed_with(
    cx: &FormatCtx<'_>,
    node: &SyntaxNode,
    style: impl Fn(&ListFacts<'_>) -> ListStyle,
) -> R {
    with_fluent(true, || {
        with_open(true, || {
            shapes::delimited_list(cx, node, shapes::StyleOf::By(&style))
        })
    })
}

fn is_single_generator(n: &SyntaxNode) -> bool {
    n.kind() == k::EXPR_GENERATOR && !n.children_with_tokens().any(|c| c.kind() == k::LPAR)
}

/// The first child that is neither trivia nor empty.
fn first_kid(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Option<SyntaxElement> {
    node.children_with_tokens()
        .find(|e| !cx.grammar.is_trivia(e.kind()) && !e.text_range().is_empty())
}

/// A tuple whose parentheses are not its own.
pub(crate) fn is_bare(cx: &FormatCtx<'_>, tuple: &SyntaxNode) -> bool {
    first_kid(cx, tuple).and_then(|e| tok_kind(&e)) != Some(k::LPAR)
}

/// The items of a tuple without parentheses, a comma after the last one when broken.
pub(crate) fn bare_tuple(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    bare_tuple_with(cx, node, true, true)
}

/// [`bare_tuple`] for the caller's own group (the parentheses ruff adds): its items break with it.
pub(crate) fn bare_tuple_ungrouped(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    bare_tuple_with(cx, node, true, false)
}

/// `breakable`: the items go on lines of their own when they do not fit; otherwise only the
/// brackets inside them split.
fn bare_tuple_with(cx: &FormatCtx<'_>, node: &SyntaxNode, breakable: bool, grouped: bool) -> R {
    let ks = kids(cx, node);
    let inner: Vec<SyntaxElement> = if ks.first().and_then(tok_kind) != Some(k::LPAR) {
        ks
    } else {
        ks[1..ks.len() - 1].to_vec()
    };
    let p = from_elements(None, None, inner);
    let n = p.runs.len();
    let mut parts = Vec::new();
    for (i, run) in p.runs.iter().enumerate() {
        if i > 0 {
            parts.push(if breakable { ln() } else { sp() });
        }
        parts.push(run_doc(cx, run)?);
        match (&p.commas[i], i + 1 == n) {
            (Some(c), false) => parts.push(cx.token(c)),
            (Some(c), true) if has_comments(cx, c) => parts.push(cx.token(c)),
            (_, true) if n == 1 => parts.push(lit(",")),
            (_, true) if breakable => parts.push(trailing_comma()),
            _ => {}
        }
    }
    Ok(if breakable && grouped {
        group(concat(parts))
    } else {
        concat(parts)
    })
}

/// A tuple that stays on one line (its last or first item is a bracket that splits).
pub(crate) fn bare_tuple_flat(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    bare_tuple_with(cx, node, false, false)
}

pub(crate) fn collection(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    if node.kind() == k::EXPR_TUPLE && is_bare(cx, node) {
        return bare_tuple(cx, node);
    }
    let tuple = node.kind() == k::EXPR_TUPLE;
    bracketed_with(cx, node, |f| {
        list_style(cx, f, tuple && is_one_item(f), false, false)
    })
}

/// The tuple `tuple` inside the parentheses of `node`, which are not its own.
pub(crate) fn parenthesized_tuple(cx: &FormatCtx<'_>, node: &SyntaxNode, tuple: &SyntaxNode) -> R {
    let (Some(open), Some(close)) = (node.first_token(), node.last_token()) else {
        return Err(shape_error("Parenthesized"));
    };
    let style = |f: &ListFacts<'_>| list_style(cx, f, is_one_item(f), false, false);
    with_fluent(true, || {
        with_open(true, || {
            shapes::delimited_list_around(
                cx,
                node,
                tuple,
                &open,
                &close,
                shapes::StyleOf::By(&style),
            )
        })
    })
}

/// Call arguments, class bases, class patterns.
pub(crate) fn arguments(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    bracketed_with(cx, node, |f| {
        let single_gen = f.last_sep.is_none() && f.only.is_some_and(is_single_generator);
        list_style(cx, f, false, single_gen, false)
    })
}

/// `class A():` prints as `class A:`.
pub(crate) fn class_arguments(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let ks = kids(cx, node);
    if ks.len() <= 2
        && !ks.iter().any(|e| match e {
            SyntaxElement::Token(t) => has_comments(cx, t),
            _ => false,
        })
    {
        return Ok(Doc::Nil);
    }
    arguments(cx, node)
}

pub(crate) fn parameters(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    bracketed(cx, node, false, false, true)
}

pub(crate) fn type_params(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    bracketed(cx, node, false, false, false)
}

/// A `case` pattern in brackets: a sequence or a mapping.
pub(crate) fn pattern_list(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let sequence = node.kind() == k::PATTERN_MATCH_SEQUENCE;
    bracketed_with(cx, node, |f| {
        let one = sequence
            && is_one_item(f)
            && first_kid(cx, node).and_then(|e| tok_kind(&e)) == Some(k::LPAR);
        list_style(cx, f, one, false, false)
    })
}

/// The names of `from x import ...`: in parentheses, one per line, when they do not fit.
pub(crate) fn import_names(cx: &FormatCtx<'_>, p: &Parts) -> R {
    with_fluent(true, || with_open(true, || import_names_inner(cx, p)))
}

fn import_names_inner(cx: &FormatCtx<'_>, p: &Parts) -> R {
    let open_comment = p.open.as_ref().is_some_and(|o| has_comments(cx, o));
    let end = p.close.as_ref().map_or_else(Vec::new, |c| {
        core_format::comments::leading_comments(cx.grammar, c)
    });
    if let Some(c) = p.close.as_ref().filter(|_| !end.is_empty()) {
        cx.hold_leading(c);
    }
    let magic = (has_magic(cx, p) && p.open.is_some()) || open_comment || !end.is_empty();
    let mut inner = items(cx, p, false, false, true)?;
    if !end.is_empty() {
        inner = concat(vec![inner, hardline(), core_format::comment_lines(&end)]);
    }
    let open = match &p.open {
        Some(o) if open_comment => cx.token(o),
        _ => if_break(lit("("), Doc::Nil),
    };
    let body = shapes::between(open, inner, if_break(lit(")"), Doc::Nil));
    Ok(if magic {
        group_broken(body)
    } else {
        group(body)
    })
}

/// Lambda parameters: no parentheses, never broken.
pub(crate) fn lambda_parameters(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let p = split(cx, node);
    let mut parts = Vec::new();
    for (i, run) in p.runs.iter().enumerate() {
        if i > 0 {
            parts.push(sp());
        }
        parts.push(run_doc(cx, run)?);
        if let Some(c) = p.commas.get(i).and_then(|c| c.as_ref())
            && i + 1 < p.runs.len()
        {
            parts.push(cx.token(c));
        }
    }
    Ok(concat(parts))
}

/// `[elt for x in y if z]`, `{k: v for ...}`, `(x for ...)`, or a bare generator in a call.
pub(crate) fn comprehension(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    with_fluent(true, || with_open(true, || comprehension_inner(cx, node)))
}

fn comprehension_inner(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let ks = kids(cx, node);
    let (mut open, mut close) = (None, None);
    let mut body: &[SyntaxElement] = &ks;
    if let (Some(SyntaxElement::Token(o)), Some(SyntaxElement::Token(c))) = (ks.first(), ks.last())
        && is_open(o.kind())
    {
        open = Some(o.clone());
        close = Some(c.clone());
        body = &ks[1..ks.len() - 1];
    }
    let mut parts = Vec::new();
    for e in body {
        match e {
            SyntaxElement::Node(n) if n.kind() == k::COMPREHENSION => {
                parts.push(ln());
                parts.push(clause(cx, n)?);
            }
            SyntaxElement::Token(t) if t.kind() == k::COLON => {
                parts.push(cx.token(t));
                parts.push(sp());
            }
            _ => parts.push(elem(cx, e)?),
        }
    }
    let inner = group(concat(parts));
    Ok(match (open, close) {
        (Some(o), Some(c)) => bracketed_by(cx, &o, inner, &c),
        _ => inner,
    })
}

/// `for target in iter if cond ...`; the conditions may break before their `if`.
fn clause(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let mut parts = Vec::new();
    for e in kids(cx, node) {
        match tok_kind(&e) {
            Some(k::IF) => {
                parts.push(ln());
                parts.push(elem(cx, &e)?);
                parts.push(sp());
            }
            Some(k::FOR) | Some(k::IN) | Some(k::ASYNC) => {
                if !parts.is_empty() {
                    parts.push(sp());
                }
                parts.push(elem(cx, &e)?);
                if tok_kind(&e) != Some(k::ASYNC) {
                    parts.push(sp());
                }
            }
            _ => parts.push(elem(cx, &e)?),
        }
    }
    Ok(concat(parts))
}
