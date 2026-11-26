//! Code held in the string literals of a data file (a notebook cell's `source`: one string, or an
//! array of strings that are its lines): the code is the decoded literals one after the other, an
//! offset of it maps back to the file through the escapes, and new code is written back in the
//! file's own form: its quote, its escape style, one literal per line and the separators between
//! them.
//!
//! concept: escaped-region
//! what: Code held in escaped string literals of a data file: decoded text, offset map, write-back in the file's form.
//! tools: none
//! reads: grammar:string

use crate::strings::StringSyntax;
use crate::{TextRange, TextSize};

/// One run of the decoded text and the file bytes it comes from: a plain run maps one to one, an
/// escape is one run.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Run {
    code: u32,
    host: u32,
    code_len: u32,
    host_len: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Escaped {
    syntax: StringSyntax,
    /// The literals, in file offsets, with their quotes.
    pub pieces: Vec<TextRange>,
    decoded: String,
    runs: Vec<Run>,
    /// The file writes non-ASCII characters as escapes.
    ascii: bool,
    /// The literals are the elements of an array.
    list: bool,
}

impl Escaped {
    /// The region made of the `pieces` of `text` (string tokens in file order) written with
    /// `syntax`; `None` when a literal has no value.
    pub fn new(
        syntax: &StringSyntax,
        pieces: Vec<TextRange>,
        list: bool,
        text: &str,
    ) -> Option<Self> {
        let mut e = Escaped {
            syntax: syntax.clone(),
            pieces,
            decoded: String::new(),
            runs: Vec::new(),
            ascii: false,
            list,
        };
        for p in e.pieces.clone() {
            let token = &text[p];
            let lit = syntax.split(token)?;
            let quote = lit.open.chars().next().unwrap_or('\0');
            let base = usize::from(p.start()) + token.len() - lit.inner.len() - lit.open.len();
            e.walk(lit.inner, quote, base)?;
        }
        Some(e)
    }

    fn walk(&mut self, inner: &str, quote: char, base: usize) -> Option<()> {
        let escape = self.syntax.escape;
        let mut at = 0;
        while at < inner.len() {
            let next = escape
                .and_then(|c| inner[at..].find(c))
                .map_or(inner.len(), |i| at + i);
            if next > at {
                self.run(&inner[at..next], next - at, base + at);
                at = next;
                continue;
            }
            let len = self.extent(inner, at);
            let piece = &inner[at..at + len];
            let value = self.syntax.decode(piece, quote)?;
            self.ascii |= !value.is_ascii();
            self.run(&value, len, base + at);
            at += len;
        }
        Some(())
    }

    fn run(&mut self, value: &str, host_len: usize, host: usize) {
        self.runs.push(Run {
            code: self.decoded.len() as u32,
            host: host as u32,
            code_len: value.len() as u32,
            host_len: host_len as u32,
        });
        self.decoded.push_str(value);
    }

    /// The length in bytes of the escape that starts at `inner[at]`: its letter, the digits of a
    /// hex escape, and the low surrogate that follows a high one.
    fn extent(&self, inner: &str, at: usize) -> usize {
        let s = &self.syntax;
        let hex = |at: usize| {
            let e = inner[at + 1..].chars().next()?;
            let &(_, n) = s.hex.iter().find(|h| h.0 == e)?;
            let from = at + 1 + e.len_utf8();
            let d = (inner[from..].bytes().take(n))
                .take_while(u8::is_ascii_hexdigit)
                .count();
            Some((
                from + d - at,
                u32::from_str_radix(&inner[from..from + d], 16).ok()?,
            ))
        };
        match hex(at) {
            Some((len, hi)) if s.surrogates && (0xD800..=0xDBFF).contains(&hi) => {
                let again = s.escape.is_some_and(|c| inner[at + len..].starts_with(c));
                len + again.then(|| hex(at + len)).flatten().map_or(0, |l| l.0)
            }
            Some((len, _)) => len,
            None => 1 + inner[at + 1..].chars().next().map_or(0, char::len_utf8),
        }
    }

    /// The code: the literals decoded and joined.
    pub fn code(&self) -> &str {
        &self.decoded
    }

    /// The file offset of `at`, an offset of the code; `end` for the end of a range, which stays
    /// with the run before it.
    pub fn to_host(&self, at: TextSize, end: bool) -> TextSize {
        let at = u32::from(at);
        let i = self
            .runs
            .partition_point(|r| r.code + r.code_len < at + u32::from(!end));
        let Some(r) = self.runs.get(i).or(self.runs.last()) else {
            return TextSize::from(0);
        };
        let inside = at.saturating_sub(r.code).min(r.code_len);
        let host = if r.code_len == r.host_len {
            r.host + inside
        } else if inside >= r.code_len && r.code_len > 0 {
            r.host + r.host_len
        } else {
            r.host
        };
        TextSize::from(host)
    }

    /// Do the literals of `written` decode to `code` together?
    pub fn reads_back(&self, written: &str, code: &str) -> bool {
        let mut joined = String::with_capacity(code.len());
        for piece in self.syntax.pieces(written) {
            match self.syntax.value(piece) {
                Some(v) => joined.push_str(&v),
                None => return false,
            }
        }
        joined == code
    }

