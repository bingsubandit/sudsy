//! Rules that read what names bind to (docs/formatting/python.md#scope-layer) and are no row of
//! the shared scope family: the model of `crate::semantic`, built on the first of them that runs
//! on a file.

use super::Check;
use crate::generated::kind as k;
use crate::semantic::model;
use core_rules::RuleContext;
use core_semantics::scopes::ScopeId;
use core_syntax::{SyntaxNode, TextRange};

/// Runs `f` on the file's model (nothing for a file with syntax errors) and reports what it
/// returns, in source order.
pub(super) fn run(
    c: &Check,
    ctx: &mut RuleContext<'_>,
    f: impl FnOnce(&crate::semantic::Cx) -> Vec<(TextRange, String)>,
) {
    let Some(cx) = model(ctx.file) else { return };
    let mut hits = f(&cx);
    hits.sort_by_key(|(r, _)| r.start());
    hits.dedup();
    for (range, name) in hits {
        c.hit(ctx, range, &name);
    }
}

pub(super) fn forward_annotation_syntax_error(
    c: &Check,
    _: &SyntaxNode,
    ctx: &mut RuleContext<'_>,
) {
    run(c, ctx, crate::forward::syntax_errors);
}

/// F403 and F406: `from m import *`, and the ones that are not at module level.
fn star_imports(cx: &crate::semantic::Cx, nested_only: bool) -> Vec<(TextRange, String)> {
    let mut out = Vec::new();
    for id in cx.s.symbol_ids() {
        let sym = cx.s.symbol(id);
        if &*sym.name != "*" || (nested_only && sym.scope == ScopeId(0)) {
            continue;
        }
        for d in &sym.decls {
            let alias = d.node(&cx.root);
            let Some(stmt) = alias.parent() else { continue };
            let module = cx
                .slot(&stmt, "module")
                .map(|m| crate::text::code_text(&m))
                .unwrap_or_default();
            let level: usize = stmt
                .children_with_tokens()
                .filter_map(|e| e.into_token())
                .map(|t| match t.kind() {
                    k::DOT => 1,
                    k::ELLIPSIS => 3,
                    _ => 0,
                })
                .sum();
            out.push((cx.range(&stmt), format!("{}{module}", ".".repeat(level))));
        }
    }
    out
}

pub(super) fn nested_import_star(c: &Check, _: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    run(c, ctx, |cx| star_imports(cx, true));
}
