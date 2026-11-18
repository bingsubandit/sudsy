//! Calls and `new`: the callee and the argument list. The layouts of the arguments (hugging a last
//! or first function or object argument, test calls, `require`, hooks) are core-format's
//! `shapes::call_arguments` over the rows of `format.rules`. Member chains are `chain.rs`.

use super::arrow::{self, Flags};
use super::ast::{Call, un};
use super::chain;
use super::util::{R, kids, shape_error, token_of};
use crate::generated::kind as k;
use core_format::calls::{CallRules, Effect};
use core_format::doc::{self, Doc, concat, group, lit};
use core_format::rules::LazyRules;
use core_format::shapes;
use core_format::{FormatCtx, FormatError, comments};
use core_syntax::{SyntaxElement, SyntaxNode, grammars};

pub(crate) fn format(node: &SyntaxNode, cx: &FormatCtx<'_>) -> Result<Option<Doc>, FormatError> {
    match node.kind() {
        k::CALL_EXPRESSION | k::NEW_EXPRESSION | k::IMPORT_EXPRESSION => {
            Ok(Some(cx.layout(node, 0, || call(node, cx))?))
        }
        _ => Ok(None),
    }
}

/// The rows of `format.rules`: test calls, `require`, hooks, simple arguments.
static ROWS: LazyRules = LazyRules::new(|| grammars::JS.get(), include_str!("../../format.rules"));

pub(crate) fn rules() -> &'static CallRules {
    ROWS.get().calls(grammars::JS.get())
}

fn call(node: &SyntaxNode, cx: &FormatCtx<'_>) -> R {
    let is_new = node.kind() == k::NEW_EXPRESSION;
    if node.kind() == k::IMPORT_EXPRESSION {
        return import(node, cx);
    }
    let Some(c) = Call::of(cx, node) else {
        return Ok(super::literal::verbatim(cx, node));
    };
    let callee_u = un(&c.callee);
    let args = &c.args;
    let uns: Vec<SyntaxNode> = args.iter().map(|a| un(&a.0)).collect();
    let found = rules().pattern(cx, node, Some(&c.callee), &uns);
    let effect = found.filter(|e| e.is_inline());
    if effect.is_none()
        && !is_new
        && super::ast::is_member(callee_u.kind())
        && !has_inner_comment(cx, &callee_u)
    {
        return chain::member_chain(cx, node);
    }
    let mut parts = Vec::new();
    if is_new {
        parts.push(elem_first(cx, node)?);
        parts.push(lit(" "));
    }
    parts.push(cx.format(&c.callee)?);
    if let Some(o) = &c.optional {
        // Block comments in front of `?.` stay with the callee.
        let before = comments::leading_comments(cx.grammar, o);
        if !before.is_empty()
            && before
                .iter()
                .all(|c| c.kind == comments::CommentKind::Block)
        {
            cx.hold_leading(o);
            for c in &before {
                parts.push(lit(" "));
                parts.push(core_format::context::comment_doc(c));
            }
        }
        parts.push(cx.token(o));
    }
    if let Some(t) = &c.type_args {
        parts.push(cx.format(t)?);
    }
    if let Some(effect) = effect {
        let printed = args
            .iter()
            .map(|(a, _)| match un(a).kind() {
                // The parameters of a test's function stay on the line.
                k::ARROW_FUNCTION_EXPRESSION | k::FUNCTION if effect == Effect::Test => {
                    Ok(arrow::print_fn(cx, &un(a), Flags::TEST)?.unwrap_or(Doc::Nil))
                }
                _ => cx.format(a),
            })
            .collect::<Result<Vec<_>, _>>()?;
        let (open, close) = parens(cx, &c)?;
        parts.push(open);
        parts.push(doc::join(&lit(", "), printed));
        parts.push(close);
        return Ok(concat(parts));
    }
    parts.push(arguments_inner(cx, node, &c, &uns, found, false)?);
    let contents = concat(parts);
    Ok(if callee_u.kind() == k::CALL_EXPRESSION {
        group(contents)
    } else {
        contents
    })
}

/// A comment after `n`; the ones in front of its first token belong to the whole expression.
fn has_inner_comment(cx: &FormatCtx<'_>, n: &SyntaxNode) -> bool {
    super::ast::has_trailing_comment(cx, n)
}

fn elem_first(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    match kids(cx, node).first().and_then(token_of) {
        Some(t) => Ok(cx.token(t)),
        None => Err(shape_error("new")),
    }
}

fn parens(cx: &FormatCtx<'_>, c: &Call) -> Result<(Doc, Doc), FormatError> {
    match (&c.open, &c.close) {
        (Some(o), Some(cl)) => Ok((cx.token(o), cx.token(cl))),
        _ => Ok((lit("("), lit(")"))),
    }
}

