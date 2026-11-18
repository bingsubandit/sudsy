//! Operator chains (`a + b + c`, `a && b || c`) read through roles: a node tagged `#binary-chain`
//! has the fields `left`, `operator` and `right`, and the grammar's `precedence` lines say which
//! operators of the same level flatten into one chain. The layout comes from a [`BinaryPolicy`]
//! (options of the language: `binary_*`, `indent_nested_binary`); what only the language can
//! judge (where the chain stands, what is inline, parentheses) comes through [`BinaryHooks`].

use super::chain::ends_in_line_comment;
use crate::FormatCtx;
use crate::comments;
use crate::doc::{self, Doc};
use crate::error::FormatError;
use core_config::{OptionValue, ResolvedOptions};
use core_syntax::roles;
use core_syntax::{Assoc, Grammar, SyntaxElement, SyntaxNode, SyntaxToken};

pub const OPERATOR_LINE: &str = "binary_operator_line";
pub const INDENT: &str = "binary_indent";
pub const INDENT_NESTED: &str = "indent_nested_binary";
pub const HUG: &str = "binary_hug";

/// Which end of a broken line the operator is on.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum OperatorLine {
    /// `a\n+ b`: one operand per line when the chain does not fit.
    Start,
    /// `a +\nb`: an operand moves down only when it does not fit; a chain that mixes node kinds
    /// breaks at every operator.
    End,
}

/// Where a chain leaves its indentation to the layout around it.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Hug {
    Never,
    /// The value of an assignment, declaration or entry.
    Assignment,
    /// Also the value of `return` and `throw`.
    Statement,
}

/// The values where reference tools differ; a language declares each as an option.
pub struct BinaryPolicy {
    pub operator_line: OperatorLine,
    /// Levels of indentation of the continuation lines.
    pub indent: usize,
    /// A chain that is an operand of a chain of another precedence indents its continuation lines.
    pub indent_nested: bool,
    pub hug: Hug,
}

impl BinaryPolicy {
    pub fn of(options: &ResolvedOptions) -> Self {
        let text = |name: &str| match options.get(name) {
            Some(OptionValue::String(s)) => s.as_str(),
            _ => "",
        };
        Self {
            operator_line: if text(OPERATOR_LINE) == "end" {
                OperatorLine::End
            } else {
                OperatorLine::Start
            },
            indent: match options.get(INDENT) {
                Some(OptionValue::Int(n)) => usize::try_from(*n).unwrap_or(1),
                _ => 1,
            },
            indent_nested: !matches!(options.get(INDENT_NESTED), Some(OptionValue::Bool(false))),
            hug: match text(HUG) {
                "assignment" => Hug::Assignment,
                "statement" => Hug::Statement,
                _ => Hug::Never,
            },
        }
    }
}

/// How the continuation lines of a chain are indented, which depends on where it stands.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Place {
    /// Inside parentheses the language prints around it: the parts only, no group of its own.
    Bare,
    /// Inside a layout that indents already: a group, no indent.
    Flush,
    /// The operand of a prefix operator, a member access or a call: the chain moves below an
    /// opening line and the closing line follows it.
    Wrapped,
    /// The value of an assignment: as `Flush`, unless the chain ends in an inline right side
    /// after a flattened chain.
    Assigned,
    /// A group, the continuation lines indented (flush when an inline right side follows no
    /// flattened chain).
    Indented,
}

/// What the language prints or judges itself; every method has a default.
pub trait BinaryHooks {
    /// An operator or an operand that is a token.
    fn token(&self, cx: &FormatCtx<'_>, t: &SyntaxToken) -> Doc {
        cx.token(t)
    }
    /// Does a line comment follow the token on its line (a line break must follow it)?
    fn comment_ends_line(&self, cx: &FormatCtx<'_>, t: &SyntaxToken) -> bool {
        ends_in_line_comment(cx, t)
    }
    /// May the operand `n` (as its parent holds it) flatten into the chain?
    fn flattens(&self, _cx: &FormatCtx<'_>, _n: &SyntaxNode) -> bool {
        true
    }
    /// The place of the chain `node`, when the roles around it do not tell.
    fn place(&self, _cx: &FormatCtx<'_>, _node: &SyntaxNode) -> Option<Place> {
        None
    }
    /// `a || {` ...: the right side stays on the operator's line (`OperatorLine::End`).
    fn inline_right(&self, _cx: &FormatCtx<'_>, _node: &SyntaxNode) -> bool {
        false
    }
    /// The comment after the last operand of a flattened chain moves after its operand.
    fn reattach_comments(&self) -> bool {
        false
    }
    /// The last operand lays itself out outside the chain's group (JSX).
    fn tail_outside(&self, _cx: &FormatCtx<'_>, _node: &SyntaxNode) -> bool {
        false
    }
}

