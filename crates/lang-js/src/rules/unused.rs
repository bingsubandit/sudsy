//! `no-unused-vars` with ESLint's defaults (`vars: all`, `args: after-used`, `caughtErrors:
//! all`) and, in TypeScript files, typescript-eslint's reading of types, `typeof` queries and
//! declaration merging. The checks follow the two rules' own helpers.

use super::Check;
use super::scope::{is_function, param_init, run};
use crate::generated::kind as k;
use crate::semantic::Cx;
use core_rules::RuleContext;
use core_semantics::scopes::{Decl, Flags, Reference, Symbol};
use core_syntax::{SyntaxNode, SyntaxNodeExt, TextRange};

fn is_loop(n: &SyntaxNode) -> bool {
    matches!(
        n.kind(),
        k::FOR_STATEMENT
            | k::FOR_IN_STATEMENT
            | k::FOR_OF_STATEMENT
            | k::WHILE_STATEMENT
            | k::DO_WHILE_STATEMENT
    )
}

fn is_export(n: &SyntaxNode) -> bool {
    matches!(
        n.kind(),
        k::EXPORT_DECLARATION
            | k::EXPORT_NAMED_DECLARATION
            | k::EXPORT_DEFAULT_DECLARATION
            | k::EXPORT_ALL_DECLARATION
    )
}

fn is_param(n: &SyntaxNode) -> bool {
    matches!(n.kind(), k::FORMAL_PARAMETER | k::FORMAL_PARAMETER_REST)
}

/// Is the identifier at `id` inside `typeof x` in a type?
fn in_type_query(id: &SyntaxNode) -> bool {
    let mut at = id.parent();
    while let Some(n) = at {
        match n.kind() {
            k::TS_TYPE_QUERY => return true,
            k::TS_QUALIFIED_NAME => at = n.parent(),
            _ => return false,
        }
    }
    false
}

/// A read, as typescript-eslint counts them: a type reference reads too.
fn is_read(r: &Reference) -> bool {
    r.is_read() || (r.is_type() && !r.is_write())
}

/// Does the expression statement (or the expression, in a sequence of them) have no use for its
/// value?
fn unused_expression(n: &SyntaxNode) -> bool {
    let Some(parent) = n.parent() else {
        return false;
    };
    match parent.kind() {
        k::EXPRESSION_STATEMENT => true,
        k::SEQUENCE_EXPRESSION => {
            parent.children().last().as_ref() != Some(n) || unused_expression(&parent)
        }
        _ => false,
    }
}

fn in_loop(n: &SyntaxNode) -> bool {
    for a in n.ancestors().skip(1) {
        if is_function(&a) {
            return false;
        }
        if is_loop(&a) {
            return true;
        }
    }
    false
}

fn is_statement_or_declaration(cx: &Cx, n: &SyntaxNode) -> bool {
    let name = cx.kind_name(n);
    name.ends_with("Statement") || name.ends_with("Declaration")
}

/// Can the function around the reference outlive the right-hand side it is in (it is stored or
/// returned, so a call to it later may still read the variable)?
fn storable_function(cx: &Cx, id: &SyntaxNode, rhs: TextRange) -> bool {
    let Some(function) = id.ancestors().skip(1).find(is_function) else {
        return false;
    };
    if !rhs.contains_range(cx.range(&function)) {
        return false;
    }
    let (mut node, mut parent) = (function.clone(), function.parent());
    while let Some(p) = parent {
        if !rhs.contains_range(cx.range(&p)) {
            break;
        }
        match p.kind() {
            k::SEQUENCE_EXPRESSION => {
                if p.children().last().as_ref() != Some(&node) {
                    return false;
                }
            }
            k::CALL_EXPRESSION | k::NEW_EXPRESSION => {
                return cx.slot(&p, "callee").as_ref() != Some(&node);
            }
            k::ASSIGNMENT_EXPRESSION | k::TAGGED_TEMPLATE_EXPRESSION | k::YIELD_EXPRESSION => {
                return true;
            }
            _ if is_statement_or_declaration(cx, &p) => return true,
            _ => {}
        }
        node = p.clone();
        parent = p.parent();
    }
    false
}

fn is_logical_assignment(cx: &Cx, assignment: &SyntaxNode) -> bool {
    assignment
        .tokens()
        .find(|t| t.kind() == k::OPERATOR && !cx.g.is_trivia(t.kind()))
        .is_some_and(|t| matches!(t.text(), "&&=" | "||=" | "??="))
}

