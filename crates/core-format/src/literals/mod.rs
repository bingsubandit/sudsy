//! Spelling of literal tokens: string quotes and escapes, number notation. Text in, text out;
//! each language passes its policy as a style value.

pub mod prefixed;

use core_syntax::strings::StringSyntax;

/// How a string token is re-quoted.
#[derive(Clone, Copy)]
pub struct StringStyle {
    /// The preferred quote is `'`.
    pub single: bool,
    /// A string that already has the chosen quote is kept as written (its escapes too).
    pub keep_quoted: bool,
    /// A `#{ }` interpolation is copied as written; a string whose interpolation holds a quote
    /// is kept.
    pub interpolation: bool,
}

/// A string token (`'..'` or `".."`) with the preferred quote, unless its text has more of that
/// quote than of the other. Other escapes stay as written, except that an escaped quote that
/// needs no escape loses its backslash and an unescaped enclosing quote gains one. `syn` is the
/// token's row: it says which character escapes (a row with none leaves the token as written).
pub fn string(raw: &str, style: StringStyle, syn: &StringSyntax) -> String {
    let Some(escape) = syn.escape else {
        return raw.to_string();
    };
    let body = &raw[1..raw.len() - 1];
    if style.interpolation
        && body
            .split("#{")
            .skip(1)
            .any(|s| s.split('}').next().is_some_and(|i| i.contains(['"', '\''])))
    {
        return raw.to_string();
    }
    let (pref, alt) = if style.single {
        ('\'', '"')
    } else {
        ('"', '\'')
    };
    let count = |q: char| body.matches(q).count();
    let (quote, other) = if count(pref) > count(alt) {
        (alt, pref)
    } else {
        (pref, alt)
    };
    if style.keep_quoted && raw.starts_with(quote) {
        return raw.to_string();
    }
    let mut out = String::with_capacity(raw.len() + 2);
    out.push(quote);
    let mut chars = body.chars();
    let (mut depth, mut prev) = (0, ' ');
    while let Some(c) = chars.next() {
        let inside = style.interpolation && (depth > 0 || (c == '{' && prev == '#'));
        prev = c;
        if inside {
            match c {
                '{' => depth += 1,
                '}' => depth -= 1,
                _ => {}
            }
            out.push(c);
            continue;
        }
        match c {
            c if c == escape => match chars.next() {
                Some(e) if e == other => out.push(e),
                Some(e) => {
                    out.push(escape);
                    out.push(e);
                }
                None => out.push(escape),
            },
            c if c == quote => {
                out.push(escape);
                out.push(c);
            }
            c => out.push(c),
        }
    }
    out.push(quote);
    out
}

/// The body with `\"` and `\'` as plain quotes (other escapes stay): the one thing a quote
/// change may alter, so the value to compare.
pub fn unescape_quotes(body: &str, syn: &StringSyntax) -> String {
    let escape = syn.escape.unwrap_or('\\');
    let mut out = String::with_capacity(body.len());
    let mut chars = body.chars();
    while let Some(c) = chars.next() {
        match (c, chars.clone().next()) {
            (e, Some(q @ ('"' | '\''))) if e == escape => {
                chars.next();
                out.push(q);
            }
            (e, Some(o)) if e == escape => {
                chars.next();
                out.push(c);
                out.push(o);
            }
            _ => out.push(c),
        }
    }
    out
}

/// What happens to the zeros at the end of a fraction.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum FracZeros {
    Keep,
    /// All go, one digit stays (`1.50` is `1.5`, `1.00` is `1.0`).
    KeepOne,
    /// All go, and the dot with them (`1.50` is `1.5`, `1.0` is `1`).
    Trim,
}

#[derive(Clone, Copy)]
pub struct NumberStyle {
    pub frac: FracZeros,
    /// `5.` is `5.0`; else `5`.
    pub dot_zero: bool,
    /// No leading zeros in the exponent and none at all when it is zero; else they stay (a `+`
    /// goes either way).
    pub trim_exponent: bool,
    /// Hex digits are uppercase (`0xAB`); else lowercase.
    pub hex_upper: bool,
    /// A trailing `j` (imaginary) is kept, lowercase.
    pub imaginary: bool,
}

