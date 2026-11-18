//! Reading the parts of Prism's nodes the rules look at. A call is `object? dot? name open?
//! arguments? block-pass? close? block?`: the arguments are one `Arguments` node, a `do` or
//! brace block one `Block` node, parentheses are own tokens.
//!
//! concept: role-accessors

use crate::generated::kind as k;
use core_rules::RuleContext;
use core_rules::tree::significant_range;
use core_syntax::roles::code_tokens;
use core_syntax::{SyntaxNode, SyntaxToken, TextRange};

/// The name of a call: its first own name or operator token (`None` for `a[1]`).
pub fn method_name(call: &SyntaxNode) -> Option<SyntaxToken> {
    code_tokens(call).find(|t| matches!(t.kind(), k::IDENTIFIER | k::CONSTANT | k::OPERATOR))
}

/// The span of `n` without the trivia around it.
pub fn span(ctx: &RuleContext<'_>, n: &SyntaxNode) -> TextRange {
    significant_range(n, ctx.file.grammar)
}
