//! The lines of a doc comment (`/** ... */`): where each starts, where its text starts after the
//! leading asterisk, and the text with the decoration removed. The parser behind the checks of
//! [`crate::doc_style`].
//!
//! concept: doc-comment-style
//! what: A doc comment split into lines of text with the decoration removed.

/// One line of a doc comment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocLine<'a> {
    /// The offset of the line's first character (in the file).
    pub start: usize,
    /// The offset of the first character of the text (after the asterisk and the blanks).
    pub text_start: usize,
    /// The text, without the decoration and the trailing blanks.
    pub text: &'a str,
    /// The line starts with an asterisk (the first line starts after the opening instead).
    pub star: bool,
    /// The number of blanks between the asterisk and the text.
    pub indent: usize,
    /// The line starts inside an inline tag (`{@code` ... `}`) of an earlier line.
    pub inline: bool,
}

impl DocLine<'_> {
    pub fn blank(&self) -> bool {
        self.text.is_empty()
    }

    /// The line is a block tag: `@param x`.
    pub fn block_tag(&self) -> bool {
        !self.inline
            && self
                .text
                .strip_prefix('@')
                .is_some_and(|r| r.starts_with(|c: char| c.is_ascii_alphabetic()))
    }
}

/// The lines of the doc comment `comment` (the text of the token, which starts at `base`).
pub fn lines(comment: &str, base: usize) -> Vec<DocLine<'_>> {
    let open = if comment.starts_with("/**") { 3 } else { 2 };
    let end = if comment.len() >= open + 2 && comment.ends_with("*/") {
        comment.len() - 2
    } else {
        comment.len()
    };
    let mut out = Vec::new();
    let mut at = open;
    let mut first = true;
    let mut braces = 0u32;
    while at <= end {
        let stop = comment[at..end].find('\n').map_or(end, |i| at + i);
        let raw = comment[at..stop].trim_end_matches(['\r', ' ', '\t']);
        let lead = raw.len() - raw.trim_start_matches([' ', '\t']).len();
        let body = &raw[lead..];
        let (star, after) = match body.strip_prefix('*') {
            Some(r) if !first => (true, r),
            _ => (false, body),
        };
        let indent = after.len() - after.trim_start_matches([' ', '\t']).len();
        let text = &after[indent..];
        let skipped = raw.len() - text.len();
        out.push(DocLine {
            start: base + at,
            text_start: base + at + skipped,
            text,
            star,
            indent,
            inline: braces > 0,
        });
        braces = inline_depth(text, braces);
        first = false;
        at = stop + 1;
    }
    out
}

/// The depth of `{@tag ...}` nesting after `text`, starting from `depth`.
fn inline_depth(text: &str, mut depth: u32) -> u32 {
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'{' if depth == 0 && bytes.get(i + 1) == Some(&b'@') => depth = 1,
            b'{' if depth > 0 => depth += 1,
            b'}' if depth > 0 => depth -= 1,
            _ => {}
        }
        i += 1;
    }
    depth
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_lines() {
        let c = "/**\n * One.\n *\n *   @param x y\n */";
        let l = lines(c, 10);
        let texts: Vec<_> = l.iter().map(|l| l.text).collect();
        assert_eq!(texts, ["", "One.", "", "@param x y", ""]);
        assert_eq!(l[1].start, 10 + 4);
        assert_eq!(l[1].text_start, 10 + 7);
        assert!(l[3].block_tag() && l[3].indent == 3 && l[1].star);
        let one = lines("/** Summary. */", 0);
        assert_eq!(one.len(), 1);
        assert_eq!(
            (one[0].text, one[0].star, one[0].text_start),
            ("Summary.", false, 4)
        );
    }
}
