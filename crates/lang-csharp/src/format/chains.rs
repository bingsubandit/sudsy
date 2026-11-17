//! Expressions that lay out as a chain: binary operators, member accesses and calls, assignments
//! and the conditional operator.

use super::options::fluid_assignments;
use super::seq::{R, paren_group, seq_of, sig};
use super::tags::{self, Tag};
use crate::generated::kind as k;
use core_format::doc::{self, Doc};
use core_format::shapes::{self, ChainPolicy, ends_in_line_comment, gap_after_operator};
use core_format::{FormatCtx, FormatError};
use core_syntax::{SyntaxElement, SyntaxNode, SyntaxToken};

fn shape(msg: &str) -> FormatError {
    FormatError::Shape(msg.into())
}

// ---------------------------------------------------------------- assignment

/// Does the right-hand side of `=` move to its own line when it does not fit?
fn breaks_after_operator(cx: &FormatCtx<'_>, rhs: &SyntaxNode) -> bool {
    // A comment on its own line before the value keeps the value on the next line.
    core_format::comments::node_leading(cx.grammar, rhs).has_comment
        || tags::has(cx.grammar, rhs.kind(), Tag::BinaryChain)
        || matches!(rhs.kind(), k::CONDITIONAL_EXPRESSION | k::QUERY_EXPRESSION)
}

/// The layout of `lead rhs` for `assignment_layout: fluid`: a string, a binary expression or the
/// initializer of a property moves to its own line when it does not fit, and anything else moves
/// only when its first line (up to its first possible break) does not fit after the operator.
fn fluid_rhs(cx: &FormatCtx<'_>, lead: Doc, rhs: &SyntaxNode, rhs_doc: Doc) -> Doc {
    let has_comment = core_format::comments::node_leading(cx.grammar, rhs).has_comment;
    // The initializer of a property moves as a whole.
    let property_initializer = rhs
        .parent()
        .and_then(|clause| clause.parent())
        .is_some_and(|decl| decl.kind() == k::PROPERTY_DECLARATION);
    let string = matches!(
        rhs.kind(),
        k::STRING_LITERAL_EXPRESSION | k::INTERPOLATED_STRING_EXPRESSION
    );
    // A string that spans lines starts on the line of the operator.
    if string && !has_comment && rhs.text().contains_char('\n') {
        return doc::concat(vec![lead, doc::lit(" "), rhs_doc]);
    }
    if has_comment
        || property_initializer
        || string
        || matches!(
            rhs.kind(),
            k::COLLECTION_EXPRESSION | k::CONDITIONAL_EXPRESSION
        )
        || tags::has(cx.grammar, rhs.kind(), Tag::BinaryChain)
    {
        return doc::concat(vec![
            lead,
            doc::group(doc::indent(doc::concat(vec![doc::line(), rhs_doc]))),
        ]);
    }
    let id = doc::new_group_id();
    doc::concat(vec![
        lead,
        doc::group_with_id(id, doc::indent(doc::line())),
        doc::indent_if_break(id, rhs_doc),
    ])
}

/// `lead rhs`, where `lead` ends with the operator.
fn with_rhs(cx: &FormatCtx<'_>, lead: Doc, op: &SyntaxToken, rhs: &SyntaxNode) -> R {
    let rhs_doc = cx.format(rhs)?;
    if fluid_assignments(cx.options) && !ends_in_line_comment(cx, op) {
        return Ok(fluid_rhs(cx, lead, rhs, rhs_doc));
    }
    Ok(if ends_in_line_comment(cx, op) {
        doc::concat(vec![
            lead,
            doc::indent(doc::concat(vec![doc::hardline(), rhs_doc])),
        ])
    } else if breaks_after_operator(cx, rhs) {
        doc::concat(vec![
            lead,
            doc::group(doc::indent(doc::concat(vec![doc::line(), rhs_doc]))),
        ])
    } else {
        doc::concat(vec![lead, doc::lit(" "), rhs_doc])
    })
}

pub fn assignment(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let els = sig(cx, node);
    let [
        SyntaxElement::Node(left),
        SyntaxElement::Token(op),
        SyntaxElement::Node(right),
    ] = els.as_slice()
    else {
        return Err(shape("assignment: expected left, operator and right"));
    };
    let lead = doc::concat(vec![cx.format(left)?, doc::lit(" "), cx.token(op)]);
    with_rhs(cx, lead, op, right)
}

fn operator_and_value(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let els = sig(cx, node);
    let [SyntaxElement::Token(op), SyntaxElement::Node(value)] = els.as_slice() else {
        return Err(shape("expected an operator token and a value"));
    };
    with_rhs(cx, cx.token(op), op, value)
}

pub fn equals_value_clause(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    operator_and_value(cx, node)
}

