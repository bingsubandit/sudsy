//! Rules that read what names bind to (docs/formatting/js.md): the model of
//! `crate::semantic`, built on the first of them that runs on a file. Positions follow ESLint's
//! reports, so the parity runners compare them as they are.

use super::{Check, code_text};
use crate::generated::kind as k;
use crate::semantic::{Cx, model};
use core_rules::RuleContext;
use core_semantics::scopes::{Decl, Flags, Reference, Symbol};
use core_syntax::{SyntaxNode, SyntaxNodeExt, TextRange, TextSize};

/// The ES2026 globals of ESLint's default configuration.
const BUILTINS: &[&str] = &[
    "AggregateError",
    "Array",
    "ArrayBuffer",
    "AsyncDisposableStack",
    "Atomics",
    "BigInt",
    "BigInt64Array",
    "BigUint64Array",
    "Boolean",
    "DataView",
    "Date",
    "DisposableStack",
    "Error",
    "EvalError",
    "FinalizationRegistry",
    "Float16Array",
    "Float32Array",
    "Float64Array",
    "Function",
    "Infinity",
    "Int16Array",
    "Int32Array",
    "Int8Array",
    "Intl",
    "Iterator",
    "JSON",
    "Map",
    "Math",
    "NaN",
    "Number",
    "Object",
    "Promise",
    "Proxy",
    "RangeError",
    "ReferenceError",
    "Reflect",
    "RegExp",
    "Set",
    "SharedArrayBuffer",
    "String",
    "SuppressedError",
    "Symbol",
    "SyntaxError",
    "TypeError",
    "URIError",
    "Uint16Array",
    "Uint32Array",
    "Uint8Array",
    "Uint8ClampedArray",
    "WeakMap",
    "WeakRef",
    "WeakSet",
    "constructor",
    "decodeURI",
    "decodeURIComponent",
    "encodeURI",
    "encodeURIComponent",
    "escape",
    "eval",
    "globalThis",
    "hasOwnProperty",
    "isFinite",
    "isNaN",
    "isPrototypeOf",
    "parseFloat",
    "parseInt",
    "propertyIsEnumerable",
    "toLocaleString",
    "toString",
    "undefined",
    "unescape",
    "valueOf",
];

/// What a `commonjs` file (`.cjs`) adds.
const COMMONJS: &[&str] = &["exports", "global", "module", "require"];


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

pub(super) fn is_function(n: &SyntaxNode) -> bool {
    matches!(n.kind(), k::FUNCTION | k::ARROW_FUNCTION_EXPRESSION)
}

fn is_jsx_ref(n: &SyntaxNode) -> bool {
    matches!(
        n.kind(),
        k::JSX_OPENING_ELEMENT | k::JSX_CLOSING_ELEMENT | k::JSX_MEMBER_EXPRESSION
    )
}

/// The reference whose name starts at `start` (references are in source order).
pub(super) fn ref_at(cx: &Cx, start: TextSize) -> Option<&Reference> {
    let refs = &cx.s.refs;
    let i = refs
        .binary_search_by_key(&start, |r| r.range.start())
        .ok()?;
    refs.get(i)
}

/// The execution context a node runs in: its function, the program, or the initializer of an
/// instance field. Static initializers and static blocks run as part of the class's context.
fn exec_ctx(cx: &Cx, from: &SyntaxNode, inclusive: bool, at: TextRange) -> SyntaxNode {
    for n in from.ancestors().skip(usize::from(!inclusive)) {
        match n.kind() {
            k::FUNCTION | k::ARROW_FUNCTION_EXPRESSION | k::PROGRAM => return n,
            k::PROPERTY_DEFINITION
                if !cx.has_own(&n, "static")
                    && cx
                        .slot(&n, "value")
                        .is_some_and(|v| v.text_range().contains_range(at)) =>
            {
                return n;
            }
            _ => {}
        }
    }
    from.clone()
}

/// Is `r` evaluated in an execution context other than the one that declares the symbol?
fn separate_context(cx: &Cx, r: &Reference, sym: &Symbol) -> bool {
    let declared = exec_ctx(cx, &cx.s.scope(sym.scope).node, true, r.range);
    exec_ctx(cx, &r.node, false, r.range) != declared
}

