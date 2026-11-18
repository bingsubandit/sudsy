//! Arrow functions: parameters, the body on the arrow's line or the next, chains
//! (`a => b => c`), and the forms used when the function is hugged as a call argument.

use super::ast::{has_comment, is_binaryish, outer, parent, starts_with, un};
use super::util::{R, kids, node_of, remove_lines, shape_error, token_of, will_break};
use crate::generated::kind as k;
use core_format::doc::{
    Doc, concat, group, group_broken, if_break, if_break_in, indent, indent_if_break, join, line,
    lit, new_group_id, softline,
};
use core_format::shapes;
use core_format::{FormatCtx, FormatError};
use core_syntax::SyntaxNode;

/// The layouts of one function the callers ask for (bits of the `layout` cache flags).
pub(crate) struct Flags;

impl Flags {
    pub const EXPAND_LAST: u32 = 1;
    pub const EXPAND_FIRST: u32 = 2;
    /// The parameters of a test call's function stay on one line.
    pub const TEST: u32 = 4;
    /// The right side of an assignment.
    pub const RHS: u32 = 8;
    /// The tail of an assignment chain whose value is an arrow chain.
    pub const CHAIN_TAIL: u32 = 16;
    pub const EXPAND: u32 = Self::EXPAND_LAST | Self::EXPAND_FIRST;
}

const BAILOUT: &str = "arguments cannot expand";

pub(crate) fn format(node: &SyntaxNode, cx: &FormatCtx<'_>) -> Result<Option<Doc>, FormatError> {
    if node.kind() != k::ARROW_FUNCTION_EXPRESSION {
        return Ok(None);
    }
    print_fn(cx, node, 0)
}

/// The number of parameters of a function.
pub(crate) fn param_count(f: &SyntaxNode) -> usize {
    f.children()
        .find(|c| c.kind() == k::FORMAL_PARAMETERS)
        .map_or(0, |p| {
            p.children()
                .filter(|c| matches!(c.kind(), k::FORMAL_PARAMETER | k::FORMAL_PARAMETER_REST))
                .count()
        })
}

/// A function or arrow function in the layout `flags` asks for; `None` when a hugged
/// function's parameters would have to break.
pub(crate) fn print_fn(
    cx: &FormatCtx<'_>,
    node: &SyntaxNode,
    flags: u32,
) -> Result<Option<Doc>, FormatError> {
    let made = cx.layout(node, flags, || match node.kind() {
        k::FUNCTION => super::decl::function(node, cx, flags)?.ok_or_else(|| shape_error(BAILOUT)),
        _ => arrow(node, cx, flags),
    });
    match made {
        Ok(d) => Ok(Some(d)),
        Err(FormatError::Shape(m)) if m == BAILOUT => Ok(None),
        Err(e) => Err(e),
    }
}

/// `async <T>(a, b): R` of one arrow function.
fn signature(cx: &FormatCtx<'_>, node: &SyntaxNode, flags: u32) -> R {
    let elems = kids(cx, node);
    let mut parts = Vec::new();
    let mut inner = Vec::new();
    let mut arrow = lit(" =>");
    for e in &elems {
        match e {
            e if token_of(e).is_some_and(|t| t.text() == "async") => {
                parts.push(super::util::elem(cx, e)?);
                parts.push(lit(" "));
            }
            e if token_of(e).is_some_and(|t| t.kind() == k::ARROW) => {
                if let Some(t) = token_of(e) {
                    arrow = concat(vec![lit(" "), cx.token(t)]);
                }
                break;
            }
            e if node_of(e).is_some_and(|n| n.kind() == k::FORMAL_PARAMETERS) => {
                let fp = node_of(e).ok_or_else(|| shape_error("arrow"))?;
                let elems = kids(cx, fp);
                // `arrow_parens: avoid` leaves a lone name without parentheses.
                let mut params = match elems.first() {
                    Some(e) if token_of(e).is_none() => super::util::elem(cx, e)?,
                    // Grouped with the return type below, so both break together.
                    _ => super::decl::parameters_in(cx, &elems, Some(false))?,
                };
                if flags & (Flags::EXPAND | Flags::TEST) != 0 {
                    if flags & Flags::EXPAND != 0 && will_break(&params) {
                        return Err(shape_error(BAILOUT));
                    }
                    params = remove_lines(&params);
                }
                inner.push(params);
            }
            e => {
                let printed = super::util::elem(cx, e)?;
                // (a test's function keeps its type parameters on the line, too)
                let flat = flags & Flags::TEST != 0
                    && node_of(e).is_some_and(|n| n.kind() == k::TS_TYPE_PARAMETER_DECLARATION);
                inner.push(if flat {
                    remove_lines(&printed)
                } else {
                    printed
                });
            }
        }
    }
    parts.push(group(concat(inner)));
    parts.push(arrow);
    Ok(concat(parts))
}