/// A language with nothing of its own.
pub struct NoBinaryHooks;

impl BinaryHooks for NoBinaryHooks {}

/// May `a op b op2 c` print as one chain without parentheses around `a op b`? The grammar's
/// `precedence` lines decide: one level and an associativity that lets the operators join.
pub fn flattens(g: &Grammar, parent_op: &str, node_op: &str) -> bool {
    let (Some(p), Some(n)) = (g.precedence(parent_op), g.precedence(node_op)) else {
        return false;
    };
    p.level == n.level
        && match p.assoc {
            Assoc::Left | Assoc::Right => n.assoc == p.assoc,
            Assoc::Same => n.assoc == Assoc::Same && parent_op == node_op,
            Assoc::None => false,
        }
}

struct Link {
    node: SyntaxNode,
    left: SyntaxElement,
    op: SyntaxToken,
    right: SyntaxElement,
}

fn link(
    cx: &FormatCtx<'_>,
    node: &SyntaxNode,
    buf: &mut Vec<SyntaxElement>,
) -> Result<Link, FormatError> {
    buf.clear();
    buf.extend(
        node.children_with_tokens()
            .filter(|e| !cx.grammar.is_trivia(e.kind()) && !e.text_range().is_empty()),
    );
    let slot = |field: &str| cx.grammar.slot_in(node.kind(), buf, field);
    match (slot("left"), slot("operator"), slot("right")) {
        (Some(left), Some(SyntaxElement::Token(op)), Some(right)) => Ok(Link {
            node: node.clone(),
            left,
            op,
            right,
        }),
        _ => Err(FormatError::Shape("binary expression".into())),
    }
}

fn last_token(cx: &FormatCtx<'_>, e: &SyntaxElement) -> Option<SyntaxToken> {
    match e {
        SyntaxElement::Token(t) => Some(t.clone()),
        SyntaxElement::Node(n) => cx.last_token(n),
    }
}

fn operand(
    cx: &FormatCtx<'_>,
    hooks: &dyn BinaryHooks,
    e: &SyntaxElement,
) -> Result<Doc, FormatError> {
    match e {
        SyntaxElement::Node(n) => cx.format(n),
        SyntaxElement::Token(t) => Ok(hooks.token(cx, t)),
    }
}

