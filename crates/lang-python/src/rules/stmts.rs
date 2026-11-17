//! Checks that read the shape of statements and expressions only: layout (`E7`, `E402`),
//! comparisons, `try` clauses, jumps and `__future__` imports.

use super::Check;
use super::scope::run;
use super::tree::{callee, compare_ops, dotted, inner, is_statement, span, statements};
use crate::generated::kind as k;
use crate::names::is_builtin_class;
use crate::semantic::{Cx, model};
use crate::text::code_text;
use core_rules::RuleContext;
use core_semantics::scopes::Flags;
use core_syntax::{SyntaxElement, SyntaxNode, TextRange};

/// E402: an import of the module after code (a docstring, `__all__`-style assignments, blocks and
/// setup of `sys.path` or `os.environ` are not code).
pub(super) fn module_import_not_at_top(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let mut code = false;
    for (i, stmt) in statements(n).enumerate() {
        match stmt.kind() {
            k::STMT_IMPORT | k::STMT_IMPORT_FROM => {
                if code {
                    c.hit(ctx, span(ctx, &stmt), "");
                }
            }
            k::STMT_EXPR
                if i == 0
                    && stmt
                        .children()
                        .next()
                        .is_some_and(|e| e.kind() == k::EXPR_STRING_LITERAL) => {}
            k::STMT_IF | k::STMT_TRY | k::STMT_WITH | k::STMT_MATCH => {}
            _ => code |= !(is_dunder_assignment(&stmt) || is_setup(&stmt)),
        }
    }
}

fn is_dunder(e: &SyntaxNode) -> bool {
    e.kind() == k::EXPR_NAME && {
        let t = code_text(e);
        t.starts_with("__") && t.ends_with("__")
    }
}

fn is_dunder_assignment(stmt: &SyntaxNode) -> bool {
    let parts: Vec<SyntaxNode> = stmt.children().collect();
    match stmt.kind() {
        k::STMT_ASSIGN => parts.len() == 2 && is_dunder(&parts[0]),
        k::STMT_ANN_ASSIGN => parts.first().is_some_and(is_dunder),
        _ => false,
    }
}

/// A statement that sets the module up before the imports (`sys.path.insert(...)`,
/// `os.environ[...] = ...`, `matplotlib.use(...)`, `pytest.importorskip(...)`).
fn is_setup(stmt: &SyntaxNode) -> bool {
    let call_path = |e: &SyntaxNode| {
        let e = inner(e);
        (e.kind() == k::EXPR_CALL)
            .then(|| callee(&e).and_then(|f| dotted(&f)))
            .flatten()
    };
    match stmt.kind() {
        k::STMT_EXPR => stmt
            .children()
            .next()
            .and_then(|e| call_path(&e))
            .is_some_and(|p| {
                p.starts_with("sys.path.")
                    || p.starts_with("os.environ.")
                    || p == "matplotlib.use"
                    || p == "pytest.importorskip"
            }),
        k::STMT_ASSIGN => {
            let parts: Vec<SyntaxNode> = stmt.children().collect();
            let Some((value, targets)) = parts.split_last() else {
                return false;
            };
            call_path(value).is_some_and(|p| p == "pytest.importorskip")
                || targets.iter().any(|t| {
                    t.kind() == k::EXPR_SUBSCRIPT
                        && t.children()
                            .next()
                            .and_then(|o| dotted(&o))
                            .is_some_and(|p| p == "os.environ")
                })
        }
        _ => false,
    }
}

/// The nodes whose header ends with a colon that a body may follow on the same line.
pub(super) const COMPOUND: &[core_syntax::SyntaxKind] = &[
    k::STMT_FUNCTION_DEF,
    k::STMT_CLASS_DEF,
    k::STMT_FOR,
    k::STMT_WHILE,
    k::STMT_IF,
    k::ELIF_ELSE_CLAUSE,
    k::STMT_WITH,
    k::STMT_TRY,
    k::EXCEPT_HANDLER_EXCEPT_HANDLER,
    k::MATCH_CASE,
];

