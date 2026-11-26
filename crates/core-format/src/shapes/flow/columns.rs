//! A long braced list is laid out in columns: every item is padded so that the next one starts in
//! the same column on each row, and the number of columns is the least that gives the fewest rows
//! within the line width (`clang-format`'s table layout of initializer lists).
//!
//! concept: column-list
//! what: A bracketed list of six or more items in rows of equal columns, each cell padded to the widest of its column.
//! tools: clang-format CommaSeparatedList
//! reads: option:column_list_min_commas

use super::Flow;
use crate::FormatCtx;
use crate::comments::{self, CommentKind};
use crate::doc::{self, Doc};
use crate::error::FormatError;
use crate::seq::{first, last};
use core_syntax::{SyntaxElement, SyntaxToken};

/// One item of the list: its elements, the doc of its text, and what follows it.
struct Item {
    text: Doc,
    /// The first token of the item.
    head: SyntaxToken,
    width: usize,
    comma: Option<SyntaxToken>,
    /// The token the trailing comment follows: the comma, else the last token of the item.
    anchor: SyntaxToken,
    /// The trailing `//` comment after its comma, its width.
    comment: Option<usize>,
}

/// A layout in `columns` columns: the width each column takes and how many rows it has.
struct Format {
    columns: usize,
    /// The columns the widest row takes, the text behind the list (`tail`) after the last row included.
    need: usize,
    sizes: Vec<usize>,
    rows: usize,
    width: usize,
}

