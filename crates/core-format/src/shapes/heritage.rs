//! concept: heritage-clause-layout
//! what: extends, implements and base lists on or below the type head.
//! tools: google-java-format class headers, ktlint class-signature, Prettier class heritage
//! reads: tag:heritage

use super::Printer;
use crate::FormatCtx;
use crate::doc::{self, Doc};
use crate::error::FormatError;
use crate::seq;
use core_syntax::{SyntaxElement, SyntaxNode};

/// How the supertypes of a type declaration are laid out.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum HeritageLayout {
    /// All on the line of the colon while they fit; else all below it.
    Fit,
    /// One per line, the first below the colon.
    Wrapped,
    /// One per line, the first after the colon on the line of the closing parenthesis of the
    /// parameter list before it.
    Hugged,
}

/// `head : first, second tail`: `head` and `colon` are printed, then the supertypes in `layout`,
/// then `tail` (the body) after a blank.
pub fn heritage(
    head: Doc,
    colon: Doc,
    items: Vec<Doc>,
    layout: HeritageLayout,
    tail: Option<Doc>,
) -> Doc {
    if layout == HeritageLayout::Fit {
        let mut all = vec![doc::line()];
        for (i, d) in items.into_iter().enumerate() {
            if i > 0 {
                all.push(doc::lit(", "));
            }
            all.push(d);
        }
        // A head that spans lines (the parameters) keeps the supertypes on the line of its end.
        let mut parts = if head.will_break() {
            all[0] = doc::lit(" ");
            vec![head, doc::lit(" "), colon, doc::concat(all)]
        } else {
            vec![doc::group(doc::concat(vec![
                head,
                doc::lit(" "),
                colon,
                doc::indent(doc::concat(all)),
            ]))]
        };
        if let Some(t) = tail {
            parts.push(doc::lit(" "));
            parts.push(t);
        }
        return doc::concat(parts);
    }
    let mut parts = vec![head, doc::lit(" "), colon];
    let mut items = items.into_iter();
    let first = items.next().unwrap_or(Doc::Nil);
    let sep = doc::hardline();
    let mut rest = Vec::new();
    for d in items {
        rest.push(doc::lit(","));
        rest.push(sep.clone());
        rest.push(d);
    }
    match layout {
        HeritageLayout::Fit => unreachable!(),
        HeritageLayout::Hugged => {
            parts.push(doc::lit(" "));
            parts.push(first);
            parts.push(doc::indent(doc::concat(rest)));
        }
        HeritageLayout::Wrapped => {
            let mut all = vec![doc::hardline(), first];
            all.extend(rest);
            parts.push(doc::indent(doc::concat(all)));
        }
    }
    if let Some(t) = tail {
        parts.push(doc::lit(" "));
        parts.push(t);
    }
    doc::concat(parts)
}

/// A clause that is a keyword and a list (`throws A, B`, `implements A, B`): a group, the list
/// after the keyword, a break after each comma that indents the next item `levels` levels.
pub fn clause_list(
    cx: &FormatCtx<'_>,
    node: &SyntaxNode,
    p: &dyn Printer,
    levels: usize,
) -> Result<Doc, FormatError> {
    let mut parts = Vec::new();
    for (i, e) in seq::kids(cx, node).iter().enumerate() {
        if i == 1 {
            parts.push(doc::lit(" "));
        }
        parts.push(p.el(e)?);
        if matches!(e, SyntaxElement::Token(t) if t.text() == ",") {
            parts.push((0..levels).fold(doc::line(), |d, _| doc::indent(d)));
        }
    }
    Ok(doc::group(doc::concat(parts)))
}
