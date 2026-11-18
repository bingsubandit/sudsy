//! What adapters of external parsers read from a grammar about the text between tokens and the
//! size of what the parser may be given: the `trivia` rows (comments and line continuations) and
//! the `limits` row.

use crate::kind::SyntaxKind;

/// One `trivia <Token> attribute...` row: the text that is a `kind` token between two tokens.
#[derive(Clone, Debug, Default)]
pub struct TriviaRow {
    pub kind: Option<SyntaxKind>,
    pub(crate) token: String,
    /// `line "//"...`: a comment from the prefix to the end of the line.
    pub line: Vec<String>,
    /// `not "#["...`: texts that are no line comment although they start with a prefix.
    pub not: Vec<String>,
    /// `first "#!"...`: a line comment that may only start the file.
    pub first: Vec<String>,
    /// `block "/*" "*/"`: a comment up to the close (or the end of the gap when there is none).
    pub block: Option<(String, String)>,
    /// `nested`: block comments inside a block comment.
    pub nested: bool,
    /// `lines "=begin" "=end"`: a block that starts at the start of a line and ends with the line
    /// that starts with the close.
    pub lines: Option<(String, String)>,
    /// `continuation`: a `\` that ends a line.
    pub continuation: bool,
    /// `unicode`: a line also ends at U+2028 and U+2029.
    pub unicode: bool,
}

/// The `trivia` rows of a grammar and a table of the bytes they can start with.
#[derive(Clone, Debug)]
pub struct TriviaSyntax {
    pub rows: Vec<TriviaRow>,
    starts: [bool; 256],
}

impl Default for TriviaSyntax {
    fn default() -> Self {
        TriviaSyntax {
            rows: Vec::new(),
            starts: [false; 256],
        }
    }
}

impl TriviaSyntax {
    pub(crate) fn new(rows: Vec<TriviaRow>) -> Self {
        let mut starts = [false; 256];
        for r in &rows {
            let texts = (r.line.iter().chain(&r.first))
                .map(String::as_str)
                .chain(r.block.iter().map(|b| b.0.as_str()))
                .chain(r.lines.iter().map(|b| b.0.as_str()))
                .chain(r.continuation.then_some("\\"));
            for t in texts {
                starts[t.as_bytes()[0] as usize] = true;
            }
        }
        TriviaSyntax { rows, starts }
    }

    /// The trivia token at the start of `rest`, the text of `src` from `pos` to the next token:
    /// its kind and length.
    pub fn scan(&self, src: &str, rest: &str, pos: usize) -> Option<(SyntaxKind, usize)> {
        if !self.starts[*rest.as_bytes().first()? as usize] {
            return None;
        }
        self.rows.iter().find_map(|r| {
            let len = r.scan(src, rest, pos)?;
            Some((r.kind?, len))
        })
    }

    /// The length of the block comment that `rest` starts with, when a row has one and the
    /// comment is closed.
    pub fn block_end(&self, rest: &str) -> Option<usize> {
        self.rows.iter().find_map(|r| r.block_end(rest))
    }
}

impl TriviaRow {
    fn scan(&self, src: &str, rest: &str, pos: usize) -> Option<usize> {
        let line_end = |s: &str| {
            let end = if self.unicode {
                s.find(['\n', '\r', '\u{2028}', '\u{2029}'])
            } else {
                s.find(['\n', '\r'])
            };
            end.unwrap_or(s.len())
        };
        let starts = |p: &String| rest.starts_with(p.as_str());
        if (self.line.iter().any(starts) && !self.not.iter().any(starts))
            || (pos == 0 && self.first.iter().any(starts))
        {
            return Some(line_end(rest));
        }
        if self.block.as_ref().is_some_and(|b| starts(&b.0)) {
            return Some(self.block_end(rest).unwrap_or(rest.len()));
        }
        if let Some((open, close)) = &self.lines
            && rest.starts_with(open.as_str())
            && (pos == 0 || src.as_bytes()[pos - 1] == b'\n')
        {
            let end = rest
                .match_indices('\n')
                .find(|(i, _)| rest[i + 1..].starts_with(close.as_str()));
            return Some(end.map_or(rest.len(), |(i, _)| i + 1 + line_end(&rest[i + 1..])));
        }
        if self.continuation {
            let after = rest.strip_prefix('\\')?;
            let n = if after.starts_with("\r\n") { 2 } else { 1 };
            return after.starts_with(['\n', '\r']).then_some(1 + n);
        }
        None
    }