/// `a = a + 1` and `a++` as statements read `a` only to write it again.
fn read_for_itself(cx: &Cx, r: &Reference, rhs: Option<TextRange>) -> bool {
    if !is_read(r) {
        return false;
    }
    let id = &r.node;
    let own = match id.parent() {
        Some(p) if p.kind() == k::ASSIGNMENT_EXPRESSION => {
            cx.slot(&p, "left").as_ref() == Some(id)
                && !is_logical_assignment(cx, &p)
                && unused_expression(&p)
        }
        Some(p) if p.kind() == k::UPDATE_EXPRESSION => unused_expression(&p),
        _ => false,
    };
    own || rhs
        .is_some_and(|rhs| rhs.contains_range(cx.range(id)) && !storable_function(cx, id, rhs))
}

/// The right-hand side of the statement `a = ...` the reference writes, while later reads of the
/// variable can only come from it.
fn rhs_node(cx: &Cx, r: &Reference, sym: &Symbol, prev: Option<TextRange>) -> Option<TextRange> {
    let id = &r.node;
    let span = cx.range(id);
    if let Some(prev) = prev
        && prev.contains_range(span)
    {
        return Some(prev);
    }
    let later = cx.s.var_scope(r.scope) != cx.s.var_scope(sym.scope) || in_loop(id);
    let p = id.parent()?;
    if p.kind() == k::ASSIGNMENT_EXPRESSION
        && cx.slot(&p, "left").as_ref() == Some(id)
        && unused_expression(&p)
        && !later
    {
        return cx.slot(&p, "right").map(|r| cx.range(&r));
    }
    None
}

/// The parameter list a binding belongs to, when it is one of a function.
fn param_list(binding: &SyntaxNode) -> Option<SyntaxNode> {
    for a in binding.ancestors().skip(1) {
        if a.kind() == k::FORMAL_PARAMETERS {
            return Some(a);
        }
        if is_function(&a) {
            return None;
        }
    }
    None
}

/// Does the binding take a default (`a = 1`, `{ a = 1 }`), which writes it?
fn has_default(binding: &SyntaxNode) -> bool {
    for a in binding.ancestors().skip(1) {
        match a.kind() {
            k::ASSIGNMENT_PATTERN => return true,
            k::FORMAL_PARAMETER => return param_init(&a).is_some(),
            k::FORMAL_PARAMETERS => return false,
            _ => {}
        }
    }
    false
}

/// Is the body of a `for ... in/of` a single `return` (the loop variable is then used as a
/// guard)?
fn only_returns(body: Option<SyntaxNode>) -> bool {
    let Some(body) = body else { return false };
    match body.kind() {
        k::RETURN_STATEMENT => true,
        k::BLOCK_STATEMENT => {
            let mut items = body.children();
            items
                .next()
                .is_some_and(|s| s.kind() == k::RETURN_STATEMENT)
                && items.next().is_none()
        }
        _ => false,
    }
}

/// Does the declarator belong to a `for ... in/of` whose body only returns?
fn returns_in_loop(cx: &Cx, decl: &SyntaxNode) -> bool {
    match decl.parent().and_then(|d| d.parent()) {
        Some(s) if matches!(s.kind(), k::FOR_IN_STATEMENT | k::FOR_OF_STATEMENT) => {
            only_returns(cx.slot(&s, "body"))
        }
        _ => false,
    }
}

/// Does the list of statements export names by a statement of its own (`export { a }`,
/// `export * from`, `export =`, `export default a`)? Declarations it holds are then not ambient.
fn overriding(body: &SyntaxNode) -> bool {
    body.children().any(|s| match s.kind() {
        k::EXPORT_NAMED_DECLARATION
        | k::EXPORT_FROM_DECLARATION
        | k::EXPORT_ALL_DECLARATION
        | k::TS_EXPORT_ASSIGNMENT => true,
        k::EXPORT_DEFAULT_DECLARATION => s.children().any(|c| c.kind() == k::IDENTIFIER_REFERENCE),
        _ => false,
    })
}

/// Is the declaration in a `.d.ts` file or a `declare` module, where it may be used from outside?
fn ambient(cx: &Cx, node: &SyntaxNode) -> bool {
    let statement = if node.kind() == k::VARIABLE_DECLARATOR {
        node.parent()
    } else {
        Some(node.clone())
    };
    let Some(container) = statement.and_then(|s| s.parent()) else {
        return false;
    };
    match container.kind() {
        k::PROGRAM => cx.definition && !overriding(&container),
        k::TS_MODULE_BLOCK => {
            let declared = cx.definition
                || container.ancestors().any(|a| {
                    matches!(
                        a.kind(),
                        k::TS_NAMESPACE_DECLARATION
                            | k::TS_EXTERNAL_MODULE_DECLARATION
                            | k::TS_GLOBAL_DECLARATION
                    ) && cx.has_own(&a, "declare")
                });
            declared && !overriding(&container)
        }
        _ => false,
    }
}

