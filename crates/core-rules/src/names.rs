//! What a key or a scalar is called, read from the grammar's tags and its `keys` line
//! ([`core_syntax::KeySyntax`]): the identity that `no-duplicate-keys`, `sorted-keys`, the key
//! naming rule and the key paths compare. A language whose keys need more than the row says
//! overrides the methods of [`RuleHooks`](crate::hooks::RuleHooks).

use crate::hooks::{Accessor, Scalar};
use core_syntax::strings::integer;
use core_syntax::{Grammar, SyntaxElement, SyntaxKind, SyntaxNode};
use rowan::{GreenNodeData, NodeOrToken};
use std::borrow::Cow;

/// The value of a string token (the grammar's `string` row); the text itself without a row.
fn decode<'a>(g: &Grammar, kind: SyntaxKind, raw: &'a str) -> Cow<'a, str> {
    g.string_value(kind, raw).unwrap_or(Cow::Borrowed(raw))
}

/// The identity of a key. A key written as a path of `#key-part` tokens (TOML's `a."b.c"`) is its
/// parts joined with `.`; any other key is its text, a `#string` one decoded or stripped of its
/// quotes, without the `strip` prefix, lowercased when the grammar says so; the `alias` field of
/// the entry, when it has one, stands for the key.
pub fn key_name<'a>(g: &Grammar, key: &SyntaxElement, text: &'a str) -> Cow<'a, str> {
    let row = g.keys();
    if row.plain && !row.parts {
        return Cow::Borrowed(strip_quotes(text[key.text_range()].trim()));
    }
    let aliased = row.alias.and_then(|kind| {
        if !row.alias_in.contains(&key.parent()?.kind()) {
            return None;
        }
        let alias = alias_before(g, key).filter(|a| a.kind() == kind)?;
        alias.first_token().map(SyntaxElement::Token)
    });
    let key = aliased.as_ref().unwrap_or(key);
    let name = match key {
        SyntaxElement::Node(node) if has_parts(g, node) => return joined(g, node, text),
        SyntaxElement::Node(_) => Cow::Borrowed(strip_quotes(text[key.text_range()].trim())),
        // A token's text has no blank space around it.
        SyntaxElement::Token(t) => {
            let raw = &text[t.text_range()];
            if row.decode && raw.starts_with(['"', '\'']) {
                decode(g, t.kind(), raw)
            } else {
                Cow::Borrowed(strip_quotes(raw.trim()))
            }
        }
    };
    if row.strip.is_none() && !row.lowercase {
        return name;
    }
    let name = match (&row.strip, name) {
        (Some(prefix), Cow::Borrowed(n)) => {
            Cow::Borrowed(n.strip_prefix(prefix.as_str()).unwrap_or(n))
        }
        (_, name) => name,
    };
    match name {
        Cow::Borrowed(n) if row.lowercase && n.bytes().any(|b| b.is_ascii_uppercase()) => {
            Cow::Owned(n.to_ascii_lowercase())
        }
        name => name,
    }
}

/// The node right in front of the key, trivia between them skipped: where an alias stands.
fn alias_before(g: &Grammar, key: &SyntaxElement) -> Option<SyntaxNode> {
    let mut prev = match key {
        SyntaxElement::Node(n) => n.prev_sibling_or_token(),
        SyntaxElement::Token(t) => t.prev_sibling_or_token(),
    };
    while let Some(e) = prev {
        match e {
            SyntaxElement::Node(n) => return Some(n),
            SyntaxElement::Token(t) if g.is_trivia(t.kind()) => prev = t.prev_sibling_or_token(),
            SyntaxElement::Token(_) => return None,
        }
    }
    None
}

fn strip_quotes(raw: &str) -> &str {
    if !raw.starts_with(['"', '\'']) {
        return raw;
    }
    ['"', '\'']
        .iter()
        .find_map(|&q| raw.strip_prefix(q)?.strip_suffix(q))
        .unwrap_or(raw)
}

fn has_parts(g: &Grammar, key: &SyntaxNode) -> bool {
    if !g.keys().parts {
        return false;
    }
    // The first token that is not trivia, read from the green tree (no red token is made).
    let green = key.green();
    for child in green.children() {
        match child {
            NodeOrToken::Node(_) => return false,
            NodeOrToken::Token(t) => {
                let kind = SyntaxKind(t.kind().0);
                if !g.is_trivia(kind) {
                    return g.has_tag(kind, "key-part");
                }
            }
        }
    }
    false
}