/// E701: the body of a compound statement starts on the line of its colon, unless it is `...`.
pub(super) fn colon_statement(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    for colon in n
        .children_with_tokens()
        .filter_map(|e| e.into_token())
        .filter(|t| t.kind() == k::COLON)
    {
        let mut next = colon.next_sibling_or_token();
        while let Some(SyntaxElement::Token(t)) = &next
            && matches!(t.kind(), k::WHITESPACE | k::COMMENT | k::CONTINUATION)
        {
            next = next.and_then(|e| e.next_sibling_or_token());
        }
        let Some(first) = next
            .and_then(|e| e.into_node())
            .filter(|s| is_statement(s.kind()))
        else {
            continue;
        };
        let first = &first;
        let lone_ellipsis = first.kind() == k::STMT_EXPR
            && first
                .children()
                .next()
                .is_some_and(|e| e.kind() == k::EXPR_ELLIPSIS_LITERAL)
            && first.next_sibling().is_none_or(|s| !is_statement(s.kind()));
        if lone_ellipsis {
            continue;
        }
        let between = TextRange::new(colon.text_range().end(), span(ctx, first).start());
        if !ctx.file.text[between].contains(['\n', '\r']) {
            c.hit(ctx, colon.text_range(), "");
        }
    }
}

/// E702 and E703: a semicolon between statements and one that ends a line. Each statement looks
/// at the token after it.
fn semicolon(c: &Check, stmt: &SyntaxNode, ctx: &mut RuleContext<'_>, useless: bool) {
    let mut next = stmt.next_sibling_or_token();
    while let Some(e) = &next {
        match e {
            SyntaxElement::Token(t)
                if matches!(t.kind(), k::WHITESPACE | k::COMMENT | k::CONTINUATION) =>
            {
                next = e.next_sibling_or_token();
            }
            _ => break,
        }
    }
    let Some(SyntaxElement::Token(semi)) = next else {
        return;
    };
    if semi.kind() != k::SEMI {
        return;
    }
    let mut after = semi.next_sibling_or_token();
    let mut ends = true;
    while let Some(e) = after {
        match &e {
            SyntaxElement::Token(t)
                if matches!(t.kind(), k::WHITESPACE | k::COMMENT | k::CONTINUATION) =>
            {
                after = e.next_sibling_or_token();
            }
            SyntaxElement::Token(t) if matches!(t.kind(), k::NEWLINE | k::END_OF_FILE) => break,
            SyntaxElement::Node(_) => {
                ends = false;
                break;
            }
            SyntaxElement::Token(_) => break,
        }
    }
    // The statement after the semicolon starts with whitespace trivia in its own node.
    if ends && let Some(SyntaxElement::Node(_)) = semi.next_sibling_or_token() {
        ends = false;
    }
    if ends == useless {
        c.hit(ctx, semi.text_range(), "");
    }
}

pub(super) fn useless_semicolon(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    semicolon(c, n, ctx, true);
}

pub(super) fn statement_semicolon(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    semicolon(c, n, ctx, false);
}

fn ambiguous(c: &Check, ctx: &mut RuleContext<'_>, class: fn(Flags) -> bool) {
    run(c, ctx, |cx| {
        let mut hits = Vec::new();
        for id in cx.s.symbol_ids() {
            let sym = cx.s.symbol(id);
            if !matches!(&*sym.name, "l" | "O" | "I") {
                continue;
            }
            for d in sym.decls.iter().filter(|d| class(d.flags)) {
                hits.push((d.name, sym.name.to_string()));
            }
        }
        hits
    });
}

/// E741: `l`, `O` and `I` look like digits.
pub(super) fn ambiguous_variable(c: &Check, _: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    ambiguous(c, ctx, |f| {
        !f.intersects(Flags::FUNCTION | Flags::CLASS | Flags::IMPORT | Flags::TYPE_PARAMETER)
    });
}

