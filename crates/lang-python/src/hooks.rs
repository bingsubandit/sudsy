//! What the shared rules ask of Python: the keys of a dict, which has no entry nodes, and the name
//! of a literal or variable key.

use crate::format::syntax;
use crate::generated::kind as k;
use core_rules::RuleHooks;
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
    fn is_name(&self, key: &SyntaxElement, _text: &str) -> bool {
        matches!(key, SyntaxElement::Node(n) if matches!(n.kind(),
            k::EXPR_STRING_LITERAL | k::EXPR_BYTES_LITERAL | k::EXPR_NUMBER_LITERAL
            | k::EXPR_BOOLEAN_LITERAL | k::EXPR_NONE_LITERAL | k::EXPR_NAME))
    }

    /// The source text without comments around it, quotes included, so `"a"` and `'a'` differ
    /// from the name `a`.
    fn key_name<'a>(&self, _g: &Grammar, key: &SyntaxElement, text: &'a str) -> Cow<'a, str> {
        let raw = text[key.text_range()].trim();
        match key {
            SyntaxElement::Node(n) if n.kind() == k::EXPR_NUMBER_LITERAL => {
                number(raw).map_or(Cow::Borrowed(raw), Cow::Owned)
            }
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

/// The identity of a number literal as a dict key: `1`, `1.0` and `True` are one key.
fn number(text: &str) -> Option<String> {
    let t = text.replace('_', "");
    if t.ends_with(['j', 'J']) {
        return None;
    }
    let int = if let Some(h) = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
        i128::from_str_radix(h, 16).ok()
    } else if let Some(o) = t.strip_prefix("0o").or_else(|| t.strip_prefix("0O")) {
        i128::from_str_radix(o, 8).ok()
    } else if let Some(b) = t.strip_prefix("0b").or_else(|| t.strip_prefix("0B")) {
        i128::from_str_radix(b, 2).ok()
    } else {
        t.parse::<i128>().ok()
    };
    if let Some(i) = int {
        return Some(format!("n{i}"));
    }
    let f: f64 = t.parse().ok()?;
    Some(if f.fract() == 0.0 && f.abs() < 1e18 {
        format!("n{}", f as i64)
    } else {
        format!("f{f}")
    })
}
