//! Rules that read what names bind to (docs/formatting/python.md#scope-layer): the model of
//! `crate::semantic`, built on the first of them that runs on a file. They follow ruff's `F`
//! rules: which binding a read finds, which reads come before a binding, which imports and
//! variables nothing read.

use super::Check;
use super::binding::{Binding, Kind, Live, branch, is_dummy, live};
use crate::generated::kind as k;
use crate::imports::Form;
use crate::names::is_builtin;
use crate::semantic::{Cx, model};
use core_rules::RuleContext;
use core_semantics::scopes::{Flags, ScopeId, SymbolId};
use core_syntax::{SyntaxNode, TextRange, TextSize};
use std::collections::{HashMap, HashSet};

pub(super) type Hit = (TextRange, String);

/// Runs `f` on the file's model (nothing for a file with syntax errors) and reports what it
/// returns, in source order.
pub(super) fn run(
    c: &Check,
    ctx: &mut RuleContext<'_>,
    f: impl FnOnce(&Cx) -> Vec<(TextRange, String)>,
) {
    let Some(cx) = model(ctx.file) else { return };
    let mut hits = f(&cx);
    hits.sort_by_key(|(r, _)| r.start());
    hits.dedup();
    for (range, name) in hits {
        c.hit(ctx, range, &name);
    }
}

fn scope_symbols(cx: &Cx, scope: ScopeId) -> impl Iterator<Item = SymbolId> + '_ {
    cx.s.scope(scope).symbols.iter().copied()
}

/// The names of the strings an assignment to `__all__` at module level lists (ruff keeps those of
/// earlier assignments too).
fn exports(cx: &Cx) -> Vec<(String, TextRange)> {
    fn collect(cx: &Cx, e: &SyntaxNode, out: &mut Vec<(String, TextRange)>) {
        match e.kind() {
            k::EXPR_LIST | k::EXPR_TUPLE => e.children().for_each(|c| collect(cx, &c, out)),
            k::EXPR_BIN_OP
                if e.children_with_tokens()
                    .filter_map(|x| x.into_token())
                    .any(|t| t.kind() == k::PLUS) =>
            {
                e.children().for_each(|c| collect(cx, &c, out));
            }
            k::EXPR_STRING_LITERAL => {
                out.push((crate::text::string_value(e), cx.range(e)));
            }
            _ => {}
        }
    }
    let mut out = Vec::new();
    let Some(id) = cx.s.own(ScopeId(0), "__all__") else {
        return out;
    };
    for d in &cx.s.symbol(id).decls {
        let target = d.binding(&cx.root);
        let Some(stmt) = target.parent() else {
            continue;
        };
        let value = match stmt.kind() {
            k::STMT_ASSIGN => stmt.children().last(),
            k::STMT_AUG_ASSIGN | k::STMT_ANN_ASSIGN => cx.slot(&stmt, "right"),
            _ => None,
        };
        if let Some(v) = value.filter(|v| *v != target) {
            collect(cx, &v, &mut out);
        }
    }
    out
}

fn unused_ok(name: &str) -> bool {
    is_dummy(name)
        || matches!(
            name,
            "__tracebackhide__" | "__traceback_info__" | "__traceback_supplement__"
        )
}

/// The scopes in which `locals()` is called.
fn uses_locals(cx: &Cx) -> HashSet<ScopeId> {
    (cx.s.unresolved())
        .filter(|r| &*r.name == "locals")
        .map(|r| r.scope)
        .collect()
}

