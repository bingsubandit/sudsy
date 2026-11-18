//! Strings, numbers, regular expressions and directives, and the printing of a node as written
//! with those restyled (what every node without layout code of its own gets).

use super::util::{semi, style};
use crate::generated::kind as k;
use core_format::context::verbatim_text;
use core_format::doc::{self, Doc};
use core_format::literals::{self, FracZeros, NumberStyle, StringStyle};
use core_format::{FormatCtx, FormatError};
use core_syntax::{SyntaxKind, SyntaxNode, SyntaxToken};

pub(crate) fn format(node: &SyntaxNode, cx: &FormatCtx<'_>) -> Result<Option<Doc>, FormatError> {
    if node.kind() != k::DIRECTIVE {
        return Ok(None);
    }
    let Some(lit) = cx.first_token(node) else {
        return Ok(None);
    };
    let text = directive(lit.text(), style(cx).single_quote);
    let doc = doc::concat(vec![
        cx.leading_comments_doc(&lit),
        doc::text(text),
        cx.trailing_comments_doc(&lit),
        semi(cx, node),
    ]);
    Ok(Some(doc))
}

/// The node's text with its comments, strings and numbers restyled; nothing else changes.
pub(crate) fn verbatim(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Doc {
    let (Some(first), Some(last)) = (cx.first_token(node), cx.last_token(node)) else {
        return Doc::Nil;
    };
    let mut raw = String::new();
    let mut tok = first.clone();
    loop {
        match restyle(cx, &tok) {
            Some(text) => raw.push_str(&text),
            None => raw.push_str(tok.text()),
        }
        if tok == last {
            break;
        }
        match tok.next_token() {
            Some(next) => tok = next,
            None => break,
        }
    }
    doc::concat(vec![
        cx.leading_comments_doc(&first),
        verbatim_text(&raw),
        cx.trailing_comments_doc(&last),
    ])
}

/// The restyled text of a literal token, `None` when it stays.
fn restyle(cx: &FormatCtx<'_>, tok: &SyntaxToken) -> Option<String> {
    let text = tok.text();
    let new = match tok.kind() {
        k::STRING => {
            // Strings of JSX attributes and directives have rules of their own.
            let owner = tok.parent()?.parent();
            if owner.is_some_and(|o| matches!(o.kind(), k::JSX_ATTRIBUTE | k::DIRECTIVE)) {
                return None;
            }
            string(text, style(cx).single_quote)
        }
        k::NUMBER => number(text),
        k::BIG_INT => text.to_ascii_lowercase(),
        k::REG_EXP => regex(text),
        _ => return None,
    };
    (new != text).then_some(new)
}

/// A directive keeps its quotes unless it has none inside, then it takes the preferred ones.
fn directive(raw: &str, single: bool) -> String {
    let content = &raw[1..raw.len() - 1];
    if content.contains(['"', '\'']) {
        return raw.to_string();
    }
    let q = if single { '\'' } else { '"' };
    format!("{q}{content}{q}")
}

const STRING: StringStyle = StringStyle {
    single: false,
    keep_quoted: false,
    interpolation: false,
};

/// The `string` row of the string token.
fn syntax() -> &'static core_syntax::strings::StringSyntax {
    core_syntax::grammars::JS.get().string_row(k::STRING)
}

/// A string with the preferred quote (see [`literals::string`]).
pub(crate) fn string(raw: &str, single: bool) -> String {
    literals::string(raw, StringStyle { single, ..STRING }, syntax())
}

/// Lowercase, no `+` and no leading zeros in an exponent (none at all when it is zero), a zero
/// before a leading dot, no trailing zeros of a fraction (one digit stays) and no trailing dot.
pub(crate) fn number(raw: &str) -> String {
    literals::number(
        raw,
        NumberStyle {
            frac: FracZeros::KeepOne,
            dot_zero: false,
            trim_exponent: true,
            hex_upper: false,
            imaginary: false,
        },
    )
}

/// The flags of a regular expression in alphabetical order.
fn regex(raw: &str) -> String {
    let Some(end) = raw.rfind('/') else {
        return raw.to_string();
    };
    let mut flags: Vec<char> = raw[end + 1..].chars().collect();
    flags.sort_unstable();
    format!("{}{}", &raw[..=end], flags.into_iter().collect::<String>())
}

/// What a literal token means: the one thing the formatter's restyling keeps.
pub(crate) fn value(kind: SyntaxKind, raw: &str) -> String {
    match kind {
        k::STRING => unquoted(raw),
        k::NUMBER => canonical_number(raw),
        k::BIG_INT => raw.to_ascii_lowercase(),
        _ => regex(raw),
    }
}

fn unquoted(raw: &str) -> String {
    literals::unescape_quotes(
        raw.get(1..raw.len().saturating_sub(1)).unwrap_or(""),
        syntax(),
    )
}

fn canonical_number(raw: &str) -> String {
    let lower = raw.to_ascii_lowercase().replace('_', "");
    if lower.starts_with("0x") || lower.starts_with("0b") || lower.starts_with("0o") {
        return lower;
    }
    match lower.parse::<f64>() {
        Ok(n) => format!("{n:e}"),
        Err(_) => lower,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strings_take_the_cheaper_quote() {
        assert_eq!(string("'a'", false), "\"a\"");
        assert_eq!(string("\"a\"", true), "'a'");
        assert_eq!(string("'say \"hi\"'", false), "'say \"hi\"'");
        assert_eq!(string("'a\"b\\'c\\'d'", false), "\"a\\\"b'c'd\"");
        assert_eq!(string("'\\d \\n'", false), "\"\\d \\n\"");
        assert_eq!(string("\"it's\"", true), "\"it's\"");
    }

    #[test]
    fn numbers_are_normalized() {
        for (from, to) in [
            ("0XAB", "0xab"),
            ("1E5", "1e5"),
            ("1e+05", "1e5"),
            ("1e-05", "1e-5"),
            ("1.0", "1.0"),
            ("1.50", "1.5"),
            (".5", "0.5"),
            ("5.", "5"),
            ("5.e3", "5e3"),
            ("0.0e0", "0.0"),
            ("1.00", "1.0"),
            ("1_0.0_0", "1_0.0_0"),
            ("1.0e10", "1.0e10"),
            ("0.10", "0.1"),
        ] {
            assert_eq!(number(from), to, "{from}");
        }
    }

    #[test]
    fn values_are_compared_not_text() {
        assert_eq!(value(k::STRING, "'a\"'"), value(k::STRING, "\"a\\\"\""));
        assert_ne!(value(k::STRING, "'a'"), value(k::STRING, "'b'"));
        assert_eq!(value(k::NUMBER, "1.50"), value(k::NUMBER, "1.5"));
        assert_ne!(value(k::NUMBER, "1.5"), value(k::NUMBER, "1.6"));
        assert_eq!(value(k::REG_EXP, "/a/gi"), value(k::REG_EXP, "/a/ig"));
    }
}