/// Symbols the language makes use of by their declaration alone (typescript-eslint's
/// `eslintUsed`), and the names the rule does not check.
fn skipped(cx: &Cx, sym: &Symbol) -> bool {
    if sym.flags.contains(Flags::ENUM_MEMBER) {
        return true;
    }
    let first = &sym.decls[0];
    // The name of a function or class expression belongs to its own scope.
    if first.flags.contains(Flags::EXPRESSION_NAME) {
        return true;
    }
    let node = &first.node;
    match node.kind() {
        k::FORMAL_PARAMETER => {
            if ["public", "private", "protected", "readonly", "override"]
                .iter()
                .any(|m| cx.has_own(node, m))
            {
                return true;
            }
        }
        // `[K in T]` declares `K` for the mapped type alone.
        k::TS_MAPPED_TYPE => return true,
        // `namespace A.B {}` declares no names of its own.
        k::TS_NAMESPACE_DECLARATION
            if node
                .children()
                .any(|c| c.kind() == k::TS_NAMESPACE_DECLARATION)
                || node
                    .parent()
                    .is_some_and(|p| p.kind() == k::TS_NAMESPACE_DECLARATION) =>
        {
            return true;
        }
        _ => {}
    }
    if cx.ts
        && (sym.decls.iter().any(|d| ambient(cx, &d.node))
            || (sym.flags.contains(Flags::IMPORT) && cx.jsx_pragma(&sym.name)))
    {
        return true;
    }
    // The parameters of signatures and of setters are not checked.
    if is_param(node)
        && let Some(owner) = param_list(&first.binding).and_then(|l| l.parent())
    {
        let setter = owner.parent().is_some_and(|p| {
            matches!(p.kind(), k::METHOD_DEFINITION | k::OBJECT_PROPERTY) && cx.has_own(&p, "set")
        });
        let signature = matches!(
            owner.kind(),
            k::TS_FUNCTION_TYPE
                | k::TS_CONSTRUCTOR_TYPE
                | k::TS_METHOD_SIGNATURE
                | k::TS_CALL_SIGNATURE_DECLARATION
                | k::TS_CONSTRUCT_SIGNATURE_DECLARATION
        ) || (owner.kind() == k::FUNCTION && cx.slot(&owner, "body").is_none());
        if setter || signature {
            return true;
        }
    }
    // `for (const a of b) return;`
    node.kind() == k::VARIABLE_DECLARATOR && returns_in_loop(cx, node)
}

/// Is the symbol exported (its declaration is, or it is a merged type that is)?
fn exported(sym: &Symbol) -> bool {
    sym.decls.iter().any(|d| {
        if is_param(&d.node) {
            return false;
        }
        let at = if d.node.kind() == k::VARIABLE_DECLARATOR {
            d.node.parent()
        } else {
            Some(d.node.clone())
        };
        let merged = d
            .flags
            .intersects(Flags::TYPE_ALIAS.union(Flags::INTERFACE))
            && sym.flags.contains(Flags::VALUE);
        at.and_then(|a| a.parent()).is_some_and(|p| is_export(&p)) && !merged
    })
}

/// The spans a reference from inside does not count as a use of the variable.
#[derive(Default)]
struct Own {
    functions: Vec<TextRange>,
    others: Vec<TextRange>,
}

fn own_spans(cx: &Cx, decls: &[&Decl]) -> Own {
    let mut own = Own::default();
    for d in decls {
        let n = &d.node;
        match n.kind() {
            k::FUNCTION => own.functions.push(cx.range(n)),
            k::VARIABLE_DECLARATOR => {
                if let Some(init) = cx.slot(n, "init")
                    && is_function(&init)
                {
                    own.functions.push(cx.range(&init));
                }
            }
            k::TS_INTERFACE_DECLARATION
            | k::TS_TYPE_ALIAS_DECLARATION
            | k::TS_NAMESPACE_DECLARATION
            | k::TS_EXTERNAL_MODULE_DECLARATION
            | k::TS_GLOBAL_DECLARATION
            | k::TS_ENUM_DECLARATION => own.others.push(cx.range(n)),
            _ => {}
        }
    }
    own
}

/// A variable as ESLint's scope manager has it: a name with its declarations and the references
/// that resolve to it. The scope model has one symbol where ESLint has two for a class
/// declaration (inside and outside its body).
struct Var<'s> {
    sym: &'s Symbol,
    decls: Vec<&'s Decl>,
    refs: Vec<&'s Reference>,
}

fn variables<'s>(cx: &'s Cx) -> Vec<Var<'s>> {
    let mut out = Vec::new();
    for id in cx.s.symbol_ids() {
        let sym = cx.s.symbol(id);
        let decls: Vec<&Decl> = sym.decls.iter().collect();
        let mut refs: Vec<&Reference> = cx.s.refs_of(id).collect();
        let first = decls[0];
        if first.node.kind() == k::CLASS && !first.flags.contains(Flags::EXPRESSION_NAME) {
            let class = cx.range(&first.node);
            refs.retain(|r| !class.contains_range(r.range));
        }
        out.push(Var { sym, decls, refs });
    }
    out
}