/// E742
pub(super) fn ambiguous_class(c: &Check, _: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    ambiguous(c, ctx, |f| f.contains(Flags::CLASS));
}

/// E743
pub(super) fn ambiguous_function(c: &Check, _: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    ambiguous(c, ctx, |f| f.contains(Flags::FUNCTION));
}

/// A string, bytes or number literal, a collection or comprehension (not a generator), or a
/// tuple of constants: what `is` never matches by identity.
fn is_fresh(e: &SyntaxNode) -> bool {
    fn literal(e: &SyntaxNode) -> bool {
        matches!(
            e.kind(),
            k::EXPR_STRING_LITERAL
                | k::EXPR_BYTES_LITERAL
                | k::EXPR_NUMBER_LITERAL
                | k::EXPR_BOOLEAN_LITERAL
                | k::EXPR_NONE_LITERAL
                | k::EXPR_ELLIPSIS_LITERAL
        )
    }
    fn constant(e: &SyntaxNode) -> bool {
        let e = inner(e);
        match e.kind() {
            k::EXPR_TUPLE => e.children().all(|c| constant(&c)),
            _ => literal(&e),
        }
    }
    let e = inner(e);
    match e.kind() {
        k::EXPR_LIST
        | k::EXPR_LIST_COMP
        | k::EXPR_SET
        | k::EXPR_SET_COMP
        | k::EXPR_DICT
        | k::EXPR_DICT_COMP => true,
        k::EXPR_TUPLE => constant(&e),
        k::EXPR_NONE_LITERAL | k::EXPR_BOOLEAN_LITERAL | k::EXPR_ELLIPSIS_LITERAL => false,
        _ => literal(&e),
    }
}

/// The operator `ops` (the tokens between two operands) of a comparison.
#[derive(PartialEq, Eq, Clone, Copy)]
enum Cmp {
    Eq,
    NotEq,
    Is,
    IsNot,
    In,
    NotIn,
    Other,
}

fn cmp(ops: &[core_syntax::SyntaxKind]) -> Cmp {
    match ops {
        [k::EQ_EQUAL] => Cmp::Eq,
        [k::NOT_EQUAL] => Cmp::NotEq,
        [k::IS] => Cmp::Is,
        [k::IS, k::NOT] => Cmp::IsNot,
        [k::IN] => Cmp::In,
        [k::NOT, k::IN] => Cmp::NotIn,
        _ => Cmp::Other,
    }
}

/// F632: `is` with a literal or a fresh collection.
pub(super) fn is_literal(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    for (l, op, r) in compare_ops(n) {
        if matches!(cmp(&op), Cmp::Is | Cmp::IsNot) && (is_fresh(&l) || is_fresh(&r)) {
            let range = TextRange::new(span(ctx, &l).start(), span(ctx, &r).end());
            c.hit(ctx, range, "");
        }
    }
}

/// Is the expression a builtin class, or `type(x)`?
fn is_type(cx: &Cx, e: &SyntaxNode) -> bool {
    let e = inner(e);
    match e.kind() {
        k::EXPR_NAME => cx
            .qualified(&e)
            .is_some_and(|q| is_builtin_class(&q) && !q.contains('.')),
        k::EXPR_CALL => callee(&e).is_some_and(|f| {
            f.kind() == k::EXPR_NAME && cx.qualified(&f).is_some_and(|q| q == "type")
        }),
        _ => false,
    }
}

/// E721: a type compared with `==`.
pub(super) fn type_comparison(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let Some(cx) = model(ctx.file) else { return };
    let types = compare_ops(n).iter().any(|(l, op, r)| {
        matches!(cmp(op), Cmp::Eq | Cmp::NotEq) && (is_type(&cx, l) || is_type(&cx, r))
    });
    if types {
        c.hit(ctx, span(ctx, n), "");
    }
}

