//! `unused-variable` and `unused-parameter` after ESLint's `no-unused-vars` (`vars: all`,
//! `args: after-used`, `caughtErrors: all`) and, in typed files, typescript-eslint's reading of
//! types, `typeof` queries and declaration merging.

use super::cx::Cx;
use crate::Re;
use core_semantics::scopes::{Decl, Flags, Reference, Symbol, SymbolId};
use core_syntax::{SyntaxNode, TextRange, TextSize};

/// How parameters are checked.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Args {
    /// Only parameters after the last one that is used.
    AfterUsed,
    All,
    None,
}

pub(super) struct Opts {
    pub parameters: bool,
    pub args: Args,
    pub caught_errors: bool,
    pub ignore_rest_siblings: bool,
    pub ignore: Option<Re>,
}

/// A finding: where, the symbol's name and what is wrong with it.
pub(super) struct Hit {
    pub range: TextRange,
    pub name: String,
    pub action: &'static str,
}

/// Is the identifier at `id` inside `typeof x` in a type?
fn in_type_query(r: &Reference) -> bool {
    r.is_type() && r.is_read()
}

/// A read, as typescript-eslint counts them: a type reference reads too.
fn is_read(r: &Reference) -> bool {
    r.is_read() || (r.is_type() && !r.is_write())
}

/// Does the expression statement (or the expression, in a sequence of them) have no use for its
/// value?
fn unused_expression(cx: &Cx, n: &SyntaxNode) -> bool {
    let Some(parent) = n.parent() else {
        return false;
    };
    if cx.tag(&parent, "expression-statement") {
        return true;
    }
    cx.tag(&parent, "sequence")
        && (parent.children().last().as_ref() != Some(n) || unused_expression(cx, &parent))
}

fn in_loop(cx: &Cx, n: &SyntaxNode) -> bool {
    for a in n.ancestors().skip(1) {
        if cx.is_function(&a) {
            return false;
        }
        if cx.tag(&a, "loop") {
            return true;
        }
    }
    false
}

fn is_statement_or_declaration(cx: &Cx, n: &SyntaxNode) -> bool {
    cx.tag(n, "statement") && !cx.tag(n, "expression")
}

/// Can the function around the reference outlive the right-hand side it is in (it is stored or
/// returned, so a call to it later may still read the variable)?
fn storable_function(cx: &Cx, id: &SyntaxNode, rhs: TextRange) -> bool {
    let Some(function) = id.ancestors().skip(1).find(|a| cx.is_function(a)) else {
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
        if cx.tag(&p, "sequence") {
            if p.children().last().as_ref() != Some(&node) {
                return false;
            }
        } else if cx.tag(&p, "call") {
            return cx.slot(&p, "callee").as_ref() != Some(&node);
        } else if cx.tag(&p, "assignment")
            || cx.tag(&p, "escapes")
            || is_statement_or_declaration(cx, &p)
        {
            return true;
        }
        node = p.clone();
        parent = p.parent();
    }
    false
}

/// `a ||= b`: the operator is a compound one over a short-circuit operator.
fn is_logical_assignment(cx: &Cx, assignment: &SyntaxNode) -> bool {
    let g = cx.file.grammar;
    assignment
        .children_with_tokens()
        .filter_map(|e| e.into_token())
        .filter(|t| !g.is_trivia(t.kind()))
        .any(|t| {
            t.text()
                .strip_suffix('=')
                .is_some_and(|op| matches!(g.operator_kind(op), Some("logical" | "coalesce")))
        })
}

/// `a = a + 1` and `a++` as statements read `a` only to write it again.
fn read_for_itself(cx: &Cx, r: &Reference, rhs: Option<TextRange>) -> bool {
    if !is_read(r) {
        return false;
    }
    let id = &cx.node_of(r);
    let own = r.is_read()
        && r.is_write()
        && id.parent().is_some_and(|p| {
            !(cx.tag(&p, "assignment") && is_logical_assignment(cx, &p))
                && unused_expression(cx, &p)
        });
    own || rhs
        .is_some_and(|rhs| rhs.contains_range(cx.range(id)) && !storable_function(cx, id, rhs))
}

/// The right-hand side of the statement `a = ...` the reference writes, while later reads of the
/// variable can only come from it.
fn rhs_node(cx: &Cx, r: &Reference, sym: &Symbol, prev: Option<TextRange>) -> Option<TextRange> {
    let id = &cx.node_of(r);
    let span = cx.range(id);
    if let Some(prev) = prev
        && prev.contains_range(span)
    {
        return Some(prev);
    }
    let later = cx.m.var_scope(r.scope) != cx.m.var_scope(sym.scope) || in_loop(cx, id);
    let p = id.parent()?;
    if cx.tag(&p, "assignment")
        && cx.slot(&p, "left").as_ref() == Some(id)
        && unused_expression(cx, &p)
        && !later
    {
        return cx.slot(&p, "right").map(|r| cx.range(&r));
    }
    None
}

