//! Checks of ruff's other families (flake8-bugbear, flake8-simplify, pyupgrade, flake8-pie,
//! flake8-comprehensions, flake8-print) that read the shape of a call, a class or a clause and the
//! names they refer to.

use super::Check;
use super::tree::{
    call_arguments, callee, compare_ops, inner, is_statement, keyword, span, statements, tokens,
};
use crate::generated::kind as k;
use crate::semantic::{Cx, model};
use crate::text::{code_text, string_value};
use core_rules::RuleContext;
use core_syntax::{SyntaxKind, SyntaxNode};
use std::collections::BTreeMap;

const KEYWORDS: &[&str] = &[
    "False", "None", "True", "and", "as", "assert", "async", "await", "break", "class", "continue",
    "def", "del", "elif", "else", "except", "finally", "for", "from", "global", "if", "import",
    "in", "is", "lambda", "nonlocal", "not", "or", "pass", "raise", "return", "try", "while",
    "with", "yield",
];

/// Names that stand for `OSError`.
const OS_ERROR_ALIASES: &[&str] = &[
    "EnvironmentError",
    "IOError",
    "WindowsError",
    "mmap.error",
    "os.error",
    "select.error",
    "socket.error",
];

/// Calls that build a mutable value.
const MUTABLE_CALLS: &[&str] = &[
    "list",
    "dict",
    "set",
    "collections.Counter",
    "collections.OrderedDict",
    "collections.defaultdict",
    "collections.deque",
];

/// Types of an annotation that say a default is not mutated (`Sequence`, `tuple`, ...).
const IMMUTABLE_TYPES: &[&str] = &[
    "bool",
    "bytes",
    "complex",
    "float",
    "frozenset",
    "int",
    "str",
    "tuple",
    "typing.AbstractSet",
    "typing.ByteString",
    "typing.Collection",
    "typing.Container",
    "typing.Final",
    "typing.FrozenSet",
    "typing.Hashable",
    "typing.Iterable",
    "typing.Iterator",
    "typing.Literal",
    "typing.Mapping",
    "typing.Reversible",
    "typing.Sequence",
    "typing.Sized",
    "typing.Tuple",
    "collections.abc.Set",
    "collections.abc.Collection",
    "collections.abc.Container",
    "collections.abc.Hashable",
    "collections.abc.Iterable",
    "collections.abc.Iterator",
    "collections.abc.Mapping",
    "collections.abc.Reversible",
    "collections.abc.Sequence",
    "collections.abc.Sized",
];

fn is_identifier(s: &str) -> bool {
    let mut chars = s.chars();
    chars
        .next()
        .is_some_and(|c| c == '_' || unicode_ident::is_xid_start(c))
        && chars.all(unicode_ident::is_xid_continue)
        && !KEYWORDS.contains(&s)
}

/// The attribute of a `getattr`/`setattr` call: a plain string that is a name (not a mangled
/// private one).
fn attribute_name(e: &SyntaxNode) -> bool {
    let e = inner(e);
    e.kind() == k::EXPR_STRING_LITERAL && {
        let v = string_value(&e);
        is_identifier(&v) && v != "__debug__" && !(v.starts_with("__") && !v.ends_with("__"))
    }
}

/// The arguments of a call: positional ones (starred included) and the keywords.
pub(super) fn call_args(call: &SyntaxNode) -> (Vec<SyntaxNode>, Vec<SyntaxNode>) {
    match call_arguments(call) {
        Some(a) => super::tree::arguments(&a),
        None => (Vec::new(), Vec::new()),
    }
}

/// The name a call's callee has when it is a plain name that resolves to the builtin of that name.
pub(super) fn builtin_call(cx: &Cx, call: &SyntaxNode) -> Option<String> {
    let f = callee(call)?;
    (f.kind() == k::EXPR_NAME)
        .then(|| cx.qualified(&f))
        .flatten()
        .filter(|q| !q.contains('.') && q.as_str() == code_text(&f))
}

