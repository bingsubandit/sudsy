//! The body of a lambda or a switch rule after its arrow: a block stays on the line of the head,
//! any other body goes on the next line, indented, when it does not fit; a comment after the
//! arrow puts the body on the next line.
//!
//! concept: arrow-body-layout
//! what: The head, the arrow and the body of a lambda or a switch rule.
//! tools: google-java-format lambdas and switch rules

use super::Printer;
use crate::FormatCtx;
use crate::doc::{self, Doc};
use crate::error::FormatError;
use crate::seq;
use core_syntax::{SyntaxKind, SyntaxNode};

/// `head arrow body` (`node`'s children; `arrow` the kind of the arrow token): the body moves
/// `levels` indentation levels in below the head. A node without an arrow or a body is a shape
/// error.
pub fn arrow_body(
    cx: &FormatCtx<'_>,
    node: &SyntaxNode,
    p: &dyn Printer,
    arrow: SyntaxKind,
    levels: usize,
) -> Result<Doc, FormatError> {
    let els = seq::kids(cx, node);
    let at = els.iter().position(|e| e.kind() == arrow);
    let Some((at, body)) = at.map(|i| (i, &els[i + 1..])).filter(|(_, b)| !b.is_empty()) else {
        return Err(FormatError::Shape("arrow body".into()));
    };
    let head = p.seq(&els[..=at])?;
    if cx.grammar.has_tag(body[0].kind(), "block") {
        return Ok(doc::concat(vec![head, doc::lit(" "), p.seq(body)?]));
    }
    let broken = seq::last(cx, &els[at]).is_some_and(|t| seq::line_comment_after(cx, &t));
    let sep = if broken { doc::hardline() } else { doc::line() };
    let moved = doc::group(doc::concat(vec![sep, p.seq(body)?]));
    let body = (0..levels).fold(moved, |d, _| doc::indent(d));
    Ok(doc::group(doc::concat(vec![head, body])))
}
