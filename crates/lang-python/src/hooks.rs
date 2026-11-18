//! What the shared rules ask of Python: the keys of a dict, which has no entry nodes, and the name
//! of a literal or variable key.
//!
//! concept: key-identity

use crate::format::syntax;
use crate::generated::kind as k;
use core_rules::RuleHooks;
use core_syntax::roles::has_any;
use core_syntax::strings::{Number, number};
use core_syntax::{Grammar, SyntaxElement, SyntaxNode};
use std::borrow::Cow;

pub struct PythonHooks;

impl RuleHooks for PythonHooks {
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
    fn is_name(&self, _g: &Grammar, key: &SyntaxElement, _text: &str) -> bool {
        matches!(key, SyntaxElement::Node(n) if has_any(n.kind(), &["literal", "reference"]))
    }

    /// The source text without comments around it, quotes included, so `"a"` and `'a'` differ
    /// from the name `a`.
    fn key_name<'a>(&self, _g: &Grammar, key: &SyntaxElement, text: &'a str) -> Cow<'a, str> {
        let raw = text[key.text_range()].trim();
        match key {
            SyntaxElement::Node(n) if n.kind() == k::EXPR_NUMBER_LITERAL => match number(raw) {
                Some(Number::Int(i)) => Cow::Owned(format!("n{i}")),
                Some(Number::Float(f)) => Cow::Owned(format!("f{f}")),
                None => Cow::Borrowed(raw),
            },
            SyntaxElement::Node(n) if n.kind() == k::EXPR_BOOLEAN_LITERAL => {
                Cow::Owned(format!("n{}", u8::from(raw == "True")))
            }
            _ => string_key(raw),
        }
    }
}

fn string_key(raw: &str) -> Cow<'_, str> {
    match syntax().split(raw) {
        Some(lit) => {
            let bytes = if lit.prefix.contains(['b', 'B']) {
                "b"
            } else {
                ""
            };
            Cow::Owned(format!("{bytes}'{}", lit.inner))
        }
        None => Cow::Borrowed(raw),
    }
}