/// F541: an f-string with no replacement field.
pub(super) fn f_string_missing_placeholders(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let parts: Vec<SyntaxNode> = n.children().filter(|p| p.kind() == k::F_STRING).collect();
    if parts
        .iter()
        .all(|p| p.children().all(|e| e.kind() != k::INTERPOLATED_ELEMENT))
    {
        for p in parts {
            c.hit(ctx, span(ctx, &p), "");
        }
    }
}

/// E722 and F707 for the clauses of a `try`: a bare `except` that does not re-raise, and one that
/// is not the last.
fn bare_handlers(n: &SyntaxNode, cx: &Cx) -> Vec<(SyntaxNode, bool)> {
    let handlers: Vec<SyntaxNode> = n
        .children()
        .filter(|h| h.kind() == k::EXCEPT_HANDLER_EXCEPT_HANDLER)
        .collect();
    let last = handlers.len().saturating_sub(1);
    handlers
        .into_iter()
        .enumerate()
        .filter(|(_, h)| cx.slot(h, "type").is_none())
        .map(|(i, h)| (h, i < last))
        .collect()
}

pub(super) fn bare_except(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let Some(cx) = model(ctx.file) else { return };
    for (h, _) in bare_handlers(n, &cx) {
        let reraises =
            statements(&h).any(|s| s.kind() == k::STMT_RAISE && s.children().next().is_none());
        if !reraises
            && let Some(t) = h
                .children_with_tokens()
                .filter_map(|e| e.into_token())
                .find(|t| t.kind() == k::EXCEPT)
        {
            c.hit(ctx, t.text_range(), "");
        }
    }
}

pub(super) fn default_except_not_last(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let Some(cx) = model(ctx.file) else { return };
    for (h, not_last) in bare_handlers(n, &cx) {
        if not_last {
            c.hit(ctx, span(ctx, &h), "");
        }
    }
}

/// E731: a name assigned a lambda.
pub(super) fn lambda_assignment(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let parts: Vec<SyntaxNode> = n.children().collect();
    let is_lambda = |v: Option<&SyntaxNode>| v.is_some_and(|v| inner(v).kind() == k::EXPR_LAMBDA);
    let hit = match n.kind() {
        k::STMT_ASSIGN => {
            parts.len() == 2 && parts[0].kind() == k::EXPR_NAME && is_lambda(parts.last())
        }
        _ => {
            let in_class = n
                .ancestors()
                .skip(1)
                .find(|a| {
                    matches!(
                        a.kind(),
                        k::STMT_FUNCTION_DEF | k::STMT_CLASS_DEF | k::EXPR_LAMBDA | k::MOD_MODULE
                    )
                })
                .is_some_and(|a| a.kind() == k::STMT_CLASS_DEF);
            // `x: T = lambda: ...`: the target, the annotation, the value.
            parts.len() == 3
                && parts[0].kind() == k::EXPR_NAME
                && is_lambda(parts.last())
                && !in_class
        }
    };
    if hit {
        c.hit(ctx, span(ctx, n), "");
    }
}

/// F633: `print >> f`.
pub(super) fn invalid_print_syntax(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let Some(cx) = model(ctx.file) else { return };
    let mut parts = n.children();
    let (Some(left), Some(_)) = (parts.next(), parts.next()) else {
        return;
    };
    let shift = super::tree::tokens(n).any(|t| t.kind() == k::RIGHT_SHIFT);
    if shift && left.kind() == k::EXPR_NAME && cx.qualified(&left).is_some_and(|q| q == "print") {
        c.hit(ctx, span(ctx, n), "");
    }
}

