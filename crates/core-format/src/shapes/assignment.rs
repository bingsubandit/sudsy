//! Assignments, declarations and entries (`left = right`): where the line breaks between the
//! operator and the right side. The layout is one of a fixed set ([`AssignLayout`]); roles pick
//! it (`#assignment`, `#binary-chain`, `#conditional`) under an [`AssignPolicy`] (options of the
//! language: `assignment_*`), and what only the language can judge (which right sides never
//! break after the operator, strings, calls, templates) comes through [`AssignHooks`].

use super::chain::ends_in_line_comment;
use crate::FormatCtx;
use crate::comments;
use crate::doc::{self, Doc};
use crate::error::FormatError;
use core_config::{OptionValue, ResolvedOptions};
use core_syntax::{SyntaxElement, SyntaxNode, SyntaxToken};

pub const LAYOUT: &str = "assignment_layout";
pub const CONDITIONAL: &str = "assignment_conditional";
pub const INDENT: &str = "assignment_indent";

/// How a right side that is not decided by its kind is laid out.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum AssignStyle {
    /// It stays on the operator's line.
    Compact,
    /// It moves below the operator when its first line does not fit after it; the layouts of
    /// assignment chains apply.
    Fluid,
    /// It moves below the operator when the whole does not fit.
    Break,
}

/// The layouts of an assignment, in Prettier's names.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum AssignLayout {
    BreakAfterOperator,
    NeverBreakAfterOperator,
    Fluid,
    BreakLhs,
    Chain,
    ChainTail,
    ChainTailArrowChain,
    /// The right side always starts a new line (a line comment ends the operator's line).
    BreakAlways,
    OnlyLeft,
}

/// The values where reference tools differ; a language declares each as an option.
pub struct AssignPolicy {
    pub style: AssignStyle,
    /// A conditional on the right moves below the operator; else only one whose test is an
    /// operator chain does.
    pub conditional_always: bool,
    /// Levels of indentation of the right side below the operator.
    pub indent: usize,
}

impl AssignPolicy {
    pub fn of(options: &ResolvedOptions) -> Self {
        let text = |name: &str| match options.get(name) {
            Some(OptionValue::String(s)) => s.as_str(),
            _ => "",
        };
        Self {
            style: match text(LAYOUT) {
                "fluid" => AssignStyle::Fluid,
                "break" => AssignStyle::Break,
                _ => AssignStyle::Compact,
            },
            conditional_always: text(CONDITIONAL) == "always",
            indent: match options.get(INDENT) {
                Some(OptionValue::Int(n)) => usize::try_from(*n).unwrap_or(1),
                _ => 1,
            },
        }
    }
}

/// An assignment with its left side and operator printed.
pub struct Assign<'a> {
    /// The assignment, declaration or entry (the roles of its kind say what it is).
    pub node: &'a SyntaxNode,
    pub left: Doc,
    /// The operator with the space before it.
    pub op: Doc,
    /// The operator token, when a line comment after it forces the right side down.
    pub op_token: Option<&'a SyntaxToken>,
    pub right: Option<&'a SyntaxElement>,
    /// The key of an entry is short (the value stays on its line more often).
    pub short_key: bool,
}

/// What the language prints or judges itself; every method has a default.
pub trait AssignHooks {
    /// The node as its parent holds it (with the parentheses around it).
    fn outer(&self, n: &SyntaxNode) -> SyntaxNode {
        n.clone()
    }
    /// The node without the parentheses around it.
    fn inner(&self, n: &SyntaxNode) -> SyntaxNode {
        n.clone()
    }
    /// Is the node (a parent of an assignment) one whose value a chain `a = b = c` continues?
    fn chain_parent(&self, cx: &FormatCtx<'_>, n: &SyntaxNode) -> bool {
        cx.grammar.has_tag(n.kind(), "assignment")
    }
    /// The end of a chain `a = b = c` whose last right side is `right` (not an assignment).
    fn chain_tail(&self, _cx: &FormatCtx<'_>, _right: &SyntaxNode) -> AssignLayout {
        AssignLayout::ChainTail
    }
    /// A chain of operators on the right that stays on the operator's line (`a || {`).
    fn inline_binary(&self, _cx: &FormatCtx<'_>, _n: &SyntaxNode) -> bool {
        false
    }
    /// What the language decides about the right side before the policy's default does.
    fn layout(
        &self,
        _cx: &FormatCtx<'_>,
        _a: &Assign<'_>,
        _right: &SyntaxElement,
        _policy: &AssignPolicy,
    ) -> Option<AssignLayout> {
        None
    }
    /// The right side printed (the layout is known: an arrow chain prints differently in a tail).
    fn right_doc(
        &self,
        cx: &FormatCtx<'_>,
        right: &SyntaxElement,
        _layout: AssignLayout,
    ) -> Result<Doc, FormatError> {
        match right {
            SyntaxElement::Node(n) => cx.format(n),
            SyntaxElement::Token(t) => Ok(cx.token(t)),
        }
    }
}

/// A language with nothing of its own.
pub struct NoAssignHooks;

impl AssignHooks for NoAssignHooks {}

