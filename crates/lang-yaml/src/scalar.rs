//! Scalar values: flow scalars (the grammar's `string` rows) and block scalar semantics.

use core_syntax::{SyntaxKind, grammars};
use std::borrow::Cow;

/// Splits at line breaks (`\n`, `\r\n`, `\r`).
fn split_lines(text: &str) -> Vec<&str> {
    let mut lines = Vec::new();
    let mut start = 0;
    let b = text.as_bytes();
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'\n' => {
                lines.push(&text[start..i]);
                i += 1;
                start = i;
            }
            b'\r' => {
                lines.push(&text[start..i]);
                i += if b.get(i + 1) == Some(&b'\n') { 2 } else { 1 };
                start = i;
            }
            _ => i += 1,
        }
    }
    lines.push(&text[start..]);
    lines
}

/// The value of a flow scalar token (plain, single or double quoted: the grammar's `string`
/// rows fold its lines and resolve its escapes); `None` if an escape is invalid.
pub fn flow_value(kind: SyntaxKind, token: &str) -> Option<Cow<'_, str>> {
    grammars::YAML.get().string_value(kind, token)
}

/// `value` written as a flow scalar of the quoted kind `kind`.
pub fn quoted(kind: SyntaxKind, value: &str) -> Option<String> {
    grammars::YAML.get().string_rows(kind).next()?.encode(value)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Chomp {
    Strip,
    Clip,
    Keep,
}

/// A parsed block scalar header.
#[derive(Clone, Copy, Debug)]
pub struct BlockHeader {
    pub folded: bool,
    pub chomp: Chomp,
    pub indent: Option<usize>,
}

pub fn block_header(text: &str) -> BlockHeader {
    let mut h = BlockHeader {
        folded: text.starts_with('>'),
        chomp: Chomp::Clip,
        indent: None,
    };
    for c in text.chars().skip(1) {
        match c {
            '+' => h.chomp = Chomp::Keep,
            '-' => h.chomp = Chomp::Strip,
            '1'..='9' => h.indent = Some(c as usize - '0' as usize),
            _ => {}
        }
    }
    h
}

/// The value of a block scalar. `content` is the content token (indentation included) and `n`
/// the indentation of the parent node (-1 at the document level). `blank_after` is the number of
/// blank lines after the token, which only `keep` chomping cares about. The end of the input
/// counts as a line break.
pub fn block_value(header: BlockHeader, content: &str, n: i64, blank_after: usize) -> String {
    if content.is_empty() {
        return match header.chomp {
            Chomp::Keep => "
"
            .repeat(blank_after),
            _ => String::new(),
        };
    }
    // Lines, each with whether a line break ended it.
    let mut lines = split_lines(content);
    let ends_with_break = content.ends_with(['\n', '\r']);
    if ends_with_break {
        lines.pop();
    }
    let indent = match header.indent {
        Some(d) => (n + d as i64).max(0) as usize,
        // Without a line of text every line is blank, whatever its spaces.
        None => lines
            .iter()
            .find(|l| l.bytes().any(|c| c != b' '))
            .map_or(usize::MAX, |l| l.bytes().take_while(|c| *c == b' ').count()),
    };
    let stripped: Vec<&str> = lines
        .iter()
        .map(|l| {
            let spaces = l.bytes().take_while(|c| *c == b' ').count();
            &l[spaces.min(indent)..]
        })
        .collect();
    let last_text = stripped.iter().rposition(|l| !l.is_empty());
    let trailing_blank = stripped.len() - last_text.map_or(0, |i| i + 1) + blank_after;
    let mut body = String::new();
    if let Some(last) = last_text {
        if header.folded {
            let mut prev_spaced = false;
            let mut blanks = 0;
            let mut first = true;
            for line in &stripped[..=last] {
                if line.is_empty() {
                    blanks += 1;
                    continue;
                }
                let spaced = line.starts_with([' ', '\t']);
                if first {
                    body.extend(std::iter::repeat_n('\n', blanks));
                } else if !prev_spaced && !spaced {
                    if blanks == 0 {
                        body.push(' ');
                    } else {
                        body.extend(std::iter::repeat_n('\n', blanks));
                    }
                } else {
                    body.extend(std::iter::repeat_n('\n', blanks + 1));
                }
                body.push_str(line);
                prev_spaced = spaced;
                blanks = 0;
                first = false;
            }
        } else {
            body = stripped[..=last].join("\n");
        }
    }
    let final_break = true;
    match header.chomp {
        Chomp::Strip => {}
        Chomp::Clip => {
            if last_text.is_some() && final_break {
                body.push('\n');
            }
        }
        Chomp::Keep => {
            if last_text.is_some() && final_break {
                body.push('\n');
            }
            body.extend(std::iter::repeat_n('\n', trailing_blank));
        }
    }
    body
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block(header: &str, content: &str, n: i64) -> String {
        block_value(block_header(header), content, n, 0)
    }

    #[test]
    fn flow_scalars_fold_lines() {
        use crate::generated::kind::{DOUBLE_QUOTED as D, PLAIN, SINGLE_QUOTED as S};
        let v = |k, t| flow_value(k, t).map(Cow::into_owned);
        assert_eq!(v(PLAIN, "a\n  b\n\n  c").unwrap(), "a b\nc");
        assert_eq!(v(S, "'it''s\n  ok'").unwrap(), "it's ok");
        assert_eq!(v(D, "\"a \\\n   b\"").unwrap(), "a b");
        assert_eq!(v(D, "\"a\\x41\\u00e9\\n\"").unwrap(), "aAé\n");
        assert_eq!(v(D, "\"a  \n  b\"").unwrap(), "a b");
        assert!(v(D, "\"\\q\"").is_none());
    }

    #[test]
    fn literal_scalars_follow_the_chomping_indicator() {
        assert_eq!(block("|", "  a\n  b", 0), "a\nb\n");
        assert_eq!(block("|-", "  a\n  b", 0), "a\nb");
        assert_eq!(block("|+", "  a\n\n\n", 0), "a\n\n\n");
        assert_eq!(block("|2", "   a", 0), " a\n");
    }

    #[test]
    fn folded_scalars_keep_more_indented_lines() {
        assert_eq!(block(">", "  a\n  b\n\n  c", 0), "a b\nc\n");
        assert_eq!(block(">", "  a\n   b\n  c", 0), "a\n b\nc\n");
    }
}