/// `itertools.count()`, `cycle(x)` or `repeat(x)` (without a count): never ends.
fn is_infinite(cx: &Cx, e: &SyntaxNode) -> bool {
    let e = inner(e);
    if e.kind() != k::EXPR_CALL {
        return false;
    }
    let (positional, keywords) = call_args(&e);
    let n = positional.len() + keywords.len();
    match callee(&e).and_then(|f| cx.qualified(&f)).as_deref() {
        Some("itertools.count" | "itertools.cycle") => true,
        Some("itertools.repeat") => n == 1,
        _ => false,
    }
}

pub(super) fn get_attr_with_constant(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let Some(cx) = model(ctx.file) else { return };
    let (args, keywords) = call_args(n);
    if builtin_call(&cx, n).as_deref() == Some("getattr")
        && args.len() == 2
        && keywords.is_empty()
        && attribute_name(&args[1])
    {
        c.hit(ctx, span(ctx, n), "");
    }
}

pub(super) fn zip_without_strict(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let Some(cx) = model(ctx.file) else { return };
    if cx.minor < 10 || builtin_call(&cx, n).as_deref() != Some("zip") {
        return;
    }
    let (args, keywords) = call_args(n);
    let strict = keywords
        .iter()
        .any(|kw| keyword(kw).0.as_deref() == Some("strict"));
    let starred = args.iter().any(|a| a.kind() == k::EXPR_STARRED);
    if !strict && (args.len() > 1 || starred) && !args.iter().any(|a| is_infinite(&cx, a)) {
        c.hit(ctx, span(ctx, n), "");
    }
}

pub(super) fn unnecessary_collection_call(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let Some(cx) = model(ctx.file) else { return };
    let Some(name) = builtin_call(&cx, n) else {
        return;
    };
    let (args, keywords) = call_args(n);
    let hit = match name.as_str() {
        "dict" => args.is_empty() && keywords.iter().all(|kw| keyword(kw).0.is_some()),
        "list" | "tuple" => args.is_empty() && keywords.is_empty(),
        _ => false,
    };
    if hit {
        c.hit(ctx, span(ctx, n), &name);
    }
}

/// `x.keys()` with no arguments.
fn is_keys_call(e: &SyntaxNode) -> bool {
    let e = inner(e);
    if e.kind() != k::EXPR_CALL {
        return false;
    }
    let (args, keywords) = call_args(&e);
    let Some(f) = callee(&e).filter(|f| f.kind() == k::EXPR_ATTRIBUTE) else {
        return false;
    };
    let mut parts = f.children();
    let (Some(object), Some(member)) = (parts.next(), parts.next()) else {
        return false;
    };
    args.is_empty()
        && keywords.is_empty()
        && code_text(&member) == "keys"
        && !(object.kind() == k::EXPR_NAME && matches!(code_text(&object).as_str(), "self" | "cls"))
}

/// SIM118: `key in dict.keys()` in a comparison, a comprehension or a `for`.
pub(super) fn in_dict_keys(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    match n.kind() {
        k::EXPR_COMPARE => {
            let ops = compare_ops(n);
            if let [(_, op, right)] = &ops[..]
                && matches!(op[..], [k::IN] | [k::NOT, k::IN])
                && is_keys_call(right)
            {
                c.hit(ctx, span(ctx, n), "");
            }
        }
        k::COMPREHENSION => {
            if n.children().nth(1).is_some_and(|i| is_keys_call(&i)) {
                c.hit(ctx, span(ctx, n), "");
            }
        }
        _ => {
            if let Some(i) = ctx.file.grammar.slot(n, "iter").and_then(|i| i.into_node())
                && is_keys_call(&i)
            {
                c.hit(ctx, span(ctx, &i), "");
            }
        }
    }
}

/// Is the expression in the test of an `if` whose body is only a `raise` (an argument check)?
fn exception_check(n: &SyntaxNode) -> bool {
    let mut child = n.clone();
    for a in n.ancestors().skip(1) {
        match a.kind() {
            k::STMT_IF | k::ELIF_ELSE_CLAUSE
                if !is_statement(child.kind()) && child.kind() != k::ELIF_ELSE_CLAUSE =>
            {
                let owner = if a.kind() == k::STMT_IF {
                    a
                } else {
                    a.parent().unwrap_or(a)
                };
                let body: Vec<SyntaxNode> = statements(&owner).collect();
                return matches!(&body[..], [only] if only.kind() == k::STMT_RAISE);
            }
            kind if is_statement(kind) => return false,
            _ => {}
        }
        child = a;
    }
    false
}

