//! Array initializers: `{a, b}` on one line, else after the `{` one per line, or filled when every
//! element is short. Elements that the source lays out in rows keep their rows
//! ([`tabular`](super::tabular)); a trailing comma of the source stays and breaks the braces.
//!
//! concept: initializer-layout
//! what: Braced element lists on one line, one element per line, filled or in the source's rows.
//! tools: google-java-format array initializers

use super::{Printer, between, tabular};
use crate::FormatCtx;
use crate::doc::{self, Doc};
use crate::error::FormatError;
use crate::seq::{self, first, last};
use core_syntax::{SyntaxElement, SyntaxNode};

fn is_comma(e: &SyntaxElement) -> bool {
    matches!(e, SyntaxElement::Token(t) if t.text() == ",")
}

/// The characters the element has in the source, without comments around it.
fn source_len(cx: &FormatCtx<'_>, e: &SyntaxElement) -> usize {
    match (first(cx, e), last(cx, e)) {
        (Some(a), Some(b)) => usize::from(b.text_range().end() - a.text_range().start()),
        _ => 0,
    }
}

/// The initializer `node`: its first and last element are the braces. The elements are filled
/// when each is shorter than `fill_below` characters in the source (0: never). An initializer
/// without elements is a shape error.
pub fn initializer(
    cx: &FormatCtx<'_>,
    node: &SyntaxNode,
    p: &dyn Printer,
    fill_below: usize,
) -> Result<Doc, FormatError> {
    let els = seq::kids(cx, node);
    let (Some(open), Some(close)) = (els.first(), els.last()) else {
        return Err(FormatError::Shape("initializer".into()));
    };
    if els.len() < 3 {
        return p.seq(&els);
    }
    let inner = &els[1..els.len() - 1];
    let items: Vec<_> = inner.iter().filter(|e| !is_comma(e)).collect();
    if items.is_empty() {
        return p.seq(&els);
    }
    let trailing = inner.last().is_some_and(is_comma);
    if let Some(columns) = tabular::columns(cx, &items) {
        let table = tabular::rows_of(inner, columns, &doc::line(), |e| p.el(e))?;
        return Ok(doc::group(doc::concat(vec![
            p.el(open)?,
            doc::indent(doc::concat(vec![doc::hardline(), table])),
            doc::hardline(),
            p.el(close)?,
        ])));
    }
    let filled = items.iter().all(|e| source_len(cx, e) < fill_below);
    let commas: Vec<_> = inner.iter().filter(|e| is_comma(e)).collect();
    let mut parts = Vec::new();
    for (i, e) in items.iter().enumerate() {
        let comma = match commas.get(i) {
            Some(c) => p.el(c)?,
            None => doc::nil(),
        };
        parts.push(doc::concat(vec![p.el(e)?, comma]));
        if i + 1 < items.len() {
            let broken = commas
                .get(i)
                .and_then(|c| c.as_token())
                .is_some_and(|c| seq::line_comment_after(cx, c));
            parts.push(if broken { doc::hardline() } else { doc::line() });
        }
    }
    let body = if filled {
        doc::fill(parts)
    } else {
        doc::concat(parts)
    };
    // The items form a group of their own, so that they stay on one line below a broken `{`.
    let body = if filled || !trailing {
        doc::group(body)
    } else {
        body
    };
    let mut out = vec![between(p.el(open)?, body, p.el(close)?)];
    if trailing {
        out.push(doc::break_parent());
    }
    Ok(doc::group(doc::concat(out)))
}