/// `left op right` as one of the layouts.
pub fn assignment(
    cx: &FormatCtx<'_>,
    a: Assign<'_>,
    policy: &AssignPolicy,
    hooks: &dyn AssignHooks,
) -> Result<Doc, FormatError> {
    let layout = choose(cx, &a, policy, hooks);
    let Some(right) = a.right else {
        return Ok(a.left);
    };
    let right = hooks.right_doc(cx, right, layout)?;
    Ok(render(layout, a.left, a.op, right, policy.indent))
}

fn indented(levels: usize, mut d: Doc) -> Doc {
    for _ in 0..levels {
        d = doc::indent(d);
    }
    d
}

fn render(layout: AssignLayout, left: Doc, op: Doc, right: Doc, levels: usize) -> Doc {
    use AssignLayout::*;
    match layout {
        BreakAfterOperator => doc::group(doc::concat(vec![
            doc::group(left),
            op,
            doc::group(indented(levels, doc::concat(vec![doc::line(), right]))),
        ])),
        NeverBreakAfterOperator => doc::group(doc::concat(vec![
            doc::group(left),
            op,
            doc::lit(" "),
            right,
        ])),
        Fluid => {
            let id = doc::new_group_id();
            let mut tail = right;
            for _ in 0..levels {
                tail = doc::indent_if_break(id, tail);
            }
            doc::group(doc::concat(vec![
                doc::group(left),
                op,
                doc::group_with_id(id, indented(levels, doc::line())),
                doc::line_suffix_boundary(),
                tail,
            ]))
        }
        BreakLhs => doc::group(doc::concat(vec![
            left,
            op,
            doc::lit(" "),
            doc::group(right),
        ])),
        Chain => doc::concat(vec![doc::group(left), op, doc::line(), right]),
        ChainTail => doc::concat(vec![
            doc::group(left),
            op,
            indented(levels, doc::concat(vec![doc::line(), right])),
        ]),
        ChainTailArrowChain => doc::concat(vec![doc::group(left), op, doc::group(right)]),
        BreakAlways => doc::concat(vec![
            left,
            op,
            indented(levels, doc::concat(vec![doc::hardline(), right])),
        ]),
        OnlyLeft => left,
    }
}

fn is_assignment(cx: &FormatCtx<'_>, n: &SyntaxNode) -> bool {
    cx.grammar.has_tag(n.kind(), "assignment")
}

/// Does a comment on a line of its own stand in front of the element?
fn own_line_comment_before(cx: &FormatCtx<'_>, e: &SyntaxElement) -> bool {
    let first = match e {
        SyntaxElement::Token(t) => Some(t.clone()),
        SyntaxElement::Node(n) => cx.first_token(n),
    };
    first.is_some_and(|t| {
        comments::leading_comments(cx.grammar, &t)
            .iter()
            .any(|c| c.breaks_after > 0)
    })
}

fn choose(
    cx: &FormatCtx<'_>,
    a: &Assign<'_>,
    policy: &AssignPolicy,
    hooks: &dyn AssignHooks,
) -> AssignLayout {
    use AssignLayout::*;
    let g = cx.grammar;
    let Some(right) = a.right else {
        return OnlyLeft;
    };
    if a.op_token.is_some_and(|t| ends_in_line_comment(cx, t)) {
        return BreakAlways;
    }
    let right_inner = right.as_node().map(|n| hooks.inner(n));
    let right_assign = right_inner.as_ref().filter(|n| is_assignment(cx, n));
    let fluid = policy.style == AssignStyle::Fluid;
    if fluid && is_assignment(cx, a.node) {
        let held = hooks.outer(a.node);
        let parent = held.parent();
        let grand = parent.as_ref().and_then(|p| hooks.outer(p).parent());
        if parent.as_ref().is_some_and(|p| hooks.chain_parent(cx, p))
            && grand.is_some_and(|gp| right_assign.is_some() || !g.has_tag(gp.kind(), "statement"))
        {
            return match (right_assign, &right_inner) {
                (Some(_), _) => Chain,
                (None, Some(r)) => hooks.chain_tail(cx, r),
                (None, None) => ChainTail,
            };
        }
    }
    let head_of_long_chain = fluid
        && right_assign
            .and_then(|r| g.slot(r, "right"))
            .and_then(|e| e.as_node().map(|n| hooks.inner(n)))
            .is_some_and(|n| is_assignment(cx, &n));
    if head_of_long_chain || own_line_comment_before(cx, right) {
        return BreakAfterOperator;
    }
    if let Some(r) = &right_inner {
        if g.has_tag(r.kind(), "binary-chain") && !hooks.inline_binary(cx, r) {
            return BreakAfterOperator;
        }
        if g.has_tag(r.kind(), "conditional")
            && (policy.conditional_always
                || g.slot(r, "condition")
                    .and_then(|e| e.as_node().map(|n| hooks.inner(n)))
                    .is_some_and(|t| {
                        g.has_tag(t.kind(), "binary-chain") && !hooks.inline_binary(cx, &t)
                    }))
        {
            return BreakAfterOperator;
        }
    }
    if let Some(l) = hooks.layout(cx, a, right, policy) {
        return l;
    }
    match policy.style {
        AssignStyle::Compact => NeverBreakAfterOperator,
        AssignStyle::Fluid => Fluid,
        AssignStyle::Break => BreakAfterOperator,
    }
}
