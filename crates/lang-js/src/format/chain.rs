//! Member expressions and member chains (`a.b().c().d()`): the chain is core-format's `call_chain`
//! in the `grouped` style; this module keeps what only
//! JavaScript knows: parentheses, comments, the argument list and `[expr]`.
//!
//! concept: member-chain

use super::ast::{Call, is_member, parent, token_comments, un};
use super::call::arguments;
use super::parens::{Need, needs_parens};
use super::util::{R, kids, shape_error};
use crate::generated::kind as k;
use core_format::doc::{Doc, concat, group, hardline, indent, softline};
use core_format::shapes::{self, ChainHooks, ChainPolicy};
use core_format::{FormatCtx, FormatError, comments};
use core_syntax::{SyntaxElement, SyntaxNode, SyntaxToken};

pub(crate) fn format(node: &SyntaxNode, cx: &FormatCtx<'_>) -> Result<Option<Doc>, FormatError> {
    match node.kind() {
        k::CHAIN_EXPRESSION => match node.first_child() {
            Some(inner) => Ok(Some(cx.format(&inner)?)),
            None => Err(shape_error("chain")),
        },
        k::STATIC_MEMBER_EXPRESSION
        | k::COMPUTED_MEMBER_EXPRESSION
        | k::PRIVATE_FIELD_EXPRESSION => Ok(Some(member(node, cx)?)),
        _ => Ok(None),
    }
}

/// The `.name`, `?.name` or `[expr]` of a member expression.
fn lookup(node: &SyntaxNode, cx: &FormatCtx<'_>) -> R {
    let elems = kids(cx, node);
    let mut parts = Vec::new();
    let mut computed = None;
    for e in elems.iter().skip(1) {
        match e {
            SyntaxElement::Token(t) if t.kind() == k::L_BRACKET => parts.push(cx.token(t)),
            SyntaxElement::Token(t) if t.kind() == k::R_BRACKET => {
                let inner = parts.pop();
                let open = parts.pop();
                let (Some(open), Some(inner)) = (open, inner) else {
                    return Err(shape_error("member"));
                };
                let numeric = computed == Some(k::NUMERIC_LITERAL);
                parts.push(if numeric {
                    concat(vec![open, inner, cx.token(t)])
                } else {
                    core_format::shapes::enclosed(open, inner, cx.token(t))
                });
            }
            SyntaxElement::Token(t) => parts.push(cx.token(t)),
            SyntaxElement::Node(n) => {
                computed = Some(n.kind());
                parts.push(cx.format(n)?);
            }
        }
    }
    Ok(concat(parts))
}

fn member(node: &SyntaxNode, cx: &FormatCtx<'_>) -> R {
    let object = node.first_child().ok_or_else(|| shape_error("member"))?;
    let object_doc = cx.format(&object)?;
    let lookup = lookup(node, cx)?;
    let first_non_member = std::iter::successors(parent(node), |p| {
        matches!(
            p.kind(),
            k::STATIC_MEMBER_EXPRESSION
                | k::COMPUTED_MEMBER_EXPRESSION
                | k::PRIVATE_FIELD_EXPRESSION
                | k::TS_NON_NULL_EXPRESSION
        )
        .then(|| parent(p))
        .flatten()
    })
    .last();
    let simple_names = un(&object).kind() == k::IDENTIFIER_REFERENCE
        && node
            .last_child()
            .is_some_and(|p| p.kind() == k::IDENTIFIER_NAME)
        && parent(node).is_none_or(|p| !is_member(p.kind()));
    let inline = first_non_member.is_some_and(|p| {
        p.kind() == k::NEW_EXPRESSION
            || (p.kind() == k::ASSIGNMENT_EXPRESSION
                && p.first_child()
                    .is_some_and(|l| un(&l).kind() != k::IDENTIFIER_REFERENCE))
    }) || node.kind() == k::COMPUTED_MEMBER_EXPRESSION
        || simple_names
        // `a.b(c).d`: the member chain of the call breaks inside its arguments first.
        || (un(&object).kind() == k::CALL_EXPRESSION
            && un(&object).first_child().is_some_and(|c| is_member(un(&c).kind())));
    Ok(concat(vec![
        object_doc,
        if inline {
            lookup
        } else {
            group(indent(concat(vec![softline(), lookup])))
        },
    ]))
}

/// What JavaScript prints and judges in a chain.
struct Hooks<'a> {
    cx: &'a FormatCtx<'a>,
    /// The first token of the chain (a comment in front of it belongs to the whole chain).
    start: Option<SyntaxToken>,
}

