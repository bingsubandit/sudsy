//! What the rules read off the text of the tree: dotted names, string literals and their values.

use crate::generated::kind as k;
use core_syntax::{SyntaxNode, TextSize};

/// The code tokens of `n` written one after the other: `a.b` for a dotted name.
pub fn code_text(n: &SyntaxNode) -> String {
    n.descendants_with_tokens()
        .filter_map(|e| e.into_token())
        .filter(|t| {
            !matches!(
                t.kind(),
                k::WHITESPACE | k::NEWLINE | k::COMMENT | k::CONTINUATION
            )
        })
        .map(|t| t.text().to_owned())
        .collect()
}

/// One quoted piece of a string literal.
pub struct Part {
    /// Where the text between the quotes starts and what it is.
    pub inner_start: TextSize,
    pub inner: String,
    pub raw: bool,
    pub bytes: bool,
}

/// The pieces of a string or bytes literal node (`ExprStringLiteral`, `ExprBytesLiteral`).
pub fn parts(n: &SyntaxNode) -> Vec<Part> {
    n.children()
        .filter(|c| matches!(c.kind(), k::STRING_LITERAL | k::BYTES_LITERAL))
        .filter_map(|c| {
            let t = c
                .descendants_with_tokens()
                .filter_map(|e| e.into_token())
                .find(|t| t.kind() == k::STRING)?;
            let text = t.text();
            let quote = text.find(['\'', '"'])?;
            let prefix = &text[..quote];
            let rest = &text[quote..];
            let q = &rest[..1];
            let width = if rest.len() >= 6 && rest.starts_with(&q.repeat(3)) {
                3
            } else {
                1
            };
            let end = rest.len().checked_sub(width)?;
            let inner = rest.get(width..end.max(width))?;
            let range = t.text_range();
            Some(Part {
                inner_start: range.start() + TextSize::try_from(quote + width).ok()?,
                inner: inner.to_owned(),
                raw: prefix.contains(['r', 'R']),
                bytes: prefix.contains(['b', 'B']),
            })
        })
        .collect()
}

/// The value of a string literal: the pieces joined, escapes resolved (`\N{...}` is one `?`).
pub fn string_value(n: &SyntaxNode) -> String {
    let mut out = String::new();
    for p in parts(n) {
        if p.raw {
            out.push_str(&p.inner);
        } else {
            unescape(&p.inner, &mut out);
        }
    }
    out
}

fn unescape(s: &str, out: &mut String) {
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        let Some(e) = chars.next() else {
            out.push('\\');
            break;
        };
        let mut hex = |n: usize, out: &mut String| {
            let digits: String = (0..n)
                .filter_map(|_| chars.next_if(char::is_ascii_hexdigit))
                .collect();
            match u32::from_str_radix(&digits, 16)
                .ok()
                .and_then(char::from_u32)
            {
                Some(c) if digits.len() == n => out.push(c),
                _ => out.push('?'),
            }
        };
        match e {
            '\n' => {}
            '\r' => {
                chars.next_if_eq(&'\n');
            }
            '\\' | '\'' | '"' => out.push(e),
            'a' => out.push('\x07'),
            'b' => out.push('\x08'),
            'f' => out.push('\x0c'),
            'n' => out.push('\n'),
            'r' => out.push('\r'),
            't' => out.push('\t'),
            'v' => out.push('\x0b'),
            'x' => hex(2, out),
            'u' => hex(4, out),
            'U' => hex(8, out),
            'N' => {
                if chars.next_if_eq(&'{').is_some() {
                    for c in chars.by_ref() {
                        if c == '}' {
                            break;
                        }
                    }
                }
                out.push('?');
            }
            '0'..='7' => {
                let mut v = e.to_digit(8).unwrap_or(0);
                for _ in 0..2 {
                    match chars.next_if(|c| ('0'..='7').contains(c)) {
                        Some(d) => v = v * 8 + d.to_digit(8).unwrap_or(0),
                        None => break,
                    }
                }
                out.push(char::from_u32(v).unwrap_or('?'));
            }
            other => {
                out.push('\\');
                out.push(other);
            }
        }
    }
}
