//! `use-before-define` (a reference that comes before the declaration, or runs while the
//! variable is being initialized) and `redeclared-variable` (a name declared again in one scope),
//! after ESLint's `no-use-before-define` and `no-redeclare`.

use super::cx::Cx;
use core_semantics::scopes::{Decl, Flags, Reference, Symbol};
use core_syntax::{SyntaxNode, TextRange, TextSize};

/// What `use-before-define` checks.
pub(super) struct Kinds {
    pub functions: bool,
    pub classes: bool,
    pub variables: bool,
}

/// The execution context a node runs in: its function, the file, or the initializer of an
/// instance field. Static initializers and static blocks run as part of the class's context.
fn exec_ctx(cx: &Cx, from: &SyntaxNode, inclusive: bool, at: TextRange) -> SyntaxNode {
    for n in from.ancestors().skip(usize::from(!inclusive)) {
        if cx.is_function(&n) || n.parent().is_none() {
            return n;
        }
        if cx.tag(&n, "field")
            && !cx.has_own(&n, "static")
            && cx
                .slot(&n, "value")
                .is_some_and(|v| v.text_range().contains_range(at))
        {
            return n;
        }
    }
    from.clone()
}

/// Is `r` evaluated in an execution context other than the one that declares the symbol?
fn separate_context(cx: &Cx, r: &Reference, sym: &Symbol) -> bool {
    let declared = exec_ctx(cx, &cx.m.scope(sym.scope).node, true, r.range);
    exec_ctx(cx, &cx.node_of(r), false, r.range) != declared
}

fn in_static_initializer(cx: &Cx, body: &SyntaxNode, at: TextSize) -> bool {
    body.children().any(|m| {
        let inside = |r: TextRange| r.start() <= at && at <= r.end();
        if cx.tag(&m, "block") {
            inside(m.text_range())
        } else {
            cx.tag(&m, "field")
                && cx.has_own(&m, "static")
                && cx.slot(&m, "value").is_some_and(|v| inside(v.text_range()))
        }
    })
}

/// Is the reference evaluated while its variable is being initialized (`var a = a`)?
fn evaluated_during_init(cx: &Cx, r: &Reference, sym: &Symbol, first: &Decl) -> bool {
    if separate_context(cx, r, sym) {
        return false;
    }
    let loc = r.range.end();
    let inside = |range: TextRange| range.start() <= loc && loc <= range.end();
    if first.flags.contains(Flags::CLASS) {
        let class = cx.decl_node(first);
        return inside(cx.range(&class))
            && !cx
                .slot(&class, "members")
                .is_some_and(|b| in_static_initializer(cx, &b, loc));
    }
    for a in cx.binding(first).ancestors().skip(1) {
        if cx.tag(&a, "declaration") && cx.file.grammar.has_field(a.kind(), "init") {
            if cx.slot(&a, "init").is_some_and(|e| inside(e.text_range())) {
                return true;
            }
            return match a.parent().and_then(|d| d.parent()) {
                Some(s) if cx.is_foreach(&s) => {
                    cx.slot(&s, "right").is_some_and(|e| inside(e.text_range()))
                }
                _ => false,
            };
        }
        if cx.tag(&a, "default-value")
            && a.children().last().is_some_and(|e| inside(e.text_range()))
        {
            return true;
        }
        if cx.tag(&a, "parameter") {
            if cx.param_init(&a).is_some_and(|e| inside(e.text_range())) {
                return true;
            }
        } else if cx.is_function(&a)
            || cx.tag(&a, "type-declaration")
            || cx.tag(&a, "catch")
            || cx.tag(&a, "import")
            || cx.tag(&a, "export")
        {
            return false;
        }
    }
    false
}

pub(super) fn use_before_define(cx: &Cx, on: &Kinds) -> Vec<(TextRange, String)> {
    let mut hits = Vec::new();
    for id in cx.m.symbol_ids() {
        let sym = cx.m.symbol(id);
        let first = &sym.decls[0];
        if (!on.functions && first.flags.contains(Flags::FUNCTION))
            || (!on.classes && first.flags.contains(Flags::CLASS))
            || (!on.variables
                && first
                    .flags
                    .intersects(Flags::VAR | Flags::LET | Flags::CONST))
        {
            continue;
        }
        let class_decorators: Vec<TextRange> = if first.flags.contains(Flags::CLASS) {
            cx.decl_node(first)
                .children()
                .filter(|d| cx.tag(d, "decorator"))
                .map(|d| d.text_range())
                .collect()
        } else {
            Vec::new()
        };
        for r in cx.m.refs_of(id) {
            if r.is_type()
                || (!cx.typed && r.is_tag())
                || class_decorators.iter().any(|d| d.contains_range(r.range))
            {
                continue;
            }
            if r.range.end() < first.name.end() || evaluated_during_init(cx, r, sym, first) {
                hits.push((r.range, sym.name.to_string()));
            }
        }
    }
    hits
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

pub(super) fn redeclared(cx: &Cx) -> Vec<(TextRange, String)> {
    let mut hits = Vec::new();
    for id in cx.m.symbol_ids() {
        let sym = cx.m.symbol(id);
        let node = &cx.m.scope(sym.scope).node;
        if !cx.lang.checks_redeclared(cx.file.grammar.name(node.kind())) {
            continue;
        }
        let mut decls: Vec<&Decl> = (sym.decls.iter())
            .filter(|d| !d.flags.contains(Flags::EXPRESSION_NAME))
            .collect();
        if cx.typed {
            // An overload signature is not a declaration of its own.
            decls.retain(|d| {
                let n = cx.decl_node(d);
                !(d.flags.contains(Flags::FUNCTION)
                    && cx.is_function(&n)
                    && cx.slot(&n, "body").is_none())
            });
            if decls.len() > 1 {
                decls = merged(decls);
            }
        }
        for d in decls.into_iter().skip(1) {
            hits.push((d.name, sym.name.to_string()));
        }
    }
    hits
}
