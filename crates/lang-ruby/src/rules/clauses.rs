//! Checks on `rescue` clauses.

use super::tree::{child, span, tokens};
use crate::generated::kind as k;
use core_rules::RuleContext;
use core_rules::checks::NodeCheck;
use core_syntax::SyntaxNode;

/// `rescue Exception`: it also catches `SystemExit` and `NoMemoryError`.
pub fn rescue_exception(check: &NodeCheck, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let hit = node
        .children()
        .filter(|c| matches!(c.kind(), k::CONSTANT_READ | k::CONSTANT_PATH))
        .any(|c| c.text().to_string().trim().trim_start_matches("::") == "Exception");
    if hit {
        let range = span(ctx, node);
        check.hit(ctx, range, "");
    }
}

/// A `rescue` with no statements, unless a comment follows it before the end of the `begin`, method
/// or block.
pub fn suppressed_exception(check: &NodeCheck, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    if child(node, k::STATEMENTS).is_some() {
        return;
    }
    let owner = node.ancestors().skip(1).find(|a| match a.kind() {
        k::DEF | k::BLOCK | k::LAMBDA => true,
        k::BEGIN => tokens(a).next().is_some_and(|t| t.text() == "begin"),
        _ => false,
    });
    let range = span(ctx, node);
    if let Some(owner) = owner {
        let lines = ctx.file.lines;
        let from = lines.line(range.start()) + 1;
        let to = lines.line(span(ctx, &owner).end());
        let commented = (from..=to).any(|line| {
            let (start, end) = lines.line_range(ctx.file.text, line);
            ctx.file.text[start..end].trim_start().starts_with('#')
        });
        if commented {
            return;
        }
    }
    check.hit(ctx, range, "");
}

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
