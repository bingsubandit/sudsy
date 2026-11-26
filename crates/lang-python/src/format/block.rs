//! Blocks: the statements of a module or a suite one per line with ruff's blank lines, each with
//! its comments.
//!
//! concept: block-and-statements, blank-lines-between-members

use super::comment::*;
use super::docstring;
use super::stmt;
use super::util::*;
use crate::format::{keep_final_semicolon, module_docstring};
use crate::generated::kind as k;
use core_format::FormatCtx;
use core_format::comments;
use core_format::doc::{Doc, concat, hardline, indent, lit};
use core_syntax::SyntaxNode;
use core_syntax::roles::{has, has_any};
use std::cell::Cell;

thread_local! {
    static DEPTH: Cell<usize> = const { Cell::new(0) };
}

/// Indentation levels of the block being built.
pub(crate) fn depth() -> usize {
    DEPTH.get()
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Seq {
    Module,
    Class,
    Function,
    Other,
}

pub(crate) fn module(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    DEPTH.set(0);
    let stmts: Vec<SyntaxNode> = node
        .children()
        .filter(|n| has(n.kind(), "statement"))
        .collect();
    let body = sequence(cx, &stmts, true, Seq::Module)?;
    let mut parts = vec![body];
    if let Some(eof) = node
        .children_with_tokens()
        .filter_map(|e| e.into_token())
        .find(|t| t.kind() == k::END_OF_FILE)
    {
        let cs = cx.remaining_comments(&eof);
        if !cs.is_empty() {
            if !stmts.is_empty() {
                parts.push(hardline());
                // Source blank lines before the comments, at most two.
                for _ in 0..comments::blank_lines_before(cx.grammar, &eof).min(2) {
                    parts.push(hardline());
                }
            }
            parts.push(core_format::comment_lines(&cs));
        }
    }
    Ok(concat(parts))
}

/// The tags of a function or class definition.
pub(crate) const DEF: &[&str] = &["function", "type-declaration"];

/// `...` alone, without comments in front of it: a body that stays on the header line.
pub(crate) fn is_ellipsis(cx: &FormatCtx<'_>, n: &SyntaxNode) -> bool {
    n.kind() == k::STMT_EXPR
        && n.children()
            .next()
            .is_some_and(|e| e.kind() == k::EXPR_ELLIPSIS_LITERAL)
        && cx
            .first_token(n)
            .is_some_and(|t| comments::leading_comments(cx.grammar, &t).is_empty())
}

/// [`is_ellipsis`] with no comment after the header's colon: the body joins the header line.
pub(crate) fn joins_header(cx: &FormatCtx<'_>, n: &SyntaxNode) -> bool {
    is_ellipsis(cx, n)
        && !cx.first_token(n).is_some_and(|t| {
            std::iter::successors(t.prev_token(), |p| p.prev_token())
                .find(|p| !cx.grammar.is_trivia(p.kind()))
                .is_some_and(|p| {
                    p.kind() == k::COLON && !comments::trailing_comments(cx.grammar, &p).is_empty()
                })
        })
}

/// A function whose body is `...` on the header line.
pub(crate) fn is_stub(cx: &FormatCtx<'_>, n: &SyntaxNode) -> bool {
    n.kind() == k::STMT_FUNCTION_DEF && {
        let body: Vec<SyntaxNode> = n
            .children()
            .filter(|c| has(c.kind(), "statement"))
            .collect();
        matches!(body.as_slice(), [only] if is_ellipsis(cx, only))
    }
}

/// Does the statement end with a function or class (at any depth)?
fn ends_with_def(n: &SyntaxNode) -> bool {
    if has_any(n.kind(), DEF) {
        return true;
    }
    n.children()
        .filter(|c| has(c.kind(), "statement") || has(c.kind(), "branch"))
        .last()
        .is_some_and(|c| ends_with_def(&c))
}

/// A string-only expression statement (an implicit concatenation of plain strings too).
fn is_docstring(n: &SyntaxNode) -> bool {
    n.kind() == k::STMT_EXPR
        && n.children().next().is_some_and(|e| {
            e.kind() == k::EXPR_STRING_LITERAL
                && e.children().all(|c| c.kind() == k::STRING_LITERAL)
        })
}

/// Can the first statement of a block of this kind be a docstring?
fn has_docstring(cx: &FormatCtx<'_>, seq: Seq) -> bool {
    match seq {
        Seq::Other => false,
        Seq::Module => module_docstring(cx.options),
        _ => true,
    }
}

/// The statements of a block, one per line with the blank lines of ruff.
pub(crate) fn sequence(cx: &FormatCtx<'_>, stmts: &[SyntaxNode], top: bool, seq: Seq) -> R {
    let cap = if top { 2 } else { 1 };
    let mut parts: Vec<Doc> = Vec::new();
    let mut region = core_format::ignore::IgnoreRun::default();
    let mut prev_skip = false;
    for (i, s) in stmts.iter().enumerate() {
        let Some(first) = cx.first_token(s) else {
            continue;
        };
        let mut src = comments::blank_lines_before(cx.grammar, &first);
        // Comments a block before has printed: the blank lines after them count.
        let consumed = cx.consumed(&first);
        if consumed > 0 {
            let all = comments::leading_comments(cx.grammar, &first);
            if let Some(c) = all.get(consumed - 1) {
                src = c.breaks_after.saturating_sub(1);
            }
        }
        // ruff counts the blank lines after a statement from its end: a `;` there hides them.
        if i > 0
            && !has_any(stmts[i - 1].kind(), COMPOUND)
            && semicolon(cx, &stmts[i - 1]).is_some()
        {
            src = 0;
        }
        // Comments followed by a blank line stay with the statement before.
        let mut own_doc = Doc::Nil;
        {
            let cs = cx.remaining_comments(&first);
            if let Some(j) = cs.iter().rposition(|c| c.breaks_after >= 2) {
                if i > 0 {
                    parts.push(hardline());
                    let before = if ends_with_def(&stmts[i - 1]) {
                        cap
                    } else {
                        src.min(cap)
                    };
                    for _ in 0..before {
                        parts.push(hardline());
                    }
                }
                own_doc = core_format::comment_lines_max(&cs[..=j], cap);
                parts.push(own_doc.clone());
                cx.consume(&first, j + 1);
                src = cs[j].breaks_after - 1;
            }
        }
        let mut blank = src.min(cap);
        let docstring = i == 0 && has_docstring(cx, seq) && is_docstring(s);
        if i > 0 {
            let prev = &stmts[i - 1];
            let prev_doc = i == 1 && has_docstring(cx, seq) && is_docstring(prev);
            let after_def = own_doc == Doc::Nil && ends_with_def(prev);
            if has_any(s.kind(), DEF) && src == 0 && is_stub(cx, prev) && own_doc == Doc::Nil {
                blank = 0;
            } else if has_any(s.kind(), DEF) || after_def {
                blank = cap;
            } else if has(prev.kind(), "import") && !has(s.kind(), "import") {
                blank = blank.max(1);
            } else if prev_doc && matches!(seq, Seq::Module | Seq::Class) {
                blank = 1;
            }
            parts.push(hardline());
        } else if own_doc != Doc::Nil {
            // (comments of the definition still to print: they keep the blank lines of the source)
            blank = if has_any(s.kind(), DEF) && cx.remaining_comments(&first).is_empty() {
                cap
            } else {
                blank
            };
            parts.push(hardline());
        } else if docstring || matches!(seq, Seq::Class | Seq::Module) {
            blank = 0;
        } else if has_any(s.kind(), DEF) && seq == Seq::Other {
            // A function or class in the block of `if`, `for`, ...: a blank line before it.
            let comments_first = !comments::leading_comments(cx.grammar, &first).is_empty();
            blank = if comments_first { blank.min(1) } else { 1 };
        } else if matches!(seq, Seq::Function) || has_any(s.kind(), DEF) {
            blank = blank.min(1);
        } else {
            blank = 0;
        }
        if i == 0 && src > 0 && comments::starts_file(cx.grammar, &first) && own_doc == Doc::Nil {
            blank = 0;
        }
        // `# fmt: off` ... `# fmt: on`, `sudsy-ignore`: statements kept as written (ignore rows).
        let mark = core_format::ignore::mark_in(
            cx.grammar,
            &comments::leading_comments(cx.grammar, &first),
        );
        let in_region = region.skips(mark);
        let skip = in_region || is_fmt_skip(cx, s);
        if i > 0 {
            blank = core_format::ignore::source_blanks(in_region || prev_skip, src, cap)
                .unwrap_or(blank);
        }
        prev_skip = in_region;
        for _ in 0..blank {
            parts.push(hardline());
        }
        parts.push(if skip {
            verbatim_item(cx, s)?
        } else {
            item(cx, s, docstring)?
        });
    }
    Ok(concat(parts))
}

/// A statement as written (its comments around it as in the source).
pub(crate) fn verbatim_item(cx: &FormatCtx<'_>, s: &SyntaxNode) -> R {
    let first = cx.first_token(s).ok_or_else(|| shape_error("statement"))?;
    let leading = cx.lead(&first);
    let last = semicolon(cx, s)
        .or_else(|| cx.last_token(s))
        .ok_or_else(|| shape_error("statement"))?;
    Ok(concat(vec![leading, cx.verbatim_range(&first, &last)]))
}

/// One statement with its leading comments and trailing comment.
pub(crate) fn item(cx: &FormatCtx<'_>, s: &SyntaxNode, docstring: bool) -> R {
    let first = cx.first_token(s).ok_or_else(|| shape_error("statement"))?;
    let leading = cx.lead(&first);
    let compound = has_any(s.kind(), COMPOUND);
    let last = (!compound).then(|| cx.last_token(s)).flatten();
    if let Some(l) = &last {
        cx.hold_trailing(l);
    }
    let mut body = if docstring {
        docstring::statement(cx, s)?
    } else {
        stmt::statement(cx, s)?
    };
    // The `;` that ends the file is code (a notebook cell's, which hides its output).
    if !compound
        && keep_final_semicolon(cx.options)
        && s.parent().is_some_and(|p| p.kind() == k::MOD_MODULE)
        && s.next_sibling().is_none()
        && semicolon(cx, s).is_some()
    {
        body = concat(vec![body, lit(";")]);
    }
    let trailing = last.map_or(Doc::Nil, |l| trailing_inline(cx, &l));
    let after_semicolon = semicolon(cx, s)
        .filter(|_| !compound)
        .map_or(Doc::Nil, |m| trailing_inline(cx, &m));
    Ok(concat(vec![leading, body, trailing, after_semicolon]))
}

/// `:` followed by the statements one level deeper.
pub(crate) fn block(cx: &FormatCtx<'_>, stmts: &[SyntaxNode], seq: Seq) -> R {
    if stmts.is_empty() {
        return Ok(Doc::Nil);
    }
    DEPTH.set(DEPTH.get() + 1);
    let body = sequence(cx, stmts, false, seq);
    DEPTH.set(DEPTH.get() - 1);
    let body = body?;
    let col = cx
        .first_token(&stmts[0])
        .map(|t| comments::column(&t))
        .unwrap_or(0);
    let end = cx.end_comments(&stmts[stmts.len() - 1], col);
    Ok(indent(concat(vec![hardline(), body, end])))
}
