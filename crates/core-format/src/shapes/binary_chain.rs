//! Operator chains (`a + b + c`, `a && b || c`) read through roles: a node tagged `#binary-chain`
//! has the fields `left`, `operator` and `right`, and the grammar's `precedence` lines say which
//! operators of the same level flatten into one chain. The layout comes from a [`BinaryPolicy`]
//! (options of the language: `binary_*`, `indent_nested_binary`); what only the language can
//! judge (where the chain stands, what is inline, parentheses) comes through [`BinaryHooks`].
//!
//! concept: binary-chain-layout
//! what: Operator chains flattened by precedence and broken at operators.
//! tools: Prettier binaryish, ruff binary expressions, ktlint binary-expression-wrapping
//! reads: tag:binary-chain, option:binary_operator_line

use super::chain::ends_in_line_comment;
use crate::FormatCtx;
use crate::comments;
use crate::doc::{self, Doc};
use crate::error::FormatError;
use core_config::ResolvedOptions;
use core_syntax::roles;
use core_syntax::{Assoc, Grammar, SyntaxElement, SyntaxNode, SyntaxToken};

pub const OPERATOR_LINE: &str = "binary_operator_line";
pub const INDENT: &str = "binary_indent";
pub const INDENT_NESTED: &str = "indent_nested_binary";
pub const HUG: &str = "binary_hug";
pub const FILL: &str = "binary_fill";
pub const ALIGN: &str = "align_operands";
pub const START_OPERATORS: &str = "binary_start_operators";
pub const CONTINUATION: &str = "continuation_indent";

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
    /// With the operator at the end of the line: the operands (each followed by its operator) are
    /// filled to the line width, one line break at a time, instead of every operator breaking at
    /// once.
    pub fill: bool,
    /// The continuation lines start under the first operand; a chain that is an operand of a
    /// chain of another precedence starts them this many columns further in.
    pub align: Option<usize>,
    /// Operators that start the line they break before, though the chain's operators end lines.
    pub start_operators: Vec<String>,
}

