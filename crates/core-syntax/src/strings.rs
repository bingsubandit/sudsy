//! The string-literal model: how one kind of string token is delimited, what its escapes mean
//! and how a value is written again. A grammar describes each kind in `string` rows (see
//! [`crate::grammar`]); decoding, unknown-escape checks and quote changes of every language go
//! through this one implementation.

use std::borrow::Cow;

/// What an escape that no table entry covers does.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Unknown {
    /// The backslash stays: `\q` is `\q`.
    #[default]
    Keep,
    /// The backslash goes: `\q` is `q`.
    Drop,
    /// The string has no value.
    Invalid,
}

/// One `string` row.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StringSyntax {
    /// The token kind the row describes.
    pub token: String,
    /// Delimiters; none for text that is not quoted (a plain YAML scalar).
    pub quotes: Vec<char>,
    /// A delimiter may be written three times in a row (`"""text"""`).
    pub triple: bool,
    /// Letters that may precede the opening delimiter, and those of them that switch escapes off.
    pub prefix: String,
    pub raw: String,
    /// Two delimiters in a row stand for one.
    pub doubled: bool,
    pub escape: Option<char>,
    /// Escape letter to the character it stands for.
    pub map: Vec<(char, char)>,
    /// Escape letter to the number of hex digits that follow.
    pub hex: Vec<(char, usize)>,
    /// Octal escapes of up to this many digits (0: none).
    pub octal: usize,
    /// `\N{name}` (the letter): no name is looked up, the replacement stands in.
    pub named: Option<char>,
    /// `😀` is one character.
    pub surrogates: bool,
    /// An escape before a line break joins the lines.
    pub continuation: bool,
    /// Line breaks fold into spaces and line feeds (YAML flow scalars).
    pub fold: bool,
    pub unknown: Unknown,
    /// What a malformed hex or named escape becomes; none: the string has no value.
    pub replacement: Option<char>,
    /// Set when the row is read: one character delimits, nothing precedes it, and only an ASCII
    /// escape character can change the content (the fast path of [`StringSyntax::value`]).
    pub simple: bool,
}

/// A token split into its parts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Literal<'a> {
    pub prefix: &'a str,
    /// The opening delimiter (`"`, `'''`); empty for unquoted text.
    pub open: &'a str,
    pub inner: &'a str,
}

impl StringSyntax {
    /// Does this row describe `text`: its opening delimiter after the prefix, or any unquoted text.
    pub fn matches(&self, text: &str) -> bool {
        self.quotes.is_empty() || text[self.prefix_len(text)..].starts_with(self.quotes.as_slice())
    }

    fn prefix_len(&self, text: &str) -> usize {
        if self.prefix.is_empty() {
            return 0;
        }
        text.find(|c: char| !(c.is_ascii_alphabetic() && self.prefix.contains(c)))
            .unwrap_or(text.len())
    }

