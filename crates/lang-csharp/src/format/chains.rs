//! Expressions that lay out as a chain: binary operators, member accesses and calls, assignments
//! and the conditional operator.

use super::options::fluid_assignments;
use super::seq::{R, seq_of};
use crate::generated::kind as k;
use core_format::doc::{self, Doc};
use core_format::seq::children;
use core_format::shapes::{self, ChainPolicy, ends_in_line_comment};
use core_format::{FormatCtx, FormatError};
use core_syntax::{SyntaxElement, SyntaxNode, SyntaxToken};

fn shape(msg: &str) -> FormatError {
    FormatError::Shape(msg.into())
}

// ---------------------------------------------------------------- assignment

/// What only C# judges about a value: under `fluid`, strings, collection expressions and the
/// initializer of a property move below the operator (a string that spans lines does not); under
/// `compact`, a query does.
struct AssignHooks;

impl shapes::AssignHooks for AssignHooks {
    fn layout(
        &self,
        _cx: &FormatCtx<'_>,
        _a: &shapes::Assign<'_>,
        right: &SyntaxElement,
        policy: &shapes::AssignPolicy,
    ) -> Option<shapes::AssignLayout> {
        use shapes::AssignLayout::*;
        let rhs = right.as_node()?;
        match policy.style {
            shapes::AssignStyle::Compact => {
                (rhs.kind() == k::QUERY_EXPRESSION).then_some(BreakAfterOperator)
            }
            shapes::AssignStyle::Fluid => {
                let string = matches!(
                    rhs.kind(),
                    k::STRING_LITERAL_EXPRESSION | k::INTERPOLATED_STRING_EXPRESSION
                );
                if string && rhs.text().contains_char('\n') {
                    return Some(NeverBreakAfterOperator);
                }
                // The initializer of a property moves as a whole.
                let property_initializer = rhs
                    .parent()
                    .and_then(|clause| clause.parent())
                    .is_some_and(|decl| decl.kind() == k::PROPERTY_DECLARATION);
                (string || property_initializer || rhs.kind() == k::COLLECTION_EXPRESSION)
                    .then_some(BreakAfterOperator)
            }
            shapes::AssignStyle::Break => None,
        }
    }
}

/// `left op right` as the shared assignment layouts.
fn with_rhs(
    cx: &FormatCtx<'_>,
    node: &SyntaxNode,
    left: Doc,
    op: Doc,
    op_token: &SyntaxToken,
    rhs: &SyntaxNode,
) -> R {
    let right = SyntaxElement::Node(rhs.clone());
    shapes::assignment(
        cx,
        shapes::Assign {
            node,
            left,
            op,
            op_token: Some(op_token),
            right: Some(&right),
            short_key: false,
        },
        &shapes::AssignPolicy::of(cx.options),
        &AssignHooks,
    )
}

pub fn assignment(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let els = children(cx, node);
    let [
        SyntaxElement::Node(left),
        SyntaxElement::Token(op),
        SyntaxElement::Node(right),
    ] = els.as_slice()
    else {
        return Err(shape("assignment: expected left, operator and right"));
    };
    let op_doc = doc::concat(vec![doc::lit(" "), cx.token(op)]);
    with_rhs(cx, node, cx.format(left)?, op_doc, op, right)
}

fn operator_and_value(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let els = children(cx, node);
    let [SyntaxElement::Token(op), SyntaxElement::Node(value)] = els.as_slice() else {
        return Err(shape("expected an operator token and a value"));
    };
    with_rhs(cx, node, Doc::Nil, cx.token(op), op, value)
}

pub fn equals_value_clause(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    operator_and_value(cx, node)
}

pub fn arrow_clause(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let els = children(cx, node);
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
    with_rhs(cx, node, Doc::Nil, lead, op, value)
}

/// A lambda with an expression body for `assignment_layout: fluid`: the body moves to its own
/// line when the lambda does not fit (always after a comment after `=>`). `None` for a block body.
pub fn lambda(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Result<Option<Doc>, FormatError> {
    let els = children(cx, node);
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
    let els = children(cx, node);
    let Some((SyntaxElement::Node(value), head)) = els.split_last() else {
        return Err(shape("switch expression arm: expected a value"));
    };
    let Some(SyntaxElement::Token(arrow)) = head.last() else {
        return Err(shape("switch expression arm: expected `=>`"));
    };
    with_rhs(cx, node, seq_of(cx, node, head)?, Doc::Nil, arrow, value)
}

// --------------------------------------------------------------- conditional

/// `a ? b : c`: the shared conditional layout under `conditional_layout`.
pub fn conditional(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    shapes::conditional(
        cx,
        node,
        &shapes::ConditionalPolicy::of(cx.options),
        &shapes::NoConditionalHooks,
    )
}

pub fn parenthesized(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    const POLICY: shapes::ParenPolicy = shapes::ParenPolicy {
        layout: shapes::ParenLayout::Group,
        unneeded: shapes::Unneeded::Keep,
    };
    shapes::parenthesized(cx, node, &POLICY, &shapes::NoParenHooks)
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
