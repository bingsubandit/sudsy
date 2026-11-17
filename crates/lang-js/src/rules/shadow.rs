//! `no-shadow`.

use super::Check;
use super::scope::run;
use crate::generated::kind as k;
use crate::semantic::Cx;
use core_rules::RuleContext;
use core_semantics::scopes::{Flags, ScopeId, Symbol, SymbolId};
use core_syntax::{SyntaxNode, TextSize};

fn line_of(text: &str, offset: TextSize) -> usize {
    text.as_bytes()[..u32::from(offset) as usize]
        .iter()
        .filter(|&&b| b == b'\n')
        .count()
        + 1
}

/// Is the parameter `binding` one of a function type, a signature or a function without a body?
fn in_signature(cx: &Cx, binding: &SyntaxNode) -> bool {
    let Some(params) = binding.ancestors().find(|a| a.kind() == k::FORMAL_PARAMETERS) else {
        return false;
    };
    let Some(owner) = params.parent() else {
        return false;
    };
    match owner.kind() {
        k::FUNCTION => cx.slot(&owner, "body").is_none(),
        k::TS_FUNCTION_TYPE
        | k::TS_CONSTRUCTOR_TYPE
        | k::TS_METHOD_SIGNATURE
        | k::TS_CALL_SIGNATURE_DECLARATION
        | k::TS_CONSTRUCT_SIGNATURE_DECLARATION => true,
        _ => false,
    }
}

/// Is the type parameter declared by `node` one of a `static` method?
fn static_method_generic(cx: &Cx, node: &SyntaxNode) -> bool {
    let mut up = node.ancestors().skip(1);
    let (_, Some(function)) = (up.next(), up.next()) else {
        return false;
    };
    function.kind() == k::FUNCTION
        && function
            .parent()
            .is_some_and(|m| m.kind() == k::METHOD_DEFINITION && cx.has_own(&m, "static"))
}

/// Is `inner` (a function or class expression) the initializer of the variable declared by
/// `outer`, directly or as an operand of `||`, `&&`, `??` or a branch of `?:`?
fn is_initializer(cx: &Cx, outer: &SyntaxNode, inner: &SyntaxNode) -> bool {
    if outer.kind() != k::VARIABLE_DECLARATOR {
        return false;
    }
    let Some(init) = cx.slot(outer, "init") else {
        return false;
    };
    let mut top = inner.clone();
    loop {
        let Some(parent) = top.parent() else { break };
        let climb = match parent.kind() {
            k::LOGICAL_EXPRESSION => true,
            k::CONDITIONAL_EXPRESSION => parent.children().next().as_ref() != Some(&top),
            _ => false,
        };
        if !climb {
            break;
        }
        top = parent;
    }
    init == top
}

/// The name of a function expression: a variable of its own, between the scope of the function
/// and the one around it.
fn is_expression_name(s: &Symbol) -> bool {
    s.flags.contains(Flags::EXPRESSION_NAME) && s.flags.contains(Flags::FUNCTION)
}

/// A variable that has the name of one in a scope around it (ESLint's defaults: functions are
/// hoisted, the others are not, and `var a = function a() {}` does not count).
pub(super) fn shadow(c: &Check, _n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let text = ctx.file.text;
    run(ctx, |cx| {
        let m = &cx.s;
        let expression_name = |scope: ScopeId| -> Option<SymbolId> {
            (m.scope(scope).symbols.iter().copied()).find(|s| is_expression_name(m.symbol(*s)))
        };
        let find = |from: ScopeId, name: &str| {
            m.ancestors(from).find_map(|s| {
                let own = m.own(s, name).filter(|o| !is_expression_name(m.symbol(*o)));
                own.or_else(|| expression_name(s).filter(|e| &*m.symbol(*e).name == name))
            })
        };
        let mut hits = Vec::new();
        for sid in m.symbol_ids() {
            let sym = m.symbol(sid);
            let scope = sym.scope;
            let name = &*sym.name;
            // `declare global { ... }` augments the global scope: nothing in it shadows.
            let global = |s: ScopeId| {
                let n = &m.scope(s).node;
                n.kind() == k::TS_MODULE_BLOCK
                    && n.parent().is_some_and(|p| p.kind() == k::TS_GLOBAL_DECLARATION)
            };
            if m.ancestors(scope).any(global) {
                continue;
            }
            let own = (!is_expression_name(sym))
                .then(|| expression_name(scope).filter(|e| &*m.symbol(*e).name == name))
                .flatten();
            let Some(outer) = own.or_else(|| m.scope(scope).parent.and_then(|p| find(p, name)))
            else {
                continue;
            };
            let outer_sym = m.symbol(outer);
            let (inner, outer_decl) = (&sym.decls[0], &outer_sym.decls[0]);
            let expression = inner.flags.contains(Flags::EXPRESSION_NAME);
            let on_initializer = if cx.ts {
                expression && is_initializer(cx, &outer_decl.node, &inner.node)
            } else {
                let declaration = (outer_decl.node.kind() == k::VARIABLE_DECLARATOR)
                    .then(|| outer_decl.node.parent().map(|d| cx.range(&d)))
                    .flatten();
                expression
                    && declaration
                        .is_some_and(|d| d.start() < inner.name.start() && inner.name.end() < d.end())
                    && m.scope(scope).parent == Some(outer_sym.scope)
            };
            let hoisted = outer_decl.flags.contains(Flags::FUNCTION)
                && !outer_decl.flags.contains(Flags::EXPRESSION_NAME);
            let in_tdz = inner.name.end() < outer_decl.name.start() && !hoisted;
            let ts_skip = cx.ts && {
                let signature = matches!(inner.node.kind(), k::FORMAL_PARAMETER | k::FORMAL_PARAMETER_REST)
                    && in_signature(cx, &inner.binding);
                let generic = inner.node.kind() == k::TS_TYPE_PARAMETER
                    && static_method_generic(cx, &inner.node);
                cx.is_value(sym) != cx.is_value(outer_sym) || signature || generic
            };
            if on_initializer || in_tdz || ts_skip {
                continue;
            }
            let line = line_of(text, outer_decl.name.start()).to_string();
            hits.push((
                inner.name,
                c.message.replace("{name}", name).replace("{line}", &line),
            ));
        }
        hits
    });
}
