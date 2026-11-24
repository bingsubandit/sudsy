//! Expressions that lay out as a chain: binary operators, member accesses and calls, assignments
//! and the conditional operator.
//!
//! concept: binary-chain-layout, member-chain, assignment-layout, conditional-expression-layout

use super::options::fluid_assignments;
use super::seq::{R, seq_of};
use crate::generated::kind as k;
use core_format::doc::{self, Doc};
use core_format::seq::children;
use core_format::shapes::{self, ends_in_line_comment};
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
    /// The test `x is null` of a conditional does not move a conditional below the operator.
    fn inline_binary(&self, _cx: &FormatCtx<'_>, n: &SyntaxNode) -> bool {
        n.kind() == k::IS_PATTERN_EXPRESSION
            && n.parent().is_some_and(|p| p.kind() == k::CONDITIONAL_EXPRESSION)
    }

    fn layout(
        &self,
        cx: &FormatCtx<'_>,
        _a: &shapes::Assign<'_>,
        right: &SyntaxElement,
        policy: &shapes::AssignPolicy,
    ) -> Option<shapes::AssignLayout> {
        use shapes::AssignLayout::*;
        let rhs = right.as_node()?;
        if super::options::nested_brace_on_new_line(cx.options)
            && cx.grammar.has_tag(rhs.kind(), "initializer-expression")
            && super::lists::value_of_assignment(rhs)
        {
            return Some(Attached);
        }
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
            _ => None,
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
        cx.assign_policy(),
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
    const STYLE: shapes::ArrowStyle = shapes::ArrowStyle {
        levels: 1,
        own_group: false,
        hug_block: false,
    };
    shapes::arrow_body(
        cx,
        node,
        &|els| seq_of(cx, node, els),
        k::EQUALS_GREATER_THAN_TOKEN,
        &STYLE,
    )
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
struct ConditionalHooks;

impl shapes::ConditionalHooks for ConditionalHooks {
    /// A ladder that is a return value or one of several arguments.
    fn ladder_indent(&self, _cx: &FormatCtx<'_>, node: &SyntaxNode) -> bool {
        node.parent().is_some_and(|p| match p.kind() {
            k::RETURN_STATEMENT => true,
            k::ARGUMENT => p
                .parent()
                .is_some_and(|l| l.children().filter(|c| c.kind() == k::ARGUMENT).count() > 1),
            _ => false,
        })
    }
}

pub fn conditional(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    shapes::conditional(cx, node, cx.conditional_policy(), &ConditionalHooks)
}

/// `when cond` after a pattern or `catch`: a group on an indented line of its own; a catch filter
/// has its condition in parentheses that hug it.
pub fn when_clause(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let els = children(cx, node);
    let body = match els.as_slice() {
        [when, open, cond @ SyntaxElement::Node(_), close] => doc::concat(vec![
            seq_of(cx, node, &[when.clone(), open.clone()])?,
            doc::group(doc::concat(vec![
                doc::indent(core_format::seq::el(cx, cond)?),
                doc::softline(),
            ])),
            core_format::seq::el(cx, close)?,
        ]),
        _ => seq_of(cx, node, &els)?,
    };
    let discard = node.parent().is_some_and(|p| {
        p.kind() == k::SWITCH_EXPRESSION_ARM
            && p.children().next().is_some_and(|t| t.kind() == k::DISCARD_PATTERN)
    });
    let line = if discard { doc::lit(" ") } else { doc::line() };
    Ok(doc::group(doc::indent(doc::concat(vec![line, body]))))
}

/// `(T)x`: the operand hangs below the cast when the whole does not fit.
pub fn cast(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    const POLICY: shapes::HangPolicy = shapes::HangPolicy {
        always: &[k::PARENTHESIZED_EXPRESSION],
        plain: &[k::IDENTIFIER_NAME],
        calls: &[k::INVOCATION_EXPRESSION],
        except_head: &[k::GENERIC_NAME],
    };
    let els = children(cx, node);
    let (Some(head_type), Some(SyntaxElement::Node(operand))) = (
        node.children().next(),
        els.last(),
    ) else {
        return seq_of(cx, node, &els);
    };
    let head = seq_of(cx, node, &els[..els.len() - 1])?;
    let printed = core_format::seq::el(cx, els.last().expect("operand"))?;
    Ok(shapes::hang_operand(head, head_type.kind(), operand, printed, &POLICY))
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
    /// The braces of a property pattern that follows `is` start on a line of their own
    /// (`wrapped_brace_line: all`).
    fn starts_with_line(&self, cx: &FormatCtx<'_>, n: &SyntaxNode) -> bool {
        n.kind() == k::RECURSIVE_PATTERN
            && super::options::nested_brace_on_new_line(cx.options)
            && n.children()
                .next()
                .is_some_and(|c| c.kind() == k::PROPERTY_PATTERN_CLAUSE)
    }

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
    shapes::binary_chain(cx, node, cx.binary_policy(), &BinaryHooks)
}

// ------------------------------------------------------------------- member

/// `a.B().C().D()`: the shared member chain (`core_format::shapes::call_chain`) under the
/// `member_chain_*` options.
pub fn member_chain(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    shapes::call_chain(cx, node, cx.chain_policy(), &shapes::NoHooks)
}
