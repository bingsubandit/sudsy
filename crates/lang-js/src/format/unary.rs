//! The small expressions: unary, update, `await`, `yield`, spread and sequence expressions, and
//! `return` / `throw` with their argument.
//!
//! concept: new unary-expression-layout
//! what: Unary, update, await, yield, spread, return and throw expressions.
//! tools: Prettier unary and keyword expressions

use super::ast::{
    has_leading_comment_in_parens, has_leading_own_line_comment, is_member, outer, parent,
    starts_with, un,
};
use super::util::{R, elem, keyword, kids, node_of, semi, shape_error, token_of};
use crate::generated::kind as k;
use core_format::doc::{Doc, concat, group, hardline, indent, line, lit, softline};
use core_format::shapes;
use core_format::{FormatCtx, FormatError};
use core_syntax::roles::has;
use core_syntax::{SyntaxElement, SyntaxNode, SyntaxToken};

pub(crate) fn format(node: &SyntaxNode, cx: &FormatCtx<'_>) -> Result<Option<Doc>, FormatError> {
    let doc = match node.kind() {
        k::UNARY_EXPRESSION => unary(node, cx)?,
        // `(x as T)++`: the parentheses of the operand are tokens of the node; the operand
        // brings the ones it needs.
        k::UPDATE_EXPRESSION => {
            let mut parts = Vec::new();
            for e in node.children_with_tokens() {
                match e {
                    SyntaxElement::Token(t) if matches!(t.kind(), k::L_PAREN | k::R_PAREN) => {}
                    SyntaxElement::Token(t) if cx.grammar.is_trivia(t.kind()) => {}
                    SyntaxElement::Token(t) => parts.push(cx.token(&t)),
                    SyntaxElement::Node(n) => parts.push(cx.format(&n)?),
                }
            }
            core_format::doc::concat(parts)
        }
        k::SPREAD_ELEMENT => cx.concat_children(node)?,
        k::AWAIT_EXPRESSION => await_expression(node, cx)?,
        k::YIELD_EXPRESSION => yield_expression(node, cx)?,
        k::SEQUENCE_EXPRESSION => sequence(node, cx)?,
        k::RETURN_STATEMENT | k::THROW_STATEMENT => return_or_throw(node, cx)?,
        _ => return Ok(None),
    };
    Ok(Some(doc))
}

fn unary(node: &SyntaxNode, cx: &FormatCtx<'_>) -> R {
    let elems = kids(cx, node);
    let (Some(op), Some(arg)) = (elems.first().and_then(token_of), node.last_child()) else {
        return Err(shape_error("unary"));
    };
    let word = op.text().ends_with(|c: char| c.is_ascii_alphabetic());
    let arg_doc = if let Some(doc) = commented_parenthesized(cx, &arg)? {
        doc
    } else if has_leading_comment_in_parens(cx, &arg) && !cx.is_ignored(&arg) {
        shapes::enclosed(lit("("), cx.format(&arg)?, lit(")"))
    } else {
        cx.format(&arg)?
    };
    Ok(concat(vec![
        cx.token(op),
        if word { lit(" ") } else { Doc::Nil },
        arg_doc,
    ]))
}

/// The pairs of parentheses (outermost first) around the argument `arg` of a unary and the
/// content, when a comment is on the pairs or the content (Prettier's `hasComment(argument)`).
fn commented_pairs(
    cx: &FormatCtx<'_>,
    arg: &SyntaxNode,
) -> Option<(Vec<(SyntaxToken, SyntaxToken)>, SyntaxNode)> {
    let mut pairs = Vec::new();
    let mut n = arg.clone();
    while n.kind() == k::PARENTHESIZED_EXPRESSION {
        let mut toks = n.children_with_tokens().filter_map(|e| e.into_token());
        let open = toks.find(|t| t.kind() == k::L_PAREN)?;
        let close = toks.find(|t| t.kind() == k::R_PAREN)?;
        pairs.push((open, close));
        n = n.children().next()?;
    }
    // (the comments in front of the outermost `(` are not in the parentheses)
    let on_pairs = pairs.iter().enumerate().any(|(i, (o, c))| {
        (i > 0 && !core_format::comments::leading_comments(cx.grammar, o).is_empty())
            || !core_format::comments::trailing_comments(cx.grammar, o).is_empty()
            || super::ast::token_comments(cx, c)
    });
    (!pairs.is_empty() && (on_pairs || super::ast::has_comment(cx, &n))).then_some((pairs, n))
}

/// Does the unary `node` print its argument in parentheses of its own, around the comments in
/// them? A binary expression then lays out its operands without the break of its own.
pub(crate) fn has_commented_parentheses(cx: &FormatCtx<'_>, node: &SyntaxNode) -> bool {
    node.kind() == k::UNARY_EXPRESSION
        && node
            .last_child()
            .is_some_and(|arg| commented_pairs(cx, &arg).is_some() && !cx.is_ignored(&arg))
}

