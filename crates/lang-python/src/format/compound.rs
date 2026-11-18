//! Compound statements and clauses: header, `:` and the block, decorators, `with` items.

use super::block::{Seq, block as block_doc, joins_header};
use super::expr;
use super::list;
use super::paren;
use super::util::*;
use crate::generated::kind as k;
use core_format::FormatCtx;
use core_format::comments;
use core_format::doc::{self, Doc, concat, group, hardline, indent, lit, softline};
use core_syntax::{SyntaxElement, SyntaxKind, SyntaxNode, SyntaxToken};

pub(crate) fn seq_of(kind: SyntaxKind) -> Seq {
    match kind {
        k::STMT_CLASS_DEF => Seq::Class,
        k::STMT_FUNCTION_DEF => Seq::Function,
        _ => Seq::Other,
    }
}

/// A compound statement or clause: header, `:` and the indented block, then its clauses.
pub(crate) fn compound(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let ks = kids(cx, node);
    let top = at_top(node);
    let seq = seq_of(node.kind());
    let mut parts: Vec<Doc> = Vec::new();
    let mut i = 0;
    let mut prev: Option<&SyntaxElement> = None;
    let with_items = node.kind() == k::STMT_WITH;
    while i < ks.len() {
        let e = &ks[i];
        match e {
            SyntaxElement::Token(t) if t.kind() == k::COLON => {
                parts.push(cx.token(t));
                if node.kind() == k::STMT_MATCH {
                    let cases: Vec<SyntaxNode> = ks[i + 1..]
                        .iter()
                        .filter_map(|e| e.as_node())
                        .filter(|n| n.kind() == k::MATCH_CASE)
                        .cloned()
                        .collect();
                    let mut inner = Vec::new();
                    for c in &cases {
                        let first = cx.first_token(c).ok_or_else(|| shape_error("case"))?;
                        inner.push(clause_break(cx, &first, false));
                        inner.push(compound(cx, c)?);
                    }
                    if let Some(last) = cases.last() {
                        let col = cx
                            .first_token(&cases[0])
                            .map_or(0, |t| comments::column(&t));
                        inner.push(cx.end_comments(last, col));
                    }
                    parts.push(indent(concat(inner)));
                    break;
                }
                let mut j = i + 1;
                let mut body = Vec::new();
                while j < ks.len() {
                    match &ks[j] {
                        SyntaxElement::Node(n) if is_stmt(n.kind()) => body.push(n.clone()),
                        SyntaxElement::Token(s) if s.kind() == k::SEMI => {}
                        _ => break,
                    }
                    j += 1;
                }
                if matches!(node.kind(), k::STMT_FUNCTION_DEF | k::STMT_CLASS_DEF)
                    && let [only] = body.as_slice()
                    && joins_header(cx, only)
                {
                    let last = cx.last_token(only).ok_or_else(|| shape_error("stub"))?;
                    cx.hold_trailing(&last);
                    parts.push(lit(" ..."));
                    parts.push(trailing_inline(cx, &last));
                } else {
                    parts.push(block_doc(cx, &body, seq)?);
                }
                prev = None;
                i = j;
                continue;
            }
            SyntaxElement::Token(t) if i > 0 && matches!(t.kind(), k::ELSE | k::FINALLY) => {
                parts.push(clause_break(cx, t, top));
                parts.push(cx.token(t));
            }
            SyntaxElement::Token(t)
                if with_items && matches!(t.kind(), k::LPAR | k::RPAR | k::COMMA) => {}
            SyntaxElement::Token(t) => {
                if prev.is_some() && !matches!(t.kind(), k::COMMA | k::COLON) {
                    parts.push(sp());
                }
                if prev.is_some() && t.kind() == k::STAR {
                    // `except*`
                    parts.pop();
                }
                parts.push(cx.token(t));
            }
            SyntaxElement::Node(n) if n.kind() == k::DECORATOR => {
                parts.push(decorator(cx, n)?);
                parts.push(hardline());
                prev = None;
                i += 1;
                continue;
            }
            SyntaxElement::Node(n) if is_clause(n.kind()) => {
                let first = cx.first_token(n).ok_or_else(|| shape_error("clause"))?;
                parts.push(clause_break(cx, &first, top));
                parts.push(compound(cx, n)?);
            }
            SyntaxElement::Node(n) => {
                if with_items && n.kind() == k::WITH_ITEM {
                    let items: Vec<SyntaxNode> = ks
                        .iter()
                        .filter_map(|e| e.as_node())
                        .filter(|n| n.kind() == k::WITH_ITEM)
                        .cloned()
                        .collect();
                    if i == first_with_item(&ks) {
                        if prev.is_some() {
                            parts.push(sp());
                        }
                        let magic = style(cx).magic_comma
                            && ks
                                .iter()
                                .position(|e| tok_kind(e) == Some(k::RPAR))
                                .is_some_and(|p| p > 0 && tok_kind(&ks[p - 1]) == Some(k::COMMA));
                        parts.push(with_items_doc(cx, &items, &ks, magic)?);
                    }
                } else {
                    let no_space = matches!(
                        n.kind(),
                        k::PARAMETERS | k::TYPE_PARAMS | k::ARGUMENTS | k::IDENTIFIER
                    ) && prev.is_some_and(|p| {
                        matches!(p, SyntaxElement::Node(x) if matches!(x.kind(), k::IDENTIFIER | k::TYPE_PARAMS))
                    });
                    if prev.is_some() && !no_space {
                        parts.push(sp());
                    }
                    parts.push(header_node(cx, node, n)?);
                }
            }
        }
        prev = Some(e);
        i += 1;
    }
    Ok(concat(parts))
}