/// `no-undef`: a name no scope declares and the globals do not list.
pub(super) fn undef(c: &Check, _n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let extra: Vec<String> = ctx
        .options
        .list("globals")
        .into_iter()
        .map(String::from)
        .collect();
    run(ctx, |cx| {
        let mut hits = Vec::new();
        for r in cx.s.unresolved() {
            let name = &*r.name;
            if BUILTINS.contains(&name)
                || (cx.commonjs && COMMONJS.contains(&name))
                || extra.iter().any(|g| g == name)
            {
                continue;
            }
            let parent = r.node.parent();
            let typeof_operand = parent
                .as_ref()
                .is_some_and(|p| p.kind() == k::UNARY_EXPRESSION && cx.has_own(p, "typeof"));
            // `arguments` is bound by the nearest ordinary function.
            let arguments = name == "arguments"
                && (cx.commonjs
                    || cx
                        .s
                        .ancestors(r.scope)
                        .any(|s| cx.s.scope(s).node.kind() == k::FUNCTION));
            if r.is_type()
                || parent.as_ref().is_some_and(is_jsx_ref)
                || typeof_operand
                || arguments
            {
                continue;
            }
            hits.push((r.range, c.message.replace("{name}", name)));
        }
        hits
    });
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
    writes(c, ctx, |_, s| s.decls[0].node.kind() == k::CLASS);
}

/// `no-func-assign`
pub(super) fn func_assign(c: &Check, _n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    writes(c, ctx, |_, s| s.decls[0].node.kind() == k::FUNCTION);
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
            let (Some(object), Some(member)) = (cx.slot(&callee, "object"), cx.slot(&callee, "member"))
            else {
                return false;
            };
            if object.kind() != k::IDENTIFIER_REFERENCE {
                return false;
            }
            let methods: &[&str] = match code_text(&object, cx.g).as_str() {
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
            return first && methods.contains(&code_text(&member, cx.g).as_str()) && global;
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
            let namespace = sym.decls[0].node.kind() == k::IMPORT_NAMESPACE_SPECIFIER;
            for r in cx.s.refs_of(id) {
                let start = write_node_start(cx, &r.node, r.range.start());
                let at = TextRange::at(start, 1.into());
                if r.is_write() {
                    hits.push((at, c.message.replace("{name}", &sym.name)));
                } else if namespace && member_written(cx, &r.node) {
                    hits.push((at, format!("The members of '{}' are read-only.", sym.name)));
                }
            }
        }
        hits
    });
}

/// `no-redeclare`
pub(super) fn redeclare(c: &Check, _n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    run(ctx, |cx| {
        let mut hits = Vec::new();
        for id in cx.s.symbol_ids() {
            let sym = cx.s.symbol(id);
            let node = &cx.s.scope(sym.scope).node;
            let visited = matches!(
                node.kind(),
                k::PROGRAM
                    | k::FUNCTION
                    | k::ARROW_FUNCTION_EXPRESSION
                    | k::BLOCK_STATEMENT
                    | k::FOR_STATEMENT
                    | k::FOR_IN_STATEMENT
                    | k::FOR_OF_STATEMENT
                    | k::SWITCH_STATEMENT
            ) || (!cx.ts && node.kind() == k::STATIC_BLOCK);
            if !visited {
                continue;
            }
            let mut decls: Vec<&Decl> = (sym.decls.iter())
                .filter(|d| !d.flags.contains(Flags::EXPRESSION_NAME))
                .collect();
            if cx.ts {
                // An overload signature is not a declaration of its own.
                decls.retain(|d| !(d.node.kind() == k::FUNCTION && cx.slot(&d.node, "body").is_none()));
                if decls.len() > 1 {
                    decls = merged(decls);
                }
            }
            for d in decls.into_iter().skip(1) {
                hits.push((d.name, c.message.replace("{name}", &sym.name)));
            }
        }
        hits
    });
}

