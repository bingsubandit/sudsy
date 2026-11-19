//! The body of a lambda or a switch rule after its arrow: a block is the language's own sequence,
//! any other body goes on the next line, indented, when it does not fit; a comment after the
//! arrow puts the body on the next line.
//!
//! concept: assignment-layout
//! what: The head, the arrow and the body of a lambda or a switch rule.
//! tools: google-java-format lambdas and switch rules, CSharpier lambdas

use super::Printer;
use crate::FormatCtx;
use crate::doc::{self, Doc};
use crate::error::FormatError;
use crate::seq;
use core_syntax::{SyntaxKind, SyntaxNode};

/// Where the body of an arrow goes.
#[derive(Clone, Copy)]
pub struct ArrowStyle {
    /// The kind of the arrow token.
    pub arrow: SyntaxKind,
    /// The body moves this many indentation levels in below the head.
    pub levels: usize,
    /// The body is a group of its own: it moves below the head only when it does not fit after
    /// the last line of the head. Else it moves whenever head and body do not fit on one line.
    pub own_group: bool,
}

/// `head arrow body` (`node`'s children): the body is placed as `style` says. A node without an
/// arrow or a body is a shape error.
pub fn arrow_body(
    cx: &FormatCtx<'_>,
    node: &SyntaxNode,
    p: &dyn Printer,
    style: ArrowStyle,
) -> Result<Doc, FormatError> {
    let els = seq::kids(cx, node);
    let at = els.iter().position(|e| e.kind() == style.arrow);
    let Some((at, body)) = at
        .map(|i| (i, &els[i + 1..]))
        .filter(|(_, b)| !b.is_empty())
    else {
        return Err(FormatError::Shape("arrow body".into()));
    };
    if cx.grammar.has_tag(body[0].kind(), "block") {
        return p.seq(&els);
    }
    let head = p.seq(&els[..=at])?;
    let broken = seq::last(cx, &els[at]).is_some_and(|t| seq::line_comment_after(cx, &t));
    let sep = if broken { doc::hardline() } else { doc::line() };
    let moved = doc::concat(vec![sep, p.seq(body)?]);
    let moved = if style.own_group {
        doc::group(moved)
    } else {
        moved
    };
    let body = (0..style.levels).fold(moved, |d, _| doc::indent(d));
    Ok(doc::group(doc::concat(vec![head, body])))
}