    /// The prefix, delimiter and content of a token (a missing closing delimiter is tolerated).
    pub fn split<'a>(&self, text: &'a str) -> Option<Literal<'a>> {
        if self.quotes.is_empty() {
            return Some(Literal {
                prefix: "",
                open: "",
                inner: text,
            });
        }
        let (prefix, rest) = text.split_at(self.prefix_len(text));
        let q = rest.chars().next().filter(|c| self.quotes.contains(c))?;
        let width = q.len_utf8();
        let triple =
            self.triple && rest.len() >= 6 * width && rest.starts_with(&q.to_string().repeat(3));
        let open = &rest[..if triple { 3 * width } else { width }];
        let body = &rest[open.len()..];
        Some(Literal {
            prefix,
            open,
            inner: body.strip_suffix(open).unwrap_or(body),
        })
    }

    /// The value of a token: its content with the escapes and doubled delimiters resolved.
    pub fn value<'a>(&self, text: &'a str) -> Option<Cow<'a, str>> {
        if let Some(inner) = self.unescaped(text) {
            return Some(Cow::Borrowed(inner));
        }
        self.value_of(self.split(text)?)
    }

    /// The content of a token with nothing to decode (see `simple`): no escape character in it.
    #[inline]
    pub fn unescaped<'a>(&self, text: &'a str) -> Option<&'a str> {
        let b = text.as_bytes();
        let (&first, &last) = (b.first()?, b.last()?);
        if !self.simple || b.len() < 2 || first != last || !self.quotes.contains(&(first as char)) {
            return None;
        }
        let inner = &text[1..b.len() - 1];
        let escaped = self
            .escape
            .is_some_and(|e| inner.as_bytes().contains(&(e as u8)));
        (!escaped).then_some(inner)
    }

    /// The value of a token already split by this row.
    pub fn value_of<'a>(&self, lit: Literal<'a>) -> Option<Cow<'a, str>> {
        if !self.raw.is_empty() && lit.prefix.chars().any(|c| self.raw.contains(c)) {
            return Some(Cow::Borrowed(lit.inner));
        }
        self.decode(lit.inner, lit.open.chars().next().unwrap_or('\0'))
    }

    /// Calls `found(byte offset of the escape character, escaped char)` for every escape of
    /// `inner` (written between the delimiter `quote`) that the row does not define: a useless one.
    pub fn unknown_escapes(&self, inner: &str, quote: char, mut found: impl FnMut(usize, char)) {
        let Some(esc) = self.escape else { return };
        let mut chars = inner.char_indices();
        while let Some((i, c)) = chars.next() {
            if c != esc {
                continue;
            }
            let Some((_, e)) = chars.next() else { continue };
            let known = e == quote
                || e == esc
                || self.map.iter().any(|m| m.0 == e)
                || self.hex.iter().any(|h| h.0 == e)
                || self.named == Some(e)
                || (self.octal > 0 && e.is_digit(8))
                || (self.continuation && matches!(e, '\n' | '\r' | '\u{2028}' | '\u{2029}'));
            if !known {
                found(i, e);
            }
        }
    }

    /// The content of a string with the escapes and doubled delimiters resolved; `quote` is the
    /// opening delimiter's first character. `None`: the string has no value.
    pub fn decode<'a>(&self, inner: &'a str, quote: char) -> Option<Cow<'a, str>> {
        let double = self.doubled && quote != '\0';
        let plain = !self.escape.is_some_and(|e| inner.contains(e))
            && !(double && inner.contains(quote))
            && !(self.fold && inner.contains(['\n', '\r']));
        if plain {
            return Some(Cow::Borrowed(inner));
        }
        let chars: Vec<char> = inner.chars().collect();
        let mut out = String::with_capacity(inner.len());
        // Output length that folding must not trim (escapes and the text before them).
        let mut protected = 0;
        let mut i = 0;
        while i < chars.len() {
            let c = chars[i];
            if double && c == quote && chars.get(i + 1) == Some(&quote) {
                out.push(quote);
                i += 2;
            } else if Some(c) == self.escape {
                i = self.escape_at(&chars, i, &mut out)?;
                protected = out.len();
            } else if self.fold && matches!(c, '\n' | '\r') {
                while out.len() > protected && out.ends_with([' ', '\t']) {
                    out.pop();
                }
                let (next, empty) = skip_following(&chars, skip_break(&chars, i));
                if empty == 0 {
                    out.push(' ');
                } else {
                    out.extend(std::iter::repeat_n('\n', empty));
                }
                i = next;
                protected = out.len();
            } else {
                out.push(c);
                i += 1;
            }
        }
        Some(Cow::Owned(out))
    }

    /// Decodes the escape at `chars[i]`; returns the index after it.
    fn escape_at(&self, chars: &[char], i: usize, out: &mut String) -> Option<usize> {
        let Some(&e) = chars.get(i + 1) else {
            return (self.unknown != Unknown::Invalid).then(|| {
                out.push(chars[i]);
                i + 1
            });
        };
        let at = i + 2;
        let esc = chars[i];
        let bad = |out: &mut String| self.replacement.map(|r| out.push(r));
        if let Some(&(_, to)) = self.map.iter().find(|m| m.0 == e) {
            out.push(to);
        } else if let Some(&(_, n)) = self.hex.iter().find(|h| h.0 == e) {
            let digits = chars[at..]
                .iter()
                .take(n)
                .take_while(|c| c.is_ascii_hexdigit())
                .count();
            let code = (digits == n).then(|| hex_value(&chars[at..at + n]));
            let (code, end) = match code {
                Some(hi @ 0xD800..=0xDBFF) if self.surrogates => {
                    let low = (chars.get(at + 4..at + 6) == Some(&[esc, e][..]))
                        .then(|| chars.get(at + 6..at + 10))
                        .flatten()
                        .filter(|d| d.iter().all(char::is_ascii_hexdigit))
                        .map(hex_value)
                        .filter(|l| (0xDC00..=0xDFFF).contains(l));
                    match low {
                        Some(lo) => (
                            Some(0x10000 + ((hi - 0xD800) << 10) + (lo - 0xDC00)),
                            at + 10,
                        ),
                        None => (None, at + 4),
                    }
                }
                code => (code, at + digits),
            };
            match code.and_then(char::from_u32) {
                Some(ch) => out.push(ch),
                None => bad(out)?,
            }
            return Some(end);
        } else if self.named == Some(e) {
            let mut j = at;
            if chars.get(j) == Some(&'{') {
                while j < chars.len() && chars[j] != '}' {
                    j += 1;
                }
                j = (j + 1).min(chars.len());
            }
            bad(out)?;
            return Some(j);
        } else if self.octal > 0 && e.is_digit(8) {
            let mut v = e.to_digit(8).unwrap_or(0);
            let mut j = at;
            while j < at + self.octal - 1 && chars.get(j).is_some_and(|c| c.is_digit(8)) {
                v = v * 8 + chars[j].to_digit(8).unwrap_or(0);
                j += 1;
            }
            match char::from_u32(v) {
                Some(ch) => out.push(ch),
                None => bad(out)?,
            }
            return Some(j);
        } else if self.continuation && matches!(e, '\n' | '\r') {
            // No space; folding turns the empty lines after the break into line feeds.
            let next = skip_break(chars, i + 1);
            if !self.fold {
                return Some(next);
            }
            let (next, empty) = skip_following(chars, next);
            out.extend(std::iter::repeat_n('\n', empty));
            return Some(next);
        } else {
            match self.unknown {
                Unknown::Invalid => return None,
                Unknown::Keep => {
                    out.push(esc);
                    out.push(e);
                }
                Unknown::Drop => out.push(e),
            }
        }
        Some(at)
    }

    /// `value` written between the row's first delimiter: the delimiter and the escape character
    /// escaped (or the delimiter doubled). `None`: the row has no way to write the delimiter.
    pub fn encode(&self, value: &str) -> Option<String> {
        let quote = *self.quotes.first()?;
        let mut out = String::with_capacity(value.len() + 2);
        out.push(quote);
        for c in value.chars() {
            if c == quote && self.doubled {
                out.push(c);
            } else if c == quote || Some(c) == self.escape {
                out.push(self.escape?);
            }
            out.push(c);
        }
        out.push(quote);
        Some(out)
    }
}