/// Is the body of a `for ... in/of` a single `return` (the loop variable is then used as a
/// guard)?
fn only_returns(cx: &Cx, body: Option<SyntaxNode>) -> bool {
    let Some(body) = body else { return false };
    if cx.tag(&body, "return") {
        return true;
    }
    if !cx.tag(&body, "block") {
        return false;
    }
    let mut items = body.children();
    items.next().is_some_and(|s| cx.tag(&s, "return")) && items.next().is_none()
}

/// Does the declarator belong to a `for ... in/of` whose body only returns?
fn returns_in_loop(cx: &Cx, decl: &SyntaxNode) -> bool {
    match decl.parent().and_then(|d| d.parent()) {
        Some(s) if cx.is_foreach(&s) => only_returns(cx, cx.slot(&s, "body")),
        _ => false,
    }
}

/// Does the list of statements export names by a statement of its own (`export { a }`,
/// `export * from`, `export =`, `export default a`)? Declarations it holds are then not ambient.
fn overriding(cx: &Cx, body: &SyntaxNode) -> bool {
    body.children().any(|s| {
        cx.tag(&s, "export-list")
            || (cx.tag(&s, "export") && s.children().any(|c| cx.tag(&c, "reference")))
    })
}

/// Is the declaration in a definition file or a `declare` module, where it may be used from outside?
fn ambient(cx: &Cx, d: &Decl) -> bool {
    let Some(container) = cx.statement_of(d).and_then(|s| s.parent()) else {
        return false;
    };
    if &container == cx.root() {
        return cx.definition && !overriding(cx, &container);
    }
    if !cx.tag(&container, "module-body") {
        return false;
    }
    let declared = cx.definition
        || container
            .ancestors()
            .any(|a| cx.tag(&a, "module") && cx.has_own(&a, "declare"));
    declared && !overriding(cx, &container)
}

/// Symbols the language makes use of by their declaration alone (typescript-eslint's
/// `eslintUsed`), and the names the rule does not check.
fn skipped(cx: &Cx, sym: &Symbol) -> bool {
    let first = &sym.decls[0];
    // The name of a function or class expression belongs to its own scope.
    if first
        .flags
        .intersects(Flags::UNCHECKED | Flags::ENUM_MEMBER | Flags::EXPRESSION_NAME)
    {
        return true;
    }
    let node = &cx.decl_node(first);
    let is_param = first.flags.contains(Flags::PARAMETER);
    if is_param && cx.lang.property_tokens().any(|m| cx.has_own(node, m)) {
        return true;
    }
    if cx.typed
        && (sym.decls.iter().any(|d| ambient(cx, d))
            || (sym.flags.contains(Flags::IMPORT)
                && cx.lang.markup_import(&sym.name, cx.root(), cx.file.grammar)))
    {
        return true;
    }
    // The parameters of signatures and of setters are not checked.
    if is_param && let Some(owner) = cx.param_list(&cx.binding(first)).and_then(|l| l.parent()) {
        let setter = owner.parent().is_some_and(|p| cx.has_own(&p, "set"));
        if setter || cx.in_signature(&cx.binding(first)) {
            return true;
        }
    }
    // `for (const a of b) return;`
    first
        .flags
        .intersects(Flags::VAR | Flags::LET | Flags::CONST)
        && returns_in_loop(cx, node)
}

