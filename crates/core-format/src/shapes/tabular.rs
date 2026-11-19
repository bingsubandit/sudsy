//! Elements the source lays out in a table stay in rows: a row starts at the column the first
//! element starts at, the elements to its right (in the source) belong to it, and the rows are as
//! wide as the first one (the last may be shorter).
//!
//! concept: tabular-rows
//! what: Argument and element lists that the source lays out in rows keep their rows.
//! tools: google-java-format tabular arguments and array initializers

use crate::FormatCtx;
use crate::comments;
use crate::doc::{self, Doc};
use crate::error::FormatError;
use core_syntax::{SyntaxElement, SyntaxKind, SyntaxToken};

/// The number of columns of the table that the elements (given by their first tokens) form in the
/// source, `None` when they do not form one: fewer than two rows, a single column, rows of
/// different widths, or a column in which no two rows have the same kind (`kinds`: the kind of
/// each element), or a row (`lens`: the source width of each element, with its comma and space)
/// wider than `limit`: a row that has to wrap is not a table.
fn columns_of(
    starts: &[SyntaxToken],
    kinds: &[SyntaxKind],
    lens: &[usize],
    limit: usize,
) -> Option<usize> {
    let cols: Vec<usize> = starts.iter().map(comments::column).collect();
    let first = *cols.first()?;
    let mut rows: Vec<usize> = Vec::new();
    for &c in &cols {
        match rows.last_mut() {
            Some(n) if c > first => *n += 1,
            _ => rows.push(1),
        }
    }
    let width = *rows.first()?;
    let (last, body) = rows.split_last()?;
    if rows.len() < 2 || width < 2 || !body.iter().all(|&n| n == width) || *last > width {
        return None;
    }
    let wide = lens
        .chunks(width)
        .any(|row| row.iter().map(|n| n + 2).sum::<usize>() > limit);
    if wide {
        return None;
    }
    let parallel = (0..width).all(|col| {
        let column: Vec<SyntaxKind> = (0..rows.len())
            .filter_map(|r| kinds.get(r * width + col).copied())
            .collect();
        column
            .iter()
            .any(|k| column.iter().filter(|o| *o == k).count() >= 2)
    });
    parallel.then_some(width)
}

/// The number of columns of the table that the `items` (the elements of a list without its
/// separators) form in the source: `columns_of` on their first tokens, kinds (a node tagged
/// `unary` counts as its operand) and source widths, rows at most `line_width - 8` wide.
pub fn columns(cx: &FormatCtx<'_>, items: &[&SyntaxElement]) -> Option<usize> {
    let g = cx.grammar;
    let (mut starts, mut kinds, mut lens) = (Vec::new(), Vec::new(), Vec::new());
    for e in items {
        let first = match e {
            SyntaxElement::Token(t) => t.clone(),
            SyntaxElement::Node(n) => {
                let mut t = n.first_token()?;
                while g.is_trivia(t.kind()) {
                    t = t.next_token()?;
                }
                t
            }
        };
        let end = match e {
            SyntaxElement::Token(t) => t.text_range().end(),
            SyntaxElement::Node(n) => {
                let mut t = n.last_token()?;
                while g.is_trivia(t.kind()) {
                    t = t.prev_token()?;
                }
                t.text_range().end()
            }
        };
        lens.push(usize::from(end - first.text_range().start()));
        starts.push(first);
        kinds.push(parallel_kind(cx, e));
    }
    let limit = crate::print_options(cx.options)
        .line_width
        .saturating_sub(8);
    columns_of(&starts, &kinds, &lens, limit)
}

/// The kind that makes elements of a table parallel: a signed operand counts as the operand.
fn parallel_kind(cx: &FormatCtx<'_>, e: &SyntaxElement) -> SyntaxKind {
    match e {
        SyntaxElement::Node(n) if cx.grammar.has_tag(n.kind(), "unary") => n
            .children_with_tokens()
            .find(|c| {
                c.as_node().is_some()
                    || c.as_token()
                        .is_some_and(|t| t.text().starts_with(char::is_alphanumeric))
            })
            .map_or(n.kind(), |c| parallel_kind(cx, &c)),
        _ => e.kind(),
    }
}

/// `items` and their commas in rows of `columns` (see [`rows`]); `doc` prints an element.
pub fn rows_of(
    items: &[SyntaxElement],
    columns: usize,
    sep: &Doc,
    doc: impl Fn(&SyntaxElement) -> Result<Doc, FormatError>,
) -> Result<Doc, FormatError> {
    let mut docs: Vec<Doc> = Vec::new();
    for e in items {
        let d = doc(e)?;
        if e.as_token().is_some_and(|t| t.text() == ",") {
            if let Some(last) = docs.pop() {
                docs.push(doc::concat(vec![last, d]));
            }
        } else {
            docs.push(d);
        }
    }
    Ok(rows(docs, columns, sep))
}

/// The elements (each with its comma, when it has one) in rows of `columns`: one row per line,
/// the elements of a row separated by `sep` (a row that is too wide wraps like a fill).
pub fn rows(items: Vec<Doc>, columns: usize, sep: &Doc) -> Doc {
    let mut out = Vec::new();
    let mut row = Vec::new();
    for item in items {
        if row.len() == columns {
            out.push(filled(std::mem::take(&mut row), sep));
        }
        row.push(item);
    }
    out.push(filled(row, sep));
    doc::join(&doc::hardline(), out)
}

fn filled(row: Vec<Doc>, sep: &Doc) -> Doc {
    let mut parts = Vec::new();
    for (i, item) in row.into_iter().enumerate() {
        if i > 0 {
            parts.push(sep.clone());
        }
        parts.push(item);
    }
    doc::fill(parts)
}
