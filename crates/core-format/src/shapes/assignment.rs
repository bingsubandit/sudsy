//! Assignments, declarations and entries (`left = right`): where the line breaks between the
//! operator and the right side. The layout is one of a fixed set ([`AssignLayout`]); roles pick
//! it (`#assignment`, `#binary-chain`, `#conditional`) under an [`AssignPolicy`] (options of the
//! language: `assignment_*`), and what only the language can judge (which right sides never
//! break after the operator, strings, calls, templates) comes through [`AssignHooks`].
//!
//! concept: assignment-layout
//! what: Where the line breaks between the operator and the right side of an assignment or entry.
//! tools: Prettier assignment layouts, google-java-format assignment wrapping
//! reads: option:assignment_layout, tag:assignment

use super::chain::ends_in_line_comment;
use crate::FormatCtx;
use crate::comments;
use crate::doc::{self, Doc};
use crate::error::FormatError;
use core_config::ResolvedOptions;
use core_syntax::roles;
use core_syntax::{SyntaxElement, SyntaxNode, SyntaxToken};

pub const LAYOUT: core_config::OptId = core_config::opt::ASSIGNMENT_LAYOUT;
pub const CONDITIONAL: core_config::OptId = core_config::opt::ASSIGNMENT_CONDITIONAL;
pub const INDENT: core_config::OptId = core_config::opt::ASSIGNMENT_INDENT;
pub const LEFT_PARAMS: core_config::OptId = core_config::opt::ASSIGNMENT_LEFT_MIN_PARAMETERS;
pub const LEFT_UNLESS: core_config::OptId = core_config::opt::ASSIGNMENT_LEFT_UNLESS_KINDS;
pub const KEEP_BREAK: core_config::OptId = core_config::opt::ASSIGNMENT_KEEP_BREAK_KINDS;
pub const OP_SKIP: core_config::OptId = core_config::opt::ASSIGNMENT_OP_TOKEN_SKIP_KINDS;
pub const DESTRUCTURE: core_config::OptId = core_config::opt::ASSIGNMENT_DESTRUCTURE_KINDS;
pub const GENERIC_CLOSURE: core_config::OptId = core_config::opt::ASSIGNMENT_GENERIC_CLOSURE_KINDS;
pub const TYPE_ARG_KINDS: core_config::OptId = core_config::opt::TYPE_ARGUMENT_COMPLEX_KINDS;

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
    /// As `Break`, but a value with no place to break a line stays on the operator's line.
    Multiline,
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
        let text = |name: core_config::OptId| options.text(name);
        Self {
            style: match text(LAYOUT) {
                "fluid" => AssignStyle::Fluid,
                "break" => AssignStyle::Break,
                "multiline" => AssignStyle::Multiline,
                _ => AssignStyle::Compact,
            },
            conditional_always: text(CONDITIONAL) == "always",
            indent: options.uint(INDENT, 1),
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
    // `multiline`: a line comment after the value is not part of it, so it neither breaks the
    // value below the operator nor counts as a break in it.
    let tail = (policy.style == AssignStyle::Multiline)
        .then(|| match right {
            SyntaxElement::Token(t) => Some(t.clone()),
            SyntaxElement::Node(n) => cx.last_token(n),
        })
        .flatten()
        .filter(|t| comments::has_trailing_comment(cx.grammar, t));
    if let Some(t) = &tail {
        cx.hold_trailing(t);
    }
    let right = hooks.right_doc(cx, right, layout)?;
    let layout = if policy.style == AssignStyle::Multiline
        && layout == AssignLayout::BreakAfterOperator
        && !right.has_break_point()
        && !a
            .node
            .children()
            .any(|p| cx.grammar.has_tag(p.kind(), "parameter-list"))
    {
        AssignLayout::NeverBreakAfterOperator
    } else {
        layout
    };
    let rendered = render(layout, a.left, a.op, right, policy.indent);
    Ok(match tail {
        Some(t) => doc::concat(vec![rendered, cx.held_trailing(&t)]),
        None => rendered,
    })
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

/// Is the token a plain or compound assignment operator (`=`, `+=`, ...)?
pub fn is_assign_op(cx: &FormatCtx<'_>, t: &SyntaxToken) -> bool {
    matches!(
        cx.grammar.operator_kind(t.text()),
        Some("assign" | "compound")
    )
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

/// The token in front of the right side (its line comment keeps the right side below it), unless
/// the kind of the node is one that keeps such a comment where it is.
fn operator_token(cx: &FormatCtx<'_>, a: &Assign<'_>) -> Option<SyntaxToken> {
    let right = a.right?;
    if cx.options.list_has(OP_SKIP, cx.grammar.name(a.node.kind())) {
        return None;
    }
    let mut prev = right.prev_sibling_or_token();
    while let Some(e) = prev {
        match e {
            SyntaxElement::Token(t) if !cx.grammar.is_trivia(t.kind()) => return Some(t),
            SyntaxElement::Token(t) => prev = t.prev_sibling_or_token(),
            SyntaxElement::Node(n) => prev = n.prev_sibling_or_token(),
        }
    }
    None
}

/// A destructuring on the left (`assignment_destructure_kinds`) of more than two entries, one of
/// them renamed or defaulted: it breaks in preference to the right side.
fn complex_target(cx: &FormatCtx<'_>, node: &SyntaxNode) -> bool {
    let g = cx.grammar;
    let Some(left) = node.first_child() else {
        return false;
    };
    cx.options.list_has(DESTRUCTURE, g.name(left.kind()))
        && left.children().nth(2).is_some()
        && left.children().any(|p| {
            !g.has_tag(p.kind(), "rest")
                && (p
                    .children_with_tokens()
                    .any(|e| e.as_token().is_some_and(|t| !g.is_trivia(t.kind())))
                    || p.children().any(|c| g.has_tag(c.kind(), "default-value")))
        })
}

/// Type arguments that make a short call chain worth breaking inside: several, one of a kind in
/// `type_argument_complex_kinds`, or one that breaks.
pub fn complex_type_args(
    cx: &FormatCtx<'_>,
    args: Option<&SyntaxNode>,
) -> Result<bool, FormatError> {
    let Some(args) = args else { return Ok(false) };
    let mut it = args.children();
    let first = it.next();
    Ok(it.next().is_some()
        || first.is_some_and(|t| {
            cx.options
                .list_has(TYPE_ARG_KINDS, cx.grammar.name(t.kind()))
        })
        || cx.format(args)?.will_break())
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
    // A line comment after the operator, or after the left side in front of it.
    let before_op = |t: &SyntaxToken| {
        let mut cur = t.prev_token();
        while let Some(p) = cur {
            if !g.is_trivia(p.kind()) {
                return comments::trailing_comments(g, &p).iter().any(|c| {
                    c.kind != comments::CommentKind::Block
                        && c.element.text_range().start() >= p.text_range().end()
                });
            }
            cur = p.prev_token();
        }
        false
    };
    let found = a.op_token.cloned().or_else(|| operator_token(cx, a));
    let op_token = found.as_ref();
    if op_token.is_some_and(|t| ends_in_line_comment(cx, t) || before_op(t)) {
        return BreakAlways;
    }
    // A break the source has after the operator stays (`assignment_keep_break_kinds`).
    if let Some(t) = op_token
        && comments::break_after(g, t)
        && cx.options.list_has(KEEP_BREAK, g.name(a.node.kind()))
    {
        return BreakAlways;
    }
    let right_inner = right.as_node().map(roles::un);
    let right_assign = right_inner.as_ref().filter(|n| is_assignment(cx, n));
    let fluid = policy.style == AssignStyle::Fluid;
    if fluid && is_assignment(cx, a.node) {
        let held = roles::outer(a.node);
        let parent = held.parent();
        let grand = parent.as_ref().and_then(|p| roles::outer(p).parent());
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
            .and_then(|e| e.as_node().map(roles::un))
            .is_some_and(|n| is_assignment(cx, &n));
    if head_of_long_chain || own_line_comment_before(cx, right) {
        return BreakAfterOperator;
    }
    if complex_target(cx, a.node) {
        return BreakLhs;
    }
    if let Some(r) = &right_inner {
        if g.has_tag(r.kind(), "binary-chain") && !hooks.inline_binary(cx, r) {
            return BreakAfterOperator;
        }
        if g.has_tag(r.kind(), "conditional")
            && (policy.conditional_always
                || g.slot(r, "condition")
                    .and_then(|e| e.as_node().map(roles::un))
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
    // A generic closure on a declarator hugs its left side when that can break.
    if cx.options.list_has(GENERIC_CLOSURE, g.name(a.node.kind()))
        && right_inner.as_ref().is_some_and(|r| {
            g.has_tag(r.kind(), "huggable-function") && g.slot(r, "type_parameters").is_some()
        })
        && a.left.has_break_point()
    {
        return BreakLhs;
    }
    // A signature with a line per parameter keeps a value that spans lines on the operator's
    // line (`) = call(`).
    let wrapped = match cx.options.get(LEFT_PARAMS) {
        Some(core_config::OptionValue::Int(n)) => *n,
        _ => 0,
    };
    let g = cx.grammar;
    if wrapped > 0
        && !a
            .node
            .children_with_tokens()
            .any(|c| cx.options.list_has(LEFT_UNLESS, g.name(c.kind())))
        && a.node.children().any(|p| {
            cx.grammar.has_tag(p.kind(), "parameter-list") && p.children().count() as i64 >= wrapped
        })
    {
        return NeverBreakAfterOperator;
    }
    match policy.style {
        AssignStyle::Compact => NeverBreakAfterOperator,
        AssignStyle::Fluid => Fluid,
        AssignStyle::Break | AssignStyle::Multiline => BreakAfterOperator,
    }
}
