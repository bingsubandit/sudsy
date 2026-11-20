//! Conditional expressions (`test ? a : b`) read through roles: a node tagged `#conditional` has
//! the fields `condition`, `then` and `else`; the tokens `?` and `:` stand in front of the last
//! two. The layout is `indented` (the test, then both branches indented one level, each
//! conditional a group of its own) or `chained` (a chain of conditionals in the `else` branch
//! breaks as one, the branches line up after `? ` and `: `). What only the language can judge
//! (branches wrapped in parentheses, the places that need the whole indented) comes through
//! [`ConditionalHooks`].
//!
//! concept: conditional-expression-layout
//! what: Conditional expressions (test ? a : b) laid out flat, indented or chained.
//! tools: Prettier ternaries, ruff conditional expressions
//! reads: option:conditional_layout, tag:conditional

use super::chain::ends_in_line_comment;
use crate::FormatCtx;
use crate::comments;
use crate::doc::{self, Doc};
use crate::error::FormatError;
use core_config::ResolvedOptions;
use core_syntax::roles;
use core_syntax::{SyntaxElement, SyntaxNode, SyntaxToken};

pub const LAYOUT: &str = "conditional_layout";
pub const INDENT: &str = "conditional_indent";

/// The values where reference tools differ; a language declares each as an option.
pub struct ConditionalPolicy {
    /// `chained` (else `indented`).
    pub chained: bool,
    /// Levels of indentation of the branches in the `indented` layout.
    pub indent: usize,
    /// Indentation width, for the alignment of a conditional in a `then` branch.
    pub tab_width: usize,
}

impl ConditionalPolicy {
    pub fn of(options: &ResolvedOptions) -> Self {
        Self {
            chained: options.is(LAYOUT, "chained"),
            indent: options.uint(INDENT, 1),
            tab_width: options.uint("indent_size", 2),
        }
    }
}

/// Where the conditional is an operand that wants the whole indented below an opening line.
#[derive(Default)]
pub struct Around {
    /// It is the left side of a cast or of a member access or call that continues after it.
    pub indent: bool,
    /// It is the object of a member access: the closing line follows it.
    pub member_operand: bool,
}

/// What the language prints or judges itself; every method has a default.
pub trait ConditionalHooks {
    /// A token (the operators, or a bare operand).
    fn token(&self, cx: &FormatCtx<'_>, t: &SyntaxToken) -> Doc {
        cx.token(t)
    }
    /// Does a line comment follow the token on its line (a line break must follow it)?
    fn comment_ends_line(&self, cx: &FormatCtx<'_>, t: &SyntaxToken) -> bool {
        ends_in_line_comment(cx, t)
    }
    /// The branches are wrapped in parentheses that show when they break (JSX); `top` is the
    /// first conditional of the chain the node is in.
    fn wrap_branches(&self, _cx: &FormatCtx<'_>, _node: &SyntaxNode, _top: &SyntaxNode) -> bool {
        false
    }
    /// A wrapped branch that stays bare (`null`).
    fn bare_branch(&self, _cx: &FormatCtx<'_>, _branch: &SyntaxNode) -> bool {
        false
    }
    fn around(&self, _cx: &FormatCtx<'_>, _node: &SyntaxNode) -> Around {
        Around::default()
    }
}

/// A language with nothing of its own.
pub struct NoConditionalHooks;

impl ConditionalHooks for NoConditionalHooks {}

struct Parts {
    test: SyntaxElement,
    q: SyntaxToken,
    then: SyntaxElement,
    colon: SyntaxToken,
    other: SyntaxElement,
}

