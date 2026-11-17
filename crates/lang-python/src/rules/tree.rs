//! Reading the parts of the nodes the rules look at: the tree has one node for each node of the
//! parser's AST, so these are the accessors of a typed AST.

use crate::generated::kind as k;
use crate::text::code_text;
use core_rules::RuleContext;
use core_rules::tree::significant_range;
use core_syntax::{SyntaxNode, SyntaxToken, TextRange};

/// The expression inside the parentheses of a `Parenthesized`, however many.
pub fn inner(n: &SyntaxNode) -> SyntaxNode {
    let mut n = n.clone();
    while n.kind() == k::PARENTHESIZED {
        match n.children().next() {
            Some(c) => n = c,
            None => break,
        }
    }
    n
}

/// The span of `n` without the trivia around it.
pub fn span(ctx: &RuleContext<'_>, n: &SyntaxNode) -> TextRange {
    significant_range(n, ctx.file.grammar)
}

/// The own tokens of a node that are code.
pub fn tokens(n: &SyntaxNode) -> impl Iterator<Item = SyntaxToken> {
    n.children_with_tokens()
        .filter_map(|e| e.into_token())
        .filter(|t| {
            !matches!(
                t.kind(),
                k::WHITESPACE | k::NEWLINE | k::COMMENT | k::CONTINUATION
            )
        })
}

/// The positional arguments and the keywords of a call or a class header.
pub fn arguments(args: &SyntaxNode) -> (Vec<SyntaxNode>, Vec<SyntaxNode>) {
    let (mut positional, mut keywords) = (Vec::new(), Vec::new());
    for c in args.children() {
        if c.kind() == k::KEYWORD {
            keywords.push(c);
        } else {
            positional.push(c);
        }
    }
    (positional, keywords)
}

/// The `Arguments` of a call.
pub fn call_arguments(call: &SyntaxNode) -> Option<SyntaxNode> {
    call.children().find(|c| c.kind() == k::ARGUMENTS)
}

/// The function a call calls.
pub fn callee(call: &SyntaxNode) -> Option<SyntaxNode> {
    call.children().next()
}

/// The name of a keyword argument (`None` for `**kwargs`) and its value.
pub fn keyword(kw: &SyntaxNode) -> (Option<String>, Option<SyntaxNode>) {
    let name = kw
        .children()
        .find(|c| c.kind() == k::IDENTIFIER)
        .map(|n| code_text(&n));
    (name, kw.children().find(|c| c.kind() != k::IDENTIFIER))
}

/// The dotted path of a chain of names and attributes.
pub fn dotted(n: &SyntaxNode) -> Option<String> {
    match n.kind() {
        k::EXPR_NAME => Some(code_text(n)),
        k::EXPR_ATTRIBUTE => {
            let mut c = n.children();
            let object = dotted(&c.next()?)?;
            Some(format!("{object}.{}", code_text(&c.next()?)))
        }
        _ => None,
    }
}

/// The statements among the children of a node, in order.
pub fn statements<'a>(n: &'a SyntaxNode) -> impl Iterator<Item = SyntaxNode> + 'a {
    n.children().filter(|c| is_statement(c.kind()))
}

pub fn is_statement(kind: core_syntax::SyntaxKind) -> bool {
    STATEMENTS.contains(&kind)
}

pub const STATEMENTS: &[core_syntax::SyntaxKind] = &[
    k::STMT_FUNCTION_DEF,
    k::STMT_CLASS_DEF,
    k::STMT_RETURN,
    k::STMT_DELETE,
    k::STMT_TYPE_ALIAS,
    k::STMT_ASSIGN,
    k::STMT_AUG_ASSIGN,
    k::STMT_ANN_ASSIGN,
    k::STMT_FOR,
    k::STMT_WHILE,
    k::STMT_IF,
    k::STMT_WITH,
    k::STMT_MATCH,
    k::STMT_RAISE,
    k::STMT_TRY,
    k::STMT_ASSERT,
    k::STMT_IMPORT,
    k::STMT_IMPORT_FROM,
    k::STMT_GLOBAL,
    k::STMT_NONLOCAL,
    k::STMT_EXPR,
    k::STMT_PASS,
    k::STMT_BREAK,
    k::STMT_CONTINUE,
];

/// The operator of a comparison between operands `i` and `i + 1`: the tokens between them.
pub fn compare_ops(n: &SyntaxNode) -> Vec<(SyntaxNode, Vec<core_syntax::SyntaxKind>, SyntaxNode)> {
    let operands: Vec<SyntaxNode> = n.children().collect();
    let mut ops: Vec<Vec<core_syntax::SyntaxKind>> = Vec::new();
    let mut current = Vec::new();
    let mut seen = 0;
    for e in n.children_with_tokens() {
        match e {
            core_syntax::SyntaxElement::Node(_) => {
                if seen > 0 {
                    ops.push(std::mem::take(&mut current));
                }
                seen += 1;
            }
            core_syntax::SyntaxElement::Token(t) => {
                if !matches!(
                    t.kind(),
                    k::WHITESPACE | k::NEWLINE | k::COMMENT | k::CONTINUATION | k::LPAR | k::RPAR
                ) {
                    current.push(t.kind());
                }
            }
        }
    }
    operands
        .windows(2)
        .zip(ops)
        .map(|(w, op)| (w[0].clone(), op, w[1].clone()))
        .collect()
}
