//! Spelling of single tokens: numbers, units, strings, hex colors and `url(`.

use core_format::literals::{self, FracZeros, NumberStyle, StringStyle};

/// Units written in their canonical case (`PX` becomes `px`, `hz` becomes `Hz`).
const UNITS: &[&str] = &[
    "em", "rem", "ex", "rex", "cap", "rcap", "ch", "rch", "ic", "ric", "lh", "rlh", "vw", "svw",
    "lvw", "dvw", "vh", "svh", "lvh", "dvh", "vi", "svi", "lvi", "dvi", "vb", "svb", "lvb", "dvb",
    "vmin", "svmin", "lvmin", "dvmin", "vmax", "svmax", "lvmax", "dvmax", "cm", "mm", "Q", "in",
    "pt", "pc", "px", "cqw", "cqh", "cqi", "cqb", "cqmin", "cqmax", "deg", "grad", "rad", "turn",
    "s", "ms", "Hz", "kHz", "dpi", "dpcm", "dppx", "x", "fr",
];

/// A number token (or the numeric start of a dimension) spelled by [`literals::number`]; the text
/// after the number is the second element.
pub fn split_number(s: &str) -> (String, &str) {
    let b = s.as_bytes();
    let digits = |from: usize| from + b[from..].iter().take_while(|c| c.is_ascii_digit()).count();
    let mut i = digits(usize::from(matches!(b.first(), Some(b'+' | b'-'))));
    if b.get(i) == Some(&b'.') && b.get(i + 1).is_some_and(u8::is_ascii_digit) {
        i = digits(i + 1);
    }
    // `1.e1` is `1e1`.
    let j = i + usize::from(b.get(i) == Some(&b'.'));
    if matches!(b.get(j), Some(b'e' | b'E')) {
        let sign_len = usize::from(matches!(b.get(j + 1), Some(b'+' | b'-')));
        if b.get(j + 1 + sign_len).is_some_and(u8::is_ascii_digit) {
            i = digits(j + 1 + sign_len);
        }
    }
    let style = NumberStyle {
        frac: FracZeros::Trim,
        dot_zero: false,
        trim_exponent: true,
        hex_upper: false,
        imaginary: false,
    };
    (literals::number(&s[..i], style), &s[i..])
}

pub fn number(s: &str) -> String {
    split_number(s).0
}

pub fn dimension(s: &str) -> String {
    let (n, unit) = split_number(s);
    let canonical = UNITS.iter().find(|u| u.eq_ignore_ascii_case(unit));
    format!("{n}{}", canonical.copied().unwrap_or(unit))
}

pub fn percentage(s: &str) -> String {
    format!("{}%", number(s.trim_end_matches('%')))
}

/// A string token with the preferred quote unless it holds more of that quote than of the other.
pub fn string(raw: &str, single: bool) -> String {
    let style = StringStyle {
        single,
        keep_quoted: true,
        interpolation: true,
    };
    let row = core_syntax::grammars::CSS
        .get()
        .string_row(crate::generated::kind::STRING);
    literals::string(raw, style, row)
}

/// `#FFF` is `#fff`; other hashes (ids, odd lengths) keep their case.
pub fn hash(s: &str) -> String {
    let hex = &s[1..];
    if matches!(hex.len(), 3 | 4 | 6 | 8) && hex.bytes().all(|c| c.is_ascii_hexdigit()) {
        s.to_ascii_lowercase()
    } else {
        s.to_string()
    }
}

/// `url( a.png )` is `url(a.png)`.
pub fn url(s: &str) -> String {
    match (s.find('('), s.strip_suffix(')')) {
        (Some(open), Some(body)) => format!("{}{})", &body[..=open], body[open + 1..].trim()),
        _ => s.to_string(),
    }
}

/// An identifier in a value: the CSS-wide keywords are lowercase.
pub fn keyword(s: &str) -> String {
    let lower = s.to_ascii_lowercase();
    if matches!(lower.as_str(), "initial" | "inherit" | "unset" | "revert") {
        lower
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers() {
        for (a, b) in [
            (".50", "0.5"),
            ("0.0", "0"),
            ("+1E3", "+1e3"),
            ("1e+05", "1e5"),
            ("1e-0", "1"),
            ("-.5", "-0.5"),
        ] {
            assert_eq!(number(a), b, "{a}");
        }
        assert_eq!(dimension("1.0PX"), "1px");
        assert_eq!(dimension("1em"), "1em");
        assert_eq!(dimension("2Q"), "2Q");
    }

    #[test]
    fn strings() {
        assert_eq!(string("'it\\'s'", false), "\"it's\"");
        assert_eq!(string("'a\"b'", false), "'a\"b'");
        assert_eq!(string("\"a\"", true), "'a'");
        assert_eq!(string("\"a\\\"b\"", false), "'a\"b'");
        assert_eq!(string("\"\\\"\\'\"", false), "\"\\\"\\'\"");
    }
}
