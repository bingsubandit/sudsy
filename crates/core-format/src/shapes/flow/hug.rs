//! A bracketed list that holds a comment at a line break lines up with its opening bracket: the
//! first item stays on the line of the bracket and every later line starts under it
//! (`align_after_open_bracket: align`); where the first line is too wide, the list hangs as the
//! source has it.
//!
//! concept: bracket-hug-list
//! what: A bracketed list with a comment at a line break: first item on the bracket line, the rest aligned under it.
//! tools: clang-format AlignAfterOpenBracket
//! reads: option:align_after_open_bracket

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
        // (a list with a preprocessor directive keeps the indentation of the source)
        let directive = |e: &SyntaxElement| {
            e.as_node().is_some_and(|n| {
                n.descendants()
                    .any(|d| cx.grammar.has_tag(d.kind(), "directive"))
            })
        };
        // (the cheap tests first: most lists have no comment at a break, and the scans of the items are long)
        let hug = rules.hug_after(open)?;
        (matches!(
            (open.text(), close.text()),
            ("(", ")") | ("[", "]") | ("{", "}")
        ) && els.iter().any(|e| self.comment_at_break(cx, e))
            && !els.iter().any(directive)
            && !els[1..els.len() - 1].iter().any(|e| match e {
                SyntaxElement::Node(n) => rules.hug_unless(n),
                SyntaxElement::Token(t) => t.text() == "{",
            }))
        .then_some(hug)
        .filter(|&hug| !(hug && self.aligned_in_source(cx, els)))
    }

    /// The comments after the `{` of a list of a `hug_comment_down` kind whose first item is on the
    /// next line: they stand on the line below the bracket, in front of the item.
    pub(super) fn moved_down(
        &self,
        cx: &FormatCtx<'_>,
        els: &[SyntaxElement],
    ) -> Option<Vec<comments::Comment>> {
        // (a language without such lists does not look at the tokens)
        if els.len() < 3 || !self.rules?.has_hug_comment_down() {
            return None;
        }
        let open = last(cx, els.first()?)?;
        let head = first(cx, els.get(1)?)?;
        if open.text() != "{" || !self.rules?.hug_comment_down(&open) {
            return None;
        }
        let trailing = comments::trailing_comments(cx.grammar, &open);
        let real = trailing
            .iter()
            .all(|c| c.kind == comments::CommentKind::Block);
        let alone = comments::leading_comments(cx.grammar, &head).is_empty();
        (!trailing.is_empty() && real && alone && comments::break_after(cx.grammar, &open))
            .then_some(trailing)
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
        // (blanks between the bracket and the first item are not kept: the list lines up with the bracket)
        if target != width_before(&open, tab).0 + 1 {
            return false;
        }
        self.lines_start_at(cx, &els[1..els.len() - 1], target)
    }

    /// Does every line the source breaks between the elements start at the column `target`?
    fn lines_start_at(&self, cx: &FormatCtx<'_>, items: &[SyntaxElement], target: usize) -> bool {
        let g = cx.grammar;
        let tab = cx.options.int(core_config::opt::INDENT_SIZE) as usize;
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

    /// Do the lines the source breaks in the flow of `els` all start under what the flow starts with: under
    /// the first item after its bracket, or under its first token? Such a flow is aligned to that column in
    /// the output, whatever the blanks in front of it are there.
    pub(super) fn lines_aligned(&self, cx: &FormatCtx<'_>, els: &[SyntaxElement]) -> bool {
        let g = cx.grammar;
        let n = els.len();
        let broke = |i: usize| match (last(cx, &els[i]), first(cx, &els[i + 1])) {
            (Some(a), Some(_)) => comments::break_after(g, &a),
            _ => false,
        };
        if n < 2 || !(0..n - 1).any(broke) {
            return false;
        }
        let bracketed = els[0]
            .as_token()
            .is_some_and(|t| matches!(t.text(), "(" | "[" | "{"));
        if bracketed {
            return n > 2 && !broke(n - 2) && self.aligned_in_source(cx, els);
        }
        // (a line behind a comment is a continuation, not an alignment)
        if els
            .iter()
            .any(|e| last(cx, e).is_some_and(|t| !comments::trailing_comments(g, &t).is_empty()))
        {
            return false;
        }
        let tab = cx.options.int(core_config::opt::INDENT_SIZE) as usize;
        let Some(start) = first(cx, &els[0]) else {
            return false;
        };
        // (a flow that starts in the middle of a line, not a sequence of lines under one another)
        let (col, indent) = width_before(&start, tab);
        col > indent && self.lines_start_at(cx, els, col)
    }

    /// Does a comment in `e` stand next to a line break (an own-line comment, or one that ends its line)?
    fn comment_at_break(&self, cx: &FormatCtx<'_>, e: &SyntaxElement) -> bool {
        let g = cx.grammar;
        // (a directive between the items is no comment)
        let real = |c: &comments::Comment| c.kind != comments::CommentKind::Directive;
        let at = |t: &core_syntax::SyntaxToken| {
            comments::leading_comments(g, t).iter().any(real)
                || (comments::break_after(g, t)
                    && comments::trailing_comments(g, t).iter().any(real))
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
                                if comments::leading_comments(g, &head).iter().any(real) {
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

    /// The columns the lines of the list need from its bracket, as the output has them (the bracket, then
    /// the widest line behind it): the text behind the closing bracket (`) {`, `;`) is added by the printer's
    /// `fits` to the widest line, so it is taken off the other lines here.
    pub(super) fn hug_width(
        &self,
        cx: &FormatCtx<'_>,
        els: &[SyntaxElement],
        docs: &[Doc],
    ) -> Option<(usize, bool)> {
        let open = last(cx, els.first()?)?;
        let n = els.len();
        // (a closer that breaks off the last line stands on a line of its own: its text is no part of any line)
        let below = n > 2
            && self
                .between(cx, &last(cx, &els[n - 2])?, &first(cx, &els[n - 1])?)
                .is_some_and(|d| d.will_break());
        let tail = if below { 0 } else { bracket_tail(&open)? };
        let (mut line, mut widest) = (0usize, 0usize);
        for i in 1..n {
            let (a, b) = (last(cx, &els[i - 1])?, first(cx, &els[i])?);
            let sep = if i == 1 {
                self.spaced(cx, &a, &b, false)
            } else {
                self.between(cx, &a, &b)
            };
            if sep.as_ref().is_some_and(Doc::will_break) {
                widest = widest.max(line.saturating_sub(tail));
                line = 0;
            } else {
                line += sep.and_then(|s| s.flat_width()).unwrap_or(0);
            }
            let d = &docs[i];
            // (a token with comments behind it: the comments count with one blank before each, as the
            // printer does not count a line comment in `fits` and aligns the others)
            if let SyntaxElement::Token(t) = &els[i] {
                let cs = comments::trailing_comments(cx.grammar, t);
                if !cs.is_empty() {
                    line += t.text().chars().count()
                        + cs.iter()
                            .map(|c| 1 + usize::from(c.element.text_range().len()))
                            .sum::<usize>();
                    continue;
                }
            }
            // (a line comment behind the last token of an item is not in the doc's width)
            let lc: usize = comments::trailing_comments(cx.grammar, &last(cx, &els[i])?)
                .iter()
                .filter(|c| c.kind != comments::CommentKind::Block)
                .map(|c| 1 + usize::from(c.element.text_range().len()))
                .sum();
            if d.will_break() {
                line += d.width_to_hardline();
                widest = widest
                    .max(line.saturating_sub(tail))
                    .max(d.inner_line_width());
                line = d.tail_width() + lc;
            } else {
                line += d.flat_width().unwrap_or_else(|| d.width_to_hardline()) + lc;
            }
        }
        Some((1 + widest.max(line), below))
    }

    /// The name token in front of the list `open` starts.
    fn name_of(&self, open: &core_syntax::SyntaxToken) -> Option<core_syntax::SyntaxToken> {
        let mut cur = open.prev_token();
        while let Some(t) = cur {
            if !t.text().trim().is_empty() {
                return Some(t);
            }
            cur = t.prev_token();
        }
        None
    }

    /// Is `name` the name of a declaration whose lined-up list holds comments and starts too wide
    /// behind the return type, but fits on a line of its own?
    #[inline]
    pub(super) fn name_break(&self, cx: &FormatCtx<'_>, name: &core_syntax::SyntaxToken) -> bool {
        let Some(rules) = self.rules.filter(|r| r.has_name_break()) else {
            return false;
        };
        let decl = name.parent();
        if !decl.as_ref().is_some_and(|d| rules.name_break(d.kind())) {
            return false;
        }
        let mut next = name.next_token();
        while let Some(t) = next.as_ref().filter(|t| t.text().trim().is_empty()) {
            next = t.next_token();
        }
        let Some(open) = next.filter(|t| t.text() == "(") else {
            return false;
        };
        let Some(list) = open.parent() else {
            return false;
        };
        // (a return type of more than one word stands in front of the name)
        let word = self.name_of(name);
        if decl
            .and_then(|d| cx.first_token(&d))
            .is_none_or(|t| Some(&t) == word.as_ref() || &t == name)
            || !word.is_some_and(|t| rules.name_break_after(t.kind()))
        {
            return false;
        }
        let limit = cx.options.int(core_config::opt::LINE_WIDTH) as usize;
        let (Some(widest), Some(first)) = (self.hug_widest(&open), self.hug_first(&open)) else {
            return false;
        };
        let indent = bracket_indent(&open, 1);
        let els = crate::seq::children(cx, &list);
        self.hugs(cx, &els) == Some(true)
            && indent + bracket_prefix(&open) + first.max(widest) > limit
            && indent + name.text().chars().count() + 1 + widest <= limit
    }

    /// The columns of the first line of the list `open` starts, behind its bracket.
    fn hug_first(&self, open: &core_syntax::SyntaxToken) -> Option<usize> {
        let text = open.parent()?.text().to_string();
        // (the line of the first item, which a break behind the bracket leaves on the next line)
        let (i, first) = text
            .lines()
            .enumerate()
            .find(|(_, l)| !matches!(l.trim(), "" | "(" | "[" | "{"))?;
        Some(squeezed_width(first.trim()) - usize::from(i == 0))
    }

    /// The columns of the widest line of the list `open` starts, behind its bracket.
    fn hug_widest(&self, open: &core_syntax::SyntaxToken) -> Option<usize> {
        let list = open.parent()?;
        let tail = bracket_tail(open)?;
        let text = list.text().to_string();
        let lines: Vec<&str> = text.lines().collect();
        let width = |i: usize, l: &str| squeezed_width(l.trim()) - usize::from(i == 0);
        let last = lines.len().checked_sub(1)?;
        let widest = lines
            .iter()
            .enumerate()
            .map(|(i, l)| width(i, l).saturating_sub(if i == last { 0 } else { tail }))
            .max()?;
        Some(widest)
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

/// The columns the line of `open` takes up to and with the bracket (its text from the first non-blank).
pub(super) fn bracket_prefix(open: &core_syntax::SyntaxToken) -> usize {
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
    before.trim_start().chars().count() + 1
}

/// The columns of the blanks the line of `open` starts with.
pub(super) fn bracket_indent(open: &core_syntax::SyntaxToken, tab: usize) -> usize {
    width_before(open, tab.max(1)).1
}

/// The columns of the text behind the list `open` starts, on the line of its last token of code
/// (`) {`, `;`; a comment is left out, a brace stands a blank apart).
pub(super) fn bracket_tail(open: &core_syntax::SyntaxToken) -> Option<usize> {
    let list = open.parent()?;
    let mut tail = String::new();
    // (the trivia that end the list belong to it: the text behind starts at its last token of code)
    let end = std::iter::successors(list.last_token(), |t| t.prev_token())
        .find(|t| !t.text().trim().is_empty());
    let mut cur = end.and_then(|t| t.next_token());
    while let Some(t) = cur {
        if let Some(i) = t.text().find('\n') {
            tail.push_str(&t.text()[..i]);
            break;
        }
        tail.push_str(t.text());
        cur = t.next_token();
    }
    let tail = tail.split("//").next()?.split("/*").next()?.trim();
    // (blanks in front of a closer are not output: `} },` is `}},`)
    let mut width = squeezed_width(tail);
    let chars: Vec<char> = tail.chars().collect();
    for w in chars.windows(2) {
        if matches!(w[0], ' ' | '\t') && matches!(w[1], ')' | ']' | '}' | ';' | ',') {
            width -= 1;
        }
    }
    Some(width + usize::from(tail.starts_with('{')))
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