/// F622: two starred names in an assignment target.
pub(super) fn multiple_starred(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    fn count(t: &SyntaxNode) -> usize {
        let t = inner(t);
        match t.kind() {
            k::EXPR_TUPLE | k::EXPR_LIST => t
                .children()
                .filter(|e| inner(e).kind() == k::EXPR_STARRED)
                .count(),
            _ => 0,
        }
    }
    let targets: Vec<SyntaxNode> = match n.kind() {
        k::STMT_ASSIGN => {
            let all: Vec<SyntaxNode> = n.children().collect();
            all[..all.len().saturating_sub(1)].to_vec()
        }
        _ => n.children().take(1).collect(),
    };
    for t in targets {
        if count(&t) > 1 {
            c.hit(ctx, span(ctx, &t), "");
        }
    }
}

/// The nearest enclosing scope-opening node, and the child of it that holds `n`.
fn enclosing(n: &SyntaxNode) -> Option<(SyntaxNode, SyntaxNode)> {
    let mut child = n.clone();
    for a in n.ancestors().skip(1) {
        if matches!(
            a.kind(),
            k::STMT_FUNCTION_DEF
                | k::STMT_CLASS_DEF
                | k::EXPR_LAMBDA
                | k::MOD_MODULE
                | k::EXPR_LIST_COMP
                | k::EXPR_SET_COMP
                | k::EXPR_DICT_COMP
                | k::EXPR_GENERATOR
        ) {
            return Some((a, child));
        }
        child = a;
    }
    None
}

/// F701 and F702: `break` and `continue` outside a loop's body.
fn outside_loop(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let mut child = n.clone();
    for a in n.ancestors().skip(1) {
        match a.kind() {
            k::STMT_FOR | k::STMT_WHILE => {
                let in_else = a
                    .children_with_tokens()
                    .filter_map(|e| e.into_token())
                    .find(|t| t.kind() == k::ELSE)
                    .is_some_and(|e| child.text_range().start() >= e.text_range().end());
                if is_statement(child.kind()) && !in_else {
                    return;
                }
            }
            k::STMT_FUNCTION_DEF | k::STMT_CLASS_DEF | k::EXPR_LAMBDA | k::MOD_MODULE => break,
            _ => {}
        }
        child = a;
    }
    c.hit(ctx, span(ctx, n), "");
}

pub(super) fn break_outside_loop(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    outside_loop(c, n, ctx);
}

pub(super) fn continue_outside_loop(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    outside_loop(c, n, ctx);
}

/// F706: `return` outside a function.
pub(super) fn return_outside_function(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    if enclosing(n).is_none_or(|(a, _)| a.kind() != k::STMT_FUNCTION_DEF) {
        c.hit(ctx, span(ctx, n), "");
    }
}

/// F704: `yield`, `yield from` or `await` outside a function.
pub(super) fn yield_outside_function(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let comprehension = |a: &SyntaxNode| {
        matches!(
            a.kind(),
            k::EXPR_LIST_COMP | k::EXPR_SET_COMP | k::EXPR_DICT_COMP | k::EXPR_GENERATOR
        )
    };
    let is_await = n.kind() == k::EXPR_AWAIT;
    let scope = n.ancestors().skip(1).find(|a| {
        matches!(
            a.kind(),
            k::STMT_FUNCTION_DEF | k::STMT_CLASS_DEF | k::EXPR_LAMBDA | k::MOD_MODULE
        ) || (!is_await && comprehension(a))
    });
    if !scope.is_some_and(|a| matches!(a.kind(), k::STMT_FUNCTION_DEF | k::EXPR_LAMBDA)) {
        c.hit(ctx, span(ctx, n), "");
    }
}

const FUTURE: &[&str] = &[
    "nested_scopes",
    "generators",
    "division",
    "absolute_import",
    "with_statement",
    "print_function",
    "unicode_literals",
    "barry_as_FLUFL",
    "generator_stop",
    "annotations",
];

