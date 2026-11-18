//! Rules that read what names bind to (docs/formatting/js.md): the model of
//! `crate::semantic`, built on the first of them that runs on a file. Positions follow ESLint's
//! reports, so the parity runners compare them as they are.

use super::{Check, code_text};
use crate::generated::kind as k;
use crate::semantic::{Cx, model};
use core_rules::RuleContext;
use core_semantics::scopes::{Flags, Reference, Symbol};
use core_syntax::{SyntaxNode, TextRange, TextSize};

pub(super) type Hit = (TextRange, String);

/// Runs `f` on the file's model (nothing for a file that does not parse) and reports what it
/// returns, in source order.
pub(super) fn run(ctx: &mut RuleContext<'_>, f: impl FnOnce(&Cx) -> Vec<Hit>) {
    let Some(cx) = model(ctx.file) else { return };
    let mut hits = f(&cx);
    hits.sort_by_key(|(r, _)| r.start());
    hits.dedup();
    for (range, message) in hits {
        ctx.report(range, message);
    }
}

/// The reference whose name starts at `start` (references are in source order).
pub(super) fn ref_at(cx: &Cx, start: TextSize) -> Option<&Reference> {
    let refs = &cx.s.refs;
    let i = refs
        .binary_search_by_key(&start, |r| r.range.start())
        .ok()?;
    refs.get(i)
}

/// The write references of the symbols `want` selects, reported at the identifier.
fn writes(c: &Check, ctx: &mut RuleContext<'_>, want: impl Fn(&Cx, &Symbol) -> bool) {
    run(ctx, |cx| {
        let mut hits = Vec::new();
        for id in cx.s.symbol_ids() {
            let sym = cx.s.symbol(id);
            if !want(cx, sym) {
                continue;
            }
            for r in cx.s.refs_of(id).filter(|r| r.is_write()) {
                hits.push((r.range, c.message.replace("{name}", &sym.name)));
            }
        }
        hits
    });
}

/// `no-const-assign`
pub(super) fn const_assign(c: &Check, _n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    writes(c, ctx, |_, s| s.flags.contains(Flags::CONST));
}

/// `no-class-assign`
pub(super) fn class_assign(c: &Check, _n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    writes(c, ctx, |_, s| s.decls[0].node_kind == k::CLASS);
}

/// `no-func-assign`
pub(super) fn func_assign(c: &Check, _n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    writes(c, ctx, |_, s| s.decls[0].node_kind == k::FUNCTION);
}

/// The start of the assignment, update, `delete`, call or `for` loop an identifier is part of.
fn write_node_start(cx: &Cx, id: &SyntaxNode, fallback: TextSize) -> TextSize {
    (id.ancestors().skip(1))
        .find(|a| {
            matches!(
                a.kind(),
                k::ASSIGNMENT_EXPRESSION
                    | k::UPDATE_EXPRESSION
                    | k::UNARY_EXPRESSION
                    | k::CALL_EXPRESSION
                    | k::FOR_IN_STATEMENT
                    | k::FOR_OF_STATEMENT
            )
        })
        .map_or(fallback, |a| cx.range(&a).start())
}

/// Is `id` (a read of a namespace import) the object of a member that is written to, or the first
/// argument of `Object.assign` and the like?
fn member_written(cx: &Cx, id: &SyntaxNode) -> bool {
    let Some(parent) = id.parent() else {
        return false;
    };
    match parent.kind() {
        k::STATIC_MEMBER_EXPRESSION | k::COMPUTED_MEMBER_EXPRESSION
            if cx.slot(&parent, "object").as_ref() == Some(id) => {}
        k::CALL_EXPRESSION => {
            let first = cx.slot(&parent, "arguments").as_ref() == Some(id);
            let Some(callee) = cx.slot(&parent, "callee") else {
                return false;
            };
            if callee.kind() != k::STATIC_MEMBER_EXPRESSION {
                return false;
            }
            let (Some(object), Some(member)) =
                (cx.slot(&callee, "object"), cx.slot(&callee, "member"))
            else {
                return false;
            };
            if object.kind() != k::IDENTIFIER_REFERENCE {
                return false;
            }
            let methods: &[&str] = match code_text(&object).as_str() {
                "Object" => &[
                    "assign",
                    "defineProperty",
                    "defineProperties",
                    "freeze",
                    "setPrototypeOf",
                ],
                "Reflect" => &["defineProperty", "deleteProperty", "set", "setPrototypeOf"],
                _ => return false,
            };
            let global = ref_at(cx, cx.range(&object).start()).is_some_and(|r| r.symbol.is_none());
            return first && methods.contains(&code_text(&member).as_str()) && global;
        }
        _ => return false,
    }
    let Some(grand) = parent.parent() else {
        return false;
    };
    match grand.kind() {
        k::ASSIGNMENT_EXPRESSION | k::FOR_IN_STATEMENT | k::FOR_OF_STATEMENT => {
            cx.slot(&grand, "left").as_ref() == Some(&parent)
        }
        k::UPDATE_EXPRESSION => true,
        k::UNARY_EXPRESSION => cx.has_own(&grand, "delete"),
        k::ARRAY_ASSIGNMENT_TARGET
        | k::OBJECT_ASSIGNMENT_TARGET
        | k::ASSIGNMENT_TARGET_REST
        | k::ASSIGNMENT_TARGET_WITH_DEFAULT
        | k::ASSIGNMENT_TARGET_PROPERTY_PROPERTY => true,
        _ => false,
    }
}

/// `no-import-assign`
pub(super) fn import_assign(c: &Check, _n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    run(ctx, |cx| {
        let mut hits = Vec::new();
        for id in cx.s.symbol_ids() {
            let sym = cx.s.symbol(id);
            if !sym.flags.contains(Flags::IMPORT) {
                continue;
            }
            // An imported type next to a local value of its name: the writes are the value's.
            if cx.ts && sym.decls.iter().any(|d| !d.flags.contains(Flags::IMPORT)) {
                continue;
            }
            let namespace = sym.decls[0].node(&cx.root).kind() == k::IMPORT_NAMESPACE_SPECIFIER;
            for r in cx.s.refs_of(id) {
                let start = write_node_start(cx, &r.node(&cx.root), r.range.start());
                let at = TextRange::at(start, 1.into());
                if r.is_write() {
                    hits.push((at, c.message.replace("{name}", &sym.name)));
                } else if namespace && member_written(cx, &r.node(&cx.root)) {
                    hits.push((at, format!("The members of '{}' are read-only.", sym.name)));
                }
            }
        }
        hits
    });
}