/// `import(source, options)`.
fn import(node: &SyntaxNode, cx: &FormatCtx<'_>) -> R {
    let elems = kids(cx, node);
    let open = elems
        .iter()
        .position(|e| token_of(e).is_some_and(|t| t.kind() == k::L_PAREN));
    let Some(open) = open else {
        return Ok(super::literal::verbatim(cx, node));
    };
    let mut head = Vec::new();
    for e in &elems[..open] {
        head.push(super::util::elem(cx, e)?);
    }
    let args: Vec<(SyntaxNode, Option<core_syntax::SyntaxToken>)> = {
        let mut v: Vec<(SyntaxNode, Option<core_syntax::SyntaxToken>)> = Vec::new();
        for e in &elems[open + 1..] {
            match e {
                SyntaxElement::Node(n) => v.push((n.clone(), None)),
                SyntaxElement::Token(t) if t.kind() == k::COMMA => {
                    if let Some(l) = v.last_mut() {
                        l.1 = Some(t.clone());
                    }
                }
                SyntaxElement::Token(_) => {}
            }
        }
        v
    };
    let c = Call {
        callee: node.clone(),
        type_args: None,
        optional: None,
        open: elems[open].as_token().cloned(),
        args,
        close: elems.last().and_then(token_of).cloned(),
    };
    // A lone string stays on the line however long it is.
    if let [(arg, None)] = &c.args[..]
        && un(arg).kind() == k::STRING_LITERAL
        && !super::ast::has_leading_comment(cx, arg)
        && let (Some(open), Some(close)) = (&c.open, &c.close)
    {
        return Ok(concat(vec![
            concat(head),
            cx.token(open),
            cx.format(arg)?,
            cx.token(close),
        ]));
    }
    let uns: Vec<SyntaxNode> = c.args.iter().map(|a| un(&a.0)).collect();
    Ok(concat(vec![
        concat(head),
        group(arguments_inner(cx, node, &c, &uns, None, true)?),
    ]))
}

pub(crate) fn arguments(cx: &FormatCtx<'_>, node: &SyntaxNode, c: &Call) -> R {
    let uns: Vec<SyntaxNode> = c.args.iter().map(|a| un(&a.0)).collect();
    let effect = rules().pattern(cx, node, Some(&c.callee), &uns);
    arguments_inner(cx, node, c, &uns, effect, false)
}

/// The parenthesized argument list (Prettier's `printCallArguments`): the shared shape over the
/// arguments, with the hugged one printed in the layout it asks for.
fn arguments_inner(
    cx: &FormatCtx<'_>,
    node: &SyntaxNode,
    c: &Call,
    uns: &[SyntaxNode],
    effect: Option<Effect>,
    dynamic_import: bool,
) -> R {
    let dangling = c
        .close
        .as_ref()
        .filter(|_| !c.args.is_empty())
        .map_or(Doc::Nil, |t| super::util::dangling(cx, t));
    let (open, close) = parens(cx, c)?;
    let args = &c.args;
    if args.is_empty() {
        return Ok(concat(vec![open, close]));
    }
    let rules = rules();
    if effect == Some(Effect::Hook) {
        return Ok(concat(vec![
            open,
            cx.format(&args[0].0)?,
            lit(", "),
            cx.format(&args[1].0)?,
            close,
        ]));
    }
    let g = cx.grammar;
    let mut items = Vec::with_capacity(args.len());
    for (i, (a, comma)) in args.iter().enumerate() {
        let last = i + 1 == args.len();
        items.push(shapes::Item {
            doc: cx.format(a)?,
            sep: match comma {
                Some(t) if !last => cx.token(t),
                _ if !last => lit(","),
                _ => Doc::Nil,
            },
            blank_after: !last
                && cx
                    .first_token(&args[i + 1].0)
                    .is_some_and(|t| comments::blank_lines_before(g, &t) > 0),
        });
    }
    // The trailing comma of a broken list; a comma the source has and a comment sits at stays.
    let trailing = match args.last().and_then(|a| a.1.as_ref()) {
        Some(t) if super::ast::token_comments(cx, t) => cx.token(t),
        _ if dynamic_import => Doc::Nil,
        _ => super::util::trailing_comma(cx, true),
    };
    let list = shapes::Args::new(open, close, items, trailing, dangling);
    let plan = shapes::plan(cx, rules, node, uns, &super::util::style(cx).call);
    let expanded = match list.hug_target(&plan) {
        Some((at, first)) => {
            let flag = if first {
                Flags::EXPAND_FIRST
            } else {
                Flags::EXPAND_LAST
            };
            match expanded_arg(cx, &args[at].0, flag)? {
                Some(d) => shapes::Expanded::Doc(d),
                None => shapes::Expanded::Bailout,
            }
        }
        None => shapes::Expanded::Same,
    };
    Ok(list.layout(&plan, expanded, shapes::is_long_curried(cx, node)))
}

/// The argument printed for hugging; `None` when its parameters cannot be kept on one line.
fn expanded_arg(
    cx: &FormatCtx<'_>,
    arg: &SyntaxNode,
    flags: u32,
) -> Result<Option<Doc>, FormatError> {
    let a = un(arg);
    match a.kind() {
        k::ARROW_FUNCTION_EXPRESSION | k::FUNCTION => arrow::print_fn(cx, &a, flags),
        _ => Ok(Some(cx.format(arg)?)),
    }
}
