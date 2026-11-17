//! Checks of pyupgrade's rules about type annotations and the typing module: the spelling that the
//! target version replaces (`Optional[X]`, `List[X]`, `TypeAlias`, `Generic[T]`, quoted names).

use super::Check;
use super::lints::has_token;
use super::tree::{callee, inner, span};
use crate::generated::kind as k;
use crate::semantic::{Cx, model};
use crate::text::code_text;
use core_rules::RuleContext;
use core_syntax::SyntaxNode;

/// The object and the subscript of `object[...]`.
fn subscript(n: &SyntaxNode) -> Option<(SyntaxNode, SyntaxNode)> {
    let mut parts = n.children();
    Some((parts.next()?, parts.next()?))
}

fn typing_name(cx: &Cx, e: &SyntaxNode) -> Option<String> {
    let q = cx.qualified(e)?;
    q.strip_prefix("typing.")
        .or_else(|| q.strip_prefix("typing_extensions."))
        .map(str::to_owned)
}

/// UP007 and UP045: `Union[X, Y]` and `Optional[X]` where `X | Y` is allowed.
fn union_or_optional(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>, name: &str) {
    let Some(cx) = model(ctx.file) else { return };
    if cx.minor < 10 {
        return;
    }
    // A forward reference in a value (`X = Union["Y", int]`) cannot be written with `|`.
    let value = n
        .ancestors()
        .skip(1)
        .find(|a| {
            matches!(
                a.kind(),
                k::STMT_ASSIGN | k::STMT_ANN_ASSIGN | k::STMT_FUNCTION_DEF | k::STMT_CLASS_DEF
            )
        })
        .is_some_and(|a| a.kind() == k::STMT_ASSIGN);
    if subscript(n).is_some_and(|(o, s)| {
        typing_name(&cx, &o).as_deref() == Some(name)
            && !(value
                && std::iter::once(s.clone())
                    .chain(s.children())
                    .any(|e| e.kind() == k::EXPR_STRING_LITERAL))
    }) {
        c.hit(ctx, span(ctx, n), "");
    }
}

pub(super) fn union(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    union_or_optional(c, n, ctx, "Union");
}

pub(super) fn optional(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    union_or_optional(c, n, ctx, "Optional");
}

/// UP006: `List[X]`, `Dict[K, V]`, `Tuple[X]`, ... where the builtin is generic.
pub(super) fn deprecated_alias(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let Some(cx) = model(ctx.file) else { return };
    let Some((object, _)) = subscript(n) else {
        return;
    };
    let Some(name) = typing_name(&cx, &object) else {
        return;
    };
    let builtin = match name.as_str() {
        "List" => "list",
        "Dict" => "dict",
        "Set" => "set",
        "FrozenSet" => "frozenset",
        "Tuple" => "tuple",
        "Type" => "type",
        _ => return,
    };
    if cx.minor >= 9 {
        c.hit(ctx, span(ctx, &object), builtin);
    }
}

/// UP043: `Generator[X, None, None]`, `AsyncGenerator[X, None]`.
pub(super) fn default_type_arguments(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let Some(cx) = model(ctx.file) else { return };
    let Some((object, slice)) = subscript(n) else {
        return;
    };
    let Some(q) = cx.qualified(&object) else {
        return;
    };
    let defaults = match q.as_str() {
        "typing.Generator" | "collections.abc.Generator" => 2,
        "typing.AsyncGenerator" | "collections.abc.AsyncGenerator" => 1,
        _ => return,
    };
    let slice = inner(&slice);
    let items: Vec<SyntaxNode> = slice.children().collect();
    if cx.minor >= 13
        && slice.kind() == k::EXPR_TUPLE
        && items.len() == defaults + 1
        && items
            .last()
            .is_some_and(|e| inner(e).kind() == k::EXPR_NONE_LITERAL)
    {
        c.hit(ctx, span(ctx, &slice), "");
    }
}

/// The quoted names of an annotation: strings that are not arguments of `Literal` or `Annotated`.
fn quoted(cx: &Cx, e: &SyntaxNode, out: &mut Vec<SyntaxNode>) {
    let e = inner(e);
    match e.kind() {
        k::EXPR_STRING_LITERAL => out.push(e),
        k::EXPR_SUBSCRIPT => {
            if let Some((object, slice)) = subscript(&e) {
                match typing_name(cx, &object).as_deref() {
                    Some("Literal") => {}
                    Some("Annotated") => {
                        if let Some(first) = inner(&slice).children().next() {
                            quoted(cx, &first, out);
                        }
                    }
                    _ => {
                        quoted(cx, &object, out);
                        quoted(cx, &slice, out);
                    }
                }
            }
        }
        k::EXPR_TUPLE | k::EXPR_LIST | k::EXPR_BIN_OP => {
            e.children().for_each(|c| quoted(cx, &c, out));
        }
        _ => {}
    }
}