/// The arrow that is the body `body`, when the chain goes on in it: any pairs of parentheses
/// around it are left out of the output and carry no comments.
pub(crate) fn arrow_body(cx: &FormatCtx<'_>, body: &SyntaxNode) -> Option<SyntaxNode> {
    let mut n = body.clone();
    while n.kind() == k::PARENTHESIZED_EXPRESSION {
        let mut parens = n.children_with_tokens().filter_map(|e| e.into_token());
        let open = parens.find(|t| t.kind() == k::L_PAREN)?;
        let close = parens.find(|t| t.kind() == k::R_PAREN)?;
        if super::ast::token_comments(cx, &open)
            || super::ast::token_comments(cx, &close)
            || !super::parens::is_dropped_close(cx, &close)
        {
            return None;
        }
        n = n.children().next()?;
    }
    (n.kind() == k::ARROW_FUNCTION_EXPRESSION).then_some(n)
}

fn arrow(node: &SyntaxNode, cx: &FormatCtx<'_>, flags: u32) -> R {
    let expand_last = flags & Flags::EXPAND_LAST != 0;
    let outer_node = outer(node);
    let p = parent(node);
    let is_callee = p.as_ref().is_some_and(|p| {
        matches!(p.kind(), k::CALL_EXPRESSION | k::NEW_EXPRESSION)
            && p.first_child().as_ref() == Some(&outer_node)
    });
    let mut sigs = Vec::new();
    let mut break_chain = false;
    let mut cur = node.clone();
    // The comments in front of a sequence body go in front of its parentheses.
    // Own-line comments after the `=>` are comments in front of the body, too.
    let (tail, body_doc, after_arrow, held) = loop {
        let arrow_tok = cur
            .children_with_tokens()
            .filter_map(|e| e.into_token())
            .find(|t| t.kind() == k::ARROW);
        let mut after_arrow = arrow_tok.as_ref().map_or_else(Vec::new, |t| {
            core_format::comments::trailing_comments(cx.grammar, t)
        });
        let own_line = after_arrow.iter().any(|c| {
            c.kind != core_format::comments::CommentKind::Block
                || c.breaks_after > 0
                || c.text().contains('\n')
        });
        let arrow_tok = arrow_tok.filter(|_| own_line);
        if let Some(t) = &arrow_tok {
            cx.hold_trailing(t);
        } else {
            after_arrow.clear();
        }
        // Comments on lines of their own in front of a signature go in front of its parameters.
        let lead = cx.first_token(&cur).filter(|t| {
            !cx.is_held_leading(t)
                && core_format::comments::leading_comments(cx.grammar, t)
                    .iter()
                    .any(|c| {
                        c.kind != core_format::comments::CommentKind::Block || c.breaks_after > 0
                    })
        });
        if let Some(t) = &lead {
            cx.hold_leading(t);
        }
        let sig = signature(cx, &cur, flags)?;
        sigs.push(match &lead {
            Some(t) => concat(vec![cx.held_leading(t), sig]),
            None => sig,
        });
        let params_not_plain = cur
            .children()
            .find(|c| c.kind() == k::FORMAL_PARAMETERS)
            .is_some_and(|fp| {
                fp.children().any(|c| {
                    c.kind() != k::FORMAL_PARAMETER
                        || c.children().count() != 1
                        || c.first_child()
                            .is_none_or(|i| i.kind() != k::BINDING_IDENTIFIER)
                })
            });
        let has_return_type = cur.children().any(|c| c.kind() == k::TS_TYPE_ANNOTATION);
        let has_type_params = cur
            .children()
            .any(|c| c.kind() == k::TS_TYPE_PARAMETER_DECLARATION);
        break_chain |= (has_return_type && super::arrow::param_count(&cur) > 0)
            || has_type_params
            || params_not_plain;
        let body = cur
            .last_child()
            .ok_or_else(|| shape_error("arrow without body"))?;
        if let Some(next) = (!expand_last).then(|| arrow_body(cx, &body)).flatten() {
            if let Some(t) = &arrow_tok {
                let sig = sigs.pop().ok_or_else(|| shape_error("arrow"))?;
                sigs.push(concat(vec![sig, cx.held_trailing(t)]));
            }
            cur = next;
        } else {
            let held = cx.first_token(&body).filter(|t| {
                un(&body).kind() == k::SEQUENCE_EXPRESSION
                    && !core_format::comments::leading_comments(cx.grammar, t).is_empty()
            });
            if let Some(t) = &held {
                cx.hold_leading(t);
            }
            break (cur.clone(), cx.format(&body)?, after_arrow, held);
        }
    };
    let body = tail
        .last_child()
        .ok_or_else(|| shape_error("arrow without body"))?;
    let body_u = un(&body);
    let is_chain = sigs.len() > 1;
    let comments = has_comment(cx, &body);
    let same_line = !super::ast::has_leading_comment(cx, &body)
        && after_arrow.is_empty()
        && (matches!(
            body_u.kind(),
            k::ARRAY_EXPRESSION
                | k::OBJECT_EXPRESSION
                | k::FUNCTION_BODY
                | k::JSX_ELEMENT
                | k::JSX_FRAGMENT
                | k::ARROW_FUNCTION_EXPRESSION
        ) || (matches!(
            body_u.kind(),
            k::TEMPLATE_LITERAL | k::TAGGED_TEMPLATE_EXPRESSION
        ) && template_literal(&body_u)
            .is_some_and(|t| core_format::calls::template_on_own_line(cx, &body_u, &t))));
    let jsx_container = p
        .as_ref()
        .is_some_and(|p| p.kind() == k::JSX_EXPRESSION_CONTAINER);
    let soft_line = (expand_last || jsx_container) && !comments;
    let trailing_comma = if expand_last {
        super::util::trailing_comma(cx, true)
    } else {
        Doc::Nil
    };
    let body_part = if held.is_some() || !after_arrow.is_empty() {
        let sequence = body_u.kind() == k::SEQUENCE_EXPRESSION && !cx.is_ignored(&body);
        concat(vec![
            indent(concat(vec![
                line(),
                core_format::context::leading_comments_doc(&after_arrow),
                held.as_ref().map_or(Doc::Nil, |t| cx.held_leading(t)),
                if sequence {
                    shapes::enclosed(lit("("), body_doc, lit(")"))
                } else {
                    body_doc
                },
            ])),
            if soft_line {
                concat(vec![trailing_comma, softline()])
            } else {
                Doc::Nil
            },
        ])
    } else if body_u.kind() == k::SEQUENCE_EXPRESSION && !cx.is_ignored(&body) {
        shapes::between(lit(" ("), body_doc, lit(")"))
    } else if same_line {
        concat(vec![lit(" "), body_doc])
    } else {
        let parens = body_u.kind() == k::CONDITIONAL_EXPRESSION
            && !starts_with(&body_u, &|n| n.kind() == k::OBJECT_EXPRESSION);
        concat(vec![
            indent(concat(vec![
                line(),
                if parens {
                    if_break(Doc::Nil, lit("("))
                } else {
                    Doc::Nil
                },
                body_doc,
                if parens {
                    if_break(Doc::Nil, lit(")"))
                } else {
                    Doc::Nil
                },
            ])),
            if soft_line {
                concat(vec![trailing_comma, softline()])
            } else {
                Doc::Nil
            },
        ])
    };
    if !is_chain {
        // The body breaks on its own: `) => body` stays on the line when only the parameters broke.
        let own_line = body_u.kind() != k::SEQUENCE_EXPRESSION && !same_line;
        let body_part = if own_line {
            group(body_part)
        } else {
            body_part
        };
        return Ok(group(concat(vec![group(sigs.remove(0)), body_part])));
    }
    let assignment = flags & Flags::RHS != 0;
    let chain_tail = flags & Flags::CHAIN_TAIL != 0;
    let signatures = signatures_doc(cx, node, flags, sigs, break_chain, is_callee);
    let (indent_sigs, break_sigs) = if is_callee || assignment {
        (true, chain_tail || (is_callee && !same_line))
    } else {
        (false, false)
    };
    let id = new_group_id();
    let sig_group = {
        let inner = if indent_sigs {
            indent(concat(vec![softline(), signatures]))
        } else {
            signatures
        };
        Doc::Group(Box::new(core_format::doc::Group {
            contents: inner,
            states: Vec::new(),
            broken: break_sigs,
            id: Some(id),
        }))
    };
    Ok(group(concat(vec![
        sig_group,
        indent_if_break(id, body_part),
        if is_callee {
            if_break_in(id, softline(), Doc::Nil)
        } else {
            Doc::Nil
        },
    ])))
}

