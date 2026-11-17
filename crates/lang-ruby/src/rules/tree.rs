//! Reading the parts of Prism's nodes the rules look at. A call is `object? dot? name open?
//! arguments? block-pass? close? block?`: the arguments are one `Arguments` node, a `do` or
//! brace block one `Block` node, parentheses are own tokens.

use crate::generated::kind as k;
use core_rules::RuleContext;
use core_rules::tree::significant_range;
use core_syntax::{SyntaxKind, SyntaxNode, SyntaxToken, TextRange};

/// The first child node of `kind`.
pub fn child(n: &SyntaxNode, kind: SyntaxKind) -> Option<SyntaxNode> {
    n.children().find(|c| c.kind() == kind)
}

/// The own tokens of a node that are code.
pub fn tokens(n: &SyntaxNode) -> impl Iterator<Item = SyntaxToken> {
    n.children_with_tokens()
        .filter_map(|e| e.into_token())
        .filter(|t| !matches!(t.kind(), k::WHITESPACE | k::NEWLINE | k::COMMENT))
}

/// Whether `n` has an own token of `kind`.
pub fn has_token(n: &SyntaxNode, kind: SyntaxKind) -> bool {
    n.children_with_tokens()
        .filter_map(|e| e.into_token())
        .any(|t| t.kind() == kind)
}

/// The name of a call: its first own name or operator token (`None` for `a[1]`).
pub fn method_name(call: &SyntaxNode) -> Option<SyntaxToken> {
    tokens(call).find(|t| matches!(t.kind(), k::IDENTIFIER | k::CONSTANT | k::OPERATOR))
}

/// The last child node of `n`.
pub fn last_node(n: &SyntaxNode) -> Option<SyntaxNode> {
    n.children().last()
}

/// The span of `n` without the trivia around it.
pub fn span(ctx: &RuleContext<'_>, n: &SyntaxNode) -> TextRange {
    significant_range(n, ctx.file.grammar)
}