impl BinaryPolicy {
    pub fn of(options: &ResolvedOptions) -> Self {
        let text = |name: &str| options.text(name);
        Self {
            operator_line: if text(OPERATOR_LINE) == "end" {
                OperatorLine::End
            } else {
                OperatorLine::Start
            },
            indent: options.uint(INDENT, 1),
            indent_nested: options.flag(INDENT_NESTED, true),
            hug: match text(HUG) {
                "assignment" => Hug::Assignment,
                "statement" => Hug::Statement,
                _ => Hug::Never,
            },
            fill: options.flag(FILL, false),
            align: options
                .flag(ALIGN, false)
                .then(|| options.uint(CONTINUATION, 4)),
            start_operators: options.strings(START_OPERATORS),
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
        // (an element beyond the three, which a parse error leaves in the node, would be lost)
        (Some(left), Some(SyntaxElement::Token(op)), Some(right)) if buf.len() <= 3 => Ok(Link {
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
    // With `fill`: the contents of the lines, an operator at the end of a line with the operand
    // before it, one that starts a line (`binary_start_operators`) with the operand after it.
    let filling = end && policy.fill;
    let mut contents: Vec<Doc> = Vec::new();
    // (a line comment ends the line: the next content starts a line)
    let mut hard: Vec<bool> = Vec::new();
    let mut current = filling.then(|| first_doc.clone());
    let mut all_start = true;
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
        if let Some(cur) = current.take() {
            let op = hooks.token(cx, &l.op);
            let ends = last_token(cx, prev).is_some_and(|t| ends_in_line_comment(cx, &t));
            if policy.start_operators.iter().any(|o| o == l.op.text()) {
                contents.push(cur);
                hard.push(ends);
                current = Some(doc::concat(vec![op, doc::lit(" "), target_doc]));
            } else {
                all_start = false;
                contents.push(doc::concat(vec![cur, doc::lit(" "), op]));
                hard.push(ends || hooks.comment_ends_line(cx, &l.op));
                current = Some(target_doc);
            }
            prev = target;
            continue;
        }
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

    if let Some(last) = current {
        contents.push(last);
        return Ok(filled(policy, contents, &hard, all_start, node_nested(g, parent.as_ref())));
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

/// The width of the widest operand of the chain at `node` (flattened by precedence, an operand
/// that is a chain of another precedence counts whole) and the operator after it, from the source
/// text with runs of blanks as one: the line a chain filled to the line width cannot go below.
pub fn widest_operand(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Option<usize> {
    let g = cx.grammar;
    let mut buf = Vec::with_capacity(6);
    let mut cur = link(cx, node, &mut buf).ok()?;
    let top_op = cur.op.text().to_string();
    let width = |e: &SyntaxElement| -> usize {
        // (literals written one after another, which break between them)
        if let SyntaxElement::Node(n) = e
            && g.has_tag(n.kind(), "opaque")
        {
            return n
                .children_with_tokens()
                .filter(|c| !g.is_trivia(c.kind()))
                .map(|c| c.to_string().trim().chars().count())
                .max()
                .unwrap_or(0);
        }
        let text = e.to_string();
        let mut w = 0;
        let mut blank = false;
        for ch in text.trim().chars() {
            if ch.is_whitespace() {
                if !blank {
                    w += 1;
                }
                blank = true;
            } else {
                w += 1;
                blank = false;
            }
        }
        w
    };
    let mut widest = 0;
    loop {
        widest = widest.max(width(&cur.right) + 1 + cur.op.text().len());
        match &cur.left {
            SyntaxElement::Node(n) => {
                let inner = roles::un(n);
                match link(cx, &inner, &mut buf) {
                    Ok(l) if g.has_tag(inner.kind(), "binary-chain") && flattens(g, &top_op, l.op.text()) => {
                        cur = l;
                    }
                    _ => {
                        widest = widest.max(width(&cur.left) + 1 + top_op.len());
                        return Some(widest);
                    }
                }
            }
            left => {
                widest = widest.max(width(left) + 1 + top_op.len());
                return Some(widest);
            }
        }
    }
}

fn node_nested(g: &Grammar, parent: Option<&SyntaxNode>) -> bool {
    parent.is_some_and(|p| g.has_tag(p.kind(), "binary-chain"))
}

/// The lines of a chain filled to the line width: each line after the first is a group of its
/// own, so that it breaks before it when it, and the text that follows it, do not fit. A chain of
/// operators that start a line (`<<`) lines them up under the first one.
fn filled(
    policy: &BinaryPolicy,
    contents: Vec<Doc>,
    hard: &[bool],
    all_start: bool,
    nested: bool,
) -> Doc {
    let mut it = contents.into_iter();
    let first = it.next().unwrap_or(Doc::Nil);
    let rest: Vec<Doc> = it.collect();
    let lines = |lead_break: bool| -> Vec<Doc> {
        rest.iter()
            .enumerate()
            .map(|(i, c)| {
                if hard.get(i).copied().unwrap_or(false) {
                    doc::concat(vec![doc::hardline(), c.clone()])
                } else if i == 0 && !lead_break {
                    c.clone()
                } else {
                    doc::group(doc::concat(vec![doc::line(), c.clone()]))
                }
            })
            .collect()
    };
    if all_start && !rest.is_empty() {
        let first_width = first.flat_width();
        let beside = doc::concat(vec![
            first.clone(),
            doc::lit(" "),
            doc::align(doc::ALIGN_TO_COLUMN, doc::concat(lines(false))),
        ]);
        // (the first operator moves to a line of its own when it, and the text after it up to
        // the next break, do not fit beside the first operand)
        let below = doc::align(
            doc::ALIGN_TO_COLUMN,
            doc::concat(vec![
                first,
                doc::hardline(),
                rest[0].clone(),
                doc::concat(lines(true).into_iter().skip(1).collect()),
            ]),
        );
        return match (first_width, rest[0].flat_width()) {
            (Some(f), Some(w)) => doc::fits_at_column_rest(f + 1 + w, beside, below),
            _ => beside,
        };
    }
    let mut body = doc::concat(lines(true));
    match policy.align {
        Some(hang) => {
            if nested {
                body = doc::align(hang, body);
            }
            doc::label(
                "chain",
                doc::align(doc::ALIGN_TO_COLUMN, doc::concat(vec![first, body])),
            )
        }
        None => doc::concat(vec![first, doc::indent(body)]),
    }
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