fn signatures_doc(
    cx: &FormatCtx<'_>,
    node: &SyntaxNode,
    flags: u32,
    sigs: Vec<Doc>,
    should_break: bool,
    is_callee: bool,
) -> Doc {
    let _ = cx;
    let p = parent(node);
    let call_like = |n: &SyntaxNode| {
        matches!(
            n.kind(),
            k::CALL_EXPRESSION | k::NEW_EXPRESSION | k::IMPORT_EXPRESSION
        )
    };
    let arrow_sep = line();
    let wrap = |d: Doc| {
        if should_break {
            group_broken(d)
        } else {
            group(d)
        }
    };
    let p_kind_call = p.as_ref().is_some_and(call_like);
    if (!is_callee && p_kind_call) || p.as_ref().is_some_and(|p| is_binaryish(p.kind())) {
        let mut it = sigs.into_iter();
        let head = it.next().unwrap_or(Doc::Nil);
        return wrap(concat(vec![
            head,
            indent(concat(vec![line(), join(&arrow_sep, it)])),
        ]));
    }
    if is_callee || flags & Flags::RHS != 0 {
        return wrap(join(&arrow_sep, sigs));
    }
    wrap(indent(join(&arrow_sep, sigs)))
}

/// The template literal of a template or a tagged template.
fn template_literal(n: &SyntaxNode) -> Option<SyntaxNode> {
    match n.kind() {
        k::TEMPLATE_LITERAL => Some(n.clone()),
        k::TAGGED_TEMPLATE_EXPRESSION => n.last_child().filter(|t| t.kind() == k::TEMPLATE_LITERAL),
        _ => None,
    }
}
