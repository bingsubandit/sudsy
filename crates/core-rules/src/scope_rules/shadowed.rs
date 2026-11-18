//! `shadowed-variable` after ESLint's `no-shadow` defaults: functions are hoisted, the other
//! declarations are not, and `var a = function a() {}` is no shadow. In typed files a type and a
//! value of one name, the parameters of signatures and the type parameters of static members are
//! left alone, as typescript-eslint does.

use super::cx::Cx;
use core_semantics::scopes::{Flags, ScopeId, Symbol, SymbolId};
use core_syntax::{SyntaxNode, TextRange, TextSize};

/// What the rule looks at.
pub(super) struct Opts<'a> {
    /// Names that may shadow.
    pub allow: Vec<&'a str>,
    /// The declarations checked, by what they are (`variable`, `parameter`, `function`, `class`,
    /// `import`, `type`); empty: all.
    pub kinds: Vec<&'a str>,
}

pub(super) struct Hit {
    pub range: TextRange,
    pub name: String,
    pub line: usize,
}

fn line_of(text: &str, offset: TextSize) -> usize {
    text.as_bytes()[..u32::from(offset) as usize]
        .iter()
        .filter(|&&b| b == b'\n')
        .count()
        + 1
}

/// Is the type parameter declared by `node` one of a `static` member?
fn static_member_generic(cx: &Cx, node: &SyntaxNode) -> bool {
    node.ancestors()
        .find(|a| cx.is_function(a))
        .and_then(|f| f.parent())
        .is_some_and(|m| cx.has_own(&m, "static"))
}

/// Is `inner` (a function or class expression) the initializer of the variable declared by
/// `outer`, directly or as an operand of `||`, `&&`, `??` or a branch of `?:`?
fn is_initializer(cx: &Cx, outer: &SyntaxNode, inner: &SyntaxNode) -> bool {
    let Some(init) = cx.slot(outer, "init") else {
        return false;
    };
    let g = cx.file.grammar;
    let logical = |n: &SyntaxNode| {
        cx.tag(n, "binary")
            && n.children_with_tokens()
                .filter_map(|e| e.into_token())
                .any(|t| matches!(g.operator_kind(t.text()), Some("logical" | "coalesce")))
    };
    let mut top = inner.clone();
    while let Some(parent) = top.parent() {
        let climb = logical(&parent)
            || (cx.tag(&parent, "conditional")
                && cx.slot(&parent, "condition").as_ref() != Some(&top));
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

/// Is a declaration with these flags one of the `kinds`?
fn of_kind(flags: Flags, kinds: &[&str]) -> bool {
    kinds.is_empty()
        || kinds.iter().any(|k| match *k {
            "parameter" => flags.contains(Flags::PARAMETER),
            "function" => flags.contains(Flags::FUNCTION),
            "class" => flags.contains(Flags::CLASS),
            "import" => flags.contains(Flags::IMPORT),
            "type" => flags.intersects(
                Flags::INTERFACE | Flags::TYPE_ALIAS | Flags::TYPE_PARAMETER | Flags::ENUM,
            ),
            _ => flags.intersects(Flags::VAR | Flags::LET | Flags::CONST | Flags::CATCH),
        })
}

pub(super) fn run(cx: &Cx, o: &Opts) -> Vec<Hit> {
    let m = cx.m;
    let expression_name = |scope: ScopeId| -> Option<SymbolId> {
        (m.scope(scope).symbols.iter().copied()).find(|s| is_expression_name(m.symbol(*s)))
    };
    let find = |from: ScopeId, name: &str| {
        m.ancestors(from).find_map(|s| {
            let own = m.own(s, name).filter(|o| !is_expression_name(m.symbol(*o)));
            own.or_else(|| expression_name(s).filter(|e| &*m.symbol(*e).name == name))
        })
    };
    // `declare global { ... }` augments the global scope: nothing in it shadows.
    let global = |s: ScopeId| {
        let n = &m.scope(s).node;
        cx.tag(n, "module-body") && n.parent().is_some_and(|p| cx.tag(&p, "global-scope"))
    };
    let mut hits = Vec::new();
    for sid in m.symbol_ids() {
        let sym = m.symbol(sid);
        let (scope, name) = (sym.scope, &*sym.name);
        if o.allow.contains(&name) || m.ancestors(scope).any(global) {
            continue;
        }
        let own = (!is_expression_name(sym))
            .then(|| expression_name(scope).filter(|e| &*m.symbol(*e).name == name))
            .flatten();
        let Some(outer) = own.or_else(|| m.scope(scope).parent.and_then(|p| find(p, name))) else {
            continue;
        };
        let outer_sym = m.symbol(outer);
        let (inner, outer_decl) = (&sym.decls[0], &outer_sym.decls[0]);
        if !of_kind(inner.flags, &o.kinds) {
            continue;
        }
        let (inner_node, outer_node) = (cx.decl_node(inner), cx.decl_node(outer_decl));
        let outer_var = outer_decl
            .flags
            .intersects(Flags::VAR | Flags::LET | Flags::CONST);
        let expression = inner.flags.contains(Flags::EXPRESSION_NAME);
        let on_initializer = if cx.typed {
            expression && outer_var && is_initializer(cx, &outer_node, &inner_node)
        } else {
            let declaration = outer_var.then(|| outer_node.parent().map(|d| cx.range(&d)));
            expression
                && declaration
                    .flatten()
                    .is_some_and(|d| d.start() < inner.name.start() && inner.name.end() < d.end())
                && m.scope(scope).parent == Some(outer_sym.scope)
        };
        let hoisted = outer_decl.flags.contains(Flags::FUNCTION)
            && !outer_decl.flags.contains(Flags::EXPRESSION_NAME)
            && cx.slot(&outer_node, "body").is_some();
        let in_tdz = inner.name.end() < outer_decl.name.start() && !hoisted;
        let typed_skip = cx.typed && {
            let signature =
                inner.flags.contains(Flags::PARAMETER) && cx.in_signature(&cx.binding(inner));
            let generic = inner.flags.contains(Flags::TYPE_PARAMETER)
                && static_member_generic(cx, &inner_node);
            cx.is_value(sym) != cx.is_value(outer_sym) || signature || generic
        };
        if on_initializer || in_tdz || typed_skip {
            continue;
        }
        hits.push(Hit {
            range: inner.name,
            name: name.to_string(),
            line: line_of(cx.file.text, outer_decl.name.start()),
        });
    }
    hits
}