/// UP037: a quoted annotation, where annotations are not evaluated when the function runs.
pub(super) fn quoted_annotation(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let Some(cx) = model(ctx.file) else { return };
    if cx.minor < 14 {
        return;
    }
    let slot = |label| cx.slot(n, label);
    let annotations = match n.kind() {
        k::STMT_FUNCTION_DEF => vec![slot("returns")],
        _ => vec![slot("annotation")],
    };
    let mut found = Vec::new();
    for a in annotations.into_iter().flatten() {
        quoted(&cx, &a, &mut found);
    }
    for s in found {
        c.hit(ctx, span(ctx, &s), "");
    }
}

/// UP040: `X: TypeAlias = ...` where the `type` statement exists.
pub(super) fn type_alias_annotation(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let Some(cx) = model(ctx.file) else { return };
    let (Some(target), Some(annotation)) = (cx.slot(n, "left"), cx.slot(n, "annotation")) else {
        return;
    };
    if cx.minor >= 12
        && cx.slot(n, "right").is_some()
        && target.kind() == k::EXPR_NAME
        && typing_name(&cx, &annotation).as_deref() == Some("TypeAlias")
    {
        c.hit(ctx, span(ctx, n), &code_text(&target));
    }
}

/// UP046: a class that lists `Generic[...]` among its bases.
pub(super) fn generic_class(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let Some(cx) = model(ctx.file) else { return };
    let Some(bases) = n.children().find(|a| a.kind() == k::ARGUMENTS) else {
        return;
    };
    let generic = bases.children().any(|b| {
        b.kind() == k::EXPR_SUBSCRIPT
            && subscript(&b)
                .is_some_and(|(o, _)| typing_name(&cx, &o).as_deref() == Some("Generic"))
    });
    if cx.minor >= 12 && generic {
        let name = n
            .children()
            .find(|i| i.kind() == k::IDENTIFIER)
            .map(|i| code_text(&i))
            .unwrap_or_default();
        c.hit(ctx, span(ctx, n), &name);
    }
}

/// Is the name a module-level `TypeVar`, `ParamSpec` or `TypeVarTuple`?
pub(super) fn is_type_variable(cx: &Cx, name: &SyntaxNode) -> bool {
    if cx.qualified(name).as_deref() == Some("typing.AnyStr") {
        return true;
    }
    let Some(sym) = cx.ref_at(cx.range(name).start()).and_then(|r| r.symbol) else {
        return false;
    };
    let Some(decl) = cx.s.symbol(sym).decls.last() else {
        return false;
    };
    let node = decl.node(&cx.root);
    let Some(assign) = node.ancestors().find(|a| a.kind() == k::STMT_ASSIGN) else {
        return false;
    };
    assign
        .children()
        .last()
        .filter(|v| v.kind() == k::EXPR_CALL)
        .and_then(|call| callee(&call))
        .and_then(|f| typing_name(cx, &f))
        .is_some_and(|q| matches!(q.as_str(), "TypeVar" | "ParamSpec" | "TypeVarTuple"))
}

/// UP047: a function whose signature uses a module-level type variable.
pub(super) fn generic_function(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let Some(cx) = model(ctx.file) else { return };
    if cx.minor < 12
        || n.ancestors()
            .skip(1)
            .any(|a| matches!(a.kind(), k::STMT_CLASS_DEF | k::STMT_FUNCTION_DEF))
    {
        return;
    }
    let signature = n
        .children()
        .filter(|p| p.kind() == k::PARAMETERS)
        .chain(cx.slot(n, "returns"));
    let uses = signature
        .flat_map(|s| s.descendants().collect::<Vec<_>>())
        .any(|d| match d.kind() {
            k::EXPR_NAME => is_type_variable(&cx, &d),
            k::EXPR_ATTRIBUTE => cx.qualified(&d).as_deref() == Some("typing.AnyStr"),
            _ => false,
        });
    if let Some(id) = n
        .children()
        .find(|i| i.kind() == k::IDENTIFIER)
        .filter(|_| uses)
    {
        c.hit(ctx, span(ctx, &id), &code_text(&id));
    }
}

/// UP017: `datetime.timezone.utc`.
pub(super) fn datetime_utc(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    if n.children()
        .nth(1)
        .and_then(|m| m.first_token())
        .is_none_or(|t| t.text() != "utc")
    {
        return;
    }
    let Some(cx) = model(ctx.file) else { return };
    if cx.minor >= 11 && cx.qualified(n).as_deref() == Some("datetime.timezone.utc") {
        c.hit(ctx, span(ctx, n), "");
    }
}