/// F401: an import that nothing reads.
pub(super) fn unused_import(c: &Check, _: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    run(c, ctx, |cx| {
        let exported: HashSet<String> = exports(cx).into_iter().map(|(n, _)| n).collect();
        let mut hits = Vec::new();
        for (i, scope) in cx.s.scopes.iter().enumerate() {
            let id = ScopeId(i as u32);
            if scope.node.kind() == k::STMT_CLASS_DEF {
                continue;
            }
            for sym in scope_symbols(cx, id) {
                if !cx.s.symbol(sym).flags.contains(Flags::IMPORT) || &*cx.s.symbol(sym).name == "*"
                {
                    continue;
                }
                let live = live(cx, sym);
                let Some(last) = live.last() else { continue };
                let Some(import) = &last.b.import else {
                    continue;
                };
                let name = &cx.s.symbol(sym).name;
                if last.used
                    || import.explicit
                    || import.form == Form::Future
                    || (i == 0 && exported.contains(&**name))
                    || (import.form == Form::Submodule && alias_used(cx, id, &import.target))
                {
                    continue;
                }
                hits.push((last.b.range, import.label.clone()));
            }
        }
        hits
    });
}

/// `import a.b` next to a used `import a as x`: reading `x` reaches `a.b` too.
fn alias_used(cx: &Cx, scope: ScopeId, target: &str) -> bool {
    scope_symbols(cx, scope).any(|sym| {
        let l = live(cx, sym);
        l.last().is_some_and(|l| {
            l.used
                && l.b.kind == Kind::Import(Form::Import)
                && l.b.import.as_ref().is_some_and(|i| i.target == target)
        })
    })
}

/// Does a new binding of `b` redefine the definition `old`?
fn redefines(b: &Binding, old: &Binding) -> bool {
    match b.kind {
        Kind::Annotation | Kind::Global | Kind::Import(Form::Future) => false,
        Kind::Import(form) => match (form, old.kind) {
            (Form::Import | Form::From, Kind::Import(Form::Submodule))
            | (Form::Submodule, Kind::Import(Form::Import | Form::Submodule)) => {
                label(b) == label(old)
            }
            _ => true,
        },
        _ => true,
    }
}

fn label(b: &Binding) -> &str {
    b.import.as_ref().map_or("", |i| i.label.as_str())
}

/// Is the function decorated with `typing.overload`?
fn is_overload(cx: &Cx, l: &Live<'_>) -> bool {
    l.decl.node(&cx.root).children().any(|d| {
        d.kind() == k::DECORATOR
            && d.first_child()
                .and_then(|e| cx.qualified(&e))
                .is_some_and(|q| {
                    matches!(q.as_str(), "typing.overload" | "typing_extensions.overload")
                })
    })
}

/// F811 and F402: a definition replaced, or covered by a name of a function, before anything
/// read it; an import replaced by a loop variable.
fn redefinitions(cx: &Cx) -> Vec<(&'static str, TextRange, String)> {
    let mut hits = Vec::new();
    for id in cx.s.symbol_ids() {
        let sym = cx.s.symbol(id);
        if &*sym.name == "*" {
            continue;
        }
        if sym.decls.len() > 1 {
            let l = live(cx, id);
            for pair in l.windows(2) {
                let (old, new) = (&pair[0], &pair[1]);
                if old.b.branch != new.b.branch {
                    continue;
                }
                let was_import = old.b.kind.is_import();
                if new.b.kind == Kind::LoopVar && was_import {
                    hits.push(("loop", new.b.range, sym.name.to_string()));
                    continue;
                }
                let definition = was_import
                    || matches!(old.b.kind, Kind::FunctionDef | Kind::ClassDef)
                    || (matches!(old.b.kind, Kind::Assignment | Kind::NamedExpr)
                        && (new.b.kind.is_import()
                            || matches!(new.b.kind, Kind::FunctionDef | Kind::ClassDef)));
                if old.used
                    || !definition
                    || old.b.kind == Kind::Import(Form::Future)
                    || (old.b.kind == Kind::FunctionDef && is_overload(cx, old))
                {
                    continue;
                }
                if redefines(&new.b, &old.b) && (!is_dummy(&sym.name) || was_import) {
                    hits.push(("redefined", new.b.range, sym.name.to_string()));
                }
            }
        }
        shadow(cx, id, &mut hits);
    }
    hits
}

