//! Names and conditions that repeat: object keys, class members, `case` tests, `else if` chains.
//!
//! concept: no-duplicate-keys, no-duplicate-case, unique-names, new no-dupe-else-if
//! what: An else-if chain that repeats an earlier condition.

use super::{Check, code_text, has_own, operator, span, test_of, un};
use crate::generated::kind as k;
use core_rules::RuleContext;
use core_rules::names::static_name;
use core_syntax::{SyntaxKind, SyntaxNode, TextRange};
use std::collections::HashMap;

/// `get` and `set` accessors pair up; anything else (a value, a method, a field) conflicts with
/// every earlier use of the name.
fn members(
    c: &Check,
    n: &SyntaxNode,
    ctx: &mut RuleContext<'_>,
    kinds: &[SyntaxKind],
    class: bool,
) {
    // name -> (init, get, set) seen, first range
    let mut seen: HashMap<String, (u8, TextRange)> = HashMap::new();
    for m in n.children().filter(|m| kinds.contains(&m.kind())) {
        let Some(key) = m.children().next() else {
            continue;
        };
        // A method without a body is a TypeScript overload signature.
        if m.kind() == k::METHOD_DEFINITION
            && !(m.children().find(|f| f.kind() == k::FUNCTION))
                .is_some_and(|f| f.children().any(|b| b.kind() == k::FUNCTION_BODY))
        {
            continue;
        }
        let Some(name) = static_name(ctx.file.grammar, &key) else {
            continue;
        };
        let kind = if has_own(&m, "get") {
            2
        } else if has_own(&m, "set") {
            4
        } else {
            1
        };
        let key_node = key.clone();
        let key = if class && has_own(&m, "static") {
            format!("static {name}")
        } else {
            name.clone()
        };
        let range = span(ctx, &m);
        let entry = seen.entry(key).or_insert((0, range));
        let clash = if kind == 1 {
            entry.0 != 0
        } else {
            entry.0 & (kind | 1) != 0
        };
        let first = entry.1;
        entry.0 |= kind;
        if clash {
            // an object's accessor is reported at its key (ESLint), a class member at its start
            let at = if class { range } else { span(ctx, &key_node) };
            c.hit(ctx, at, &name).related(first, "first defined here");
        }
    }
}

pub(super) fn dupe_class_members(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    members(
        c,
        n,
        ctx,
        &[k::METHOD_DEFINITION, k::PROPERTY_DEFINITION],
        true,
    );
}

/// The operands of `node` joined by `op` (`||` or `&&`), parentheses looked through.
fn split(node: SyntaxNode, op: &str, out: &mut Vec<SyntaxNode>) {
    let node = un(&node);
    if node.kind() == k::LOGICAL_EXPRESSION && operator(&node) == op {
        let mut parts = node.children();
        if let (Some(l), Some(r)) = (parts.next(), parts.last()) {
            split(l, op, out);
            split(r, op, out);
            return;
        }
    }
    out.push(node);
}

/// `node` as OR of ANDs of source texts.
fn dnf(node: SyntaxNode) -> Vec<Vec<String>> {
    let mut ors = Vec::new();
    split(node, "||", &mut ors);
    ors.into_iter()
        .map(|o| {
            let mut ands = Vec::new();
            split(o, "&&", &mut ands);
            ands.iter().map(code_text).collect()
        })
        .collect()
}

/// A condition of an `else if` that an earlier one of the chain already covers.
pub(super) fn dupe_else_if(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let g = ctx.file.grammar;
    let Some(test) = test_of(n).map(|n| un(&n)) else {
        return;
    };
    let mut conditions = vec![test.clone()];
    if test.kind() == k::LOGICAL_EXPRESSION && operator(&test) == "&&" {
        split(test.clone(), "&&", &mut conditions);
    }
    let mut list: Vec<Vec<Vec<String>>> = conditions.into_iter().map(dnf).collect();
    let mut current = n.clone();
    while let Some(parent) = current.parent() {
        let after_else = std::iter::successors(current.prev_sibling_or_token(), |e| {
            e.prev_sibling_or_token()
        })
        .find(|e| !e.as_token().is_some_and(|t| g.is_trivia(t.kind())))
        .is_some_and(|e| e.as_token().is_some_and(|t| t.text() == "else"));
        if parent.kind() != k::IF_STATEMENT || !after_else {
            return;
        }
        current = parent;
        let Some(earlier) = test_of(&current) else {
            return;
        };
        let earlier = dnf(earlier);
        for ors in &mut list {
            ors.retain(|o| !earlier.iter().any(|e| e.iter().all(|a| o.contains(a))));
        }
        if list.iter().any(Vec::is_empty) {
            c.hit(ctx, span(ctx, &test), "");
            return;
        }
    }
}