/// (name, first minor version that has it) of `typing_extensions` members that moved to `typing`.
const TYPING_EXTENSIONS: &[(&str, u32)] = &[
    ("Annotated", 9),
    ("Concatenate", 10),
    ("ParamSpec", 10),
    ("ParamSpecArgs", 10),
    ("ParamSpecKwargs", 10),
    ("TypeAlias", 10),
    ("TypeGuard", 10),
    ("Self", 11),
    ("Unpack", 11),
    ("Never", 11),
    ("LiteralString", 11),
    ("NotRequired", 11),
    ("Required", 11),
    ("TypeVarTuple", 11),
    ("assert_never", 11),
    ("assert_type", 11),
    ("reveal_type", 11),
    ("dataclass_transform", 11),
    ("override", 12),
    ("TypeAliasType", 12),
    ("TypeVar", 13),
    ("TypeIs", 13),
    ("ReadOnly", 13),
    ("NoDefault", 13),
];

/// `typing` members that are `collections.abc` classes.
const TYPING_ABC: &[&str] = &[
    "AsyncGenerator",
    "AsyncIterable",
    "AsyncIterator",
    "Awaitable",
    "Callable",
    "Collection",
    "Container",
    "Coroutine",
    "Generator",
    "Hashable",
    "ItemsView",
    "Iterable",
    "Iterator",
    "KeysView",
    "Mapping",
    "MappingView",
    "MutableMapping",
    "MutableSequence",
    "MutableSet",
    "Reversible",
    "Sequence",
    "Sized",
    "ValuesView",
];

/// UP035: an import from where the names have moved, or of a deprecated alias.
pub(super) fn deprecated_import(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let Some(cx) = model(ctx.file) else { return };
    let Some(module) = cx.slot(n, "module").map(|m| code_text(&m)) else {
        return;
    };
    if has_token(n, k::DOT) {
        return;
    }
    let mut moved: Vec<(&str, String)> = Vec::new();
    for alias in n.children().filter(|a| a.kind() == k::ALIAS) {
        let Some(name) = cx.slot(&alias, "name").map(|i| code_text(&i)) else {
            continue;
        };
        let target = match module.as_str() {
            "typing_extensions" => match name.as_str() {
                "Buffer" if cx.minor >= 12 => Some("collections.abc"),
                "deprecated" if cx.minor >= 13 => Some("warnings"),
                _ => TYPING_EXTENSIONS
                    .iter()
                    .find(|(n, v)| *n == name && cx.minor >= *v)
                    .map(|_| "typing"),
            },
            "typing" if TYPING_ABC.contains(&name.as_str()) && cx.minor >= 9 => {
                Some("collections.abc")
            }
            "typing" => {
                let builtin = match name.as_str() {
                    "List" => "list",
                    "Dict" => "dict",
                    "Set" => "set",
                    "FrozenSet" => "frozenset",
                    "Tuple" => "tuple",
                    "Type" => "type",
                    _ => "",
                };
                if !builtin.is_empty() && cx.minor >= 9 {
                    c.hit(
                        ctx,
                        span(ctx, n),
                        &format!("`typing.{name}` is deprecated, use `{builtin}` instead"),
                    );
                }
                None
            }
            _ => None,
        };
        if let Some(t) = target {
            moved.push((t, name));
        }
    }
    for target in ["typing", "collections.abc", "warnings"] {
        let names: Vec<&str> = moved
            .iter()
            .filter(|(t, _)| *t == target)
            .map(|(_, n)| n.as_str())
            .collect();
        if !names.is_empty() {
            c.hit(
                ctx,
                span(ctx, n),
                &format!("Import from `{target}` instead: `{}`", names.join("`, `")),
            );
        }
    }
}

/// UP036: a version test that the target version decides.
pub(super) fn outdated_version_block(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let Some(cx) = model(ctx.file) else { return };
    let Some(test) = cx.slot(n, "condition") else {
        return;
    };
    let test = inner(&test);
    if test.kind() != k::EXPR_COMPARE {
        return;
    }
    let mut parts = test.children();
    let (Some(left), Some(right)) = (parts.next(), parts.next()) else {
        return;
    };
    let left_text = code_text(&inner(&left));
    let tuple: Vec<String> = inner(&right).children().map(|e| code_text(&e)).collect();
    let decided = match left_text.as_str() {
        "sys.version_info" | "sys.version_info[:2]" => {
            matches!(tuple.first().map(String::as_str), Some("3"))
                && tuple
                    .get(1)
                    .and_then(|v| v.parse::<u32>().ok())
                    .is_some_and(|v| v < cx.minor)
        }
        "sys.version_info[0]" => inner(&right).kind() == k::EXPR_NUMBER_LITERAL,
        _ => false,
    };
    if decided {
        c.hit(ctx, span(ctx, &test), "");
    }
}
