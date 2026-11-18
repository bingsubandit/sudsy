use crate::doc::{self, Doc};

/// How the supertypes of a type declaration are laid out.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum HeritageLayout {
    /// All on the line of the colon.
    Inline,
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
    let mut parts = vec![head, doc::lit(" "), colon];
    let mut items = items.into_iter();
    let first = items.next().unwrap_or(Doc::Nil);
    let sep = if layout == HeritageLayout::Inline {
        doc::lit(" ")
    } else {
        doc::hardline()
    };
    let mut rest = Vec::new();
    for d in items {
        rest.push(doc::lit(","));
        rest.push(sep.clone());
        rest.push(d);
    }
    match layout {
        HeritageLayout::Inline => {
            parts.push(doc::lit(" "));
            parts.push(first);
            parts.extend(rest);
        }
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
