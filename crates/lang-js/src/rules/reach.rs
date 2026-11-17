//! Rules that ask the flow graph (`core_flow::build`, from the statement roles) where control can
//! go: `no-unreachable`, `getter-return` and `no-fallthrough`. A rule builds the graph of the unit
//! it is dispatched on (a function, the file, a `switch`).

use super::dupes::key_name;
use super::{Check, code_token, has_own, span};
use crate::generated::kind as k;
use core_flow::{Reached, build};
use core_rules::RuleContext;
use core_syntax::{Grammar, SyntaxKind, SyntaxNode, SyntaxToken, TextRange};

/// What `no-unreachable` is dispatched on: each has statements of its own to build.
pub(super) const UNITS: &[SyntaxKind] = &[
    k::PROGRAM,
    k::FUNCTION,
    k::ARROW_FUNCTION_EXPRESSION,
    k::STATIC_BLOCK,
    k::TS_MODULE_BLOCK,
];

/// The statements ESLint reports when unreachable; function declarations, `;` and `var` without a
/// value are left out (they hoist or do nothing).
fn is_target_kind(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        k::VARIABLE_DECLARATION
            | k::BLOCK_STATEMENT
            | k::BREAK_STATEMENT
            | k::CLASS
            | k::CONTINUE_STATEMENT
            | k::DEBUGGER_STATEMENT
            | k::DO_WHILE_STATEMENT
            | k::EXPRESSION_STATEMENT
            | k::FOR_IN_STATEMENT
            | k::FOR_OF_STATEMENT
            | k::FOR_STATEMENT
            | k::IF_STATEMENT
            | k::IMPORT_DECLARATION
            | k::LABELED_STATEMENT
            | k::RETURN_STATEMENT
            | k::SWITCH_STATEMENT
            | k::THROW_STATEMENT
            | k::TRY_STATEMENT
            | k::WHILE_STATEMENT
            | k::WITH_STATEMENT
            | k::EXPORT_NAMED_DECLARATION
            | k::EXPORT_DEFAULT_DECLARATION
            | k::EXPORT_ALL_DECLARATION
    )
}

fn is_target(n: &SyntaxNode, g: &Grammar) -> bool {
    let initialized = |d: SyntaxNode| {
        d.children_with_tokens()
            .any(|e| e.as_token().is_some_and(|t| t.text() == "="))
    };
    is_target_kind(n.kind())
        && (n.kind() != k::VARIABLE_DECLARATION
            || !has_own(n, g, "var")
            || n.children().any(initialized))
}

fn start(ctx: &RuleContext<'_>, n: &SyntaxNode) -> u32 {
    span(ctx, n).start().into()
}

fn walk(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>, reached: &Reached) {
    let g = ctx.file.grammar;
    let list = matches!(
        n.kind(),
        k::PROGRAM
            | k::BLOCK_STATEMENT
            | k::FUNCTION_BODY
            | k::SWITCH_CASE
            | k::STATIC_BLOCK
            | k::TS_MODULE_BLOCK
    );
    let mut reported = false;
    for child in n.children() {
        // units have their own graph
        if UNITS.contains(&child.kind()) {
            continue;
        }
        // a handler that no throw can reach
        if child.kind() == k::CATCH_CLAUSE
            && let Some(handler) = g.slot(&child, "body").and_then(|b| b.into_node())
            && reached.is_dead(start(ctx, &handler))
        {
            c.hit(ctx, span(ctx, &handler), "");
            continue;
        }
        let statement = list
            && (is_target_kind(child.kind())
                || matches!(child.kind(), k::EMPTY_STATEMENT | k::FUNCTION));
        if statement && reached.is_dead(start(ctx, &child)) {
            // a run of unreachable statements is one report; a statement that is not reported ends it
            let target = is_target(&child, g);
            if target && !reported {
                c.hit(ctx, span(ctx, &child), "");
            }
            reported = target;
            continue;
        }
        walk(c, &child, ctx, reached);
    }
}

/// `no-unreachable`
pub(super) fn unreachable(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    if let Some(flow) = build(ctx.file.grammar, ctx.file.text, n) {
        walk(c, n, ctx, &flow.reach());
    }
}