    /// The text that replaces `range` (the region: the literal, or the brackets of the array)
    /// to hold `code`: the same form as `text` has there.
    pub fn write(&self, range: TextRange, text: &str, code: &str) -> String {
        let quote = self
            .pieces
            .first()
            .and_then(|p| text[*p].chars().next())
            .unwrap_or('"');
        let literal = |s: &str| format!("{quote}{}{quote}", self.escape(s, quote));
        let Some(first) = self.pieces.first() else {
            return text[range].to_string();
        };
        let whole = &text[range];
        let open = usize::from(first.start() - range.start());
        let close = usize::from(range.end() - self.pieces[self.pieces.len() - 1].end());
        let (head, tail) = (&whole[..open], &whole[whole.len() - close..]);
        if !self.list {
            return format!("{head}{}{tail}", literal(code));
        }
        let lines: Vec<&str> = code.split_inclusive('\n').collect();
        let sep = match &self.pieces[..] {
            [a, b, ..] => text[TextRange::new(a.end(), b.start())].to_string(),
            _ => {
                // One literal for several lines: start each on a line of its own, as the first.
                let gap = head.trim_start_matches('[');
                format!(",{}", if gap.contains('\n') { gap } else { " " })
            }
        };
        let items: Vec<String> = lines.iter().map(|l| literal(l)).collect();
        format!("{head}{}{tail}", items.join(&sep))
    }

    /// `value` as the inside of a literal: the quote, the escape character and the controls
    /// escaped, non-ASCII characters too when the file does.
    fn escape(&self, value: &str, quote: char) -> String {
        let s = &self.syntax;
        let escape = s.escape.unwrap_or('\\');
        let hex = s.hex.first();
        let mut out = String::with_capacity(value.len());
        let unit = |out: &mut String, u: u32| {
            if let Some(&(letter, digits)) = hex {
                out.push_str(&format!("{escape}{letter}{u:0digits$x}"));
            }
        };
        for c in value.chars() {
            let named = s.map.iter().find(|m| m.1 == c && (m.1 as u32) < 0x20);
            if c == quote || c == escape {
                out.push(escape);
                out.push(c);
            } else if let Some(&(letter, _)) = named {
                out.push(escape);
                out.push(letter);
            } else if (c as u32) < 0x20 || (self.ascii && !c.is_ascii()) {
                let mut units = [0u16; 2];
                c.encode_utf16(&mut units)
                    .iter()
                    .for_each(|&u| unit(&mut out, u32::from(u)));
            } else {
                out.push(c);
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn json() -> StringSyntax {
        let row = r#"string S quote "\"" escape "\\" unknown drop replace 65533 surrogates map "n" 10 "t" 9 "r" 13 "b" 8 "f" 12 "/" 47 hex "u" 4"#;
        let g = crate::Grammar::parse(&format!("language t 1\ntoken S\n{row}")).unwrap();
        g.string_rows(g.kind_by_name("S").unwrap())
            .next()
            .unwrap()
            .clone()
    }

    fn region(text: &str, pieces: &[(u32, u32)]) -> Escaped {
        let list = text.starts_with('[');
        let pieces = pieces
            .iter()
            .map(|&(a, b)| TextRange::new(a.into(), b.into()))
            .collect();
        Escaped::new(&json(), pieces, list, text).expect("decodes")
    }

    #[test]
    fn decodes_and_maps_offsets() {
        let b = '\\';
        let text = format!("\"a{b}n{b}u00e9{b}ud83d{b}ude00b\"");
        let e = region(&text, &[(0, text.len() as u32)]);
        assert_eq!(e.code(), "a\n\u{e9}\u{1f600}b");
        assert_eq!(u32::from(e.to_host(1.into(), false)), 2);
        assert_eq!(u32::from(e.to_host(2.into(), false)), 4);
        assert_eq!(u32::from(e.to_host(2.into(), true)), 4);
        assert_eq!(u32::from(e.to_host(4.into(), false)), 10);
        assert_eq!(u32::from(e.to_host(8.into(), false)), 22);
    }

    #[test]
    fn writes_the_form_of_the_file() {
        let text = "[\n  \"a\\n\",\n  \"b\"\n]";
        let e = region(text, &[(4, 9), (13, 16)]);
        let range = TextRange::new(0.into(), (text.len() as u32).into());
        assert_eq!(
            e.write(range, text, "x\ny\nz"),
            "[\n  \"x\\n\",\n  \"y\\n\",\n  \"z\"\n]"
        );
        let one = "\"a\\tb\"";
        let e = region(one, &[(0, 6)]);
        let range = TextRange::new(0.into(), 6.into());
        assert_eq!(e.write(range, one, "a\n\"b"), "\"a\\n\\\"b\"");
    }

    #[test]
    fn escapes_non_ascii_when_the_file_does() {
        let b = '\\';
        let text = format!("\"{b}u00e9\"");
        let want = format!("\"{b}u00e9{b}ud83d{b}ude00\"");
        let e = region(&text, &[(0, 8)]);
        let range = TextRange::new(0.into(), 8.into());
        assert_eq!(e.write(range, &text, "\u{e9}\u{1f600}"), want);
        let raw = "\"\u{e9}\"";
        let e = region(raw, &[(0, raw.len() as u32)]);
        let range = TextRange::new(0.into(), (raw.len() as u32).into());
        assert_eq!(
            e.write(range, raw, "\u{e9}\u{1f600}"),
            "\"\u{e9}\u{1f600}\""
        );
    }
}
