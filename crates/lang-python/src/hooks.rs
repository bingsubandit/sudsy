//! What the shared rules ask of Python: the keys of a dict, which has no entry nodes, and the name
//! of a literal or variable key.

use crate::generated::kind as k;
use core_rules::RuleHooks;
use core_syntax::{SyntaxElement, SyntaxNode};
use std::borrow::Cow;

pub struct PythonHooks;

impl RuleHooks for PythonHooks {
    fn custom_entries(&self) -> bool {
        true
    }

    /// The first node of each `key: value` item; a `**mapping` item has none.
    fn flat_keys(&self, mapping: &SyntaxNode) -> Option<Vec<SyntaxElement>> {
        let (mut at_start, mut skip) = (true, false);
        let mut keys = Vec::new();
        for e in mapping.children_with_tokens() {
            match e {
                SyntaxElement::Token(t) => match t.text() {
                    "," => (at_start, skip) = (true, false),
                    "**" => skip = true,
                    _ => {}
                },
                SyntaxElement::Node(n) => {
                    if at_start && !skip {
                        keys.push(SyntaxElement::Node(n));
                    }
                    at_start = false;
                }
            }
        }
        Some(keys)
    }

    /// A literal or a name; anything else (a call, an f-string) is not a key that repeats.
    fn is_name(&self, key: &SyntaxElement, _text: &str) -> bool {
        matches!(key, SyntaxElement::Node(n) if matches!(n.kind(),
            k::EXPR_STRING_LITERAL | k::EXPR_BYTES_LITERAL | k::EXPR_NUMBER_LITERAL
            | k::EXPR_BOOLEAN_LITERAL | k::EXPR_NONE_LITERAL | k::EXPR_NAME))
    }

    /// The source text without comments around it, quotes included, so `"a"` and `'a'` differ
    /// from the name `a`.
    fn key_name<'a>(&self, key: &SyntaxElement, text: &'a str) -> Cow<'a, str> {
        let raw = text[key.text_range()].trim();
        let body = raw.trim_start_matches(|c: char| c.is_ascii_alphabetic());
        let quote = body.chars().next().filter(|c| matches!(c, '"' | '\''));
        match quote {
            Some(q) => {
                let n = if body.starts_with(&q.to_string().repeat(3)) && body.len() >= 6 {
                    3
                } else {
                    1
                };
                let inner = &body[n..body.len().saturating_sub(n)];
                let bytes = raw[..raw.len() - body.len()].contains(['b', 'B']);
                Cow::Owned(format!("{}'{inner}", if bytes { "b" } else { "" }))
            }
            None => Cow::Borrowed(raw),
        }
    }
}