impl Flow<'_> {
    /// The list as a table, `None` where it is no table (too short, a comment or a nested brace in it,
    /// one row) and the ordinary layout is used.
    pub(super) fn table(
        &self,
        cx: &FormatCtx<'_>,
        els: &[SyntaxElement],
        doc: &impl Fn(&SyntaxElement) -> Result<Doc, FormatError>,
    ) -> Option<Result<Doc, FormatError>> {
        let rules = self.rules?;
        let open = els.first()?.as_token()?;
        let close = els.last()?.as_token()?;
        // (a language that does not have the option has no column lists)
        let least = match cx.options.get(core_config::opt::COLUMN_LIST_MIN_COMMAS) {
            Some(core_config::OptionValue::Int(n)) if *n > 0 => *n as usize,
            _ => return None,
        };
        if els.len() < 4 || open.text() != "{" {
            return None;
        }
        let g = cx.grammar;
        if !comments::trailing_comments(g, open).is_empty() {
            return None;
        }
        // (too few commas: the items are not laid out)
        let commas = els
            .iter()
            .filter(|e| e.as_token().is_some_and(|t| t.text() == ","))
            .count();
        if commas < least {
            return None;
        }
        let items = self.table_items(cx, &els[1..els.len() - 1], doc)?;
        let items = items.ok()?;
        let commas = items.iter().filter(|i| i.comma.is_some()).count();
        let limit = cx.options.int(core_config::opt::LINE_WIDTH) as usize;
        let nested = open
            .parent()
            .is_some_and(|p| p.ancestors().skip(1).any(|a| rules.in_hug_node(&a)));
        if commas < least || (nested && commas < least + 14) {
            return None;
        }
        let trailing = items
            .last()
            .is_some_and(|i| i.comma.is_some() || i.comment.is_some());
        let cont = cx.options.int(core_config::opt::CONTINUATION_INDENT) as usize;
        let tail = super::hug::bracket_tail(open)?;
        let formats_fit = formats(&items, limit, 0);
        // (one row: the list stays as it is)
        if formats_fit.iter().all(|f| f.rows <= 1) {
            return None;
        }
        let levels = {
            let unit = cx.options.int(core_config::opt::INDENT_SIZE).max(1) as usize;
            (cont / unit).max(1)
        };
        // (the format with the most columns that fits, then the fewest columns with as many rows)
        let best_of = |all: &[Format], f: &Format| -> Option<usize> {
            all.iter()
                .filter(|o| o.rows == f.rows && o.columns <= f.columns)
                .min_by_key(|o| o.columns)
                .map(|o| o.columns)
        };
        // (formats with as many rows share the layout of their fewest columns: each is built once)
        let built: std::cell::RefCell<Vec<(usize, bool, Option<Doc>)>> = Default::default();
        let doc_of = |all: &[Format], f: &Format, levels: Option<usize>| {
            let columns = best_of(all, f)?;
            if let Some(d) = built
                .borrow()
                .iter()
                .find(|b| b.0 == columns && b.1 == levels.is_some())
            {
                return d.2.clone();
            }
            let one = all.iter().find(|o| o.columns == columns)?;
            let d = self
                .table_doc(cx, open, close, &items, one, trailing, levels)
                .map(doc::shared);
            built
                .borrow_mut()
                .push((columns, levels.is_some(), d.clone()));
            d
        };
        // The layout with its rows under the bracket's line: a format `f` with `rows` rows, the
        // formats of at most `max_rows` rows tried first (most columns first), else `otherwise`.
        let mut broken: Vec<(usize, usize, Doc)> = Vec::new();
        let mut by_columns: Vec<&Format> = formats_fit.iter().collect();
        by_columns.sort_by_key(|f| std::cmp::Reverse(f.columns));
        for f in &by_columns {
            broken.push((f.rows, f.width, doc_of(&formats_fit, f, Some(levels))?));
        }
        let break_chain = |max_rows: usize, otherwise: Option<Doc>| -> Option<Doc> {
            let mut chain = otherwise;
            for (_, width, one) in broken.iter().rev().filter(|b| b.0 <= max_rows) {
                chain = Some(match chain {
                    None => one.clone(),
                    // (a closing brace on a line of its own ends the last row: `;` behind it does not count)
                    Some(rest) if trailing => {
                        doc::fits_below_line(levels, *width, one.clone(), rest)
                    }
                    Some(rest) => doc::fits_below(levels, *width, one.clone(), rest),
                });
            }
            chain
        };
        if trailing || items.iter().any(|i| i.comment.is_some()) {
            return break_chain(usize::MAX, None).map(Ok);
        }
        // No trailing comma and no comment: the first row stays on the line of the bracket and the
        // others stand under it, where the output column leaves room for it, and unless breaking
        // after the bracket saves more than the line it takes.
        let all = formats(&items, limit, tail);
        let mut hugging: Vec<&Format> = all.iter().collect();
        hugging.sort_by_key(|f| std::cmp::Reverse(f.columns));
        let mut chain = break_chain(usize::MAX, None)?;
        for f in hugging.iter().rev() {
            let one = doc_of(&all, f, None)?;
            let alt = break_chain(f.rows.saturating_sub(2), Some(one))?;
            chain = doc::fits_at_cursor_line(1 + f.need, alt, chain);
        }
        Some(Ok(chain))
    }

    fn table_items(
        &self,
        cx: &FormatCtx<'_>,
        els: &[SyntaxElement],
        doc: &impl Fn(&SyntaxElement) -> Result<Doc, FormatError>,
    ) -> Option<Result<Vec<Item>, FormatError>> {
        let g = cx.grammar;
        let gap = cx
            .options
            .int(core_config::opt::SPACES_BEFORE_TRAILING_COMMENTS)
            .max(1) as usize;
        let mut items = Vec::new();
        let mut cur: Vec<&SyntaxElement> = Vec::new();
        let mut flush = |cur: &mut Vec<&SyntaxElement>, comma: Option<SyntaxToken>| -> Option<()> {
            if cur.is_empty() {
                return Some(());
            }
            let (head, tail) = (first(cx, cur[0])?, last(cx, cur[cur.len() - 1])?);
            // (a designated initializer, a blank line or a comment in front of an item keeps the list as it is)
            if !comments::leading_comments(g, &head).is_empty()
                || matches!(head.text(), "." | "[")
                || comments::blank_lines_before(g, &head) > 0
                || (comma.is_some() && !comments::trailing_comments(g, &tail).is_empty())
                || cur.iter().any(|e| match e {
                    SyntaxElement::Node(n) => n
                        .descendants_with_tokens()
                        .any(|d| d.as_token().is_some_and(|t| matches!(t.text(), "{" | "}"))),
                    SyntaxElement::Token(t) => matches!(t.text(), "{" | "}"),
                })
            {
                return None;
            }
            let mut parts = Vec::new();
            let mut prev: Option<SyntaxToken> = None;
            for e in cur.iter() {
                if let (Some(p), Some(b)) = (prev.as_ref(), first(cx, e)) {
                    let sep = self.between(cx, p, &b);
                    if sep.as_ref().is_some_and(Doc::will_break) {
                        return None;
                    }
                    parts.extend(sep);
                }
                // (the comment after the last item is the row's, not part of the cell)
                let has_comment =
                    comma.is_none() && !comments::trailing_comments(g, &tail).is_empty();
                parts.push(match e {
                    SyntaxElement::Token(t) if has_comment => cx.token_text(t),
                    // (a node that ends the list in a comment: its text as written, when it is one line)
                    SyntaxElement::Node(n) if has_comment => {
                        let text = n.text().to_string();
                        let end = usize::from(tail.text_range().end() - n.text_range().start());
                        let text = text.get(..end).unwrap_or(&text).trim();
                        if text.contains(['\n', '/']) {
                            return None;
                        }
                        doc::text(text)
                    }
                    _ => doc(e).ok()?,
                });
                prev = last(cx, e);
            }
            let text = doc::concat(parts);
            let width = text.flat_width()? + 1;
            // (the comment after the comma, or after the last item when it has none)
            let anchor = comma.clone().unwrap_or(tail);
            let cs = comments::trailing_comments(g, &anchor);
            let comment = match cs.as_slice() {
                [] => None,
                [one] if one.kind == CommentKind::Line => Some(gap + one.text().chars().count()),
                _ => return None,
            };
            items.push(Item {
                text,
                head,
                width,
                comma,
                anchor,
                comment,
            });
            cur.clear();
            Some(())
        };
        for e in els {
            match e {
                SyntaxElement::Token(t) if t.text() == "," => {
                    flush(&mut cur, Some(t.clone()))?;
                }
                _ => cur.push(e),
            }
        }
        flush(&mut cur, None)?;
        Some(Ok(items))
    }

    #[allow(clippy::too_many_arguments)]
    fn table_doc(
        &self,
        cx: &FormatCtx<'_>,
        open: &SyntaxToken,
        close: &SyntaxToken,
        items: &[Item],
        f: &Format,
        trailing: bool,
        levels: Option<usize>,
    ) -> Option<Doc> {
        // (a comment in front of the list stays in front of its bracket)
        let opener = doc::concat(vec![cx.leading_comments_doc(open), cx.token_text(open)]);
        let mut rows: Vec<Doc> = Vec::new();
        let (mut col, mut line): (usize, Vec<Doc>) = (0, Vec::new());
        let mut break_next = false;
        for (i, item) in items.iter().enumerate() {
            if col == f.columns || break_next {
                rows.push(doc::concat(std::mem::take(&mut line)));
                col = 0;
            }
            line.push(item.text.clone());
            if let Some(comma) = &item.comma {
                line.push(cx.token_text(comma));
                let last = i + 1 == items.len();
                break_next = item.comment.is_some();
                let ends_row = col + 1 == f.columns || break_next || last;
                if !ends_row {
                    line.push(doc::pad(1 + f.sizes[col] - item.width));
                } else if item.comment.is_some() {
                    if col + 1 == f.columns {
                        line.push(doc::pad(f.sizes[col] - item.width));
                    }
                    line.push(cx.trailing_comments_doc(comma));
                }
            } else if item.comment.is_some() {
                if col + 1 == f.columns {
                    line.push(doc::pad(f.sizes[col] - item.width));
                }
                line.push(cx.trailing_comments_doc(&item.anchor));
            } else {
                line.push(doc::lit(""));
            }
            col += 1;
        }
        rows.push(doc::concat(line));
        let mut body = Vec::new();
        for (i, r) in rows.into_iter().enumerate() {
            if i > 0 {
                body.push(doc::hardline());
            }
            body.push(r);
        }
        let Some(levels) = levels else {
            // (the rows stand under the first, after the bracket)
            let (head, last) = (items.first()?.head.clone(), items.last()?.anchor.clone());
            return Some(doc::align(
                doc::ALIGN_FROM_LINE,
                doc::concat(vec![
                    opener.clone(),
                    doc::align(
                        doc::ALIGN_TO_COLUMN,
                        doc::concat(vec![
                            // (a blank behind the brace only where the list is padded)
                            if self.pad_blank(cx, open, &head) == Some(true) {
                                doc::lit(" ")
                            } else {
                                Doc::Nil
                            },
                            doc::concat(body),
                        ]),
                    ),
                    self.between(cx, &last, close).unwrap_or(Doc::Nil),
                    cx.token_text(close),
                ]),
            ));
        };
        let mut inner = doc::concat(vec![doc::hardline(), doc::concat(body)]);
        for _ in 0..levels {
            inner = doc::indent(inner);
        }
        let tail = if trailing {
            doc::concat(vec![doc::hardline(), cx.token_text(close)])
        } else {
            cx.token_text(close)
        };
        Some(doc::concat(vec![opener.clone(), inner, tail]))
    }
}