/// A decimal number (a sign is allowed) in lowercase, with a digit before the dot. Hex, octal
/// and binary numbers are only lowercased (hex digits as the style says).
pub fn number(raw: &str, style: NumberStyle) -> String {
    let mut lower = raw.to_ascii_lowercase();
    if let Some(hex) = lower.strip_prefix("0x") {
        return if style.hex_upper {
            format!("0x{}", hex.to_ascii_uppercase())
        } else {
            lower
        };
    }
    if lower.starts_with("0b") || lower.starts_with("0o") {
        return lower;
    }
    if style.imaginary && lower.ends_with('j') {
        lower.pop();
        return number(&lower, style) + "j";
    }
    let (sign, lower) = match lower.strip_prefix(['+', '-']) {
        Some(rest) => (&raw[..1], rest),
        None => ("", lower.as_str()),
    };
    let (mantissa, exponent) = match lower.split_once('e') {
        Some((m, e)) => (m, Some(e)),
        None => (lower, None),
    };
    let mut out = sign.to_string();
    let (int, frac) = match mantissa.split_once('.') {
        Some((i, f)) => (i, Some(f)),
        None => (mantissa, None),
    };
    out.push_str(if int.is_empty() { "0" } else { int });
    match frac {
        Some("") if style.dot_zero => out.push_str(".0"),
        Some(f) if !f.is_empty() => {
            let kept = if f.bytes().all(|b| b.is_ascii_digit()) {
                let trimmed = f.trim_end_matches('0');
                match style.frac {
                    FracZeros::Keep => f,
                    FracZeros::KeepOne => &f[..trimmed.len().max(1)],
                    FracZeros::Trim => trimmed,
                }
            } else {
                f
            };
            if !kept.is_empty() {
                out.push('.');
                out.push_str(kept);
            }
        }
        _ => {}
    }
    if let Some(e) = exponent {
        let (neg, digits) = match e.strip_prefix('-') {
            Some(d) => ("-", d),
            None => ("", e.strip_prefix('+').unwrap_or(e)),
        };
        let digits = if style.trim_exponent {
            digits.trim_start_matches('0')
        } else {
            digits
        };
        if !digits.is_empty() {
            out.push('e');
            out.push_str(neg);
            out.push_str(digits);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const JS: StringStyle = StringStyle {
        single: false,
        keep_quoted: false,
        interpolation: false,
    };

    fn syn() -> StringSyntax {
        StringSyntax {
            escape: Some('\\'),
            ..Default::default()
        }
    }

    #[test]
    fn strings_take_the_cheaper_quote() {
        let single = StringStyle { single: true, ..JS };
        assert_eq!(string("'a'", JS, &syn()), "\"a\"");
        assert_eq!(string("\"a\"", single, &syn()), "'a'");
        assert_eq!(string("'say \"hi\"'", JS, &syn()), "'say \"hi\"'");
        assert_eq!(string("'a\"b\\'c\\'d'", JS, &syn()), "\"a\\\"b'c'd\"");
        assert_eq!(string("\"it's\"", single, &syn()), "\"it's\"");
        let css = StringStyle {
            keep_quoted: true,
            interpolation: true,
            ..JS
        };
        assert_eq!(string("\"\\\"\\'\"", css, &syn()), "\"\\\"\\'\"");
        assert_eq!(string("'#{\"a\"}'", css, &syn()), "'#{\"a\"}'");
    }

    #[test]
    fn quote_escapes_are_values() {
        assert_eq!(
            unescape_quotes("a\\\"b\\'c\\\\d\\n", &syn()),
            "a\"b'c\\\\d\\n"
        );
    }

    #[test]
    fn numbers_follow_the_style() {
        let n = |frac, dot_zero, trim_exponent| NumberStyle {
            frac,
            dot_zero,
            trim_exponent,
            hex_upper: false,
            imaginary: false,
        };
        let js = n(FracZeros::KeepOne, false, true);
        for (from, to) in [
            ("1.50", "1.5"),
            (".5", "0.5"),
            ("5.e3", "5e3"),
            ("0.0e0", "0.0"),
        ] {
            assert_eq!(number(from, js), to, "{from}");
        }
        assert_eq!(number("-1.0E+05", n(FracZeros::Trim, false, true)), "-1e5");
        assert_eq!(number("1.e+05", n(FracZeros::Keep, true, false)), "1.0e05");
    }
}