/// The name of the function the node is in (empty in a lambda or outside of functions).
fn function_name(n: &SyntaxNode) -> String {
    for a in n.ancestors().skip(1) {
        match a.kind() {
            k::STMT_FUNCTION_DEF => {
                return a
                    .children()
                    .find(|c| c.kind() == k::IDENTIFIER)
                    .map(|i| code_text(&i))
                    .unwrap_or_default();
            }
            k::EXPR_LAMBDA => return String::new(),
            _ => {}
        }
    }
    String::new()
}

pub(super) fn has_token(n: &SyntaxNode, kind: SyntaxKind) -> bool {
    tokens(n).any(|t| t.kind() == kind)
}

/// SIM201, SIM202 and SIM208: a negated comparison or a double negation.
fn unary(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>, want: Option<SyntaxKind>) {
    if !has_token(n, k::NOT) {
        return;
    }
    let Some(operand) = n.children().next().map(|o| inner(&o)) else {
        return;
    };
    let hit = match operand.kind() {
        k::EXPR_COMPARE => {
            let ops = compare_ops(&operand);
            let skip =
                exception_check(n) || matches!(function_name(n).as_str(), "__eq__" | "__ne__");
            matches!(&ops[..], [(_, op, _)] if !skip && op[..] == [want.unwrap_or(k::UNKNOWN)])
        }
        k::EXPR_UNARY_OP => want.is_none() && has_token(&operand, k::NOT),
        _ => false,
    };
    if hit {
        c.hit(ctx, span(ctx, n), "");
    }
}

pub(super) fn negate_equal_op(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    unary(c, n, ctx, Some(k::EQ_EQUAL));
}

pub(super) fn negate_not_equal_op(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    unary(c, n, ctx, Some(k::NOT_EQUAL));
}

pub(super) fn double_negation(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    unary(c, n, ctx, None);
}

pub(super) fn boolean(e: &SyntaxNode) -> Option<bool> {
    let e = inner(e);
    (e.kind() == k::EXPR_BOOLEAN_LITERAL).then(|| code_text(&e) == "True")
}

/// SIM210, SIM211 and SIM212: a conditional expression that is a boolean or a swapped one.
fn if_expr(
    c: &Check,
    n: &SyntaxNode,
    ctx: &mut RuleContext<'_>,
    f: impl FnOnce(&[SyntaxNode; 3]) -> bool,
) {
    let parts: Vec<SyntaxNode> = n.children().collect();
    if let Ok(parts) = <[SyntaxNode; 3]>::try_from(parts)
        && f(&parts)
    {
        c.hit(ctx, span(ctx, n), "");
    }
}

pub(super) fn if_expr_true_false(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    if_expr(c, n, ctx, |[body, _, orelse]| {
        boolean(body) == Some(true) && boolean(orelse) == Some(false)
    });
}

pub(super) fn if_expr_false_true(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    if_expr(c, n, ctx, |[body, _, orelse]| {
        boolean(body) == Some(false) && boolean(orelse) == Some(true)
    });
}

pub(super) fn if_expr_twisted_arms(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    if_expr(c, n, ctx, |[body, test, orelse]| {
        let (test, orelse) = (inner(test), inner(orelse));
        boolean(body).is_none()
            && boolean(&orelse).is_none()
            && test.kind() == k::EXPR_UNARY_OP
            && has_token(&test, k::NOT)
            && test.children().next().map(|o| inner(&o)).is_some_and(|o| {
                o.kind() == k::EXPR_NAME
                    && orelse.kind() == k::EXPR_NAME
                    && code_text(&o) == code_text(&orelse)
            })
    });
}

