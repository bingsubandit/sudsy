//! Rules about how modules are imported that need to know what an imported name is used for.

use super::scope::{ref_at, run};
use super::{Check, code_text, span};
use crate::generated::kind as k;
use crate::semantic::{Cx, model};
use core_rules::RuleContext;
use core_semantics::scopes::Reference;
use core_syntax::SyntaxNode;

/// `no-require-imports`: `require(...)` of the global `require`, and `import a = require(...)`.
pub(super) fn require_imports(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let range = span(ctx, n);
    if n.kind() == k::CALL_EXPRESSION {
        let Some(callee) = n.children().next() else {
            return;
        };
        if code_text(&callee, ctx.file.grammar) != "require" {
            return;
        }
        // A `require` the file declares itself is not the module loader.
        let start = span(ctx, &callee).start();
        let declared = model(ctx.file)
            .is_some_and(|cx| !ref_at(&cx, start).is_some_and(|r| r.symbol.is_none()));
        if declared {
            return;
        }
    }
    c.hit(ctx, range, "");
}

/// Is the value reference `id` safe in a type-only import (`typeof a`, `a.b` in a type, a
/// computed key of a type literal)?
fn value_use_is_type(id: &SyntaxNode) -> bool {
    let mut child = id.clone();
    while let Some(parent) = child.parent() {
        match parent.kind() {
            k::TS_TYPE_QUERY => return true,
            k::TS_QUALIFIED_NAME | k::STATIC_MEMBER_EXPRESSION | k::COMPUTED_MEMBER_EXPRESSION => {}
            k::TS_PROPERTY_SIGNATURE => return parent.children().next().as_ref() == Some(&child),
            _ => return false,
        }
        child = parent;
    }
    false
}

fn only_type_use(r: &Reference) -> bool {
    if r.is_read() || r.is_write() {
        return value_use_is_type(&r.node);
    }
    r.is_type()
}

/// `1`, `1 and 2`, `1, 2 and 3` with each name in quotes.
fn word_list(names: &[String]) -> String {
    match names {
        [] => String::new(),
        [one] => format!("\"{one}\""),
        [rest @ .., last] => {
            let head: Vec<String> = rest.iter().map(|n| format!("\"{n}\"")).collect();
            format!("{} and \"{last}\"", head.join(", "))
        }
    }
}

/// The names of an import declaration's specifiers that are only used as types, and how many
/// specifiers are not.
fn type_only_names(cx: &Cx, d: &SyntaxNode) -> (Vec<String>, usize) {
    let (mut types, mut others) = (Vec::new(), 0);
    for s in d.children().filter(|s| {
        matches!(
            s.kind(),
            k::IMPORT_SPECIFIER | k::IMPORT_DEFAULT_SPECIFIER | k::IMPORT_NAMESPACE_SPECIFIER
        )
    }) {
        if s.kind() == k::IMPORT_SPECIFIER && cx.has_own(&s, "type") {
            continue;
        }
        let local = s.descendants().find(|n| n.kind() == k::BINDING_IDENTIFIER);
        let name = local
            .as_ref()
            .map(|l| code_text(l, cx.g))
            .unwrap_or_default();
        let sym = local.as_ref().and_then(|l| cx.symbol_of(l));
        let mut refs = sym.into_iter().flat_map(|sid| cx.s.refs_of(sid)).peekable();
        if !cx.jsx_pragma(&name) && refs.peek().is_some() && refs.all(only_type_use) {
            types.push(name);
        } else {
            others += 1;
        }
    }
    (types, others)
}

/// `consistent-type-imports`: an import only used as a type is written `import type`, and a type
/// is not imported with `import("a")`.
pub(super) fn type_imports(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    if n.kind() == k::TS_IMPORT_TYPE {
        c.hit(ctx, span(ctx, n), "");
        return;
    }
    let message = c.message.clone();
    run(ctx, |cx| {
        let mut hits = Vec::new();
        if !cx.ts {
            return hits;
        }
        for d in cx
            .root
            .descendants()
            .filter(|d| d.kind() == k::IMPORT_DECLARATION)
        {
            if cx.has_own(&d, "type") {
                continue;
            }
            let (types, others) = type_only_names(cx, &d);
            if types.is_empty() {
                continue;
            }
            let text = if others == 0 && cx.slot(&d, "with_clause").is_none() {
                "All imports in the declaration are only used as types. Use `import type`."
                    .to_string()
            } else if others == 0 {
                continue;
            } else {
                message.replace("{name}", &word_list(&types))
            };
            hits.push((cx.range(&d), text));
        }
        hits
    });
}