impl ChainHooks for Hooks<'_> {
    fn link(&self, node: &SyntaxNode, last: bool) -> Option<Result<(Doc, bool), FormatError>> {
        let cx = self.cx;
        match node.kind() {
            k::CALL_EXPRESSION => Some(call_link(cx, node, last)),
            k::COMPUTED_MEMBER_EXPRESSION => Some(lookup(node, cx).map(|d| {
                let commented = kids(cx, node)
                    .iter()
                    .any(|e| e.as_token().is_some_and(|t| token_comments(cx, t)));
                (d, commented)
            })),
            _ => None,
        }
    }

    /// Parentheses that are not needed are not part of the chain's text.
    fn through(&self, paren: &SyntaxNode) -> bool {
        let inner = un(paren);
        (matches!(inner.kind(), k::CALL_EXPRESSION | k::TS_NON_NULL_EXPRESSION)
            || is_member(inner.kind()))
            && needs_parens(self.cx, &inner) == Need::No
            && !paren_commented(self.cx, paren, &self.start)
    }

    fn simple_argument(&self, arg: &SyntaxNode) -> bool {
        super::call::rules().is_simple(self.cx, arg, 0)
    }

    fn statement(&self, chain: &SyntaxNode) -> bool {
        parent(chain).is_some_and(|p| p.kind() == k::EXPRESSION_STATEMENT)
    }
}

/// `?.`, type arguments and arguments of a call link; a comment after the `)` of a link goes after
/// the link, and a blank line after it stays.
fn call_link(cx: &FormatCtx<'_>, n: &SyntaxNode, last: bool) -> Result<(Doc, bool), FormatError> {
    let c = Call::of(cx, n).ok_or_else(|| shape_error("call"))?;
    let mut v = Vec::new();
    if let Some(o) = &c.optional {
        v.push(cx.token(o));
    }
    if let Some(t) = &c.type_args {
        v.push(cx.format(t)?);
    }
    let held = c
        .close
        .as_ref()
        .filter(|t| !last && comments::has_trailing_comment(cx.grammar, t));
    if let Some(t) = held {
        cx.hold_trailing(t);
    }
    v.push(arguments(cx, n, &c)?);
    if let Some(t) = held {
        v.push(cx.held_trailing(t));
    }
    let blank = !last
        && cx
            .last_token(n)
            .and_then(|l| l.next_token())
            .map(|mut t| {
                while cx.grammar.is_trivia(t.kind()) {
                    match t.next_token() {
                        Some(x) => t = x,
                        None => break,
                    }
                }
                t
            })
            .is_some_and(|t| {
                t.kind() == k::DOT && comments::blank_lines_before(cx.grammar, &t) > 0
            });
    if blank {
        v.push(hardline());
    }
    Ok((concat(v), blank || held.is_some()))
}

/// `a.b().c()` printed as a chain: the call `call_node` is the last link.
pub(crate) fn member_chain(cx: &FormatCtx<'_>, call_node: &SyntaxNode) -> R {
    let start = cx.first_token(call_node);
    // Comments on lines of their own in front of the chain go before it: their line breaks must not
    // break the chain's group.
    let lead = start.clone().filter(|t| {
        !cx.is_held_leading(t)
            && comments::leading_comments(cx.grammar, t)
                .iter()
                .any(|c| c.breaks_after > 0)
    });
    if let Some(t) = &lead {
        cx.hold_leading(t);
    }
    let chain = shapes::call_chain(
        cx,
        call_node,
        &ChainPolicy::of(cx.options),
        &Hooks { cx, start },
    )?;
    Ok(match &lead {
        Some(t) => concat(vec![cx.held_leading(t), chain]),
        None => chain,
    })
}

/// A comment in front of the chain's first token belongs to the whole chain, not to the parentheses.
fn paren_commented(cx: &FormatCtx<'_>, n: &SyntaxNode, start: &Option<SyntaxToken>) -> bool {
    let mut cur = n.clone();
    while cur.kind() == k::PARENTHESIZED_EXPRESSION {
        if cur
            .children_with_tokens()
            .filter_map(|e| e.into_token())
            .any(|t| {
                if Some(&t) == start.as_ref() {
                    comments::has_trailing_comment(cx.grammar, &t)
                } else {
                    token_comments(cx, &t)
                }
            })
        {
            return true;
        }
        match cur.first_child() {
            Some(c) => cur = c,
            None => break,
        }
    }
    false
}

/// Does the call print as a chain that may break (more than a short `a.b.c()` access)?
pub(crate) fn is_chain_label(doc: &Doc) -> bool {
    doc.label_of() == Some(shapes::CHAIN_LABEL)
}
