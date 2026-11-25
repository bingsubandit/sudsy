//! Rewrites of instruction forms that Docker reads as the same value: the legacy `ENV key value`
//! and `LABEL key value` as `key=value`, and the JSON array of `COPY`/`ADD` as plain arguments.
//! Each rewrite is made only where both forms are provably equal (see `env_pair` and `words`), and
//! checked again on its own result; the layout is then checked against the rewritten tree.
//!
//! concept: none (glue: Dockerfile forms)

use crate::generated::kind as k;
use core_format::rewrite::{apply_edits, comment_space};
use core_syntax::{Grammar, SyntaxNode, SyntaxToken};

/// Characters of a path that mean the same in the JSON form and as a plain argument.
fn plain_word(s: &str) -> bool {
    !s.starts_with('-')
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || "_./:@%+,=~*?^-".contains(c))
}

/// `key=value` for the legacy `key value`: the value is the rest of the line after the blanks.
/// A value of one word, or one quoted string, stays as it is; a value with blanks and no quote or
/// backslash goes in double quotes (its blanks stay); a backslash stays only in one unquoted word.
fn env_pair(text: &str) -> Option<String> {
    let name_end = text.find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))?;
    let (name, rest) = text.split_at(name_end);
    if name.is_empty() || name.starts_with(|c: char| c.is_ascii_digit()) {
        return None;
    }
    let value = rest.strip_prefix([' ', '\t'])?.trim();
    // (a backslash reads the same in both forms only in one unquoted word)
    let one_word = !value.contains([' ', '\t', '"', '\'']);
    if value.is_empty() || value.contains(['\n', '\r']) || value.contains('\\') && !one_word {
        return None;
    }
    let quoted = |q: char| {
        value.len() > 1
            && value.starts_with(q)
            && value.ends_with(q)
            && !value[1..value.len() - 1].contains(q)
    };
    let new = if value.contains(['"', '\'']) {
        if !(quoted('"') || quoted('\'')) {
            return None;
        }
        format!("{name}={value}")
    } else if value.contains([' ', '\t']) {
        format!("{name}=\"{value}\"")
    } else {
        format!("{name}={value}")
    };
    // The proof: the value of the new pair is the value of the old one.
    let new_value = new.split_once('=')?.1;
    let unwrapped = if new_value.len() > value.len() {
        &new_value[1..new_value.len() - 1]
    } else {
        new_value
    };
    (unwrapped == value).then_some(new)
}

/// The edit that turns the JSON array `[` ... `]` of an instruction into plain arguments, when
/// every element is a plain word and the array has no comment, line break or continuation.
fn json_array(open: &SyntaxToken, text: &str) -> Option<(usize, usize, String)> {
    let mut words = Vec::new();
    let mut expect_string = true;
    let mut close = None;
    let mut next = open.next_token();
    while let Some(t) = next {
        match t.kind() {
            k::WHITESPACE => {}
            k::JSON_STRING if expect_string => {
                let t = t.text();
                words.push(t.strip_prefix('"')?.strip_suffix('"')?.to_string());
                expect_string = false;
            }
            k::COMMA if !expect_string => expect_string = true,
            k::R_BRACKET if !expect_string => {
                close = Some(t);
                break;
            }
            _ => return None,
        }
        next = t.next_token();
    }
    let close = close?;
    let (start, end) = (
        usize::from(open.text_range().start()),
        usize::from(close.text_range().end()),
    );
    let new = words.join(" ");
    // The proof: the plain arguments are the elements, one by one.
    let same = new.split(' ').eq(words.iter().map(String::as_str));
    (words.len() > 1
        && words.iter().all(|w| plain_word(w))
        && same
        && !text[start..end].contains('\n'))
    .then_some((start, end, new))
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
    for t in tree
        .descendants_with_tokens()
        .filter_map(|e| e.into_token())
    {
        let Some(keyword) = t
            .parent()
            .filter(|p| p.kind() == k::INSTRUCTION)
            .and_then(|p| {
                p.children_with_tokens()
                    .filter_map(|e| e.into_token())
                    .find(|t| t.kind() == k::KEYWORD)
            })
        else {
            continue;
        };
        let name = keyword.text().to_ascii_uppercase();
        match t.kind() {
            k::WORD if pairs && matches!(name.as_str(), "ENV" | "LABEL") => {
                if let Some(new) = env_pair(t.text()) {
                    let r = t.text_range();
                    edits.push((usize::from(r.start()), usize::from(r.end()), new));
                }
            }
            k::L_BRACKET if words && matches!(name.as_str(), "COPY" | "ADD") => {
                edits.extend(json_array(&t, text));
            }
            _ => {}
        }
    }
    (!edits.is_empty()).then(|| apply_edits(text, edits))
}
