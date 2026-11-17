//! Checks of ruff's rules about declarations: class attributes, import aliases.

use super::Check;
use super::lints::{immutable_annotation, is_mutable};
use super::tree::{callee, inner, span, statements};
use crate::generated::kind as k;
use crate::semantic::{Cx, model};
use crate::text::code_text;
use core_rules::RuleContext;
use core_syntax::SyntaxNode;

/// The name a `ClassVar[...]` or `Final[...]` annotation is written with, through imports.
fn is_marker(cx: &Cx, a: &SyntaxNode) -> bool {
    let a = inner(a);
    let head = if a.kind() == k::EXPR_SUBSCRIPT {
        a.children().next().unwrap_or(a)
    } else {
        a
    };
    cx.qualified(&head).is_some_and(|q| {
        matches!(
            q.as_str(),
            "typing.ClassVar"
                | "typing.Final"
                | "typing_extensions.ClassVar"
                | "typing_extensions.Final"
        )
    })
}

fn is_special(name: &str) -> bool {
    matches!(name, "__slots__" | "__match_args__" | "_fields_")
}

/// Do the decorators or bases of a class give it fields that are copied per instance (a data
/// class, a Pydantic model, a named tuple)?
fn copies_defaults(cx: &Cx, class: &SyntaxNode) -> bool {
    let decorated = class
        .children()
        .filter(|d| d.kind() == k::DECORATOR)
        .any(|d| {
            d.children().next().is_some_and(|e| {
                let e = if e.kind() == k::EXPR_CALL {
                    callee(&e).unwrap_or(e)
                } else {
                    e
                };
                cx.qualified(&e).is_some_and(|q| {
                    q.starts_with("dataclasses.")
                        || q.starts_with("attr.")
                        || q.starts_with("attrs.")
                })
            })
        });
    let based = class
        .children()
        .find(|c| c.kind() == k::ARGUMENTS)
        .is_some_and(|a| {
            a.children().any(|b| {
                cx.qualified(&b).is_some_and(|q| {
                    q.starts_with("pydantic.")
                        || q.starts_with("msgspec.")
                        || q.starts_with("sqlmodel.")
                        || matches!(q.as_str(), "typing.NamedTuple" | "typing.TypedDict")
                })
            })
        });
    decorated || based
}

/// RUF012: a mutable class attribute that is not a `ClassVar`.
pub(super) fn mutable_class_default(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let Some(cx) = model(ctx.file) else { return };
    if copies_defaults(&cx, n) {
        return;
    }
    for s in statements(n) {
        let (value, special, annotation) = match s.kind() {
            k::STMT_ASSIGN => {
                let parts: Vec<SyntaxNode> = s.children().collect();
                let Some((value, targets)) = parts.split_last() else {
                    continue;
                };
                let special = targets
                    .iter()
                    .all(|t| t.kind() == k::EXPR_NAME && is_special(&code_text(t)));
                (value.clone(), special, None)
            }
            k::STMT_ANN_ASSIGN => {
                let (Some(target), Some(annotation), Some(value)) = (
                    cx.slot(&s, "left"),
                    cx.slot(&s, "annotation"),
                    cx.slot(&s, "right"),
                ) else {
                    continue;
                };
                (value, is_special(&code_text(&target)), Some(annotation))
            }
            _ => continue,
        };
        if special || !is_mutable(&cx, &value) {
            continue;
        }
        if annotation.is_some_and(|a| is_marker(&cx, &a) || immutable_annotation(&cx, &a)) {
            continue;
        }
        c.hit(ctx, span(ctx, &value), "");
    }
}

/// PLC0414: `import a as a`, `from m import a as a`.
pub(super) fn useless_import_alias(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let names: Vec<String> = n
        .children()
        .filter(|i| i.kind() == k::IDENTIFIER)
        .map(|i| code_text(&i))
        .collect();
    if let [name, alias] = &names[..]
        && name == alias
    {
        c.hit(ctx, span(ctx, n), name);
    }
}
