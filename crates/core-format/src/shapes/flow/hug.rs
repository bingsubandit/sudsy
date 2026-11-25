//! A bracketed list that holds a comment at a line break lines up with its opening bracket: the
//! first item stays on the line of the bracket and every later line starts under it
//! (`align_after_open_bracket: align`); where the first line is too wide, the list hangs as the
//! source has it.
//!
//! concept: bracket-hug-list
//! what: A bracketed list with a comment at a line break: first item on the bracket line, the rest aligned under it.
//! tools: clang-format AlignAfterOpenBracket
//! reads: option:align_after_open_bracket, kinds:hug_list

use super::Flow;
use crate::FormatCtx;
use crate::comments;
use crate::doc::{self, Doc};
use crate::seq::{first, last};
use core_syntax::SyntaxElement;

impl Flow<'_> {
    /// Is `els` a bracketed list of a kind that lines up with its bracket, with a comment next to
    /// a line break of the source (the other lists keep the indentation of the source)? `Some(true)`:
    /// its first item may go on the line of the bracket; `Some(false)`: the list hangs (a `{` after `=`).
    pub(super) fn hugs(&self, cx: &FormatCtx<'_>, els: &[SyntaxElement]) -> Option<bool> {
        let aligned = matches!(
            cx.options.get(core_config::opt::ALIGN_AFTER_OPEN_BRACKET),
            Some(core_config::OptionValue::String(s)) if s == "align"
        );
        let (Some(rules), true) = (self.rules, aligned && els.len() > 2) else {
            return None;
        };
        let (Some(open), Some(close)) = (
            els.first().and_then(|e| e.as_token()),
            els.last().and_then(|e| e.as_token()),
        ) else {
            return None;
        };
        (matches!((open.text(), close.text()), ("(", ")") | ("[", "]") | ("{", "}"))
            && rules.in_hug_list(open)
            && els.iter().any(|e| self.comment_at_break(cx, e)))
        .then(|| rules.hug_after(open))
        .filter(|&hug| !(hug && self.aligned_in_source(cx, els)))
    }

    /// The first item is on the line of the bracket and every line of the list starts under it.
    fn aligned_in_source(&self, cx: &FormatCtx<'_>, els: &[SyntaxElement]) -> bool {
        let g = cx.grammar;
        let tab = cx.options.int(core_config::opt::INDENT_SIZE) as usize;
        let Some(open) = last(cx, &els[0]) else {
            return false;
        };
        if comments::break_after(g, &open) {
            return false;
        }
        // The column of the first thing after the bracket.
        let mut next = open.next_token();
        while let Some(t) = next.as_ref().filter(|t| t.text().trim().is_empty()) {
            next = t.next_token();
        }
        let Some(target) = next.map(|t| width_before(&t, tab).0) else {
            return false;
        };
        let items = &els[1..els.len() - 1];
        items.windows(2).all(|w| {
            let (Some(a), Some(b)) = (last(cx, &w[0]), first(cx, &w[1])) else {
                return true;
            };
            if !comments::break_after(g, &a) {
                return true;
            }
            let mut starts: Vec<_> = comments::leading_comments(g, &b)
                .into_iter()
                .filter_map(|c| c.element.into_token())
                .collect();
            starts.push(b);
            starts.iter().all(|t| width_before(t, tab).1 == target)
        })
    }

    /// Does a comment in `e` stand next to a line break (an own-line comment, or one that ends its line)?
    fn comment_at_break(&self, cx: &FormatCtx<'_>, e: &SyntaxElement) -> bool {
        let g = cx.grammar;
        let at = |t: &core_syntax::SyntaxToken| {
            !comments::leading_comments(g, t).is_empty()
                || (comments::break_after(g, t) && !comments::trailing_comments(g, t).is_empty())
        };
        let is_open = |t: &core_syntax::SyntaxToken| matches!(t.text(), "(" | "[" | "{");
        match e {
            SyntaxElement::Token(t) => at(t),
            SyntaxElement::Node(n) => {
                // The comments of this list only: a nested bracket group lays out its own.
                let mut walk = n.preorder_with_tokens();
                while let Some(ev) = walk.next() {
                    match ev {
                        core_syntax::WalkEvent::Enter(SyntaxElement::Node(inner)) => {
                            if let Some(head) = cx.first_token(&inner).filter(is_open) {
                                if !comments::leading_comments(g, &head).is_empty() {
                                    return true;
                                }
                                walk.skip_subtree();
                            }
                        }
                        core_syntax::WalkEvent::Enter(SyntaxElement::Token(t))
                            if !g.is_trivia(t.kind()) && at(&t) =>
                        {
                            return true;
                        }
                        _ => {}
                    }
                }
                false
            }
        }
    }

    /// The columns the lines of the list need from the start of the line of its bracket, as far as
    /// the printer's `fits` does not count them itself: that adds the text behind the closing
    /// bracket (`) {`, `;`) to the widest line, so it is taken off the other lines here.
    pub(super) fn hug_width(&self, open: &core_syntax::SyntaxToken) -> Option<usize> {
        let list = open.parent()?;
        let mut before = String::new();
        let mut cur = open.prev_token();
        while let Some(t) = cur {
            if let Some(i) = t.text().rfind('\n') {
                before.insert_str(0, &t.text()[i + 1..]);
                break;
            }
            before.insert_str(0, t.text());
            cur = t.prev_token();
        }
        let mut tail = String::new();
        let mut cur = list.last_token().and_then(|t| t.next_token());
        while let Some(t) = cur {
            if let Some(i) = t.text().find('\n') {
                tail.push_str(&t.text()[..i]);
                break;
            }
            tail.push_str(t.text());
            cur = t.next_token();
        }
        let tail = squeezed_width(tail.split("//").next()?.split("/*").next()?.trim_end());
        let text = list.text().to_string();
        let lines: Vec<&str> = text.lines().collect();
        let width = |i: usize, l: &str| squeezed_width(l.trim()) - usize::from(i == 0);
        let last = lines.len().checked_sub(1)?;
        let widest = lines
            .iter()
            .enumerate()
            .map(|(i, l)| width(i, l).saturating_sub(if i == last { 0 } else { tail }))
            .max()?;
        Some(before.trim_start().chars().count() + 1 + widest)
    }

    /// The list with its first item on the line of the bracket and the lines the source breaks
    /// under the column after the bracket; `None` where a comment keeps the first item off that line.
    pub(super) fn hugged(
        &self,
        cx: &FormatCtx<'_>,
        els: &[SyntaxElement],
        docs: &[Doc],
    ) -> Option<Doc> {
        let g = cx.grammar;
        let n = els.len();
        let open = last(cx, &els[0])?;
        let head = first(cx, &els[1])?;
        if (comments::break_after(g, &open) && !comments::trailing_comments(g, &open).is_empty())
            || !comments::leading_comments(g, &head).is_empty()
        {
            return None;
        }
        let mut inner = vec![self.spaced(cx, &open, &head, false).unwrap_or(Doc::Nil)];
        let mut prev = open;
        for i in 1..n - 1 {
            let b = first(cx, &els[i])?;
            if i > 1 {
                inner.extend(self.between(cx, &prev, &b));
            }
            inner.push(docs[i].clone());
            prev = last(cx, &els[i])?;
        }
        let close = first(cx, &els[n - 1])?;
        Some(doc::align(
            doc::ALIGN_FROM_LINE,
            doc::concat(vec![
                docs[0].clone(),
                doc::align(doc::ALIGN_TO_COLUMN, doc::concat(inner)),
                self.between(cx, &prev, &close).unwrap_or(Doc::Nil),
                docs[n - 1].clone(),
            ]),
        ))
    }
}

/// The columns the text before `t` on its line takes (a tab goes to the next stop), and the columns of
/// the blanks the line starts with.
fn width_before(t: &core_syntax::SyntaxToken, tab: usize) -> (usize, usize) {
    let mut line = String::new();
    let mut cur = t.prev_token();
    while let Some(p) = cur {
        if let Some(i) = p.text().rfind('\n') {
            line.insert_str(0, &p.text()[i + 1..]);
            break;
        }
        line.insert_str(0, p.text());
        cur = p.prev_token();
    }
    let (mut col, mut indent, mut blank) = (0, 0, true);
    for c in line.chars() {
        let w = if c == '\t' { tab - col % tab } else { 1 };
        col += w;
        if blank && matches!(c, ' ' | '\t') {
            indent = col;
        } else {
            blank = false;
        }
    }
    (col, indent)
}

/// The columns of `line` with every run of blanks as one: the width of its code and comments before
/// the trailing comments are aligned.
fn squeezed_width(line: &str) -> usize {
    let mut blank = false;
    line.chars()
        .filter(|&c| {
            let b = matches!(c, ' ' | '\t');
            let skip = b && blank;
            blank = b;
            !skip
        })
        .count()
}
