//! Checks on `rescue` clauses.
//!
//! concept: new clause-checks
//! what: Checks on case and rescue clauses and loop conditions.
//! tools: RuboCop Lint

use super::tree::span;
use crate::generated::kind as k;
use core_rules::RuleContext;
use core_rules::checks::NodeCheck;
use core_syntax::SyntaxNode;
use core_syntax::SyntaxNodeExt;
use core_syntax::roles::code_tokens;

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
    if node.child(k::STATEMENTS).is_none() && !commented {
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
    if code_tokens(&body)
        .next()
        .is_some_and(|t| t.text() == "begin")
        && let Some(keyword) = code_tokens(node).find(|t| t.kind() == k::KEYWORD)
    {
        check.hit(ctx, keyword.text_range(), keyword.text());
    }
}