/// Is the symbol exported (its declaration is, or it is a merged type that is)?
fn exported(cx: &Cx, sym: &Symbol) -> bool {
    sym.decls.iter().any(|d| {
        if d.flags.contains(Flags::PARAMETER) {
            return false;
        }
        let merged = d
            .flags
            .intersects(Flags::TYPE_ALIAS.union(Flags::INTERFACE))
            && sym.flags.contains(Flags::VALUE);
        cx.statement_of(d)
            .and_then(|a| a.parent())
            .is_some_and(|p| cx.tag(&p, "export"))
            && !merged
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
        let n = &cx.decl_node(d);
        if d.flags.contains(Flags::FUNCTION) && cx.is_function(n) {
            own.functions.push(cx.range(n));
        } else if d.flags.intersects(Flags::VAR | Flags::LET | Flags::CONST) {
            if let Some(init) = cx.slot(n, "init")
                && cx.is_function(&init)
            {
                own.functions.push(cx.range(&init));
            }
        } else if d
            .flags
            .intersects(Flags::INTERFACE | Flags::TYPE_ALIAS | Flags::NAMESPACE | Flags::ENUM)
        {
            own.others.push(cx.range(n));
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

fn variable<'s>(cx: &'s Cx, id: SymbolId) -> Var<'s> {
    let sym = cx.m.symbol(id);
    let decls: Vec<&Decl> = sym.decls.iter().collect();
    let mut refs: Vec<&Reference> = cx.m.refs_of(id).collect();
    let first = decls[0];
    if first.flags.contains(Flags::CLASS) && !first.flags.contains(Flags::EXPRESSION_NAME) {
        let class = cx.range(&cx.decl_node(first));
        refs.retain(|r| !class.contains_range(r.range));
    }
    Var { sym, decls, refs }
}

/// Is the variable read, as ESLint's `isUsedVariable` decides (typescript-eslint's version in
/// typed files; the two agree on untyped ones)?
fn is_used(cx: &Cx, v: &Var<'_>) -> bool {
    let own = own_spans(cx, &v.decls);
    let imported_as_type = v.decls.iter().all(|d| cx.is_type_import(d));
    let mut rhs: Option<TextRange> = None;
    for &r in &v.refs {
        let id = &cx.node_of(r);
        let span = cx.range(id);
        let for_itself = read_for_itself(cx, r, rhs);
        rhs = rhs_node(cx, r, v.sym, rhs);
        let inside = |spans: &[TextRange]| spans.iter().any(|s| s.contains_range(span));
        let type_only = !imported_as_type && in_type_query(r);
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
            && cx.is_foreach(&p)
            && only_returns(cx, cx.slot(&p, "body"))
        {
            return true;
        }
    }
    false
}

/// Do the parameters of the list after this one hold a used one?
fn after_last_used(cx: &Cx, sym: &Symbol, start: TextSize, list: &SyntaxNode) -> bool {
    for &oid in &cx.m.scope(sym.scope).symbols {
        let other = cx.m.symbol(oid);
        let first = &other.decls[0];
        if first.name.start() <= start || !first.flags.contains(Flags::PARAMETER) {
            continue;
        }
        if cx.param_list(&cx.binding(first)).as_ref() != Some(list) {
            continue;
        }
        let referenced = cx.m.refs_of(oid).any(|r| !in_type_query(r));
        if referenced || skipped(cx, other) || cx.has_default(&cx.binding(first)) {
            return false;
        }
    }
    true
}

/// Is the declaration a name beside a rest element (`const { a, ...rest } = o`)?
fn rest_sibling(cx: &Cx, d: &Decl) -> bool {
    cx.binding(d)
        .ancestors()
        .skip(1)
        .take_while(|a| !cx.tag(a, "declaration"))
        .any(|a| {
            a.children().any(|c| cx.tag(&c, "rest"))
                && a.children()
                    .any(|c| !cx.tag(&c, "rest") && c.text_range().contains_range(d.name))
        })
}

/// The findings of `unused-variable` (parameters: false) or `unused-parameter` (true).
pub(super) fn run(cx: &Cx, o: &Opts) -> Vec<Hit> {
    let mut hits = Vec::new();
    for id in cx.m.symbol_ids() {
        let sym = cx.m.symbol(id);
        let first = &sym.decls[0];
        if first.flags.contains(Flags::PARAMETER) != o.parameters {
            continue;
        }
        if (o.parameters && o.args == Args::None)
            || (first.flags.contains(Flags::CATCH) && !o.caught_errors)
            || o.ignore.as_ref().is_some_and(|re| re.is_match(&sym.name))
            || skipped(cx, sym)
            || exported(cx, sym)
        {
            continue;
        }
        let v = variable(cx, id);
        // A plain parameter only counts when no later one is used.
        if o.parameters
            && o.args == Args::AfterUsed
            && cx.tag(&cx.decl_node(first), "parameter")
            && cx.param_init(&cx.decl_node(first)).is_none()
            && cx
                .binding(first)
                .parent()
                .is_some_and(|p| cx.tag(&p, "parameter"))
            && let Some(list) = cx.param_list(&cx.binding(first))
            && !after_last_used(cx, sym, first.name.start(), &list)
        {
            continue;
        }
        if is_used(cx, &v) {
            continue;
        }
        if o.ignore_rest_siblings && v.decls.iter().all(|d| rest_sibling(cx, d)) {
            continue;
        }
        // `import { T } from "t"; type U = typeof T;` and `T` as a type only: not unused.
        let queried = v.refs.iter().any(|r| in_type_query(r));
        if queried && sym.flags.contains(Flags::IMPORT) {
            continue;
        }
        let home = cx.m.var_scope(sym.scope);
        let write = (v.refs.iter())
            .filter(|r| r.is_write() && cx.m.var_scope(r.scope) == home)
            .map(|r| r.range)
            .chain(
                v.decls
                    .iter()
                    .filter(|d| cx.writes_declaration(d))
                    .map(|d| d.name),
            )
            .max_by_key(|s| s.start());
        let assigned = write.is_some() || v.refs.iter().any(|r| r.is_write());
        hits.push(Hit {
            range: write.unwrap_or(first.name),
            name: sym.name.to_string(),
            action: if queried {
                "assigned a value but only used as a type"
            } else if assigned {
                "assigned a value but never used"
            } else {
                "defined but never used"
            },
        });
    }
    hits
}