fn is_future(cx: &Cx, stmt: &SyntaxNode) -> bool {
    stmt.kind() == k::STMT_IMPORT_FROM
        && cx
            .slot(stmt, "module")
            .is_some_and(|m| code_text(&m) == "__future__")
        && !stmt
            .children_with_tokens()
            .filter_map(|e| e.into_token())
            .any(|t| t.kind() == k::DOT)
}

/// F404: a `from __future__` import after other statements.
pub(super) fn late_future_import(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let Some(cx) = model(ctx.file) else { return };
    if !is_future(&cx, n) {
        return;
    }
    let late = match n.parent() {
        Some(p) if p.kind() == k::MOD_MODULE => statements(&p)
            .take_while(|s| s != n)
            .enumerate()
            .any(|(i, s)| {
                !(is_future(&cx, &s)
                    || i == 0
                        && s.kind() == k::STMT_EXPR
                        && s.children()
                            .next()
                            .is_some_and(|e| e.kind() == k::EXPR_STRING_LITERAL))
            }),
        _ => true,
    };
    if late {
        c.hit(ctx, span(ctx, n), "");
    }
}

/// F407: a name imported from `__future__` that the module does not have.
pub(super) fn future_feature_not_defined(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let Some(cx) = model(ctx.file) else { return };
    if !is_future(&cx, n) {
        return;
    }
    for alias in n.children().filter(|a| a.kind() == k::ALIAS) {
        let Some(name) = cx.slot(&alias, "name").map(|x| code_text(&x)) else {
            continue;
        };
        if !FUTURE.contains(&name.as_str()) && name != "*" {
            c.hit(ctx, span(ctx, &alias), &name);
        }
    }
}

/// The syntax errors CPython finds when it compiles a file the grammar accepts: a duplicate
/// parameter, `nonlocal` at module level, a repeated keyword argument, `await` in a function that
/// is not async. The message is the error's.
pub(super) fn invalid_syntax(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    match n.kind() {
        k::PARAMETERS => {
            let mut seen = std::collections::HashSet::new();
            for p in n.descendants().filter(|p| p.kind() == k::PARAMETER) {
                let name = p
                    .children()
                    .find(|i| i.kind() == k::IDENTIFIER)
                    .map(|i| code_text(&i))
                    .unwrap_or_default();
                if !seen.insert(name.clone()) {
                    c.hit(
                        ctx,
                        span(ctx, &p),
                        &format!("Duplicate parameter \"{name}\""),
                    );
                }
            }
        }
        k::ARGUMENTS => {
            let mut seen = std::collections::HashSet::new();
            for kw in n.children().filter(|kw| kw.kind() == k::KEYWORD) {
                if let (Some(name), _) = super::tree::keyword(&kw)
                    && !seen.insert(name.clone())
                {
                    c.hit(
                        ctx,
                        span(ctx, &kw),
                        &format!("Duplicate keyword argument `{name}`"),
                    );
                }
            }
        }
        k::STMT_NONLOCAL => {
            let in_module = n
                .ancestors()
                .skip(1)
                .find(|a| {
                    matches!(
                        a.kind(),
                        k::STMT_FUNCTION_DEF | k::STMT_CLASS_DEF | k::EXPR_LAMBDA | k::MOD_MODULE
                    )
                })
                .is_some_and(|a| a.kind() == k::MOD_MODULE);
            if in_module {
                c.hit(
                    ctx,
                    span(ctx, n),
                    "nonlocal declaration not allowed at module level",
                );
            }
        }
        _ => {
            let sync = n
                .ancestors()
                .skip(1)
                .find(|a| matches!(a.kind(), k::STMT_FUNCTION_DEF | k::EXPR_LAMBDA))
                .is_none_or(|f| {
                    f.kind() == k::EXPR_LAMBDA
                        || !super::tree::tokens(&f).any(|t| t.kind() == k::ASYNC)
                });
            if sync {
                c.hit(
                    ctx,
                    span(ctx, n),
                    "`await` outside of an asynchronous function",
                );
            }
        }
    }
}