/// `return` statements of a function body, nested functions left out.
fn returns(n: &SyntaxNode, out: &mut Vec<SyntaxNode>) {
    for c in n.children() {
        if c.kind() == k::RETURN_STATEMENT {
            out.push(c.clone());
        }
        if !matches!(
            c.kind(),
            k::FUNCTION | k::ARROW_FUNCTION_EXPRESSION | k::CLASS
        ) {
            returns(&c, out);
        }
    }
}

/// A getter must return a value on every path.
pub(super) fn getter_return(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let g = ctx.file.grammar;
    if !has_own(n, g, "get") {
        return;
    }
    let Some(function) = n.children().find(|f| f.kind() == k::FUNCTION) else {
        return;
    };
    let Some(body) = function.children().find(|b| b.kind() == k::FUNCTION_BODY) else {
        return;
    };
    let key = n
        .children()
        .next()
        .and_then(|key| key_name(&key, ctx.file.text));
    let name = key.map_or_else(|| "getter".to_string(), |key| format!("getter '{key}'"));
    let mut found = Vec::new();
    returns(&body, &mut found);
    for r in found.iter().filter(|r| r.children().next().is_none()) {
        c.hit(ctx, span(ctx, r), &name);
    }
    if let Some(flow) = build(g, ctx.file.text, &function)
        && flow.reach().block[flow.end]
    {
        let start = span(ctx, n).start();
        let end = n
            .children()
            .next()
            .map_or(start, |key| span(ctx, &key).end());
        c.hit(ctx, TextRange::new(start, end), &name);
    }
}

/// The comment closest before `t` (only trivia between).
fn comment_before(t: &SyntaxToken, g: &Grammar) -> Option<String> {
    std::iter::successors(t.prev_token(), |p| p.prev_token())
        .take_while(|p| g.is_trivia(p.kind()))
        .find(|p| p.text().starts_with("//") || p.text().starts_with("/*"))
        .map(|p| p.text().to_string())
}

/// Does the comment say the fallthrough is meant (`falls through`, `fall through`, `fallthrough`)?
fn says_fallthrough(comment: &str) -> bool {
    let text = comment.to_ascii_lowercase();
    let directive = text
        .trim_start_matches(['/', '*', ' '])
        .starts_with("eslint");
    !directive
        && [
            "fallthrough",
            "fallsthrough",
            "fall through",
            "falls through",
        ]
        .iter()
        .any(|p| text.contains(p))
}

/// `no-fallthrough`: a `case` whose statements can reach the next one, without a comment that says so.
pub(super) fn fallthrough(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let g = ctx.file.grammar;
    let Some(flow) = build(g, ctx.file.text, n) else {
        return;
    };
    let reached = flow.reach();
    let cases: Vec<_> = n
        .children()
        .filter(|c| c.kind() == k::SWITCH_CASE)
        .collect();
    for pair in cases.windows(2) {
        let (case, next) = (&pair[0], &pair[1]);
        let skip = usize::from(!has_own(case, g, "default"));
        let body: Vec<_> = case.children().skip(skip).collect();
        let Some(next_token) = code_token(next, g) else {
            continue;
        };
        let gap = TextRange::new(span(ctx, case).end(), next_token.text_range().start());
        let blank = ctx.file.text[gap].matches('\n').count() >= 2;
        let at = start(ctx, case);
        let alive = flow
            .case_ends
            .iter()
            .any(|&(s, end)| s == at && reached.block[end]);
        if !alive || !(!body.is_empty() || blank) {
            continue;
        }
        let in_block = match body.as_slice() {
            [b] if b.kind() == k::BLOCK_STATEMENT => b
                .last_token()
                .and_then(|close| comment_before(&close, g))
                .is_some_and(|m| says_fallthrough(&m)),
            _ => false,
        };
        if !in_block && !comment_before(&next_token, g).is_some_and(|m| says_fallthrough(&m)) {
            let name = if has_own(next, g, "default") {
                "default"
            } else {
                "case"
            };
            c.hit(ctx, span(ctx, next), name);
        }
    }
}
