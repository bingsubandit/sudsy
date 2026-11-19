//! The parenthesized argument list that breaks after its `(` and puts the arguments on one
//! continuation line, else one per line. A sole argument that hugs (`hug_last_argument`) or a sole
//! lambda with an expression body keeps its head on the line of the `(`; arguments in pairs that
//! the source lays out in rows keep their rows ([`tabular`]); a blank line in front of a comment
//! stays; after a format string the arguments are a group of their own. The list of a call
//! (`#arguments`) is broken by these rules, the list of an annotation (`#annotation-arguments`)
//! all or none.
//!
//! concept: hanging-argument-list
//! what: Argument lists on one line, after the parenthesis on a continuation line, or one per line.
//! tools: google-java-format argument lists
//! reads: tag:arguments

use super::{CallStyle, Printer, hugs_last, tabular};
use crate::FormatCtx;
use crate::comments;
use crate::doc::{self, Doc};
use crate::error::FormatError;
use crate::seq::{self, first};
use core_syntax::{SyntaxElement, SyntaxNode};

type R = Result<Doc, FormatError>;

fn is_comma(e: &SyntaxElement) -> bool {
    matches!(e, SyntaxElement::Token(t) if t.text() == ",")
}

/// `d` `levels` levels in.
fn cont(levels: usize, d: Doc) -> Doc {
    (0..levels).fold(d, |d, _| doc::indent(d))
}

/// A blank line stands in front of the element in the source (comments in front of it excluded).
fn blank_before(cx: &FormatCtx<'_>, e: &SyntaxElement) -> bool {
    match e {
        SyntaxElement::Node(n) => super::blank_before(cx, n),
        SyntaxElement::Token(t) => comments::blank_lines_before(cx.grammar, t) > 0,
    }
}

/// A format string with its arguments: the first item is a string (or a sum of strings) with a
/// `%`, and arguments follow it.
fn format_call(cx: &FormatCtx<'_>, items: &[SyntaxElement]) -> bool {
    let g = cx.grammar;
    items.len() > 2
        && items.first().is_some_and(|e| {
            first(cx, e).is_some_and(|t| g.has_tag(t.kind(), "string"))
                && (g.has_tag(e.kind(), "string") || g.has_tag(e.kind(), "binary"))
                && match e {
                    SyntaxElement::Token(t) => t.text().contains('%'),
                    SyntaxElement::Node(n) => n.text().to_string().contains('%'),
                }
        })
}

/// A function with an expression body.
fn expression_function(cx: &FormatCtx<'_>, e: &SyntaxElement) -> bool {
    e.as_node().is_some_and(|n| {
        cx.grammar.has_tag(n.kind(), "huggable-function")
            && n.last_child()
                .is_some_and(|b| !cx.grammar.has_tag(b.kind(), "block"))
    })
}

/// The list `node` (`(a, b)`; an empty one stays `()`). `levels`: the indentation levels of a
/// continuation line, one more set of them when `extra` (the list stands in the head of a broken
/// member chain, where an argument does not hug). A node that is no list is a shape error.
pub fn paren_list(
    cx: &FormatCtx<'_>,
    node: &SyntaxNode,
    p: &dyn Printer,
    call: &CallStyle,
    levels: usize,
    extra: bool,
) -> R {
    let els = seq::kids(cx, node);
    if els.len() < 2 {
        return Err(FormatError::Shape("argument list".into()));
    }
    if els.len() == 2 {
        return p.seq(&els);
    }
    let g = cx.grammar;
    let ends = || Ok::<_, FormatError>((p.el(&els[0])?, p.el(&els[els.len() - 1])?));
    let outer = |inner: Doc| if extra { cont(levels, inner) } else { inner };
    let items = &els[1..els.len() - 1];
    let args = g.has_tag(node.kind(), "arguments");
    // Arguments in pairs that the source lays out in rows stay in rows.
    let values: Vec<&SyntaxElement> = items.iter().filter(|e| !is_comma(e)).collect();
    if args && values.len().is_multiple_of(2) && tabular::columns(cx, &values) == Some(2) {
        let table = tabular::rows_of(items, 2, &doc::line(), |e| p.el(e))?;
        let inner = cont(levels, doc::concat(vec![doc::hardline(), table]));
        let (open, close) = ends()?;
        return Ok(doc::group(doc::concat(vec![open, outer(inner), close])));
    }
    let format = args && format_call(cx, items);
    let (mut parts, mut rest) = (Vec::new(), Vec::new());
    for (i, e) in items.iter().enumerate() {
        let sink = if format && i >= 2 {
            &mut rest
        } else {
            &mut parts
        };
        if i > 0
            && blank_before(cx, e)
            && first(cx, e).is_some_and(|t| !comments::leading_comments(g, &t).is_empty())
        {
            sink.push(doc::hardline());
        }
        sink.push(p.el(e)?);
        if is_comma(e) {
            sink.push(doc::line());
        }
    }
    if format {
        parts.push(doc::group(doc::concat(rest)));
    }
    if args && items.len() == 1 && expression_function(cx, &items[0]) {
        // The function goes on a line of its own when it fits there, else its head stays.
        let item = doc::shared(doc::concat(parts));
        let hugged = outer(item.clone());
        let body = match item.flat_width() {
            Some(width) => {
                let own = cont(levels, doc::concat(vec![doc::softline(), item]));
                doc::fits_below(levels * (1 + usize::from(extra)), width, outer(own), hugged)
            }
            None => hugged,
        };
        let (open, close) = ends()?;
        return Ok(doc::group(doc::concat(vec![open, body, close])));
    }
    let (open, close) = ends()?;
    let hug = !extra
        && items.len() == 1
        && items[0]
            .as_node()
            .is_some_and(|n| hugs_last(cx, call, n, 1));
    if hug {
        return Ok(doc::group(doc::concat(vec![
            open,
            doc::concat(parts),
            close,
        ])));
    }
    let body = doc::concat(parts);
    let body = if g.has_tag(node.kind(), "annotation-arguments") {
        body
    } else {
        doc::group(body)
    };
    let inner = cont(levels, doc::concat(vec![doc::softline(), body]));
    Ok(doc::group(doc::concat(vec![open, outer(inner), close])))
}
