//! Content between delimiters on lines of their own when it does not fit: "parenthesize if the
//! content breaks" (ruff's `maybe_parenthesize`, Prettier's `return (...)`), and the same layout
//! with delimiters that are always there.

use crate::FormatCtx;
use crate::doc::{self, Doc};
use core_syntax::SyntaxNode;
use core_syntax::roles::un;

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

/// What follows a keyword that takes an expression (`return a`, `throw a`): a blank and the
/// argument. A binary chain (`#binary-chain`) goes in parentheses on lines of its own when it does
/// not fit; a sequence (`#sequence`) always has its parentheses, on one line.
pub fn keyword_argument(cx: &FormatCtx<'_>, arg: &SyntaxNode, printed: Doc) -> Doc {
    let g = cx.grammar;
    let sequence = g.has_tag(arg.kind(), "sequence");
    if !sequence && !g.has_tag(arg.kind(), "binary-chain") {
        return doc::concat(vec![doc::lit(" "), printed]);
    }
    let paren = |text| {
        if sequence {
            doc::lit(text)
        } else {
            doc::if_break(doc::lit(text), Doc::Nil)
        }
    };
    doc::group(doc::concat(vec![
        doc::lit(" "),
        between(paren("("), printed, paren(")")),
    ]))
}

/// The test between the parentheses of a compound head (`if (` `)`): `soft` lets it move onto
/// lines of its own when it does not fit, except a negated chain (`!(a && b)`), which keeps the
/// head's line and breaks inside its own parentheses.
pub fn head_test(cx: &FormatCtx<'_>, test: &SyntaxNode, printed: Doc, soft: bool) -> Doc {
    if soft && !negated_chain(cx, test) {
        enclosed(Doc::Nil, printed, Doc::Nil)
    } else {
        printed
    }
}

/// `!(a && b)`: a negation of a binary chain.
fn negated_chain(cx: &FormatCtx<'_>, test: &SyntaxNode) -> bool {
    let g = cx.grammar;
    g.has_tag(test.kind(), "unary")
        && g.slot(test, "operator")
            .and_then(|o| o.into_token())
            .is_some_and(|o| {
                // (a comment in front of the `!` is part of the test's text)
                o.text() == "!"
                    && test
                        .descendants_with_tokens()
                        .filter_map(|e| e.into_token())
                        .find(|t| {
                            !g.has_tag(t.kind(), "whitespace") && !g.has_tag(t.kind(), "newline")
                        })
                        == Some(o)
            })
        && g.slot(test, "operand")
            .and_then(|e| e.into_node())
            .is_some_and(|n| g.has_tag(un(&n).kind(), "binary-chain"))
}
