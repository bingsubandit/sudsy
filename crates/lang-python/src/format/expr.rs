//! Expressions: dispatch on the node kind, names, literals, attribute and call chains, subscripts,
//! parameters and patterns.
//!
//! concept: member-chain, literal-spelling

use super::literal;
use super::util::*;
use super::{list, op};
use crate::generated::kind as k;
use core_format::doc::{self, Doc, concat, group};
use core_format::literals::prefixed;
use core_format::{FormatCtx, FormatError};
use core_syntax::{SyntaxElement, SyntaxNode, SyntaxToken};

/// The document of an expression (or of another node below one). A trailing comment goes after
/// the largest expression that ends at its token, so that only the groups around it break.
pub(crate) fn expr(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let hoisted = core_format::comments::comment_at_end(cx.grammar, node)
        .then(|| cx.last_token(node))
        .flatten()
        .filter(|last| {
            core_format::comments::has_trailing_comment(cx.grammar, last)
                && node
                    .parent()
                    .is_some_and(|p| cx.last_token(&p).is_some_and(|l| l != *last))
        });
    if let Some(last) = &hoisted {
        cx.hold_trailing(last);
    }
    // Comments in front of the largest expression that starts at a token go in front of it.
    let leading = cx
        .first_token(node)
        .filter(|first| {
            !core_format::comments::leading_comments(cx.grammar, first).is_empty()
                && node
                    .parent()
                    .is_some_and(|p| cx.first_token(&p).is_some_and(|f| f != *first))
        })
        .map(|first| {
            cx.hold_leading(&first);
            core_format::context::leading_comments_doc(&core_format::comments::leading_comments(
                cx.grammar, &first,
            ))
        });
    let d = expr_inner(cx, node)?;
    let d = match leading {
        Some(l) => concat(vec![l, d]),
        None => d,
    };
    Ok(match &hoisted {
        Some(last) => concat(vec![d, cx.held_trailing(last)]),
        None => d,
    })
}

fn expr_inner(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    match node.kind() {
        k::PARENTHESIZED => parenthesized_node(cx, node),
        k::PATTERN_MATCH_VALUE
        | k::PATTERN_MATCH_SEQUENCE
        | k::PATTERN_MATCH_MAPPING
        | k::PATTERN_MATCH_CLASS
        | k::PATTERN_MATCH_AS
        | k::PATTERN_MATCH_OR
        | k::PATTERN_MATCH_SINGLETON
        | k::PATTERN_MATCH_STAR
        | k::PATTERN_KEYWORD => pattern(cx, node),
        k::WITH_ITEM => with_item(cx, node),
        k::EXPR_NAME
        | k::EXPR_BOOLEAN_LITERAL
        | k::EXPR_NONE_LITERAL
        | k::EXPR_ELLIPSIS_LITERAL
        | k::IDENTIFIER => plain(cx, node),
        k::EXPR_NUMBER_LITERAL => number(cx, node),
        k::EXPR_STRING_LITERAL | k::EXPR_BYTES_LITERAL | k::EXPR_F_STRING | k::EXPR_T_STRING => {
            strings(cx, node, true)
        }
        k::EXPR_ATTRIBUTE | k::EXPR_CALL | k::EXPR_SUBSCRIPT => trailers(cx, node),
        k::EXPR_SLICE => slice(cx, node),
        k::EXPR_STARRED | k::KEYWORD => concat_kids(cx, node),
        k::EXPR_BIN_OP => op::binary(cx, node),
        k::EXPR_BOOL_OP => op::bool_op(cx, node),
        k::EXPR_COMPARE => op::compare(cx, node),
        k::EXPR_UNARY_OP => op::unary(cx, node),
        k::EXPR_IF => op::ternary(cx, node),
        k::EXPR_LAMBDA => op::lambda(cx, node),
        k::EXPR_NAMED => op::named(cx, node),
        k::EXPR_AWAIT | k::EXPR_YIELD | k::EXPR_YIELD_FROM => op::prefix(cx, node),
        k::EXPR_LIST | k::EXPR_SET | k::EXPR_TUPLE | k::EXPR_DICT => list::collection(cx, node),
        k::EXPR_LIST_COMP | k::EXPR_SET_COMP | k::EXPR_DICT_COMP | k::EXPR_GENERATOR => {
            list::comprehension(cx, node)
        }
        k::ARGUMENTS | k::PATTERN_ARGUMENTS => list::arguments(cx, node),
        k::PARAMETERS => list::parameters(cx, node),
        k::PARAMETER => parameter(cx, node),
        k::PARAMETER_WITH_DEFAULT => parameter_with_default(cx, node),
        k::TYPE_PARAM_TYPE_VAR | k::TYPE_PARAM_TYPE_VAR_TUPLE | k::TYPE_PARAM_PARAM_SPEC => {
            type_param(cx, node)
        }
        k::ALIAS => alias(cx, node),
        _ => Ok(cx.verbatim_node(node)),
    }
}