fn first_with_item(ks: &[SyntaxElement]) -> usize {
    ks.iter()
        .position(|e| e.as_node().is_some_and(|n| n.kind() == k::WITH_ITEM))
        .unwrap_or(0)
}

/// Is the statement (or the compound statement a clause belongs to) at module level?
fn at_top(node: &SyntaxNode) -> bool {
    let n = if is_clause(node.kind()) {
        node.parent()
    } else {
        Some(node.clone())
    };
    n.and_then(|n| n.parent())
        .is_some_and(|p| p.kind() == k::MOD_MODULE)
}

/// A line break before a clause keyword, with the blank lines and comments in front of it.
fn clause_break(cx: &FormatCtx<'_>, tok: &SyntaxToken, top: bool) -> Doc {
    let cap = if top { 2 } else { 1 };
    let mut parts = vec![hardline()];
    for _ in 0..comments::blank_lines_before(cx.grammar, tok).min(cap) {
        parts.push(hardline());
    }
    parts.push(cx.lead(tok));
    concat(parts)
}

/// `@expr`.
fn decorator(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let mut parts = Vec::new();
    let last = cx.last_token(node);
    if let Some(l) = &last {
        cx.hold_trailing(l);
    }
    for e in kids(cx, node) {
        match &e {
            SyntaxElement::Token(t) => parts.push(cx.token(t)),
            SyntaxElement::Node(n) => parts.push(paren::maybe_parenthesize(cx, n)?),
        }
    }
    if let Some(l) = &last {
        parts.push(trailing_inline(cx, l));
    }
    Ok(concat(parts))
}

/// A header's expression, in the way of the statement it belongs to.
fn header_node(cx: &FormatCtx<'_>, parent: &SyntaxNode, n: &SyntaxNode) -> R {
    match n.kind() {
        k::PARAMETERS => list::parameters(cx, n),
        k::ARGUMENTS => list::class_arguments(cx, n),
        k::TYPE_PARAMS => list::type_params(cx, n),
        k::IDENTIFIER | k::WITH_ITEM => expr::expr(cx, n),
        k::PATTERN_MATCH_VALUE
        | k::PATTERN_MATCH_SEQUENCE
        | k::PATTERN_MATCH_MAPPING
        | k::PATTERN_MATCH_CLASS
        | k::PATTERN_MATCH_AS
        | k::PATTERN_MATCH_OR
        | k::PATTERN_MATCH_SINGLETON
        | k::PATTERN_MATCH_STAR => paren::pattern(cx, n),
        k::PARENTHESIZED if parent.kind() == k::MATCH_CASE && is_pattern_in_parens(n) => {
            paren::pattern(cx, n)
        }
        _ if n.prev_sibling_or_token().is_some_and(|p| {
            p.kind() == k::RARROW
                || p.kind() == k::WHITESPACE
                    && p.prev_sibling_or_token()
                        .is_some_and(|q| q.kind() == k::RARROW)
        }) =>
        {
            // The parameters split before the annotation does; without parameters the
            // annotation is parenthesized when it does not fit.
            let params = parent
                .children()
                .find(|c| c.kind() == k::PARAMETERS)
                .is_some_and(|p| kids(cx, &p).len() > 2);
            if params && !n.descendants_with_tokens().any(|e| e.kind() == k::COMMENT) {
                Ok(flatten(expr::expr(cx, n)?))
            } else if params {
                expr::expr(cx, n)
            } else {
                paren::maybe_parenthesize(cx, n)
            }
        }
        _ => {
            let target = parent.kind() == k::STMT_FOR
                && n.next_sibling_or_token()
                    .is_some_and(|_| header_is_target(cx, parent, n));
            if target {
                paren::for_target(cx, n)
            } else if matches!(
                parent.kind(),
                k::STMT_IF | k::STMT_WHILE | k::ELIF_ELSE_CLAUSE | k::MATCH_CASE
            ) {
                paren::maybe_parenthesize_test(cx, n)
            } else {
                paren::maybe_parenthesize(cx, n)
            }
        }
    }
}