    fn block_end(&self, rest: &str) -> Option<usize> {
        let (open, close) = self.block.as_ref()?;
        if !rest.starts_with(open.as_str()) {
            return None;
        }
        if !self.nested {
            return rest[open.len()..]
                .find(close.as_str())
                .map(|i| i + open.len() + close.len());
        }
        let (open, close, b) = (open.as_bytes(), close.as_bytes(), rest.as_bytes());
        let (mut depth, mut i) = (0, 0);
        while i < b.len() {
            if b[i] == open[0] && b[i..].starts_with(open) {
                depth += 1;
                i += open.len();
            } else if b[i] == close[0] && b[i..].starts_with(close) {
                depth -= 1;
                i += close.len();
                if depth == 0 {
                    return Some(i);
                }
            } else {
                i += 1;
            }
        }
        None
    }
}

/// The `limits` row (`run` is a table of the bytes of `run "chars"`): what a parser that recurses is given. Sizes are in MB.
#[derive(Clone, Debug)]
pub struct Limits {
    /// `stack`: the stack of the thread the parser runs on (0: the caller's thread).
    pub stack_mb: usize,
    /// `bytes`: a larger file is not parsed (0: no limit).
    pub bytes_mb: usize,
    /// `depth`: the deepest bracket nesting, and the longest run of the `run` characters (0: no
    /// limit).
    pub depth: usize,
    pub run: [bool; 256],
    /// `tagged "@" 12`: the most `(` groups nested that follow a name that follows the mark.
    pub tagged: Option<(u8, usize)>,
}

impl Default for Limits {
    fn default() -> Self {
        Limits {
            stack_mb: 0,
            bytes_mb: 0,
            depth: 0,
            run: [false; 256],
            tagged: None,
        }
    }
}

impl Limits {
    /// Why a parser must not run on `src`, when it must not.
    pub fn refuse(&self, src: &str) -> Option<String> {
        if self.bytes_mb > 0 && src.len() > self.bytes_mb << 20 {
            return Some(format!("file over {} MB", self.bytes_mb));
        }
        if self.depth > 0 {
            return self.too_deep(src.as_bytes()).map(String::from);
        }
        None
    }

    /// A rough scan: strings and comments are not skipped, which only matters for text with
    /// thousands of unclosed brackets.
    fn too_deep(&self, b: &[u8]) -> Option<&'static str> {
        let (mark, most) = self.tagged.unwrap_or((0, usize::MAX));
        let (limit, run_of) = (self.depth, &self.run);
        let (mut depth, mut run, mut live) = (0usize, 0usize, 0usize);
        let mut tagged: Vec<bool> = Vec::new();
        for (i, &c) in b.iter().enumerate() {
            match c {
                b'(' | b'[' | b'{' => {
                    run = 0;
                    depth += 1;
                    if depth > limit {
                        return Some("nesting too deep");
                    }
                    let mut is = false;
                    if c == b'(' {
                        let mut j = i;
                        while j > 0
                            && (b[j - 1].is_ascii_alphanumeric() || matches!(b[j - 1], b'_' | b'.'))
                        {
                            j -= 1;
                        }
                        is = j < i && j > 0 && b[j - 1] == mark;
                    }
                    tagged.push(is);
                    live += usize::from(is);
                    if live > most {
                        return Some("tagged arguments nested too deeply");
                    }
                }
                b')' | b']' | b'}' => {
                    run = 0;
                    depth = depth.saturating_sub(1);
                    live -= usize::from(tagged.pop().unwrap_or(false));
                }
                b' ' => {}
                _ if run_of[c as usize] => {
                    run += 1;
                    if run > limit {
                        return Some("nesting too deep");
                    }
                }
                _ => run = 0,
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nested_block_ends_at_its_own_close_over_multibyte_text() {
        let row = TriviaRow {
            block: Some(("/*".into(), "*/".into())),
            nested: true,
            ..TriviaRow::default()
        };
        assert_eq!(row.block_end("/* é /* ü */ */ x"), Some(17));
        assert_eq!(row.block_end("/* /* */"), None);
    }

    #[test]
    fn limits_refuse_deep_nesting_and_big_runs() {
        let mut l = Limits {
            depth: 3,
            ..Limits::default()
        };
        l.run[b'!' as usize] = true;
        assert!(l.refuse("((()))").is_none());
        assert!(l.refuse("(((())))").is_some());
        assert!(l.refuse("!!!! x").is_some());
    }
}
