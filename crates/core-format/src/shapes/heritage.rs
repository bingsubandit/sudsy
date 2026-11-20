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
/// then `tail` (the body) after a blank. `head_breaks`: the parameters in the head span lines
/// (other line breaks in it, annotations on a line of their own, do not count).
pub fn heritage(
    head: Doc,
    head_breaks: bool,
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
        let mut parts = if head_breaks {
            all[0] = doc::lit(" ");
            // (the arguments of a call among them go one level in, like the closing parenthesis)
            vec![head, doc::lit(" "), colon, doc::indent(doc::concat(all))]
        } else {
            // (the group measures from the column of the colon: lines above the head do not count)
            vec![
                head,
                doc::lit(" "),
                colon,
                doc::group(doc::indent(doc::concat(all))),
            ]
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
            parts.push(doc::indent(first));
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

/// Where the colon and the commas of a clause that follows the head go when the clause breaks
/// (`Ctor() : a(1), b(2)`; the option values `before_colon`, `before_comma`, `after_colon`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ClauseBreak {
    /// The colon starts the first line below the head; commas end the lines.
    BeforeColon,
    /// The colon starts the first line below the head; commas start the next lines.
    BeforeComma,
    /// The colon ends the head's line; commas end the lines.
    AfterColon,
}

impl ClauseBreak {
    pub fn from_option(value: &str) -> Option<ClauseBreak> {
        match value {
            "before_colon" => Some(ClauseBreak::BeforeColon),
            "before_comma" => Some(ClauseBreak::BeforeComma),
            "after_colon" => Some(ClauseBreak::AfterColon),
            _ => None,
        }
    }
}

/// `: a, b` after a head: a group that stays on the head's line when it fits and else breaks as
/// `style` says, indented one level.
pub fn clause_break(items: Vec<Doc>, style: ClauseBreak) -> Doc {
    let mut body = Vec::new();
    for (i, item) in items.into_iter().enumerate() {
        match (style, i) {
            (ClauseBreak::AfterColon, 0) => body.extend([doc::lit(" :"), doc::line(), item]),
            (_, 0) => body.extend([doc::line(), doc::lit(": "), item]),
            (ClauseBreak::BeforeComma, _) => body.extend([doc::line(), doc::lit(", "), item]),
            _ => body.extend([doc::lit(","), doc::line(), item]),
        }
    }
    doc::group(doc::indent(doc::concat(body)))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::printer::{PrintOptions, print};

    fn render(style: ClauseBreak, width: usize) -> String {
        let items = vec![doc::lit("a(1)"), doc::lit("b(2)")];
        let mut d = doc::concat(vec![doc::lit("C()"), clause_break(items, style)]);
        let opts = PrintOptions {
            line_width: width,
            ..PrintOptions::default()
        };
        print(&mut d, &opts)
    }

    #[test]
    fn the_clause_stays_on_the_head_line_when_it_fits() {
        assert_eq!(render(ClauseBreak::BeforeColon, 80), "C() : a(1), b(2)");
        assert_eq!(render(ClauseBreak::AfterColon, 80), "C() : a(1), b(2)");
    }

    #[test]
    fn a_broken_clause_puts_the_colon_and_commas_where_the_style_says() {
        assert_eq!(
            render(ClauseBreak::BeforeColon, 8),
            "C()\n  : a(1),\n  b(2)"
        );
        assert_eq!(
            render(ClauseBreak::BeforeComma, 8),
            "C()\n  : a(1)\n  , b(2)"
        );
        assert_eq!(render(ClauseBreak::AfterColon, 8), "C() :\n  a(1),\n  b(2)");
        assert_eq!(
            ClauseBreak::from_option("after_colon"),
            Some(ClauseBreak::AfterColon)
        );
        assert_eq!(ClauseBreak::from_option("x"), None);
    }
}
