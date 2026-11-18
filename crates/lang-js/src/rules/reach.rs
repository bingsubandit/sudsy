//! `getter-return`: asks the flow graph (`core_flow::build`, from the statement roles) whether a
//! getter's end can be reached.
//!
//! concept: new getter-return
//! what: A getter whose end can be reached without a return.

use super::{Check, has_own, span};
use crate::generated::kind as k;
use core_flow::build;
use core_rules::RuleContext;
use core_rules::names::static_name;
use core_syntax::{SyntaxNode, TextRange};

/// `return` statements of a function body, nested functions left out.
fn returns(n: &SyntaxNode, out: &mut Vec<SyntaxNode>) {
    for c in n.children() {
        if c.kind() == k::RETURN_STATEMENT {
            out.push(c.clone());
        }
        if !matches!(
            c.kind(),
            k::FUNCTION | k::ARROW_FUNCTION_EXPRESSION | k::CLASS
        ) {
            returns(&c, out);
        }
    }
}

/// A getter must return a value on every path.
pub(super) fn getter_return(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let g = ctx.file.grammar;
    if !has_own(n, "get") {
        return;
    }
    let Some(function) = n.children().find(|f| f.kind() == k::FUNCTION) else {
        return;
    };
    let Some(body) = function.children().find(|b| b.kind() == k::FUNCTION_BODY) else {
        return;
    };
    let key = n
        .children()
        .next()
        .and_then(|key| static_name(ctx.file.grammar, &key));
    let name = key.map_or_else(|| "getter".to_string(), |key| format!("getter '{key}'"));
    let mut found = Vec::new();
    returns(&body, &mut found);
    for r in found.iter().filter(|r| r.children().next().is_none()) {
        c.hit(ctx, span(ctx, r), &name);
    }
    if let Some(flow) = build(g, ctx.file.text, &function)
        && flow.reach().block[flow.end]
    {
        let start = span(ctx, n).start();
        let end = n
            .children()
            .next()
            .map_or(start, |key| span(ctx, &key).end());
        c.hit(ctx, TextRange::new(start, end), &name);
    }
}