/// An element: a token with its comments, a node as an expression.
pub(crate) fn elem(cx: &FormatCtx<'_>, e: &SyntaxElement) -> R {
    match e {
        SyntaxElement::Token(t) => Ok(cx.token(t)),
        SyntaxElement::Node(n) => expr(cx, n),
    }
}

fn concat_kids(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let parts: Result<Vec<Doc>, FormatError> = kids(cx, node).iter().map(|e| elem(cx, e)).collect();
    Ok(concat(parts?))
}

/// A node with a single token (and nothing else that prints).
fn plain(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    concat_kids(cx, node)
}

fn number(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let Some(t) = tokens(cx, node).into_iter().next() else {
        return Ok(Doc::Nil);
    };
    Ok(restyled(cx, &t, &literal::number(t.text())))
}

/// The restyled text of a string-like token.
pub(crate) fn string_token(cx: &FormatCtx<'_>, t: &SyntaxToken) -> Doc {
    restyled(
        cx,
        t,
        &prefixed::string(syntax(), t.text(), style(cx).quote),
    )
}

/// Strings of an implicit concatenation, one per line when they do not fit.
/// `own_group`: the parts break on their own; otherwise the group around the string decides.
pub(crate) fn strings(cx: &FormatCtx<'_>, node: &SyntaxNode, own_group: bool) -> R {
    // A comment after the last part, with more of the expression after it, goes after the whole
    // string, so that it does not keep the parts from joining.
    let mid = node
        .children()
        .last()
        .and_then(|p| tokens(cx, &p).into_iter().next())
        .filter(|t| {
            node.children().count() > 1
                && core_format::comments::has_trailing_comment(cx.grammar, t)
                && !cx.is_held(t)
        });
    if let Some(t) = &mid {
        cx.hold_trailing(t);
    }
    let mut parts = string_parts(cx, node);
    let flat = joined_with(cx, node, mid.is_some()).filter(|_| parts.len() > 1);
    let multiline = node
        .descendants_with_tokens()
        .filter_map(|e| e.into_token())
        .any(|t| matches!(t.kind(), k::STRING | k::F_STRING_MIDDLE) && t.text().contains('\n'));
    if parts.len() == 1 {
        let d = parts.remove(0);
        // A string over several lines does not fit anywhere.
        return Ok(if multiline {
            concat(vec![d, doc::break_parent()])
        } else {
            d
        });
    }
    let expanded = doc::join(&brk(), parts);
    let d = match flat {
        Some(flat) => doc::if_break(expanded, doc::text(flat)),
        None => expanded,
    };
    let d = if own_group || mid.is_some() {
        group(d)
    } else {
        d
    };
    Ok(match &mid {
        Some(t) => concat(vec![d, cx.held_trailing(t).without_break_parents()]),
        None => d,
    })
}