pub fn arrow_clause(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let els = sig(cx, node);
    let [SyntaxElement::Token(op), SyntaxElement::Node(value)] = els.as_slice() else {
        return Err(shape("expected an operator token and a value"));
    };
    // Directives and comments in front of `=>` are indented with it.
    let lead = doc::concat(vec![
        doc::indent(cx.leading_comments_doc(op)),
        cx.token_text(op),
        cx.trailing_comments_doc(op),
    ]);
    if fluid_assignments(cx.options) && !ends_in_line_comment(cx, op) {
        // `=>` and then the body, which moves to its own line as a whole when it does not fit.
        // The group holds `=>` too: a directive in front of it breaks the line after it as well.
        return Ok(doc::group(doc::concat(vec![
            lead,
            doc::indent(doc::concat(vec![doc::line(), cx.format(value)?])),
        ])));
    }
    with_rhs(cx, lead, op, value)
}

/// A lambda with an expression body for `assignment_layout: fluid`: the body moves to its own
/// line when the lambda does not fit (always after a comment after `=>`). `None` for a block body.
pub fn lambda(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Result<Option<Doc>, FormatError> {
    let els = sig(cx, node);
    let Some((SyntaxElement::Node(body), head)) = els.split_last() else {
        return Ok(None);
    };
    let arrow = head.last().and_then(|e| e.as_token());
    let Some(arrow) = arrow.filter(|_| body.kind() != k::BLOCK) else {
        return Ok(None);
    };
    let sep = if ends_in_line_comment(cx, arrow) {
        doc::hardline()
    } else {
        doc::line()
    };
    Ok(Some(doc::group(doc::concat(vec![
        seq_of(cx, node, head)?,
        doc::indent(doc::concat(vec![sep, cx.format(body)?])),
    ]))))
}

/// `pattern => expr` in a switch expression, laid out like an assignment (`switch_arm_layout`).
pub fn switch_arm(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let els = sig(cx, node);
    let Some((SyntaxElement::Node(value), head)) = els.split_last() else {
        return Err(shape("switch expression arm: expected a value"));
    };
    let Some(SyntaxElement::Token(arrow)) = head.last() else {
        return Err(shape("switch expression arm: expected `=>`"));
    };
    with_rhs(cx, super::seq::seq_of(cx, node, head)?, arrow, value)
}

// --------------------------------------------------------------- conditional

pub fn conditional(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let els = sig(cx, node);
    let [
        SyntaxElement::Node(cond),
        SyntaxElement::Token(q),
        SyntaxElement::Node(yes),
        SyntaxElement::Token(c),
        SyntaxElement::Node(no),
    ] = els.as_slice()
    else {
        return Err(shape("conditional expression: unexpected children"));
    };
    Ok(doc::group(doc::concat(vec![
        cx.format(cond)?,
        doc::indent(doc::concat(vec![
            doc::line(),
            cx.token(q),
            gap_after_operator(cx, q),
            cx.format(yes)?,
            doc::line(),
            cx.token(c),
            gap_after_operator(cx, c),
            cx.format(no)?,
        ])),
    ])))
}

pub fn parenthesized(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let els = sig(cx, node);
    let [
        SyntaxElement::Token(open),
        SyntaxElement::Node(expr),
        SyntaxElement::Token(close),
    ] = els.as_slice()
    else {
        return Err(shape("parenthesized expression: unexpected children"));
    };
    paren_group(cx, open, expr, close)
}

// ------------------------------------------------------------------- binary

/// The layouts that indent their own value: a chain inside them leaves its continuation lines
/// flush.
struct BinaryHooks;

impl shapes::BinaryHooks for BinaryHooks {
    fn place(&self, _cx: &FormatCtx<'_>, node: &SyntaxNode) -> Option<shapes::Place> {
        let kind = node.parent()?.kind();
        matches!(
            kind,
            k::EQUALS_VALUE_CLAUSE
                | k::ARROW_EXPRESSION_CLAUSE
                | k::PARENTHESIZED_EXPRESSION
                | k::IF_STATEMENT
                | k::WHILE_STATEMENT
                | k::DO_STATEMENT
                | k::LOCK_STATEMENT
                | k::SWITCH_STATEMENT
        )
        .then_some(shapes::Place::Flush)
    }
}

/// `a + b + c`: the shared operator chain (`core_format::shapes::binary_chain`) under the
/// `binary_*` options.
pub fn binary_chain(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    shapes::binary_chain(
        cx,
        node,
        &shapes::BinaryPolicy::of(cx.options),
        &BinaryHooks,
    )
}

// ------------------------------------------------------------------- member

/// `a.B().C().D()`: the shared member chain (`core_format::shapes::call_chain`) under the
/// `member_chain_*` options.
pub fn member_chain(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    shapes::call_chain(cx, node, &ChainPolicy::of(cx.options), &shapes::NoHooks)
}
