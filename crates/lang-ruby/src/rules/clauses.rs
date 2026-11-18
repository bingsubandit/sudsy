//! Checks on `rescue` clauses.

use super::tree::{child, span, tokens};
use crate::generated::kind as k;
use core_rules::RuleContext;
use core_rules::checks::NodeCheck;
use core_syntax::SyntaxNode;

/// A `when` with no body and no comment on its own lines.
pub fn empty_when(check: &NodeCheck, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let own = node
        .children_with_tokens()
        .skip_while(|e| e.kind() != k::KEYWORD)
        .any(|e| e.kind() == k::COMMENT);
    let next = node
        .next_sibling_or_token()
        .is_some_and(|n| match n.into_node() {
            Some(branch) => branch
                .children_with_tokens()
                .find(|e| !matches!(e.kind(), k::WHITESPACE | k::NEWLINE))
                .is_some_and(|f| f.kind() == k::COMMENT),
            None => false,
        });
    let commented = own || next;
    if child(node, k::STATEMENTS).is_none() && !commented {
        let range = span(ctx, node);
        check.hit(ctx, range, "");
    }
}

/// `begin ... end while cond`: it is `loop do ... break unless cond end`.
pub fn post_condition_loop(check: &NodeCheck, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let Some(body) = node
        .first_child()
        .filter(|b| b.kind() == k::STATEMENTS)
        .and_then(|b| b.children().next().filter(|s| s.kind() == k::BEGIN))
    else {
        return;
    };
    if tokens(&body).next().is_some_and(|t| t.text() == "begin")
        && let Some(keyword) = tokens(node).find(|t| t.kind() == k::KEYWORD)
    {
        check.hit(ctx, keyword.text_range(), keyword.text());
    }
}