/// PIE810: `a.startswith(x) or a.startswith(y)`.
pub(super) fn multiple_starts_ends_with(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    if !has_token(n, k::OR) {
        return;
    }
    let mut seen: BTreeMap<(String, String), u32> = BTreeMap::new();
    for v in n.children() {
        let v = inner(&v);
        if v.kind() != k::EXPR_CALL {
            continue;
        }
        let (args, keywords) = call_args(&v);
        let Some(f) = callee(&v).filter(|f| f.kind() == k::EXPR_ATTRIBUTE) else {
            continue;
        };
        let mut parts = f.children();
        let (Some(object), Some(member)) = (parts.next(), parts.next()) else {
            continue;
        };
        let name = code_text(&member);
        if matches!(name.as_str(), "startswith" | "endswith")
            && args.len() == 1
            && keywords.is_empty()
        {
            let text = ctx.file.text[span(ctx, &object)].to_owned();
            *seen.entry((name, text)).or_default() += 1;
        }
    }
    let mut names: Vec<&String> = seen
        .iter()
        .filter(|(_, n)| **n > 1)
        .map(|((a, _), _)| a)
        .collect();
    names.sort_unstable();
    for name in names {
        c.hit(ctx, span(ctx, n), name);
    }
}

pub(super) fn assert_false(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    if n.children().next().and_then(|t| boolean(&t)) == Some(false) {
        c.hit(ctx, span(ctx, n), "");
    }
}

pub(super) fn set_attr_with_constant(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let Some(cx) = model(ctx.file) else { return };
    let Some(call) = n.children().next().filter(|e| e.kind() == k::EXPR_CALL) else {
        return;
    };
    let (args, keywords) = call_args(&call);
    if builtin_call(&cx, &call).as_deref() == Some("setattr")
        && args.len() == 3
        && keywords.is_empty()
        && attribute_name(&args[1])
    {
        c.hit(ctx, span(ctx, &call), "");
    }
}

/// The body-statement groups of a node: the `else` and `finally` parts are groups of their own.
pub(super) fn bodies(n: &SyntaxNode) -> Vec<Vec<SyntaxNode>> {
    let mut out = vec![Vec::new()];
    for e in n.children_with_tokens() {
        match e {
            core_syntax::SyntaxElement::Token(t) if matches!(t.kind(), k::ELSE | k::FINALLY) => {
                out.push(Vec::new())
            }
            core_syntax::SyntaxElement::Node(s) if is_statement(s.kind()) => {
                if let Some(last) = out.last_mut() {
                    last.push(s);
                }
            }
            _ => {}
        }
    }
    out
}

pub(super) const BODIES: &[SyntaxKind] = &[
    k::MOD_MODULE,
    k::STMT_FUNCTION_DEF,
    k::STMT_CLASS_DEF,
    k::STMT_FOR,
    k::STMT_WHILE,
    k::STMT_IF,
    k::STMT_WITH,
    k::STMT_TRY,
    k::ELIF_ELSE_CLAUSE,
    k::EXCEPT_HANDLER_EXCEPT_HANDLER,
    k::MATCH_CASE,
];

/// Is the class a `Protocol`?
fn is_protocol(cx: &Cx, class: &SyntaxNode) -> bool {
    call_arguments_of_class(class).iter().any(|b| {
        let b = if b.kind() == k::EXPR_SUBSCRIPT {
            b.children().next().unwrap_or(b.clone())
        } else {
            b.clone()
        };
        cx.qualified(&b)
            .is_some_and(|q| matches!(q.as_str(), "typing.Protocol" | "typing_extensions.Protocol"))
    })
}

fn call_arguments_of_class(class: &SyntaxNode) -> Vec<SyntaxNode> {
    class
        .children()
        .find(|c| c.kind() == k::ARGUMENTS)
        .map(|a| super::tree::arguments(&a).0)
        .unwrap_or_default()
}

/// Is the function decorated with `abc.abstractmethod`?
fn is_abstract(cx: &Cx, f: &SyntaxNode) -> bool {
    f.children().filter(|d| d.kind() == k::DECORATOR).any(|d| {
        d.children()
            .next()
            .and_then(|e| cx.qualified(&e))
            .as_deref()
            == Some("abc.abstractmethod")
    })
}