/// `!(` comments, the argument and `)` on lines of their own when they do not fit.
fn commented_parenthesized(
    cx: &FormatCtx<'_>,
    arg: &SyntaxNode,
) -> Result<Option<Doc>, FormatError> {
    use core_format::comments::{leading_comments, trailing_comments};
    use core_format::context::comment_doc;
    let Some((pairs, inner)) = commented_pairs(cx, arg).filter(|_| !cx.is_ignored(arg)) else {
        return Ok(None);
    };
    let mut parts = Vec::new();
    // (a comment on the line of the `(` stays there)
    let mut after_open = Vec::new();
    for (i, (open, _)) in pairs.iter().enumerate() {
        if i > 0 {
            for c in &leading_comments(cx.grammar, open) {
                parts.extend([comment_doc(c), hardline()]);
            }
        }
        after_open.extend(trailing_comments(cx.grammar, open));
    }
    let content = cx.format(&inner)?;
    // (the content keeps the parentheses it needs of its own)
    parts.push(
        if super::parens::needs_parens(cx, &inner) == super::parens::Need::Yes
            && !has(inner.kind(), "binary")
        {
            concat(vec![lit("("), content, lit(")")])
        } else {
            content
        },
    );
    for (_, close) in pairs.iter().rev() {
        for c in &leading_comments(cx.grammar, close) {
            parts.extend([hardline(), comment_doc(c)]);
        }
        parts.push(cx.trailing_comments_doc(close));
    }
    Ok(Some(group(concat(vec![
        lit("("),
        core_format::context::trailing_comments_doc(&cx.comment_style, &after_open),
        indent(concat(vec![softline(), concat(parts)])),
        softline(),
        lit(")"),
    ]))))
}

fn await_expression(node: &SyntaxNode, cx: &FormatCtx<'_>) -> R {
    let elems = kids(cx, node);
    let [kw, arg] = &elems[..] else {
        return cx.concat_children(node);
    };
    let parts = concat(vec![elem(cx, kw)?, lit(" "), elem(cx, arg)?]);
    let held = outer(node);
    let p = parent(node);
    let head = p.as_ref().is_some_and(|p| {
        p.first_child().as_ref() == Some(&held)
            && (matches!(p.kind(), k::CALL_EXPRESSION) || is_member(p.kind()))
    });
    if !head {
        return Ok(parts);
    }
    let indented = concat(vec![indent(concat(vec![softline(), parts])), softline()]);
    let enclosing = node
        .ancestors()
        .skip(1)
        .find(|a| matches!(a.kind(), k::AWAIT_EXPRESSION | k::BLOCK_STATEMENT));
    let leftmost_of_await = enclosing.as_ref().is_some_and(|a| {
        a.kind() == k::AWAIT_EXPRESSION
            && a.last_child()
                .is_some_and(|arg| starts_with(&arg, &|n| n == node))
    });
    Ok(if leftmost_of_await {
        indented
    } else {
        group(indented)
    })
}

fn yield_expression(node: &SyntaxNode, cx: &FormatCtx<'_>) -> R {
    let elems = kids(cx, node);
    let mut parts = Vec::new();
    for e in &elems {
        if node_of(e).is_some() {
            parts.push(lit(" "));
        }
        parts.push(elem(cx, e)?);
    }
    Ok(concat(parts))
}

fn sequence(node: &SyntaxNode, cx: &FormatCtx<'_>) -> R {
    let elems = kids(cx, node);
    let in_statement = parent(node)
        .is_some_and(|p| matches!(p.kind(), k::EXPRESSION_STATEMENT | k::FOR_STATEMENT));
    let mut items = Vec::new();
    let mut comma = None;
    for e in &elems {
        match e {
            e if token_of(e).is_some() => comma = Some(elem(cx, e)?),
            e => items.push((comma.take(), elem(cx, e)?)),
        }
    }
    let mut it = items.into_iter();
    let Some((_, head)) = it.next() else {
        return Err(shape_error("sequence"));
    };
    if in_statement {
        let mut parts = vec![head];
        for (comma, d) in it {
            parts.push(comma.unwrap_or_else(|| lit(",")));
            parts.push(indent(concat(vec![line(), d])));
        }
        return Ok(group(concat(parts)));
    }
    let sep = |c: Option<Doc>| concat(vec![c.unwrap_or_else(|| lit(",")), line()]);
    let mut parts = vec![head];
    for (comma, d) in it {
        parts.push(sep(comma));
        parts.push(d);
    }
    Ok(group(concat(parts)))
}

/// `return a;`, `throw b;`: a binary or sequence argument that does not fit goes in parentheses
/// on its own lines.
fn return_or_throw(node: &SyntaxNode, cx: &FormatCtx<'_>) -> R {
    let elems = kids(cx, node);
    let (lead, kw) = keyword(cx, &elems[0])?;
    let arg = elems.get(1).and_then(node_of);
    let Some(arg) = arg else {
        return Ok(concat(vec![lead, kw, semi(cx, node)]));
    };
    let arg_u = un(arg);
    let asi = super::util::detach_asi(cx, node);
    let printed = cx.format(arg)?;
    let body = if has_leading_own_line_comment(cx, arg) {
        concat(vec![
            lit(" ("),
            indent(concat(vec![hardline(), printed])),
            hardline(),
            lit(")"),
        ])
    } else {
        shapes::keyword_argument(cx, &arg_u, printed)
    };
    let end = super::util::reattach_asi(cx, asi);
    Ok(concat(vec![lead, kw, body, semi(cx, node), end]))
}