/// A parenthesized pattern (not the guard).
fn is_pattern_in_parens(n: &SyntaxNode) -> bool {
    let mut cur = n.clone();
    while cur.kind() == k::PARENTHESIZED {
        let Some(inner) = cur.children().next() else {
            return false;
        };
        cur = inner;
    }
    matches!(
        cur.kind(),
        k::PATTERN_MATCH_VALUE
            | k::PATTERN_MATCH_SEQUENCE
            | k::PATTERN_MATCH_MAPPING
            | k::PATTERN_MATCH_CLASS
            | k::PATTERN_MATCH_AS
            | k::PATTERN_MATCH_OR
            | k::PATTERN_MATCH_SINGLETON
            | k::PATTERN_MATCH_STAR
    )
}

/// Is `n` the loop variable of a `for` (the node before `in`)?
fn header_is_target(cx: &FormatCtx<'_>, parent: &SyntaxNode, n: &SyntaxNode) -> bool {
    let ks = kids(cx, parent);
    ks.iter()
        .position(|e| e.as_node() == Some(n))
        .and_then(|p| ks.get(p + 1))
        .is_some_and(|e| tok_kind(e) == Some(k::IN))
}

/// The items of a `with`, in parentheses (one per line) when they do not fit.
fn with_items_doc(
    cx: &FormatCtx<'_>,
    items: &[SyntaxNode],
    ks: &[SyntaxElement],
    magic: bool,
) -> R {
    let token = |kind| {
        ks.iter()
            .find_map(|e| e.as_token().filter(|t| t.kind() == kind))
            .cloned()
    };
    let (open, close) = (token(k::LPAR), token(k::RPAR));
    let commas: Vec<SyntaxToken> = ks
        .iter()
        .filter_map(|e| e.as_token().filter(|t| t.kind() == k::COMMA))
        .cloned()
        .collect();
    let open_comment = open.as_ref().is_some_and(|o| has_comments(cx, o));
    let end = close
        .as_ref()
        .map_or_else(Vec::new, |c| comments::leading_comments(cx.grammar, c));
    let comma_comment = commas.iter().any(|c| has_comments(cx, c));
    if items.len() == 1 && !open_comment && end.is_empty() && !comma_comment {
        return expr::expr(cx, &items[0]);
    }
    if let Some(c) = close.as_ref().filter(|_| !end.is_empty()) {
        cx.hold_leading(c);
    }
    let mut parts = Vec::new();
    for (i, it) in items.iter().enumerate() {
        if i > 0 {
            parts.push(ln());
        }
        parts.push(expr::expr(cx, it)?);
        match commas.get(i) {
            Some(c) => {
                parts.push(lit(","));
                parts.push(cx.trailing_comments_doc(c));
            }
            None => parts.push(trailing_comma()),
        }
    }
    if !end.is_empty() {
        parts.push(hardline());
        parts.push(core_format::comment_lines(&end));
    }
    let open_doc = match &open {
        Some(o) if open_comment => cx.token(o),
        _ => doc::if_break(lit("("), Doc::Nil),
    };
    let body = concat(vec![
        open_doc,
        indent(concat(vec![softline(), concat(parts)])),
        softline(),
        doc::if_break(lit(")"), Doc::Nil),
    ]);
    Ok(
        if magic || open_comment || !end.is_empty() || comma_comment {
            doc::group_broken(body)
        } else {
            group(body)
        },
    )
}
