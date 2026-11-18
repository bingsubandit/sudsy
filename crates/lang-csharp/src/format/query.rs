//! LINQ query expressions: `from x in xs where ... select ...` on one line if it fits, else one
//! clause per line below the `from`.

use super::options::preserve_query_breaks;
use super::seq::{R, seq};
use crate::generated::kind as k;
use core_format::seq::{children, el};
use core_format::{FormatCtx, doc};
use core_syntax::{SyntaxElement, SyntaxNode};

pub fn query_expression(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let els = children(cx, node);
    let [SyntaxElement::Node(from), SyntaxElement::Node(body)] = els.as_slice() else {
        return seq(cx, node);
    };
    // After `=` the assignment's own indentation applies.
    let clauses = doc::concat(vec![doc::line(), cx.format(body)?]);
    let after_equals = node.parent().is_some_and(|p| {
        p.kind() == k::EQUALS_VALUE_CLAUSE || cx.grammar.has_tag(p.kind(), "assignment")
    });
    Ok(doc::group(doc::concat(vec![
        if preserve_query_breaks(cx.options) && cx.has_line_break(node, node.text_range()) {
            doc::break_parent()
        } else {
            doc::nil()
        },
        cx.format(from)?,
        if after_equals {
            clauses
        } else {
            doc::indent(clauses)
        },
    ])))
}

/// The clauses, each after a `line` of the enclosing query group.
pub fn query_body(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let mut parts = Vec::new();
    for (i, e) in children(cx, node).iter().enumerate() {
        if i > 0 {
            parts.push(doc::line());
        }
        parts.push(el(cx, e)?);
    }
    Ok(doc::concat(parts))
}

/// `into x`, then the rest of the query.
pub fn query_continuation(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let els = children(cx, node);
    let [
        SyntaxElement::Token(into),
        SyntaxElement::Token(name),
        SyntaxElement::Node(body),
    ] = els.as_slice()
    else {
        return seq(cx, node);
    };
    Ok(doc::concat(vec![
        cx.token(into),
        doc::lit(" "),
        cx.token(name),
        doc::line(),
        cx.format(body)?,
    ]))
}
