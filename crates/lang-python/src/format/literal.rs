//! Strings and numbers: quote and prefix normalization, number spelling, docstrings.

use super::util::syntax;
use core_format::literals::{self, FracZeros, NumberStyle, prefixed};
use std::borrow::Cow;

/// A number as ruff writes it: lowercase prefix and exponent, uppercase hex digits, `j` suffix
/// lowercase, a zero before or after a lone dot.
pub(crate) fn number(text: &str) -> String {
    let style = NumberStyle {
        frac: FracZeros::Keep,
        dot_zero: true,
        trim_exponent: false,
        hex_upper: true,
        imaginary: true,
    };
    literals::number(text, style)
}

/// What the safety check compares of a restyled token (see `PythonFormatter::value`).
pub(crate) fn value(t: &core_syntax::SyntaxToken) -> Cow<'_, str> {
    use crate::generated::kind as k;
    match t.kind() {
        k::INT | k::FLOAT | k::COMPLEX => Cow::Owned(number(t.text()).replace('_', "")),
        k::STRING => {
            let v = prefixed::string_value(syntax(), t.text());
            let statement = t
                .parent_ancestors()
                .nth(2)
                .is_some_and(|p| p.kind() == k::STMT_EXPR);
            if statement {
                Cow::Owned(
                    v.split('\n')
                        .map(str::trim)
                        .collect::<Vec<_>>()
                        .join("\n")
                        .trim()
                        .to_string(),
                )
            } else {
                Cow::Owned(v)
            }
        }
        k::F_STRING_START | k::T_STRING_START => Cow::Owned(prefixed::prefix_value(
            t.text().trim_end_matches(['"', '\'']),
        )),
        k::F_STRING_END | k::T_STRING_END => Cow::Borrowed(""),
        k::F_STRING_MIDDLE | k::T_STRING_MIDDLE => {
            Cow::Owned(literals::unescape_quotes(t.text(), syntax()))
        }
        _ => Cow::Borrowed(t.text()),
    }
}

/// The value of a string expression: the prefix of its first part and the bodies of all parts,
/// so that joining an implicit concatenation keeps it (the safety check compares this).
pub(crate) fn node_value(n: &core_syntax::SyntaxNode) -> String {
    use crate::generated::kind as k;
    let mut out = String::new();
    for (i, part) in n.children().enumerate() {
        let text: String = part
            .descendants_with_tokens()
            .filter_map(|e| e.into_token())
            .filter(|t| {
                !matches!(
                    t.kind(),
                    k::WHITESPACE | k::NEWLINE | k::COMMENT | k::CONTINUATION
                )
            })
            .map(|t| t.text().to_string())
            .collect();
        match prefixed::split(syntax(), &text) {
            Some((p, _, body)) => {
                if i == 0 {
                    out.push_str(&prefixed::prefix_value(p));
                }
                out.push_str(&literals::unescape_quotes(body, syntax()));
            }
            None => out.push_str(&text),
        }
    }
    // A docstring is compared without the indentation and trailing blanks the formatter changes.
    if n.parent().is_some_and(|p| p.kind() == k::STMT_EXPR) {
        return out
            .split('\n')
            .map(str::trim)
            .collect::<Vec<_>>()
            .join("\n")
            .trim()
            .to_string();
    }
    out
}