/// The chain that starts at `node` (a `#binary-chain`): the operands of one precedence level,
/// flattened, on one line if they fit, else broken at the operators.
pub fn binary_chain(
    cx: &FormatCtx<'_>,
    node: &SyntaxNode,
    policy: &BinaryPolicy,
    hooks: &dyn BinaryHooks,
) -> Result<Doc, FormatError> {
    let g = cx.grammar;
    let mut buf = Vec::with_capacity(6);
    // The links, outermost first: down the left side, or down the right for a right-associative
    // operator (`a ?? b ?? c`).
    let mut spine = vec![link(cx, node, &mut buf)?];
    let right_assoc = g
        .precedence(spine[0].op.text())
        .is_some_and(|p| p.assoc == Assoc::Right);
    loop {
        let top = &spine[spine.len() - 1];
        let next = if right_assoc { &top.right } else { &top.left };
        let SyntaxElement::Node(n) = next else { break };
        let inner = roles::un(n);
        if !g.has_tag(inner.kind(), "binary-chain") || !hooks.flattens(cx, n) {
            break;
        }
        match link(cx, &inner, &mut buf) {
            Ok(l) if flattens(g, top.op.text(), l.op.text()) => spine.push(l),
            _ => break,
        }
    }
    let end = policy.operator_line == OperatorLine::End;
    let steps = spine.len();
    let operand_of = |i: usize| {
        if right_assoc && i + 1 < steps {
            &spine[i + 1].left
        } else {
            &spine[i].right
        }
    };
    // Source order: the innermost link first on the left side, the outermost on the right.
    let order = |k: usize| if right_assoc { k } else { steps - 1 - k };
    let first = if right_assoc {
        &spine[0].left
    } else {
        &spine[steps - 1].left
    };

    let outer = roles::outer(node);
    let parent = outer.parent();
    let place = hooks
        .place(cx, node)
        .unwrap_or_else(|| default_place(cx, policy, parent.as_ref()));
    let top_op = spine[0].op.text();
    let logical = matches!(g.operator_kind(top_op), Some("logical" | "coalesce"));
    // `a op b` alone: its operator and right side form a group of their own, unless a node of the
    // same kind is around them.
    let group_step = end
        && steps == 1
        && !(place == Place::Bare && logical)
        && [
            parent.as_ref().map(SyntaxNode::kind),
            node_kind(&spine[0].left),
            node_kind(&spine[0].right),
        ]
        .iter()
        .all(|k| *k != Some(node.kind()));

    let first_doc = {
        let d = operand(cx, hooks, first)?;
        if end { doc::group(d) } else { d }
    };
    let mut tail: Vec<Doc> = Vec::with_capacity(steps * if end { 2 } else { 4 } + 1);
    let mut prev = first;
    for k in 0..steps {
        let l = &spine[order(k)];
        let target = operand_of(order(k));
        // The comment after a nested chain goes after its last operand.
        let nested = (hooks.reattach_comments() && !right_assoc && k + 1 < steps)
            .then(|| cx.last_token(&l.node))
            .flatten()
            .filter(|t| comments::has_trailing_comment(g, t));
        if let Some(t) = &nested {
            cx.detach_trailing(t);
        }
        let target_doc = operand(cx, hooks, target)?;
        if end {
            let inline = hooks.inline_right(cx, &l.node);
            tail.push(doc::lit(" "));
            let step = doc::concat(vec![
                hooks.token(cx, &l.op),
                if inline { doc::lit(" ") } else { doc::line() },
                target_doc,
            ]);
            tail.push(if group_step { doc::group(step) } else { step });
        } else {
            let before = last_token(cx, prev).is_some_and(|t| ends_in_line_comment(cx, &t));
            tail.push(if before { doc::hardline() } else { doc::line() });
            tail.push(hooks.token(cx, &l.op));
            tail.push(if hooks.comment_ends_line(cx, &l.op) {
                doc::hardline()
            } else {
                doc::lit(" ")
            });
            tail.push(target_doc);
        }
        if let Some(t) = &nested {
            tail.push(cx.reattach_trailing(t));
        }
        prev = target;
    }

    let inline = end && hooks.inline_right(cx, node);
    let same_precedence_left = steps > 1;
    let flush = match place {
        Place::Flush => true,
        Place::Assigned => !(inline && same_precedence_left),
        Place::Indented => inline && !same_precedence_left,
        _ => false,
    };
    Ok(match place {
        Place::Bare => {
            let mut parts = vec![first_doc];
            parts.extend(tail);
            doc::concat(parts)
        }
        Place::Wrapped => {
            let mut parts = vec![first_doc];
            parts.extend(tail);
            super::enclosed(doc::Doc::Nil, doc::concat(parts), doc::Doc::Nil)
        }
        _ if flush => {
            let mut parts = vec![first_doc];
            parts.extend(tail);
            doc::group(doc::concat(parts))
        }
        _ => {
            let outside = if end && hooks.tail_outside(cx, node) {
                tail.pop()
            } else {
                None
            };
            let mut rest = doc::concat(tail);
            for _ in 0..policy.indent {
                rest = doc::indent(rest);
            }
            let chain = doc::concat(vec![first_doc, rest]);
            match outside {
                None => doc::group(chain),
                Some(last) => {
                    let id = doc::new_group_id();
                    doc::group(doc::concat(vec![
                        doc::group_with_id(id, chain),
                        doc::indent_if_break(id, last),
                    ]))
                }
            }
        }
    })
}

fn node_kind(e: &SyntaxElement) -> Option<core_syntax::SyntaxKind> {
    e.as_node().map(|n| roles::un(n).kind())
}

/// The place from the roles around the chain and the policy.
fn default_place(cx: &FormatCtx<'_>, policy: &BinaryPolicy, parent: Option<&SyntaxNode>) -> Place {
    let g = cx.grammar;
    let Some(p) = parent else {
        return Place::Indented;
    };
    let kind = p.kind();
    if g.has_tag(kind, "binary-chain") && !policy.indent_nested {
        return Place::Flush;
    }
    let assigned = ["assignment", "declaration", "key-value"]
        .iter()
        .any(|t| g.has_tag(kind, t));
    let statement = g.has_tag(kind, "return") || g.has_tag(kind, "throw");
    if policy.hug == Hug::Statement && statement {
        return Place::Flush;
    }
    if policy.hug != Hug::Never && assigned {
        return Place::Assigned;
    }
    Place::Indented
}
