//! The rewrites of instruction forms that Docker reads as the same value (the legacy `ENV key
//! value` and `LABEL key value` as `key=value`, the JSON array of `COPY`/`ADD` as plain arguments)
//! and of comments, on the grammar's kinds; the shared rewrite module does the work.
//!
//! concept: none (glue: Dockerfile forms)

use crate::generated::kind as k;
use core_format::rewrite::{KeywordForms, apply_edits, comment_space, keyword_forms};
use core_syntax::{Grammar, SyntaxNode, SyntaxToken};

/// Where the equivalent forms are: the instructions whose `key value` argument is one `Word`
/// token, and the JSON arrays.
const FORMS: KeywordForms<'static> = KeywordForms {
    instruction: k::INSTRUCTION,
    keyword: k::KEYWORD,
    pair: k::WORD,
    pair_keywords: &["ENV", "LABEL"],
    array: [k::L_BRACKET, k::JSON_STRING, k::COMMA, k::R_BRACKET],
    array_keywords: &["COPY", "ADD"],
};

/// The escape character of the file: the one its `# escape=` directive names, else a backslash.
pub fn escape_char(grammar: &Grammar, tree: &SyntaxNode) -> char {
    let root = tree.ancestors().last();
    let named = root.and_then(|r| core_format::comments::parser_directive(grammar, &r, "escape"));
    named
        .filter(|v| v.chars().count() == 1)
        .and_then(|v| v.chars().next())
        .unwrap_or('\\')
}

/// The source with the rewrites the options ask for made, or `None` when there are none.
pub fn rewrite(
    tree: &SyntaxNode,
    text: &str,
    grammar: &Grammar,
    (pairs, words, space): (bool, bool, bool),
) -> Option<String> {
    if !(pairs || words || space) {
        return None;
    }
    let mut edits = Vec::new();
    if space {
        // (a parser directive, `#syntax=...`, stands before the first instruction and stays)
        let first = tree
            .descendants_with_tokens()
            .filter_map(|e| e.into_token())
            .find(|t| !grammar.is_trivia(t.kind()))
            .map_or(usize::MAX, |t| usize::from(t.text_range().start()));
        let directive = |t: &SyntaxToken| {
            usize::from(t.text_range().start()) < first
                && t.text()
                    .split_whitespace()
                    .next()
                    .is_some_and(|w| w.contains('='))
        };
        comment_space(grammar, tree, directive, &mut edits);
    }
    if pairs || words {
        let escape = escape_char(grammar, tree);
        keyword_forms(tree, text, &FORMS, (pairs, words, escape), &mut edits);
    }
    (!edits.is_empty()).then(|| apply_edits(text, edits))
}
