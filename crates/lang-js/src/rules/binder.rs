//! Rules over the binder's references: `prefer-const`, `prefer-rest-params` and the global types
//! `Function` and the wrapper objects.

use super::Check;
use super::scope::{Hit, run};
use crate::generated::kind as k;
use crate::semantic::Cx;
use core_rules::RuleContext;
use core_semantics::scopes::{Flags, Reference, Symbol};
use core_syntax::{SyntaxNode, TextRange};

/// The assignment, or declarator, an identifier is the target of, through the patterns around it.
fn host_of(id: &SyntaxNode) -> Option<SyntaxNode> {
    id.ancestors().skip(1).find(|a| {
        !matches!(
            a.kind(),
            k::ARRAY_ASSIGNMENT_TARGET
                | k::OBJECT_ASSIGNMENT_TARGET
                | k::ASSIGNMENT_TARGET_REST
                | k::ASSIGNMENT_TARGET_WITH_DEFAULT
                | k::ASSIGNMENT_TARGET_PROPERTY_IDENTIFIER
                | k::ASSIGNMENT_TARGET_PROPERTY_PROPERTY
        )
    })
}

/// Can the assignment that `id` is part of become a declaration: an expression statement of a
/// block, a program or a `case`?
fn can_declare(id: &SyntaxNode) -> bool {
    let Some(assign) = host_of(id).filter(|h| h.kind() == k::ASSIGNMENT_EXPRESSION) else {
        return false;
    };
    let mut up = assign
        .ancestors()
        .skip(1)
        .skip_while(|a| a.kind() == k::PARENTHESIZED_EXPRESSION);
    up.next()
        .is_some_and(|s| s.kind() == k::EXPRESSION_STATEMENT)
        && up.next().is_some_and(|h| {
            matches!(
                h.kind(),
                k::PROGRAM
                    | k::BLOCK_STATEMENT
                    | k::FUNCTION_BODY
                    | k::STATIC_BLOCK
                    | k::SWITCH_CASE
            )
        })
}

/// Does the destructuring assignment `left` assign to a member, or (one level deep) to a name
/// from outside the scope of `sym` or to a parameter?
fn blocked_destructuring(cx: &Cx, left: &SyntaxNode, sym: &Symbol) -> bool {
    let members = left.descendants().any(|d| {
        matches!(
            d.kind(),
            k::STATIC_MEMBER_EXPRESSION | k::COMPUTED_MEMBER_EXPRESSION
        )
    });
    let outer = left
        .children()
        .filter_map(|c| match c.kind() {
            k::IDENTIFIER_REFERENCE => Some(c),
            k::ASSIGNMENT_TARGET_PROPERTY_IDENTIFIER | k::ASSIGNMENT_TARGET_PROPERTY_PROPERTY => {
                c.children().find(|n| n.kind() == k::IDENTIFIER_REFERENCE)
            }
            _ => None,
        })
        .filter_map(|id| super::scope::ref_at(cx, cx.range(&id).start()))
        .any(|r| {
            r.symbol.is_some_and(|s| {
                let target = cx.s.symbol(s);
                target.scope != sym.scope || target.flags.contains(Flags::PARAMETER)
            })
        });
    members || outer
}

/// What `prefer-const` finds for a `let` symbol: where it would be declared instead.
fn const_site(cx: &Cx, id: core_semantics::scopes::SymbolId) -> Option<TextRange> {
    let sym = cx.s.symbol(id);
    let decl = sym.decls.iter().find(|d| d.flags.contains(Flags::LET))?;
    if decl.node.kind() != k::VARIABLE_DECLARATOR {
        return None;
    }
    let declaration = decl.node.parent()?;
    let loop_kind = declaration.parent().map(|p| p.kind());
    if loop_kind == Some(k::FOR_STATEMENT)
        && cx.slot(&declaration.parent()?, "declaration").as_ref() == Some(&declaration)
    {
        return None;
    }
    let init = cx.slot(&decl.node, "init").is_some()
        || matches!(loop_kind, Some(k::FOR_IN_STATEMENT | k::FOR_OF_STATEMENT));
    // The writes in source order; the initializer is the one at the name.
    let mut writer: Option<TextRange> = init.then_some(decl.name);
    let mut read_first = false;
    let mut by_ref = false;
    for r in cx.s.refs_of(id) {
        if r.is_write() {
            if writer.is_some() {
                return None;
            }
            if let Some(host) = host_of(&r.node).filter(|h| h.kind() == k::ASSIGNMENT_EXPRESSION)
                && let Some(left) = cx.slot(&host, "left")
                && matches!(
                    left.kind(),
                    k::ARRAY_ASSIGNMENT_TARGET | k::OBJECT_ASSIGNMENT_TARGET
                )
                && blocked_destructuring(cx, &left, sym)
            {
                return None;
            }
            by_ref = r.scope == sym.scope && can_declare(&r.node);
            writer = Some(r.range);
        } else if r.is_read() && writer.is_none() {
            read_first = true;
        }
    }
    let at = writer?;
    if !init && !by_ref {
        return None;
    }
    Some(if read_first { decl.name } else { at })
}

/// `prefer-const`: a `let` that is written once, where it could be declared.
pub(super) fn prefer_const(c: &Check, _n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    run(ctx, |cx| {
        cx.s.symbol_ids()
            .filter_map(|id| {
                let at = const_site(cx, id)?;
                Some((at, c.message.replace("{name}", &cx.s.symbol(id).name)))
            })
            .collect()
    });
}

/// `prefer-rest-params`: `arguments` in an ordinary function, other than `arguments.name`.
pub(super) fn prefer_rest_params(c: &Check, _n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    run(ctx, |cx| {
        cx.s.unresolved()
            .filter(|r| &*r.name == "arguments" && !r.is_type())
            .filter(|r| {
                cx.s.ancestors(r.scope)
                    .any(|s| cx.s.scope(s).node.kind() == k::FUNCTION)
            })
            .filter(|r| {
                !r.node.parent().is_some_and(|p| {
                    p.kind() == k::STATIC_MEMBER_EXPRESSION
                        && cx.slot(&p, "object").as_ref() == Some(&r.node)
                })
            })
            .map(|r| (r.range, c.message.clone()))
            .collect()
    });
}

/// The global types a name in a type position stands for (the type is the global one when no
/// scope declares the name).
fn global_types<'a>(cx: &'a Cx, names: &'a [&str]) -> impl Iterator<Item = &'a Reference> {
    cx.s.unresolved().filter(move |r| {
        names.contains(&&*r.name)
            && r.is_type()
            && !r.is_read()
            && r.node.parent().is_some_and(|p| {
                matches!(
                    p.kind(),
                    k::TS_TYPE_REFERENCE | k::TS_CLASS_IMPLEMENTS | k::TS_INTERFACE_HERITAGE
                )
            })
    })
}

/// `no-unsafe-function-type`
pub(super) fn unsafe_function(c: &Check, _n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    run(ctx, |cx| {
        global_types(cx, &["Function"])
            .map(|r| (r.range, c.message.clone()))
            .collect()
    });
}

/// `no-wrapper-object-types`
pub(super) fn wrapper_types(c: &Check, _n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    const NAMES: &[&str] = &["BigInt", "Boolean", "Number", "Object", "String", "Symbol"];
    run(ctx, |cx| -> Vec<Hit> {
        global_types(cx, NAMES)
            .map(|r| {
                let primitive = r.name.to_lowercase();
                (
                    r.range,
                    c.message
                        .replace("{name}", &r.name)
                        .replace("{primitive}", &primitive),
                )
            })
            .collect()
    });
}
