//! Content between delimiters on lines of their own when it does not fit: "parenthesize if the
//! content breaks" (ruff's `maybe_parenthesize`, Prettier's `return (...)`), and the same layout
//! with delimiters that are always there.

use crate::doc::{self, Doc};

/// `open`, the content indented on its own lines, `close`: no group, the group around decides.
pub fn between(open: Doc, inner: Doc, close: Doc) -> Doc {
    doc::concat(vec![
        open,
        doc::indent(doc::concat(vec![doc::softline(), inner])),
        doc::softline(),
        close,
    ])
}

/// [`between`] in a group of its own: the content moves to its own lines when it does not fit.
pub fn enclosed(open: Doc, inner: Doc, close: Doc) -> Doc {
    doc::group(between(open, inner, close))
}

/// The content in parentheses that show only when it breaks.
pub fn maybe_parenthesize(inner: Doc) -> Doc {
    enclosed(
        doc::if_break(doc::lit("("), Doc::Nil),
        inner,
        doc::if_break(doc::lit(")"), Doc::Nil),
    )
}