/// A key of `#key-part` tokens as one name: a bare key as the text itself, otherwise its decoded
/// parts joined with `.`.
fn joined<'a>(g: &Grammar, key: &SyntaxNode, text: &'a str) -> Cow<'a, str> {
    let mut parts = Vec::new();
    let start = key.text_range().start().into();
    let end = key_parts(g, &key.green(), start, text, &mut parts);
    match parts.len() {
        0 => Cow::Borrowed(""),
        1 => parts.pop().unwrap_or_default(),
        n => {
            let span = &text[start as usize..end as usize];
            let bare = parts.iter().all(|p| matches!(p, Cow::Borrowed(_)));
            if bare && span.len() == parts.iter().map(|p| p.len()).sum::<usize>() + n - 1 {
                Cow::Borrowed(span)
            } else {
                Cow::Owned(parts.join("."))
            }
        }
    }
}

/// The parts of a key that is a path, one by one; `None` for a key of one name (the default).
pub fn parts_of<'a>(g: &Grammar, key: &SyntaxElement, text: &'a str) -> Option<Vec<Cow<'a, str>>> {
    let SyntaxElement::Node(node) = key else {
        return None;
    };
    has_parts(g, node).then(|| {
        let mut out = Vec::new();
        key_parts(
            g,
            &node.green(),
            node.text_range().start().into(),
            text,
            &mut out,
        );
        out
    })
}

/// Replaces `out` with the decoded parts of the key at `start` (`a."b.c".d` has three); returns
/// where the last one ends. Walks the green tree: a dotted key is nested keys, and a red node per
/// token would cost more than the comparison of the names.
pub fn key_parts<'a>(
    g: &Grammar,
    key: &GreenNodeData,
    start: u32,
    text: &'a str,
    out: &mut Vec<Cow<'a, str>>,
) -> u32 {
    out.clear();
    let (mut at, mut end) = (start, start);
    push_parts(g, key, text, out, &mut at, &mut end);
    end
}

fn push_parts<'a>(
    g: &Grammar,
    node: &GreenNodeData,
    text: &'a str,
    out: &mut Vec<Cow<'a, str>>,
    at: &mut u32,
    end: &mut u32,
) {
    for child in node.children() {
        match child {
            NodeOrToken::Node(n) => push_parts(g, n, text, out, at, end),
            NodeOrToken::Token(t) => {
                let from = *at;
                *at += u32::from(t.text_len());
                let kind = SyntaxKind(t.kind().0);
                if !g.has_tag(kind, "key-part") {
                    continue;
                }
                let raw = &text[from as usize..*at as usize];
                out.push(if g.has_tag(kind, "string") {
                    decode(g, kind, raw)
                } else {
                    Cow::Borrowed(raw)
                });
                *end = *at;
            }
        }
    }
}

/// The string or number a sequence item is: the `value` token of a `#literal` node, a string
/// decoded (a multi-line string, the `"""` and `'''` kind, is left alone), a number read with its
/// radix and digit separators. `None` for anything else.
pub fn scalar<'a>(g: &Grammar, item: &SyntaxNode, text: &'a str) -> Option<Scalar<'a>> {
    let token = item
        .children_with_tokens()
        .filter_map(|e| e.into_token())
        .find(|t| g.has_tag(t.kind(), "literal"))?;
    let raw = &text[token.text_range()];
    if g.has_tag(token.kind(), "string") {
        return (!raw.starts_with("\"\"\"") && !raw.starts_with("'''"))
            .then(|| Scalar::String(decode(g, token.kind(), raw)));
    }
    if g.has_tag(token.kind(), "number") {
        let number = integer(raw)
            .map(|i| i as f64)
            .or_else(|| core_syntax::strings::digits(raw).parse().ok());
        return number.map(Scalar::Number);
    }
    None
}

/// Whether the entry is a getter or a setter: its first own token is one of the grammar's
/// `accessors` words.
pub fn accessor(g: &Grammar, entry: &SyntaxNode) -> Option<Accessor> {
    let words = &g.keys().accessors;
    let first = entry
        .children_with_tokens()
        .filter_map(|e| e.into_token())
        .find(|t| words.iter().any(|w| w == t.text()))?;
    Some(if first.text() == words[0] {
        Accessor::Get
    } else {
        Accessor::Set
    })
}