/// The declarations TypeScript's declaration merging leaves reported. Interfaces merge with
/// each other, namespaces with each other, and a namespace or interface with a class, a function
/// or an enum; of those, one class, function or enum stands and several are redeclarations.
fn merged(decls: Vec<&Decl>) -> Vec<&Decl> {
    let tags: Vec<char> = decls
        .iter()
        .map(|d| match d.flags {
            f if f.contains(Flags::INTERFACE) => 'i',
            f if f.contains(Flags::NAMESPACE) => 'm',
            f if f.contains(Flags::CLASS) => 'c',
            f if f.contains(Flags::FUNCTION) => 'f',
            f if f.contains(Flags::ENUM) => 'e',
            _ => '-',
        })
        .collect();
    let all = |allowed: &str| tags.iter().all(|t| allowed.contains(*t));
    if tags.iter().all(|t| *t == 'i') || tags.iter().all(|t| *t == 'm') {
        return Vec::new();
    }
    for (allowed, main) in [("cim", 'c'), ("fm", 'f'), ("em", 'e')] {
        if all(allowed) {
            let mains: Vec<&Decl> = (decls.iter().zip(&tags))
                .filter(|(_, t)| **t == main)
                .map(|(d, _)| *d)
                .collect();
            return if mains.len() == 1 { Vec::new() } else { mains };
        }
    }
    decls
}

fn in_static_initializer(cx: &Cx, body: &SyntaxNode, at: TextSize) -> bool {
    body.children().any(|m| {
        let inside = |r: TextRange| r.start() <= at && at <= r.end();
        match m.kind() {
            k::STATIC_BLOCK => inside(m.text_range()),
            k::PROPERTY_DEFINITION => {
                cx.has_own(&m, "static")
                    && cx.slot(&m, "value").is_some_and(|v| inside(v.text_range()))
            }
            _ => false,
        }
    })
}

/// The initializer of a parameter (`a = 1`): the node after its `=`.
pub(super) fn param_init(param: &SyntaxNode) -> Option<SyntaxNode> {
    let eq = param.tokens().find(|t| t.text() == "=")?;
    param
        .children()
        .find(|c| c.text_range().start() >= eq.text_range().end())
}

/// Is the reference evaluated while its variable is being initialized (`var a = a`)?
fn evaluated_during_init(cx: &Cx, r: &Reference, sym: &Symbol, first: &Decl) -> bool {
    if separate_context(cx, r, sym) {
        return false;
    }
    let loc = r.range.end();
    let inside = |range: TextRange| range.start() <= loc && loc <= range.end();
    if first.node.kind() == k::CLASS {
        return inside(cx.range(&first.node))
            && !cx
                .slot(&first.node, "members")
                .is_some_and(|b| in_static_initializer(cx, &b, loc));
    }
    for a in first.binding.ancestors().skip(1) {
        match a.kind() {
            k::VARIABLE_DECLARATOR => {
                if cx.slot(&a, "init").is_some_and(|e| inside(e.text_range())) {
                    return true;
                }
                let stmt = a.parent().and_then(|d| d.parent());
                return match stmt {
                    Some(s) if matches!(s.kind(), k::FOR_IN_STATEMENT | k::FOR_OF_STATEMENT) => {
                        cx.slot(&s, "right").is_some_and(|e| inside(e.text_range()))
                    }
                    _ => false,
                };
            }
            k::ASSIGNMENT_PATTERN if a.children().last().is_some_and(|e| inside(e.text_range())) => {
                return true;
            }
            k::FORMAL_PARAMETER if param_init(&a).is_some_and(|e| inside(e.text_range())) => {
                return true;
            }
            k::FUNCTION
            | k::ARROW_FUNCTION_EXPRESSION
            | k::CLASS
            | k::CATCH_CLAUSE
            | k::IMPORT_DECLARATION
            | k::EXPORT_DECLARATION
            | k::EXPORT_NAMED_DECLARATION => return false,
            _ => {}
        }
    }
    false
}

/// `no-use-before-define`
pub(super) fn use_before_define(c: &Check, _n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    run(ctx, |cx| {
        let mut hits = Vec::new();
        for id in cx.s.symbol_ids() {
            let sym = cx.s.symbol(id);
            let first = &sym.decls[0];
            let class_decorators: Vec<TextRange> = if first.node.kind() == k::CLASS {
                (first.node.children())
                    .filter(|d| d.kind() == k::DECORATOR)
                    .map(|d| d.text_range())
                    .collect()
            } else {
                Vec::new()
            };
            for r in cx.s.refs_of(id) {
                if r.is_type()
                    || (!cx.ts && r.node.parent().as_ref().is_some_and(is_jsx_ref))
                    || class_decorators.iter().any(|d| d.contains_range(r.range))
                {
                    continue;
                }
                if r.range.end() < first.name.end() || evaluated_during_init(cx, r, sym, first) {
                    hits.push((r.range, c.message.replace("{name}", &sym.name)));
                }
            }
        }
        hits
    });
}
