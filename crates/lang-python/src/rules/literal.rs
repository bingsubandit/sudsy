//! Checks of ruff's rules about collections of string literals (flake8-implicit-str-concat, `__all__`
//! and `__slots__` that are not sorted).
//!
//! concept: sorted-array-values, new implicit-string-concat
//! what: Implicit concatenation of adjacent string literals.
//! tools: ruff ISC

use super::Check;
use super::tree::span;
use crate::generated::kind as k;
use crate::text::string_value;
use core_rules::RuleContext;
use core_syntax::SyntaxNode;
use std::cmp::Ordering;

/// ISC004: an element of a list, tuple or set that is several string literals written one after
/// the other, with no parentheses to say they belong together.
pub(super) fn implicit_concat_in_collection(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    for e in n.children() {
        if matches!(
            e.kind(),
            k::EXPR_STRING_LITERAL | k::EXPR_BYTES_LITERAL | k::EXPR_F_STRING
        ) && e.children().count() > 1
        {
            c.hit(ctx, span(ctx, &e), "");
        }
    }
}

/// The order of `__all__`: constants, then classes, then the rest.
fn member_rank(name: &str) -> u8 {
    let upper = name.chars().any(|c| c.is_ascii_uppercase());
    if name.len() > 1 && upper && !name.chars().any(|c| c.is_ascii_lowercase()) {
        0
    } else if name.starts_with(|c: char| c.is_ascii_uppercase()) {
        1
    } else {
        2
    }
}

/// The names of a list or tuple of plain strings, when it is one.
fn plain_strings(value: &SyntaxNode) -> Option<Vec<String>> {
    if !matches!(value.kind(), k::EXPR_LIST | k::EXPR_TUPLE) {
        return None;
    }
    value
        .children()
        .map(|e| {
            (e.kind() == k::EXPR_STRING_LITERAL && e.children().count() == 1)
                .then(|| string_value(&e))
        })
        .collect()
}

fn unsorted(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>, target: &str, ranked: bool) {
    let (Some(left), Some(value)) = (n.children().next(), n.children().last()) else {
        return;
    };
    if left.kind() != k::EXPR_NAME || crate::text::code_text(&left) != target {
        return;
    }
    let Some(names) = plain_strings(&value) else {
        return;
    };
    let key = |a: &String, b: &String| {
        let rank = |s: &str| if ranked { member_rank(s) } else { 0 };
        rank(a).cmp(&rank(b)).then_with(|| core_rules::sorting::natural(a, b))
    };
    if names
        .windows(2)
        .any(|w| key(&w[0], &w[1]) == Ordering::Greater)
    {
        let class = n
            .parent()
            .filter(|p| p.kind() == k::STMT_CLASS_DEF)
            .and_then(|p| p.children().find(|i| i.kind() == k::IDENTIFIER))
            .map(|i| format!("{}.", crate::text::code_text(&i)));
        c.hit(
            ctx,
            span(ctx, &value),
            &format!("{}{target}", class.unwrap_or_default()),
        );
    }
}

/// RUF022: `__all__` out of order.
pub(super) fn unsorted_dunder_all(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    unsorted(c, n, ctx, "__all__", true);
}

/// RUF023: `__slots__` out of order.
pub(super) fn unsorted_dunder_slots(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    unsorted(c, n, ctx, "__slots__", false);
}