/// May a `...` stand in this body: in a protocol class, or an abstract method?
fn ellipsis_ok(cx: &Cx, n: &SyntaxNode) -> bool {
    let protocol = n
        .ancestors()
        .find(|a| a.kind() == k::STMT_CLASS_DEF)
        .is_some_and(|c| is_protocol(cx, &c));
    protocol
        || n.ancestors()
            .find(|a| matches!(a.kind(), k::STMT_FUNCTION_DEF | k::STMT_CLASS_DEF))
            .is_some_and(|f| f.kind() == k::STMT_FUNCTION_DEF && is_abstract(cx, &f))
}

/// PIE790: a `pass` or `...` in a body that has other statements.
pub(super) fn unnecessary_placeholder(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let Some(cx) = model(ctx.file) else { return };
    for body in bodies(n).iter().filter(|b| b.len() > 1) {
        for s in body {
            match s.kind() {
                k::STMT_PASS => c.hit(ctx, span(ctx, s), "pass"),
                k::STMT_EXPR
                    if s.children()
                        .next()
                        .is_some_and(|e| e.kind() == k::EXPR_ELLIPSIS_LITERAL)
                        && !ellipsis_ok(&cx, n) =>
                {
                    c.hit(ctx, span(ctx, s), "...")
                }
                _ => continue,
            };
        }
    }
}

/// UP004: `class A(object)`.
pub(super) fn useless_object_inheritance(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let Some(cx) = model(ctx.file) else { return };
    let name = n
        .children()
        .find(|c| c.kind() == k::IDENTIFIER)
        .map(|i| code_text(&i))
        .unwrap_or_default();
    for a in call_arguments_of_class(n) {
        if a.kind() == k::EXPR_NAME && cx.qualified(&a).as_deref() == Some("object") {
            c.hit(ctx, span(ctx, &a), &name);
        }
    }
}

/// UP024: a name that stands for `OSError`.
fn os_error_alias(c: &Check, cx: &Cx, ctx: &mut RuleContext<'_>, e: &SyntaxNode) {
    if let Some(q) = cx.qualified(e)
        && OS_ERROR_ALIASES.contains(&q.as_str())
    {
        c.hit(ctx, span(ctx, e), &q);
    }
}

/// The exception type of an `except` clause.
pub(super) fn handler_type(ctx: &RuleContext<'_>, h: &SyntaxNode) -> Option<SyntaxNode> {
    ctx.file.grammar.slot(h, "type")?.into_node()
}

pub(super) fn os_error_alias_check(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let Some(cx) = model(ctx.file) else { return };
    if n.kind() == k::STMT_RAISE {
        let Some(exc) = n.children().next().map(|e| inner(&e)) else {
            return;
        };
        let target = if exc.kind() == k::EXPR_CALL {
            callee(&exc).unwrap_or(exc)
        } else {
            exc
        };
        os_error_alias(c, &cx, ctx, &target);
    } else if let Some(t) = handler_type(ctx, n).map(|t| inner(&t)) {
        if t.kind() == k::EXPR_TUPLE {
            t.children().for_each(|e| os_error_alias(c, &cx, ctx, &e));
        } else {
            os_error_alias(c, &cx, ctx, &t);
        }
    }
}

/// A list, dict or set display or comprehension, or a call that builds one.
pub(super) fn is_mutable(cx: &Cx, e: &SyntaxNode) -> bool {
    let d = inner(e);
    match d.kind() {
        k::EXPR_LIST
        | k::EXPR_DICT
        | k::EXPR_SET
        | k::EXPR_LIST_COMP
        | k::EXPR_SET_COMP
        | k::EXPR_DICT_COMP => true,
        k::EXPR_CALL => callee(&d)
            .and_then(|f| cx.qualified(&f))
            .is_some_and(|q| MUTABLE_CALLS.contains(&q.as_str())),
        _ => false,
    }
}

/// B006: a default that is mutable and shared by every call.
pub(super) fn mutable_argument_default(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let Some(cx) = model(ctx.file) else { return };
    if n.parent()
        .and_then(|p| p.parent())
        .is_none_or(|f| f.kind() != k::STMT_FUNCTION_DEF)
    {
        return;
    }
    let mut parts = n.children();
    let (Some(parameter), Some(default)) = (parts.next(), parts.next()) else {
        return;
    };
    let mutable = is_mutable(&cx, &default);
    let immutable = ctx
        .file
        .grammar
        .slot(&parameter, "annotation")
        .and_then(|a| a.into_node())
        .is_some_and(|a| immutable_annotation(&cx, &a));
    if mutable && !immutable {
        c.hit(ctx, span(ctx, &default), "");
    }
}