/// Does the declaration write the variable (an initializer, a loop variable, a default)?
fn writes_declaration(cx: &Cx, d: &Decl) -> bool {
    match d.node.kind() {
        k::VARIABLE_DECLARATOR => {
            cx.slot(&d.node, "init").is_some()
                || d.node
                    .parent()
                    .and_then(|p| p.parent())
                    .is_some_and(|s| matches!(s.kind(), k::FOR_IN_STATEMENT | k::FOR_OF_STATEMENT))
        }
        k::FORMAL_PARAMETER => has_default(&d.binding),
        _ => false,
    }
}

/// Is the variable read, as ESLint's `isUsedVariable` decides (typescript-eslint's version in
/// TypeScript files; the two agree on JavaScript)?
fn is_used(cx: &Cx, v: &Var<'_>) -> bool {
    let own = own_spans(cx, &v.decls);
    let imported_as_type = v.decls.iter().all(|d| cx.is_type_import(d));
    let mut rhs: Option<TextRange> = None;
    for &r in &v.refs {
        let id = &r.node;
        let span = cx.range(id);
        let for_itself = read_for_itself(cx, r, rhs);
        rhs = rhs_node(cx, r, v.sym, rhs);
        let inside = |spans: &[TextRange]| spans.iter().any(|s| s.contains_range(span));
        let type_only = cx.ts && !imported_as_type && in_type_query(id);
        if is_read(r)
            && !for_itself
            && !type_only
            && !inside(&own.functions)
            && !inside(&own.others)
        {
            return true;
        }
        // `for (a of b) return;` with the loop variable an existing one.
        if let Some(p) = id.parent()
            && matches!(p.kind(), k::FOR_IN_STATEMENT | k::FOR_OF_STATEMENT)
            && only_returns(cx.slot(&p, "body"))
        {
            return true;
        }
    }
    false
}

/// Do the parameters of the list after this one hold a used one?
fn after_last_used(cx: &Cx, sym: &Symbol, start: core_syntax::TextSize, list: &SyntaxNode) -> bool {
    for other in
        cx.s.scope(sym.scope)
            .symbols
            .iter()
            .map(|s| (*s, cx.s.symbol(*s)))
    {
        let (oid, other) = other;
        let first = &other.decls[0];
        if first.name.start() <= start || !is_param(&first.node) {
            continue;
        }
        if param_list(&first.binding).as_ref() != Some(list) {
            continue;
        }
        let referenced =
            cx.s.refs_of(oid)
                .any(|r| !(cx.ts && in_type_query(&r.node)));
        if referenced || skipped(cx, other) || has_default(&first.binding) {
            return false;
        }
    }
    true
}

/// `no-unused-vars`
pub(super) fn unused_vars(c: &Check, _n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    run(ctx, |cx| {
        let mut hits = Vec::new();
        for v in variables(cx) {
            let first = v.decls[0];
            if skipped(cx, v.sym) || exported(v.sym) {
                continue;
            }
            // A plain parameter only counts when no later one is used.
            if first.node.kind() == k::FORMAL_PARAMETER
                && param_init(&first.node).is_none()
                && first
                    .binding
                    .parent()
                    .is_some_and(|p| p.kind() == k::FORMAL_PARAMETER)
                && let Some(list) = param_list(&first.binding)
                && !after_last_used(cx, v.sym, first.name.start(), &list)
            {
                continue;
            }
            if is_used(cx, &v) {
                continue;
            }
            // `import { T } from "t"; type U = typeof T;` and `T` as a type only: not unused.
            let queried = cx.ts && v.refs.iter().any(|r| in_type_query(&r.node));
            if queried && v.sym.flags.contains(Flags::IMPORT) {
                continue;
            }
            let home = cx.s.var_scope(v.sym.scope);
            let writes = (v.refs.iter())
                .filter(|r| r.is_write() && cx.s.var_scope(r.scope) == home)
                .map(|r| r.range)
                .chain(
                    v.decls
                        .iter()
                        .filter(|d| writes_declaration(cx, d))
                        .map(|d| d.name),
                );
            let write = writes.max_by_key(|s| s.start());
            let assigned = write.is_some() || v.refs.iter().any(|r| r.is_write());
            let action = if queried {
                "assigned a value but only used as a type"
            } else if assigned {
                "assigned a value but never used"
            } else {
                "defined but never used"
            };
            hits.push((
                write.unwrap_or(first.name),
                c.message
                    .replace("{name}", &v.sym.name)
                    .replace("{action}", action),
            ));
        }
        hits
    });
}