/// The documents of the parts of a string.
pub(crate) fn string_parts(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Vec<Doc> {
    let mut parts = Vec::new();
    for part in node.children() {
        match part.kind() {
            k::F_STRING | k::T_STRING => parts.push(fstring(cx, &part)),
            _ => {
                let t = tokens(cx, &part).into_iter().next();
                parts.push(match t {
                    Some(t) => string_token(cx, &t),
                    None => Doc::Nil,
                });
            }
        }
    }
    parts
}

/// The parts of an implicit concatenation as one string, when they are plain strings of one
/// kind that need no escapes to share a quote (ruff joins them when they fit on a line).
pub(crate) fn joined(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Option<String> {
    joined_with(cx, node, false)
}

/// [`joined`]; `end_comment`: a comment after the last part is the line's and stays out of it.
pub(crate) fn joined_with(
    cx: &FormatCtx<'_>,
    node: &SyntaxNode,
    end_comment: bool,
) -> Option<String> {
    let last_part = node.children().last();
    let mut prefix = None;
    let mut quote = None;
    let mut body = String::new();
    for part in node.children() {
        if part.kind() != k::STRING_LITERAL && part.kind() != k::BYTES_LITERAL {
            return None;
        }
        let t = part
            .children_with_tokens()
            .filter_map(|e| e.into_token())
            .find(|t| t.kind() == k::STRING)?;
        let tail = end_comment && last_part.as_ref() == Some(&part);
        if !core_format::comments::leading_comments(cx.grammar, &t).is_empty()
            || !tail && !core_format::comments::trailing_comments(cx.grammar, &t).is_empty()
        {
            return None;
        }
        let text = prefixed::string(syntax(), t.text(), style(cx).quote);
        let (p, q, b) = prefixed::split(syntax(), &text)?;
        if q.len() != 1 || p.contains(['r', 'R']) || *prefix.get_or_insert(p.to_string()) != p {
            return None;
        }
        if *quote.get_or_insert(q.to_string()) != q {
            return None;
        }
        body.push_str(b);
    }
    let (p, q) = (prefix?, quote?);
    Some(format!("{p}{q}{body}{q}"))
}

/// An f-string as written, with the quotes of the option unless a replacement field holds a quote.
fn fstring(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Doc {
    let toks: Vec<SyntaxToken> = node
        .descendants_with_tokens()
        .filter_map(|e| e.into_token())
        .filter(|t| !cx.grammar.is_trivia(t.kind()))
        .collect();
    let (Some(first), Some(last)) = (toks.first(), toks.last()) else {
        return Doc::Nil;
    };
    let text: String = node
        .descendants_with_tokens()
        .filter_map(|e| e.into_token())
        .skip_while(|t| t != first)
        .take_while(|t| t.text_range().start() <= last.text_range().start())
        .map(|t| t.text().to_string())
        .collect();
    concat(vec![
        cx.leading_comments_doc(first),
        doc::text(prefixed::fstring(syntax(), &text, style(cx).quote).into_owned()),
        cx.trailing_comments_doc(last),
    ])
}

fn parenthesized_node(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let ks = kids(cx, node);
    let (Some(SyntaxElement::Token(open)), Some(SyntaxElement::Token(close))) =
        (ks.first(), ks.last())
    else {
        return Err(shape_error("Parenthesized"));
    };
    if let [_, SyntaxElement::Node(inner), _] = ks.as_slice()
        && inner.kind() == k::EXPR_TUPLE
        && list::is_bare(cx, inner)
    {
        return list::parenthesized_tuple(cx, node, inner);
    }
    let inner: Result<Vec<Doc>, FormatError> = with_fluent(true, || {
        with_open(true, || {
            ks[1..ks.len() - 1].iter().map(|e| elem(cx, e)).collect()
        })
    });
    Ok(bracketed_by(cx, open, concat(inner?), close))
}

/// One link of an attribute, call and subscript chain.
enum Link {
    Attr(SyntaxNode),
    Call(SyntaxNode),
    Sub(SyntaxNode),
}

/// The head of a chain and its links, innermost first.
fn links(node: &SyntaxNode) -> (SyntaxNode, Vec<Link>) {
    let mut out = Vec::new();
    let mut cur = node.clone();
    loop {
        let next = match cur.kind() {
            k::EXPR_ATTRIBUTE | k::EXPR_CALL | k::EXPR_SUBSCRIPT => cur.children().next(),
            _ => None,
        };
        let Some(inner) = next else { break };
        out.push(match cur.kind() {
            k::EXPR_ATTRIBUTE => Link::Attr(cur.clone()),
            k::EXPR_CALL => Link::Call(cur.clone()),
            _ => Link::Sub(cur.clone()),
        });
        cur = inner;
    }
    out.reverse();
    (cur, out)
}

/// Does a comment stand after the head of an attribute chain, between the dot and the names, or between a
/// callee and its `(` or `[`?
pub(crate) fn chain_has_comments(cx: &FormatCtx<'_>, node: &SyntaxNode) -> bool {
    let mut cur = node.clone();
    loop {
        match cur.kind() {
            k::EXPR_ATTRIBUTE => {
                if cur
                    .children()
                    .next()
                    .is_some_and(|h| core_format::comments::comment_at_end(cx.grammar, &h))
                {
                    return true;
                }
                for e in cur.children_with_tokens().skip(1) {
                    let hit = match &e {
                        SyntaxElement::Token(t) => {
                            !cx.grammar.is_trivia(t.kind()) && has_comments(cx, t)
                        }
                        SyntaxElement::Node(c) => {
                            cx.first_token(c).is_some_and(|t| has_comments(cx, &t))
                        }
                    };
                    if hit {
                        return true;
                    }
                }
            }
            k::EXPR_CALL | k::EXPR_SUBSCRIPT => {
                let after = cur.children().nth(1).and_then(|a| cx.first_token(&a));
                if after.is_some_and(|t| {
                    !core_format::comments::leading_comments(cx.grammar, &t).is_empty()
                }) {
                    return true;
                }
            }
            _ => return false,
        }
        match cur.children().next() {
            Some(c) => cur = c,
            None => return false,
        }
    }
}

/// ruff's fluent layout: two or more attributes that follow a call or subscript.
pub(crate) fn is_fluent(_cx: &FormatCtx<'_>, node: &SyntaxNode) -> bool {
    if !fluent_allowed() {
        return false;
    }
    let (_, ls) = links(node);
    let mut after = 0;
    let mut prev_bracket = false;
    for l in &ls {
        match l {
            Link::Attr(_) => {
                if prev_bracket {
                    after += 1;
                }
                prev_bracket = false;
            }
            _ => prev_bracket = true,
        }
    }
    after >= 2
}

/// `value.attr(args)[index]`, with a line break before each `.attr` that follows a call or
/// subscript when the chain is fluent.
fn trailers(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let (head, ls) = links(node);
    let fluent = is_fluent(cx, node);
    let head_doc = expr(cx, &head)?;
    let mut first = vec![if needs_dot_parens(&head, &ls) {
        concat(vec![doc::lit("("), head_doc, doc::lit(")")])
    } else {
        head_doc
    }];
    let mut rest: Vec<Doc> = Vec::new();
    let mut after_bracket = false;
    // A comment after the head ends its line: the first `.attr` goes to the next one (ruff).
    let mut after_comment = matches!(ls.first(), Some(Link::Attr(_)))
        && core_format::comments::comment_at_end(cx.grammar, &head)
        && cx
            .last_token(&head)
            .is_some_and(|l| core_format::comments::has_trailing_comment(cx.grammar, &l));
    for l in &ls {
        let in_rest = !rest.is_empty();
        match l {
            Link::Attr(n) => {
                if ((fluent || in_rest) && after_bracket) || after_comment {
                    rest.push(soft_brk());
                    after_comment = false;
                }
                let target = if rest.is_empty() {
                    &mut first
                } else {
                    &mut rest
                };
                for e in kids(cx, n).iter().skip(1) {
                    target.push(elem(cx, e)?);
                }
                after_bracket = false;
            }
            Link::Call(n) => {
                let target = if in_rest { &mut rest } else { &mut first };
                if let Some(a) = n.children().nth(1) {
                    target.push(list::arguments(cx, &a)?);
                }
                after_bracket = true;
            }
            Link::Sub(n) => {
                let target = if in_rest { &mut rest } else { &mut first };
                target.push(subscript_tail(cx, n)?);
                after_bracket = true;
            }
        }
    }
    if rest.is_empty() {
        return Ok(concat(first));
    }
    Ok(group(concat(vec![concat(first), concat(rest)])))
}

/// A decimal integer or a float is parenthesized before `.attr` (`(1).real`); `0x1.real` and `1j.real`
/// stay as they are.
fn needs_dot_parens(head: &SyntaxNode, ls: &[Link]) -> bool {
    head.kind() == k::EXPR_NUMBER_LITERAL
        && matches!(ls.first(), Some(Link::Attr(_)))
        && head.first_token().is_some_and(|t| {
            t.kind() == k::FLOAT
                || t.kind() == k::INT
                    && !t.text().get(..2).is_some_and(|p| {
                        ["0x", "0b", "0o"].contains(&p.to_ascii_lowercase().as_str())
                    })
        })
}

/// `[index]` of a subscript.
fn subscript_tail(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    with_fluent(true, || with_open(true, || subscript_tail_inner(cx, node)))
}

fn subscript_tail_inner(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let ks = kids(cx, node);
    let (Some(SyntaxElement::Token(open)), Some(SyntaxElement::Token(close))) =
        (ks.get(1), ks.last())
    else {
        return Err(shape_error("Subscript"));
    };
    let inner = &ks[2..ks.len() - 1];
    let inner_doc = match inner {
        [SyntaxElement::Node(t)] if t.kind() == k::EXPR_TUPLE && list::is_bare(cx, t) => {
            list::bare_tuple(cx, t)?
        }
        _ => concat(
            inner
                .iter()
                .map(|e| elem(cx, e))
                .collect::<Result<_, _>>()?,
        ),
    };
    Ok(bracketed_by(cx, open, inner_doc, close))
}

/// `lower:upper:step`; with an operand that is not simple the colons get spaces next to operands.
fn slice(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let ks = kids(cx, node);
    let complex = ks
        .iter()
        .filter_map(|e| e.as_node())
        .any(|n| !is_simple_slice_part(n));
    let mut parts = Vec::new();
    for (i, e) in ks.iter().enumerate() {
        let is_colon = tok_kind(e) == Some(k::COLON);
        if is_colon && complex && i > 0 && ks[i - 1].as_node().is_some() {
            parts.push(sp());
        }
        parts.push(elem(cx, e)?);
        if is_colon && complex && ks.get(i + 1).is_some_and(|n| n.as_node().is_some()) {
            parts.push(sp());
        }
    }
    Ok(concat(parts))
}

/// A name, number, attribute of those, or a unary of one: slices of these have tight colons.
fn is_simple_slice_part(n: &SyntaxNode) -> bool {
    match n.kind() {
        k::EXPR_NAME
        | k::EXPR_NUMBER_LITERAL
        | k::EXPR_STRING_LITERAL
        | k::EXPR_NONE_LITERAL
        | k::EXPR_BOOLEAN_LITERAL
        | k::EXPR_ELLIPSIS_LITERAL => true,
        k::EXPR_UNARY_OP => n.children().all(|c| is_simple_slice_part(&c)),
        _ => false,
    }
}

/// `name: annotation` or `*args` / `**kwargs`.
fn parameter(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let mut parts = Vec::new();
    for e in kids(cx, node) {
        if tok_kind(&e) == Some(k::COLON) {
            parts.push(elem(cx, &e)?);
            parts.push(sp());
        } else {
            parts.push(elem(cx, &e)?);
        }
    }
    Ok(concat(parts))
}

/// `name=default`, `name: annotation = default`.
fn parameter_with_default(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let ks = kids(cx, node);
    let annotated = ks
        .first()
        .and_then(|e| e.as_node())
        .is_some_and(|p| tokens(cx, p).iter().any(|t| t.kind() == k::COLON));
    let mut parts = Vec::new();
    for e in &ks {
        match e {
            SyntaxElement::Token(t) if t.kind() == k::EQUAL => {
                if annotated {
                    parts.push(sp());
                }
                parts.push(cx.token(t));
                if annotated {
                    parts.push(sp());
                }
            }
            _ => parts.push(elem(cx, e)?),
        }
    }
    Ok(concat(parts))
}

/// `T: bound = default`, `*Ts`, `**P`.
fn type_param(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let mut parts = Vec::new();
    for e in kids(cx, node) {
        match &e {
            SyntaxElement::Token(t) if t.kind() == k::COLON => {
                parts.push(cx.token(t));
                parts.push(sp());
            }
            SyntaxElement::Token(t) if t.kind() == k::EQUAL => {
                parts.push(sp());
                parts.push(cx.token(t));
                parts.push(sp());
            }
            _ => parts.push(elem(cx, &e)?),
        }
    }
    Ok(concat(parts))
}

/// `name as alias`, `*`.
fn alias(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let mut parts = Vec::new();
    for (i, e) in kids(cx, node).iter().enumerate() {
        if i > 0 {
            parts.push(sp());
        }
        parts.push(elem(cx, e)?);
    }
    Ok(concat(parts))
}

/// `expr as target` of a `with`.
fn with_item(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let mut parts = Vec::new();
    for (i, e) in kids(cx, node).iter().enumerate() {
        match e {
            SyntaxElement::Node(n) if i == 0 => {
                parts.push(super::paren::maybe_parenthesize(cx, n)?)
            }
            SyntaxElement::Token(t) => {
                parts.push(sp());
                parts.push(cx.token(t));
                parts.push(sp());
            }
            _ => parts.push(elem(cx, e)?),
        }
    }
    Ok(concat(parts))
}

/// Patterns of `match`: brackets as lists, `|` and `as` with spaces.
pub(crate) fn pattern(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    match node.kind() {
        k::PATTERN_MATCH_SEQUENCE | k::PATTERN_MATCH_MAPPING => {
            let ks = kids(cx, node);
            if ks.first().and_then(tok_kind).is_some_and(is_open) {
                return list::pattern_list(cx, node);
            }
            let p = list::from_elements(None, None, ks);
            let mut parts = Vec::new();
            for (i, run) in p.runs.iter().enumerate() {
                if i > 0 {
                    parts.push(sp());
                }
                for e in run {
                    parts.push(elem(cx, e)?);
                }
                if let Some(c) = &p.commas[i] {
                    parts.push(cx.token(c));
                }
            }
            Ok(concat(parts))
        }
        k::PATTERN_MATCH_OR => {
            let mut parts = Vec::new();
            for e in kids(cx, node) {
                match &e {
                    SyntaxElement::Token(t) => {
                        let (comment, op) = op::operator(cx, t);
                        parts.push(comment);
                        parts.push(ln());
                        parts.push(op);
                        parts.push(sp());
                    }
                    _ => parts.push(elem(cx, &e)?),
                }
            }
            Ok(group(concat(parts)))
        }
        k::PATTERN_MATCH_AS => {
            let mut parts = Vec::new();
            for e in kids(cx, node) {
                match &e {
                    SyntaxElement::Token(t) if t.kind() == k::AS => {
                        parts.push(sp());
                        parts.push(cx.token(t));
                        parts.push(sp());
                    }
                    _ => parts.push(elem(cx, &e)?),
                }
            }
            Ok(concat(parts))
        }
        _ => concat_kids(cx, node),
    }
}
