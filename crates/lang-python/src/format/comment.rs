//! Python comments: the `;` that ends a statement, `# fmt: skip`, and the comment text normalized
//! before formatting.

use super::util::*;
use crate::generated::kind as k;
use core_format::FormatCtx;
use core_syntax::{SyntaxElement, SyntaxNode, SyntaxToken};

/// The `;` right after the statement's last token, if there is one (for a compound statement,
/// the `;` of its last inner statement). It is the next token, not a sibling: at the end of a
/// block it lies outside the enclosing nodes, which end where their last statement ends.
pub(crate) fn semicolon(cx: &FormatCtx<'_>, s: &SyntaxNode) -> Option<SyntaxToken> {
    let mut next = cx.last_token(s)?.next_token();
    while let Some(t) = next {
        match t.kind() {
            k::SEMI => return Some(t),
            _ if cx.grammar.is_trivia(t.kind()) => next = t.next_token(),
            _ => return None,
        }
    }
    None
}

/// A simple statement with a trailing `# fmt: skip` (after its `;` when it has one).
pub(crate) fn is_fmt_skip(cx: &FormatCtx<'_>, s: &SyntaxNode) -> bool {
    !is_compound(s.kind())
        && semicolon(cx, s)
            .or_else(|| cx.last_token(s))
            .is_some_and(|l| core_format::ignore::line_ignored(cx.grammar, &l))
}

/// What formatting changes in the text that no layout decision can undo, done before the tree is
/// formatted (the safety check follows it): `#comment` becomes `# comment` (not `#!`, `#:`, `##`
/// or `#'`), a comment loses its trailing blanks, `class A():` loses its empty parentheses.
/// `None` when nothing changes.
pub(crate) fn rewrite(tree: &SyntaxNode, text: &str) -> Option<String> {
    let mut edits: Vec<(usize, usize, String)> = Vec::new();
    for e in tree.descendants_with_tokens() {
        match e {
            SyntaxElement::Token(t) if t.kind() == k::COMMENT => {
                let src = t.text();
                let trimmed = src.trim_end();
                let rest = &trimmed[1..];
                let fixed = match rest.chars().next() {
                    None => "#".to_string(),
                    Some(c) if " !:#'".contains(c) => trimmed.to_string(),
                    Some('\u{a0}') if !rest.trim_start().starts_with("type:") => {
                        format!("# {}", &rest['\u{a0}'.len_utf8()..])
                    }
                    Some(_) => format!("# {rest}"),
                };
                if fixed != src {
                    edits.push((
                        t.text_range().start().into(),
                        t.text_range().end().into(),
                        fixed,
                    ));
                }
            }
            SyntaxElement::Node(n) if n.kind() == k::STMT_CLASS_DEF => {
                let Some(args) = n.children().find(|c| c.kind() == k::ARGUMENTS) else {
                    continue;
                };
                let toks: Vec<SyntaxToken> = args
                    .descendants_with_tokens()
                    .filter_map(|e| e.into_token())
                    .collect();
                let significant: Vec<&SyntaxToken> = toks
                    .iter()
                    .filter(|t| !matches!(t.kind(), k::WHITESPACE | k::NEWLINE | k::CONTINUATION))
                    .collect();
                if let [open, close] = significant.as_slice()
                    && open.kind() == k::LPAR
                    && close.kind() == k::RPAR
                {
                    edits.push((
                        open.text_range().start().into(),
                        close.text_range().end().into(),
                        String::new(),
                    ));
                }
            }
            _ => {}
        }
    }
    if edits.is_empty() {
        return None;
    }
    edits.sort_by_key(|e| e.0);
    let mut out = String::new();
    let mut last = 0;
    for (start, end, fixed) in edits {
        out.push_str(&text[last..start]);
        out.push_str(&fixed);
        last = end;
    }
    out.push_str(&text[last..]);
    Some(out)
}