/// The layouts of `items` in 1 to `limit / 3` columns that fit `limit`.
/// The layouts of the items in 1, 2, ... columns that fit `limit` columns; `tail` columns follow the last
/// row (`;`).
fn formats(items: &[Item], limit: usize, tail: usize) -> Vec<Format> {
    let mut out = Vec::new();
    for columns in 1..=(limit / 3).max(1) {
        let mut sizes = vec![0usize; columns];
        let mut mins = vec![usize::MAX; columns];
        let (mut rows, mut col, mut full) = (1, 0, false);
        let mut forced = false;
        for item in items {
            if forced || col == columns {
                rows += 1;
                col = 0;
            }
            if col == columns - 1 {
                full = true;
            }
            sizes[col] = sizes[col].max(item.width);
            mins[col] = mins[col].min(item.width);
            forced = item.comment.is_some();
            col += 1;
        }
        if !full {
            break;
        }
        // (the width of a padded row; a comment goes behind the row, or behind its last item
        // where the row ends early)
        let base = columns - 1 + sizes.iter().sum::<usize>();
        let mut width = base;
        let (mut col, mut forced) = (0, false);
        for item in items {
            if forced || col == columns {
                col = 0;
            }
            if let Some(c) = item.comment {
                let end = if col == columns - 1 {
                    base
                } else {
                    sizes[..col].iter().sum::<usize>() + col + item.width
                };
                width = width.max(end + c);
            }
            forced = item.comment.is_some();
            col += 1;
        }
        if (0..columns.saturating_sub(1)).any(|c| sizes[c] - mins[c] > 10) {
            continue;
        }
        if width > limit && columns > 1 {
            continue;
        }
        // (the last row is as wide as its cells, and the text behind the list follows it)
        let last = (items.len() - 1) % columns;
        let uncommented = items.iter().all(|i| i.comment.is_none());
        let last_row =
            sizes[..last].iter().sum::<usize>() + last + items[items.len() - 1].width + tail;
        if uncommented && last_row > limit && columns > 1 {
            continue;
        }
        out.push(Format {
            columns,
            need: width.max(if uncommented { last_row } else { 0 }),
            sizes,
            rows,
            width,
        });
    }
    out
}