/// A first binding in a function's scope (or a comprehension's) of a name an import of an
/// enclosing scope has: reported when nothing read the import.
fn shadow(cx: &Cx, id: SymbolId, hits: &mut Vec<(&'static str, TextRange, String)>) {
    let sym = cx.s.symbol(id);
    let scope = cx.s.scope(sym.scope);
    if scope.node.kind() == k::STMT_CLASS_DEF
        || sym.scope == ScopeId(0)
        || !sym.flags.contains(Flags::VALUE)
    {
        return;
    }
    let Some(first) = sym.decls.first() else {
        return;
    };
    if first.from != sym.scope || first.flags.contains(Flags::TYPE_PARAMETER) {
        return;
    }
    let by = super::binding::binding(cx, first);
    if matches!(
        by.kind,
        Kind::Annotation | Kind::Global | Kind::Import(Form::Future)
    ) || by.branch.is_some()
    {
        return;
    }
    let outer =
        cx.s.ancestors(sym.scope)
            .skip(1)
            .find_map(|s| cx.s.own(s, &sym.name));
    let Some(outer) = outer else { return };
    if !cx.s.symbol(outer).flags.contains(Flags::IMPORT) {
        return;
    }
    let l = live(cx, outer);
    let Some(old) = l.last() else { return };
    if !old.b.kind.is_import() || old.b.branch.is_some() || old.b.kind == Kind::Import(Form::Future)
    {
        return;
    }
    if by.kind == Kind::LoopVar {
        hits.push(("loop", first.name, sym.name.to_string()));
    } else if !old.used && redefines(&by, &old.b) {
        hits.push(("redefined", first.name, sym.name.to_string()));
    }
}

pub(super) fn redefined_while_unused(c: &Check, _: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    run(c, ctx, |cx| {
        redefinitions(cx)
            .into_iter()
            .filter(|h| h.0 == "redefined")
            .map(|h| (h.1, h.2))
            .collect()
    });
}

pub(super) fn import_shadowed_by_loop_var(c: &Check, _: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    run(c, ctx, |cx| {
        redefinitions(cx)
            .into_iter()
            .filter(|h| h.0 == "loop")
            .map(|h| (h.1, h.2))
            .collect()
    });
}

/// Is the name read where a `try` that handles `NameError` is running its body?
fn name_error_handled(cx: &Cx, at: &SyntaxNode) -> bool {
    let mut child = at.clone();
    for a in at.ancestors().skip(1) {
        match a.kind() {
            k::STMT_FUNCTION_DEF | k::EXPR_LAMBDA | k::STMT_CLASS_DEF => return false,
            k::STMT_TRY => {
                let handlers: Vec<SyntaxNode> = a
                    .children()
                    .filter(|h| h.kind() == k::EXCEPT_HANDLER_EXCEPT_HANDLER)
                    .collect();
                let in_body = handlers
                    .first()
                    .is_none_or(|h| child.text_range().end() <= h.text_range().start())
                    && cx.g.name(child.kind()).starts_with("Stmt");
                if in_body
                    && handlers.iter().any(|h| {
                        cx.slot(h, "type").is_some_and(|t| {
                            let is = |n: &SyntaxNode| {
                                n.kind() == k::EXPR_NAME
                                    && crate::semantic::last_name(n).as_deref() == Some("NameError")
                            };
                            is(&t) || (t.kind() == k::EXPR_TUPLE && t.children().any(|e| is(&e)))
                        })
                    })
                {
                    return true;
                }
            }
            _ => {}
        }
        child = a;
    }
    false
}

/// Where the first binding of a symbol takes effect; `None` when `global` or `nonlocal` binds the
/// name for the whole scope.
fn first_effect(cx: &Cx, id: SymbolId) -> Option<TextSize> {
    let sym = cx.s.symbol(id);
    if sym
        .decls
        .iter()
        .any(|d| d.from != sym.scope || d.flags.contains(Flags::REDIRECT))
    {
        return None;
    }
    Some(super::binding::binding(cx, sym.decls.first()?).effect)
}

/// The reads of names that nothing binds: undefined names, and the names a star import may bring.
/// A read that runs before the first binding of its name in the scope finds the name outside.
fn unresolved(cx: &Cx) -> (Vec<Hit>, Vec<Hit>) {
    let (mut undefined, mut starred) = (Vec::new(), Vec::new());
    let mut firsts: HashMap<SymbolId, Option<TextSize>> = HashMap::new();
    for r in cx.s.refs.iter().filter(|r| r.is_read()) {
        let name = &*r.name;
        let sym = r.symbol.map(|s| cx.s.symbol(s));
        let scope_class = cx.s.scope(r.scope).node.kind() == k::STMT_CLASS_DEF;
        // `del x` unbinds a name of its own scope only.
        if r.is_delete() {
            if cx.s.own(r.scope, name).is_none() && cx.s.redirect(r.scope, name).is_none() {
                undefined.push((r.range, name.to_owned()));
            }
            continue;
        }
        // A name that has only `x: int` is not bound.
        let bare = sym.is_some_and(|s| {
            cx.s.scope(s.scope).node.kind() != k::STMT_CLASS_DEF
                && s.decls.iter().all(|d| d.flags.contains(Flags::BARE))
        });
        if bare {
            undefined.push((r.range, name.to_owned()));
            continue;
        }
        let before = match (r.symbol, sym) {
            (Some(id), Some(s)) if !r.is_type() && !cx.deferred(r.scope, s.scope) => {
                let first = *firsts.entry(id).or_insert_with(|| first_effect(cx, id));
                first.is_some_and(|e| r.range.start() < e)
                    && !cx
                        .s
                        .ancestors(s.scope)
                        .skip(1)
                        .any(|o| !cx.s.scope(o).hidden && cx.s.own(o, name).is_some())
            }
            _ => false,
        };
        if (sym.is_some() && !before) || is_builtin(name) {
            continue;
        }
        let in_class =
            cx.s.ancestors(r.scope)
                .any(|s| cx.s.scope(s).node.kind() == k::STMT_CLASS_DEF);
        if (scope_class && matches!(name, "__module__" | "__qualname__"))
            || (in_class && name == "__class__")
            || (cx.init && name == "__path__")
        {
            continue;
        }
        if cx.s.ancestors(r.scope).any(|s| cx.s.own(s, "*").is_some()) {
            starred.push((r.range, name.to_owned()));
        } else if !name_error_handled(cx, &r.node(&cx.root)) {
            undefined.push((r.range, name.to_owned()));
        }
    }
    (undefined, starred)
}

pub(super) fn undefined_name(c: &Check, _: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    run(c, ctx, |cx| unresolved(cx).0);
}

pub(super) fn star_usage(c: &Check, _: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    run(c, ctx, |cx| {
        let mut hits = unresolved(cx).1;
        let stars = cx.s.own(ScopeId(0), "*").is_some();
        for (name, range) in exports(cx) {
            if cx.s.own(ScopeId(0), &name).is_none() && stars {
                hits.push((range, name));
            }
        }
        hits
    });
}

pub(super) fn forward_annotation_syntax_error(
    c: &Check,
    _: &SyntaxNode,
    ctx: &mut RuleContext<'_>,
) {
    run(c, ctx, |cx| cx.forward_errors.clone());
}

pub(super) fn undefined_export(c: &Check, _: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    run(c, ctx, |cx| {
        if cx.init || cx.s.own(ScopeId(0), "*").is_some() {
            return Vec::new();
        }
        exports(cx)
            .into_iter()
            .filter(|(name, _)| cx.s.own(ScopeId(0), name).is_none())
            .map(|(name, range)| (range, name))
            .collect()
    });
}

/// F823: a name read in a function before the function assigns it, while an enclosing scope
/// binds it too.
pub(super) fn undefined_local(c: &Check, _: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    run(c, ctx, |cx| {
        let mut hits = Vec::new();
        for id in cx.s.symbol_ids() {
            let sym = cx.s.symbol(id);
            if cx.s.scope(sym.scope).node.kind() != k::STMT_FUNCTION_DEF {
                continue;
            }
            let Some(first) = sym.decls.first().filter(|d| d.from == sym.scope) else {
                continue;
            };
            let b = super::binding::binding(cx, first);
            if matches!(
                b.kind,
                Kind::Annotation
                    | Kind::Argument
                    | Kind::Except
                    | Kind::Global
                    | Kind::Import(Form::Future)
            ) {
                continue;
            }
            let read =
                cx.s.refs_of(id)
                    .find(|r| r.is_read() && r.scope == sym.scope && r.range.start() < b.effect);
            let Some(read) = read else { continue };
            let outer = cx.s.ancestors(sym.scope).skip(1).find_map(|s| {
                let node = cx.s.scope(s).node.kind();
                (node == k::STMT_FUNCTION_DEF || node == k::MOD_MODULE)
                    .then(|| cx.s.own(s, &sym.name))
                    .flatten()
            });
            if outer.is_some() {
                hits.push((read.range, sym.name.to_string()));
            }
        }
        hits
    });
}

/// F841 and F842: a local variable assigned or annotated and never read, and a name an `except`
/// clause binds and its body never reads.
fn unused_locals(cx: &Cx, annotation: bool) -> Vec<Hit> {
    let locals = uses_locals(cx);
    let mut hits = Vec::new();
    for id in cx.s.symbol_ids() {
        let sym = cx.s.symbol(id);
        if !sym.flags.contains(Flags::VALUE)
            || unused_ok(&sym.name) && !sym.flags.contains(Flags::CATCH)
        {
            continue;
        }
        if !annotation && sym.flags.contains(Flags::CATCH) {
            for d in sym.decls.iter().filter(|d| d.flags.contains(Flags::CATCH)) {
                let handler = d.node(&cx.root).text_range();
                let read =
                    cx.s.refs_of(id)
                        .any(|r| r.is_read() && handler.contains_range(r.range));
                if !read {
                    hits.push((d.name, sym.name.to_string()));
                }
            }
        }
        if !cx.is_function(sym.scope) || locals.contains(&sym.scope) || unused_ok(&sym.name) {
            continue;
        }
        let l = live(cx, id);
        let Some(last) = l.last() else { continue };
        let kind_ok = if annotation {
            last.b.kind == Kind::Annotation
        } else {
            matches!(
                last.b.kind,
                Kind::Assignment | Kind::NamedExpr | Kind::WithItem
            ) && !last.b.unpacked
        };
        if kind_ok && !last.used {
            hits.push((last.decl.name, sym.name.to_string()));
        }
    }
    hits
}

pub(super) fn unused_variable(c: &Check, _: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    run(c, ctx, |cx| unused_locals(cx, false));
}

pub(super) fn unused_annotation(c: &Check, _: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    run(c, ctx, |cx| unused_locals(cx, true));
}

/// F403 and F406: `from m import *`, and the ones that are not at module level.
fn star_imports(cx: &Cx) -> Vec<(TextRange, String, bool)> {
    let mut out = Vec::new();
    for id in cx.s.scope(ScopeId(0)).symbols.iter().chain(
        cx.s.scopes
            .iter()
            .enumerate()
            .skip(1)
            .flat_map(|(_, s)| s.symbols.iter()),
    ) {
        let sym = cx.s.symbol(*id);
        if &*sym.name != "*" {
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
            out.push((
                cx.range(&stmt),
                format!("{}{module}", ".".repeat(level)),
                sym.scope != ScopeId(0),
            ));
        }
    }
    out
}

pub(super) fn import_star(c: &Check, _: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    run(c, ctx, |cx| {
        star_imports(cx)
            .into_iter()
            .map(|(r, n, _)| (r, n))
            .collect()
    });
}

pub(super) fn nested_import_star(c: &Check, _: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    run(c, ctx, |cx| {
        star_imports(cx)
            .into_iter()
            .filter(|h| h.2)
            .map(|(r, n, _)| (r, n))
            .collect()
    });
}

#[allow(dead_code)]
fn _unused(cx: &Cx, n: &SyntaxNode) {
    let _ = branch(cx, n);
}