fn hex_value(digits: &[char]) -> u32 {
    digits
        .iter()
        .fold(0, |v, c| v * 16 + c.to_digit(16).unwrap_or(0))
}

fn skip_break(chars: &[char], i: usize) -> usize {
    if chars[i] == '\r' && chars.get(i + 1) == Some(&'\n') {
        i + 2
    } else {
        i + 1
    }
}

/// Skips blanks, then empty lines with their blanks: the index of the next content and the number
/// of empty lines passed.
fn skip_following(chars: &[char], mut i: usize) -> (usize, usize) {
    let mut empty = 0;
    loop {
        while i < chars.len() && matches!(chars[i], ' ' | '\t') {
            i += 1;
        }
        if i < chars.len() && matches!(chars[i], '\n' | '\r') {
            i = skip_break(chars, i);
            empty += 1;
        } else {
            return (i, empty);
        }
    }
}

/// A number's text without its `_` separators (borrowed when it has none).
pub fn digits(raw: &str) -> Cow<'_, str> {
    if raw.contains('_') {
        Cow::Owned(raw.replace('_', ""))
    } else {
        Cow::Borrowed(raw)
    }
}

/// The value of an integer token (`1_000`, `0xff`, `0o17`, `0b11`); `None` for anything else.
pub fn integer(raw: &str) -> Option<i128> {
    let digits = digits(raw);
    let radix = |p: &str, base| i128::from_str_radix(digits.strip_prefix(p)?, base).ok();
    radix("0x", 16)
        .or_else(|| radix("0o", 8))
        .or_else(|| radix("0b", 2))
        .or_else(|| digits.parse().ok())
}

#[cfg(test)]
mod tests {
    use crate::Grammar;

    fn value(rows: &str, text: &str) -> Option<String> {
        let g = Grammar::parse(&format!("language t 1\ntoken S\ntoken Q\n{rows}")).unwrap();
        let kind = g.kind_by_name("S").unwrap();
        g.string_value(kind, text).map(|v| v.into_owned())
    }

    #[test]
    fn rows_decode_escapes_prefixes_and_doubled_quotes() {
        let json = r#"string S quote "\"" escape "\\" unknown drop replace 65533 surrogates map "n" 10 hex "u" 4"#;
        assert_eq!(
            value(json, r#""a\n\/\ud83d\ude00""#).unwrap(),
            "a\n/\u{1F600}"
        );
        assert_eq!(value(json, r#""\ud83d""#).unwrap(), "\u{FFFD}");
        let py = r#"string S quote "'" triple prefix "r" raw "r" escape "\\" continuation octal 3 replace 63 named "N" hex "x" 2"#;
        assert_eq!(value(py, r"'\101\x4\N{DOT}\q'").unwrap(), r"A??\q");
        assert_eq!(value(py, r"r'\n'").unwrap(), r"\n");
        let sql = r#"string S quote "'" double"#;
        assert_eq!(value(sql, "'it''s'").unwrap(), "it's");
        let strict = r#"string S quote "\"" escape "\\" unknown invalid"#;
        assert_eq!(value(strict, r#""\q""#), None);
    }

    #[test]
    fn encode_escapes_or_doubles_the_delimiter() {
        let g = Grammar::parse(
            "language t 1\ntoken S\nstring S quote \"'\" double\nstring S quote \"\\\"\" escape \"\\\\\"",
        )
        .unwrap();
        let rows: Vec<_> = g.string_rows(g.kind_by_name("S").unwrap()).collect();
        assert_eq!(rows[0].encode("it's").unwrap(), "'it''s'");
        assert_eq!(rows[1].encode("a\"b\\").unwrap(), r#""a\"b\\""#);
    }
}
