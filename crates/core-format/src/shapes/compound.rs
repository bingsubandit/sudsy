//! Compound statements: a head, a body and the continuation clauses that follow it (`else`, the
//! `while` of a `do`). A body that is a block (`#block`) or an empty statement
//! (`#empty-statement`) stays on the head's line; any other statement goes on the next line,
//! indented, when it does not fit. A continuation keyword joins the `}` of a block body and
//! stands on a line of its own after any other body.

use crate::FormatCtx;
use crate::comments::{self, CommentKind};
use crate::doc::{self, Doc};
use crate::shapes::flow::{first, last};
use core_syntax::{SyntaxElement, SyntaxToken};

/// The body of a compound statement after its head. `inline`: a body that is not a block still
/// follows on the line (`else if`); `grouped`: the move to the next line is a group of its own.
pub fn body(
    cx: &FormatCtx<'_>,
    body: &SyntaxElement,
    printed: Doc,
    inline: bool,
    grouped: bool,
) -> Doc {
    let (g, kind) = (cx.grammar, body.kind());
    if g.has_tag(kind, "empty-statement") {
        printed
    } else if inline || g.has_tag(kind, "block") {
        doc::concat(vec![gap_before(cx, body), printed])
    } else {
        let moved = doc::indent(doc::concat(vec![doc::line(), printed]));
        if grouped { doc::group(moved) } else { moved }
    }
}

/// A blank before a body, or a line break when a line comment ends the line of the keyword in
/// front of it (`else // c`).
fn gap_before(cx: &FormatCtx<'_>, body: &SyntaxElement) -> Doc {
    let prev = first(cx, body)
        .and_then(|t| t.prev_token())
        .and_then(|mut t| {
            while cx.grammar.is_trivia(t.kind()) {
                t = t.prev_token()?;
            }
            Some(t)
        });
    let ends_line = prev.is_some_and(|t| {
        !matches!(t.text(), ")" | ";" | "]" | "}")
            && comments::trailing_comments(cx.grammar, &t)
                .iter()
                .any(|c| c.kind != CommentKind::Block)
    });
    if ends_line {
        doc::hardline()
    } else {
        doc::lit(" ")
    }
}

/// What stands between `prev` (the body before a continuation keyword `kw`) and `kw`: a blank
/// after a block, a line break after anything else or after a line comment, and the blank line
/// the source has before a comment in front of `kw`.
pub fn continuation(cx: &FormatCtx<'_>, prev: &SyntaxElement, kw: &SyntaxToken) -> Doc {
    let g = cx.grammar;
    let line_comment = last(cx, prev).is_some_and(|t| {
        comments::trailing_comments(g, &t)
            .iter()
            .any(|c| c.kind == CommentKind::Line)
    });
    let mut parts = vec![if g.has_tag(prev.kind(), "block") && !line_comment {
        doc::lit(" ")
    } else {
        doc::hardline()
    }];
    if !comments::leading_comments(g, kw).is_empty() && blank_before_first_comment(cx, kw) {
        parts.extend([doc::hardline(), doc::hardline()]);
    }
    doc::concat(parts)
}

/// Do two line breaks come before the first comment in front of `tok`?
fn blank_before_first_comment(cx: &FormatCtx<'_>, tok: &SyntaxToken) -> bool {
    let mut run = Vec::new();
    let mut cur = tok.prev_token();
    while let Some(t) = cur.filter(|t| cx.grammar.is_trivia(t.kind())) {
        cur = t.prev_token();
        run.push(t);
    }
    run.iter()
        .rev()
        .take_while(|t| !cx.grammar.has_tag(t.kind(), "comment"))
        .map(|t| comments::count_breaks(t.text()))
        .sum::<usize>()
        > 1
}
