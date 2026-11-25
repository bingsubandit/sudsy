//! The Rust rules of SQL; rules that fit a pattern are files in `rules/`.
//!
//! concept: new null-comparison, new distinct-group-by, new unused-cte, new self-alias
//! what: null-comparison: `= NULL`, `<> NULL` or `!= NULL`, which is never true: use `IS NULL` or `IS NOT NULL` (sqlfluff CV05)
//! what: distinct-group-by: `SELECT DISTINCT` in a query that has `GROUP BY`: the grouping already makes the rows distinct (sqlfluff AM01)
//! what: unused-cte: A common table expression that nothing in the `WITH` statement refers to (sqlfluff ST03)
//! what: self-alias: An alias that repeats the name it is given to, `a AS a` (sqlfluff AL09)

use crate::generated::kind as k;
use core_rules::{Rule, RuleContext, rule};
use core_syntax::{SyntaxElement, SyntaxNode, SyntaxNodeExt};

pub static RULES: &[&dyn Rule] = &[&NullComparison, &DistinctGroupBy, &UnusedCte, &SelfAlias];

fn is_trivia(kind: core_syntax::SyntaxKind) -> bool {
    matches!(
        kind,
        k::WHITESPACE | k::NEWLINE | k::LINE_COMMENT | k::BLOCK_COMMENT
    ) || kind == k::INDENT
        || kind == k::IMPLICIT_INDENT
        || kind == k::DEDENT
}

/// The text of a name with its quotes and case removed.
fn name(text: &str) -> String {
    text.trim_matches(['"', '`', '[', ']']).to_lowercase()
}

fn is_null(e: &SyntaxElement) -> bool {
    matches!(e, SyntaxElement::Token(t) if t.kind() == k::WORD_LITERAL && t.text().eq_ignore_ascii_case("null"))
}

rule! {
    pub NullComparison {
        id: "null-comparison",
        category: Correctness,
        default: Warn,
        description: "Reports `= NULL`, `<> NULL` and `!= NULL`.",
        why: "A comparison with `NULL` gives `NULL`, not true or false, so the condition is never \
                  true and the query returns no rows (or, with `<>`, none of the rows that it should).\n\
                  To fix, write `IS NULL` or `IS NOT NULL`.",
        example: "select a from t where a = null",
        kinds: &[k::COMPARISON_OPERATOR],
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let text = node.text().to_string();
        if !matches!(text.trim(), "=" | "<>" | "!=") {
            return;
        }
        let before = std::iter::successors(node.prev_sibling_or_token(), |e| e.prev_sibling_or_token())
            .find(|e| !is_trivia(e.kind()));
        let after = std::iter::successors(node.next_sibling_or_token(), |e| e.next_sibling_or_token())
            .find(|e| !is_trivia(e.kind()));
        if before.is_some_and(|e| is_null(&e)) || after.is_some_and(|e| is_null(&e)) {
            ctx.report(node.text_range(), "compare with NULL using IS NULL or IS NOT NULL");
        }
    }
}

rule! {
    pub DistinctGroupBy {
        id: "distinct-group-by",
        category: Suspicious,
        default: Warn,
        description: "Reports `SELECT DISTINCT` in a query that has `GROUP BY`.",
        why: "When the select list has all the grouping columns, the rows are distinct already and \
                  `DISTINCT` does nothing. When it leaves one out (`SELECT DISTINCT a ... GROUP BY a, \
                  b`), `DISTINCT` does change the result, and the grouping by the other column is \
                  probably a mistake.\n\
                  To fix, remove `DISTINCT`, or group by the selected columns only.",
        example: "select distinct a from t group by a",
        kinds: &[k::SELECT_STATEMENT],
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        if node.child(k::GROUPBY_CLAUSE).is_none() {
            return;
        }
        let modifier = node
            .child(k::SELECT_CLAUSE)
            .and_then(|c| c.child(k::SELECT_CLAUSE_MODIFIER))
            .filter(|m| m.text().to_string().trim().eq_ignore_ascii_case("distinct"));
        if let Some(m) = modifier {
            ctx.report(m.text_range(), "DISTINCT is redundant with GROUP BY");
        }
    }
}

rule! {
    pub UnusedCte {
        id: "unused-cte",
        category: Suspicious,
        default: Warn,
        description: "Reports a common table expression that nothing in the `WITH` statement refers to.",
        why: "The expression is defined and never used, so the reader has to read the whole \
                  statement to find out that it does not matter.\n\
                  To fix, remove it, or use it.",
        example: "with a as (select 1) select 2",
        kinds: &[k::WITH_COMPOUND_STATEMENT],
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let ctes: Vec<_> = node.children().filter(|c| c.kind() == k::COMMON_TABLE_EXPRESSION).collect();
        for cte in &ctes {
            let Some(first) = cte
                .children_with_tokens()
                .filter_map(|e| e.into_token())
                .find(|t| !is_trivia(t.kind()))
            else {
                continue;
            };
            let wanted = name(first.text());
            let used = node.descendants_with_tokens().filter_map(|e| e.into_token()).any(|t| {
                t.text_range() != first.text_range()
                    && matches!(t.kind(), k::IDENTIFIER | k::QUOTED_IDENTIFIER)
                    && name(t.text()) == wanted
            });
            if !used {
                ctx.report(first.text_range(), "this common table expression is never used");
            }
        }
    }
}

rule! {
    pub SelfAlias {
        id: "self-alias",
        category: Cleanup,
        default: Warn,
        description: "Reports an alias that repeats the name it is given to, as in `a AS a`.",
        why: "The alias does nothing: the name of the column is already `a`.\n\
                  To fix, remove the alias.",
        example: "select a as a from t",
        kinds: &[k::ALIAS_EXPRESSION],
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let Some(alias) = node
            .children_with_tokens()
            .filter_map(|e| e.into_token())
            .filter(|t| t.kind() == k::IDENTIFIER)
            .last()
        else {
            return;
        };
        let Some(target) = node.prev_sibling() else { return };
        let mut words = target
            .descendants_with_tokens()
            .filter_map(|e| e.into_token())
            .filter(|t| !is_trivia(t.kind()));
        let (first, rest) = (words.next(), words.next());
        if rest.is_none() && first.is_some_and(|t| t.kind() == k::IDENTIFIER && t.text() == alias.text()) {
            ctx.report(alias.text_range(), "the alias is the same as the name");
        }
    }
}