pub(super) fn immutable_annotation(cx: &Cx, a: &SyntaxNode) -> bool {
    let a = inner(a);
    match a.kind() {
        k::EXPR_SUBSCRIPT => {
            let mut parts = a.children();
            let (Some(object), Some(slice)) = (parts.next(), parts.next()) else {
                return false;
            };
            match cx.qualified(&object).as_deref() {
                Some("typing.Optional") => immutable_annotation(cx, &slice),
                Some("typing.Union") => {
                    let slice = inner(&slice);
                    if slice.kind() == k::EXPR_TUPLE {
                        slice.children().all(|e| immutable_annotation(cx, &e))
                    } else {
                        immutable_annotation(cx, &slice)
                    }
                }
                _ => immutable_annotation(cx, &object),
            }
        }
        k::EXPR_BIN_OP if has_token(&a, k::VBAR) => {
            a.children().all(|e| immutable_annotation(cx, &e))
        }
        k::EXPR_NONE_LITERAL => true,
        k::EXPR_STRING_LITERAL => false,
        _ => cx
            .qualified(&a)
            .is_some_and(|q| IMMUTABLE_TYPES.contains(&q.as_str())),
    }
}

/// B013: `except (E,)`.
pub(super) fn redundant_tuple_in_handler(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let Some(t) = handler_type(ctx, n) else {
        return;
    };
    let tuple = inner(&t);
    if tuple.kind() == k::EXPR_TUPLE {
        let elts: Vec<SyntaxNode> = tuple.children().collect();
        if elts.len() == 1 && inner(&elts[0]).kind() != k::EXPR_STARRED {
            c.hit(ctx, span(ctx, &t), "");
        }
    }
}

/// The `raise` statements of a clause's body: nested blocks are read (not the `else` of a loop),
/// `try` statements, functions and classes are not.
pub(super) fn collect_raises(body: impl Iterator<Item = SyntaxNode>, out: &mut Vec<SyntaxNode>) {
    for s in body {
        match s.kind() {
            k::STMT_RAISE => out.push(s),
            k::STMT_IF => {
                collect_raises(statements(&s), out);
                for clause in s.children().filter(|c| c.kind() == k::ELIF_ELSE_CLAUSE) {
                    collect_raises(statements(&clause), out);
                }
            }
            k::STMT_FOR | k::STMT_WHILE => collect_raises(
                bodies(&s)
                    .into_iter()
                    .next()
                    .unwrap_or_default()
                    .into_iter(),
                out,
            ),
            k::STMT_WITH => collect_raises(statements(&s), out),
            k::STMT_MATCH => {
                for case in s.children().filter(|c| c.kind() == k::MATCH_CASE) {
                    collect_raises(statements(&case), out);
                }
            }
            _ => {}
        }
    }
}

/// B904: `raise` in an `except` clause without `from`.
pub(super) fn raise_without_from(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let bound = n
        .children()
        .find(|c| c.kind() == k::IDENTIFIER)
        .map(|i| code_text(&i));
    let kind = if has_token(n, k::STAR) {
        "except*"
    } else {
        "except"
    };
    let mut raises = Vec::new();
    collect_raises(statements(n), &mut raises);
    for r in raises {
        let Some(exc) = r.children().next() else {
            continue;
        };
        let named = exc.kind() == k::EXPR_NAME && Some(code_text(&exc)) == bound;
        if has_token(&r, k::FROM) || named {
            continue;
        }
        c.hit(ctx, span(ctx, &r), kind);
    }
}

/// UP025: a string with the `u` prefix.
pub(super) fn unicode_literal(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    if tokens(n)
        .next()
        .is_some_and(|t| t.text().starts_with(['u', 'U']))
    {
        c.hit(ctx, span(ctx, n), "");
    }
}