fn parts(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Result<Parts, FormatError> {
    let buf: Vec<SyntaxElement> = node
        .children_with_tokens()
        .filter(|e| !cx.grammar.is_trivia(e.kind()) && !e.text_range().is_empty())
        .collect();
    let slot = |f: &str| cx.grammar.slot_in(node.kind(), &buf, f);
    let before = |e: &SyntaxElement| {
        let at = buf.iter().position(|x| x == e)?;
        match buf.get(at.checked_sub(1)?)? {
            SyntaxElement::Token(t) => Some(t.clone()),
            SyntaxElement::Node(_) => None,
        }
    };
    if let (Some(test), Some(then), Some(other)) = (slot("condition"), slot("then"), slot("else"))
        && let (Some(q), Some(colon)) = (before(&then), before(&other))
    {
        return Ok(Parts {
            test,
            q,
            then,
            colon,
            other,
        });
    }
    Err(FormatError::Shape("conditional expression".into()))
}

fn operand(
    cx: &FormatCtx<'_>,
    hooks: &dyn ConditionalHooks,
    e: &SyntaxElement,
) -> Result<Doc, FormatError> {
    match e {
        SyntaxElement::Node(n) => cx.format(n),
        SyntaxElement::Token(t) => Ok(hooks.token(cx, t)),
    }
}

fn gap(cx: &FormatCtx<'_>, hooks: &dyn ConditionalHooks, t: &SyntaxToken) -> Doc {
    if hooks.comment_ends_line(cx, t) {
        doc::hardline()
    } else {
        doc::lit(" ")
    }
}

/// The marker (`?` or `:`) and its branch aligned after it. A line comment after the marker
/// (`ends_line`) puts the branch on the next line; a `nested` conditional in the branch keeps
/// its own alignment.
pub fn marked_branch(marker: Doc, ends_line: bool, nested: bool, branch: Doc) -> Doc {
    let (gap, lead) = if ends_line {
        (Doc::Nil, doc::hardline())
    } else {
        (doc::lit(" "), Doc::Nil)
    };
    let branch = if nested && !ends_line {
        branch
    } else {
        doc::align(2, doc::concat(vec![lead, branch]))
    };
    doc::concat(vec![marker, gap, branch])
}

/// `test ? a : b` on one line if it fits, else one branch per line.
pub fn conditional(
    cx: &FormatCtx<'_>,
    node: &SyntaxNode,
    policy: &ConditionalPolicy,
    hooks: &dyn ConditionalHooks,
) -> Result<Doc, FormatError> {
    let p = parts(cx, node)?;
    if policy.chained {
        return chained(cx, node, &p, policy, hooks);
    }
    let before = |e: &SyntaxElement| {
        let last = match e {
            SyntaxElement::Token(t) => Some(t.clone()),
            SyntaxElement::Node(n) => cx.last_token(n),
        };
        if last.is_some_and(|t| hooks.comment_ends_line(cx, &t)) {
            doc::hardline()
        } else {
            doc::line()
        }
    };
    let mut branches = doc::concat(vec![
        before(&p.test),
        hooks.token(cx, &p.q),
        gap(cx, hooks, &p.q),
        operand(cx, hooks, &p.then)?,
        before(&p.then),
        hooks.token(cx, &p.colon),
        gap(cx, hooks, &p.colon),
        operand(cx, hooks, &p.other)?,
    ]);
    for _ in 0..policy.indent {
        branches = doc::indent(branches);
    }
    Ok(doc::group(doc::concat(vec![
        operand(cx, hooks, &p.test)?,
        branches,
    ])))
}

fn chained(
    cx: &FormatCtx<'_>,
    node: &SyntaxNode,
    p: &Parts,
    policy: &ConditionalPolicy,
    hooks: &dyn ConditionalHooks,
) -> Result<Doc, FormatError> {
    use doc::{align, concat, group, hardline, if_break, indent, lit, softline};
    let g = cx.grammar;
    let kind = node.kind();
    let held = roles::outer(node);
    let parent = held.parent();
    let same_parent = parent.as_ref().is_some_and(|p| p.kind() == kind);
    let slot_is = |up: &SyntaxNode, field: &str, n: &SyntaxNode| {
        g.slot(up, field).and_then(|e| e.into_node()).as_ref() == Some(n)
    };
    let is_parent_test = same_parent
        && parent
            .as_ref()
            .is_some_and(|p| slot_is(p, "condition", &held));
    let is_alt_of_parent =
        same_parent && parent.as_ref().is_some_and(|p| slot_is(p, "else", &held));
    // The top of the chain of conditionals this one is in (through branches, not tests).
    let mut top = node.clone();
    let first_non_cond = loop {
        let up = roles::outer(&top).parent();
        match up {
            Some(up) if up.kind() == kind && !slot_is(&up, "condition", &roles::outer(&top)) => {
                top = up
            }
            other => break other,
        }
    };
    let inner_of = |e: &SyntaxElement| e.as_node().map(roles::un);
    let is_cond = |n: &Option<SyntaxNode>| n.as_ref().is_some_and(|n| n.kind() == kind);
    let (then_u, other_u) = (inner_of(&p.then), inner_of(&p.other));
    let wrap_mode = hooks.wrap_branches(cx, node, &top);
    // Comments in front of `?` and `:` move to the front of their branch.
    let moved = |t: &SyntaxToken| {
        let cs = comments::leading_comments(g, t);
        if cs.is_empty() || cs.iter().any(|c| c.kind != comments::CommentKind::Line) {
            return Doc::Nil;
        }
        cx.hold_leading(t);
        let mut parts = Vec::new();
        for c in &comments::leading_comments(g, t) {
            parts.push(crate::context::comment_doc(c));
            parts.push(
                if c.kind == comments::CommentKind::Block && c.breaks_after == 0 {
                    lit(" ")
                } else {
                    hardline()
                },
            );
        }
        concat(parts)
    };
    let (q_moved, colon_moved) = (moved(&p.q), moved(&p.colon));
    let (q, colon) = (hooks.token(cx, &p.q), hooks.token(cx, &p.colon));
    let (then_doc, other_doc) = (operand(cx, hooks, &p.then)?, operand(cx, hooks, &p.other)?);
    let mut out = Vec::new();
    if wrap_mode {
        let wrap = |d: Doc| {
            super::between(
                if_break(lit("("), Doc::Nil),
                d,
                if_break(lit(")"), Doc::Nil),
            )
        };
        let bare = |n: &Option<SyntaxNode>| n.as_ref().is_some_and(|n| hooks.bare_branch(cx, n));
        out.extend([lit(" "), q, lit(" ")]);
        out.push(if bare(&then_u) {
            then_doc
        } else {
            wrap(then_doc)
        });
        out.extend([lit(" "), colon, lit(" ")]);
        out.push(if is_cond(&other_u) || bare(&other_u) {
            other_doc
        } else {
            wrap(other_doc)
        });
    } else {
        let then_cond = is_cond(&then_u);
        let paren = |open: bool| match (then_cond, open) {
            (false, _) => Doc::Nil,
            (true, true) => if_break(Doc::Nil, lit("(")),
            (true, false) => if_break(Doc::Nil, lit(")")),
        };
        // A line comment after `?` ends the line: the branch starts on the next one. A
        // conditional in a branch indents its own branches once.
        let branch = |t: &SyntaxToken, cond: bool, mv: Doc, d: Doc| {
            let lead = if hooks.comment_ends_line(cx, t) {
                hardline()
            } else {
                Doc::Nil
            };
            if cond {
                if matches!(lead, Doc::Nil) {
                    concat(vec![align(2, concat(vec![lead, mv])), d])
                } else {
                    align(2, concat(vec![lead, mv, d]))
                }
            } else {
                align(2, concat(vec![lead, mv, d]))
            }
        };
        let space = |t: &SyntaxToken| {
            if hooks.comment_ends_line(cx, t) {
                Doc::Nil
            } else {
                lit(" ")
            }
        };
        let branches = concat(vec![
            doc::line(),
            q,
            space(&p.q),
            paren(true),
            branch(&p.q, then_cond, q_moved, then_doc),
            paren(false),
            doc::line(),
            colon,
            space(&p.colon),
            branch(&p.colon, is_cond(&other_u), colon_moved, other_doc),
        ]);
        out.push(if !same_parent || is_alt_of_parent || is_parent_test {
            branches
        } else {
            align(policy.tab_width.saturating_sub(2), branches)
        });
    }
    let after_comment = same_parent
        && !is_parent_test
        && parent.as_ref().is_some_and(|up| {
            let t = if is_alt_of_parent {
                parts(cx, up).ok().map(|u| u.colon)
            } else {
                parts(cx, up).ok().map(|u| u.q)
            };
            t.is_some_and(|t| hooks.comment_ends_line(cx, &t))
        });
    let test_doc = {
        let d = operand(cx, hooks, &p.test)?;
        if is_alt_of_parent { align(2, d) } else { d }
    };
    let around = hooks.around(cx, node);
    let closing = !wrap_mode && around.member_operand && !around.indent;
    let body = concat(vec![
        test_doc,
        // A branch that follows a line comment is aligned by its parent already.
        if wrap_mode || after_comment {
            concat(out)
        } else {
            indent(concat(out))
        },
        if closing { softline() } else { Doc::Nil },
    ]);
    let result = if parent == first_non_cond {
        group(body)
    } else {
        body
    };
    Ok(if is_parent_test || around.indent {
        super::enclosed(Doc::Nil, result, Doc::Nil)
    } else {
        result
    })
}
