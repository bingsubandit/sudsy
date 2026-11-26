//! Template actions in markup (`{{ if .x }}` of Go templates and the like): text a parser keeps
//! as written, with no markup of its own. A block of actions (an opener word up to a closer
//! word) must open and close inside one element, so the tree of the page without the actions is
//! the tree of every page they produce. The language gives its delimiters and words as an
//! [`Actions`] value; the shared code finds where an action ends (a code action, such as Mako's
//! `${}`, past the string literals of its language's `string` row, its comments and brackets),
//! skips actions inside a quoted value, classifies an action by its first word and counts the
//! open blocks of each owner.
//!
//! concept: template-actions
//! what: Delimited template actions in markup, their blocks counted per owning element.
//! tools: html-eslint indent, helm lint
//! reads: option:dialect, grammar:string

use crate::strings::StringSyntax;
use std::collections::HashMap;

/// What an action does to the blocks of its owner.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Step {
    Open,
    Continue,
    Close,
}

/// The delimiters and block words of a template language.
pub struct Actions {
    pub open: &'static str,
    pub close: &'static str,
    /// Written right after the opening delimiter to trim whitespace (`-` of `{{- x }}`).
    pub trim: char,
    pub openers: &'static [&'static str],
    pub continuers: &'static [&'static str],
    pub closers: &'static [&'static str],
}

/// How the code inside an action is read when its end is searched: its string literals and
/// comments hide a delimiter, and with `nest` so do brackets.
pub struct Code<'a> {
    pub strings: &'a StringSyntax,
    /// The byte that starts a comment up to the end of the line.
    pub comment: Option<u8>,
    pub nest: bool,
}

/// Go templates (`text/template`: Hugo, Helm).
pub const GO: Actions = Actions {
    open: "{{",
    close: "}}",
    trim: '-',
    openers: &["if", "range", "with", "define", "block"],
    continuers: &["else"],
    closers: &["end"],
};

impl Actions {
    /// The end of the code of an action that starts at `from` (after the opening delimiter): the
    /// first of the `extra` terminators or the closing delimiter that is outside comments and
    /// string literals (and, with `code.nest`, outside brackets), and which one it is. A
    /// string that never closes is no string.
    pub fn code_end<'t>(
        &self,
        src: &str,
        from: usize,
        extra: &[&'t str],
        code: &Code,
    ) -> Option<(usize, &'t str)>
    where
        'static: 't,
    {
        let b = src.as_bytes();
        let mut i = from;
        let mut level: i32 = 0;
        while i < b.len() {
            if Some(b[i]) == code.comment {
                if let Some(n) = src[i..].find('\n') {
                    i += n + 1;
                    continue;
                }
            } else if code.strings.quotes.contains(&(b[i] as char)) {
                let (end, closed) = code.strings.scan(src, i, false);
                if closed {
                    i = end;
                    continue;
                }
            }
            let term = (extra.iter().copied())
                .chain([self.close])
                .find(|t| b[i..].starts_with(t.as_bytes()));
            if let Some(t) = term
                && !(code.nest && level > 0)
            {
                return Some((i, t));
            }
            match b[i] {
                b'{' | b'(' | b'[' => level += 1,
                b'}' | b')' | b']' => level -= 1,
                _ => {}
            }
            i += 1;
        }
        None
    }

    /// Does `src` have an action (its opening delimiter)?
    pub fn occurs_in(&self, src: &str) -> bool {
        src.contains(self.open)
    }

    /// The end of the action that starts at `at` (after its closing delimiter), when it closes.
    pub fn end(&self, src: &str, at: usize) -> Option<usize> {
        let rest = src.get(at..)?.strip_prefix(self.open)?;
        rest.find(self.close)
            .map(|i| at + self.open.len() + i + self.close.len())
    }

    /// The first word of the action `inner`, after the trim marker.
    pub fn first_word<'a>(&self, inner: &'a str) -> &'a str {
        let inner = inner.trim_start_matches(self.trim).trim_start();
        inner
            .split(|c: char| !c.is_ascii_alphabetic())
            .next()
            .unwrap_or("")
    }

    /// What the action `inner` (the text between the delimiters) does: its first word, after the
    /// trim marker, names it.
    pub fn step(&self, inner: &str) -> Option<Step> {
        let word = self.first_word(inner);
        let has = |words: &[&str]| words.contains(&word);
        if has(self.openers) {
            Some(Step::Open)
        } else if has(self.continuers) {
            Some(Step::Continue)
        } else if has(self.closers) {
            Some(Step::Close)
        } else {
            None
        }
    }

    /// The end of a quoted value after the closing `quote` (`from` is after the opening one);
    /// an action inside is skipped, so a quote it holds does not end the value.
    pub fn value_end(&self, src: &str, from: usize, quote: u8) -> Option<usize> {
        let b = src.as_bytes();
        let mut i = from;
        while i < b.len() {
            if let Some(end) = self.end(src, i) {
                i = end;
            } else if b[i] == quote {
                return Some(i + 1);
            } else {
                i += 1;
            }
        }
        None
    }

    /// The pieces of `text`, `(is an action, text)`, in order.
    pub fn pieces<'a>(&self, text: &'a str) -> Vec<(bool, &'a str)> {
        let mut out = Vec::new();
        let (mut from, mut at) = (0, 0);
        while at < text.len() {
            match (text.is_char_boundary(at), self.end(text, at)) {
                (true, Some(end)) => {
                    if from < at {
                        out.push((false, &text[from..at]));
                    }
                    out.push((true, &text[at..end]));
                    from = end;
                    at = end;
                }
                _ => at += 1,
            }
        }
        if from < text.len() {
            out.push((false, &text[from..]));
        }
        out
    }
}

/// The open blocks of each owner (an element, by an id of the parser's; the root has its own)
/// and where the last one opened.
#[derive(Default)]
pub struct Blocks(HashMap<usize, (i32, usize)>);

impl Blocks {
    /// Counts an action at `at` in `owner`; `false` when it continues or closes a block the
    /// owner has not opened.
    pub fn step(&mut self, owner: usize, at: usize, step: Step) -> bool {
        let entry = self.0.entry(owner).or_insert((0, at));
        match step {
            Step::Open => *entry = (entry.0 + 1, at),
            Step::Continue | Step::Close if entry.0 == 0 => return false,
            Step::Continue => {}
            Step::Close => entry.0 -= 1,
        }
        true
    }

    /// Where the blocks still open began.
    pub fn unclosed(&self) -> Vec<usize> {
        self.0
            .values()
            .filter(|&&(n, _)| n != 0)
            .map(|&(_, at)| at)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ends_and_steps() {
        assert_eq!(GO.end("a {{ x }} b", 2), Some(9));
        assert_eq!(GO.end("a {{ x", 2), None);
        assert_eq!(GO.step("- if .x"), Some(Step::Open));
        assert_eq!(GO.step(" end "), Some(Step::Close));
        assert_eq!(GO.step(" .x "), None);
    }

    #[test]
    fn value_skips_actions() {
        let s = r#"{{ "a" }}b" c"#;
        assert_eq!(GO.value_end(s, 0, b'"'), Some(s.len() - 2));
    }

    #[test]
    fn pieces_split() {
        assert_eq!(
            GO.pieces("a{{ b }}c"),
            vec![(false, "a"), (true, "{{ b }}"), (false, "c")]
        );
    }

    #[test]
    fn code_ends_outside_strings_comments_and_brackets() {
        let row = r#"string S quote "\"" "'" triple escape "\\" continuation"#;
        let g = crate::Grammar::parse(&format!("language t 1\ntoken S\n{row}")).unwrap();
        let strings = g.string_rows(g.kind_by_name("S").unwrap()).next().unwrap();
        let code = Code {
            strings,
            comment: Some(b'#'),
            nest: true,
        };
        let expr = Actions {
            open: "${",
            close: "}",
            ..GO
        };
        let s = r#"${ {"}": '|'} | h }"#;
        assert_eq!(expr.code_end(s, 2, &["|"], &code), Some((14, "|")));
        assert_eq!(
            expr.code_end("${ 'a\\'}' }", 2, &[], &code),
            Some((10, "}"))
        );
        assert_eq!(
            expr.code_end("${ x # }\n }", 2, &[], &code),
            Some((10, "}"))
        );
        assert_eq!(
            expr.code_end("${ \"\"\"}\"\"\" }", 2, &[], &code),
            Some((11, "}"))
        );
        // An unclosed quote is no string.
        assert_eq!(expr.code_end("${ don't }", 2, &[], &code), Some((9, "}")));
        let flat = Code {
            nest: false,
            ..code
        };
        let block = Actions {
            open: "<%",
            close: "%>",
            ..GO
        };
        assert_eq!(block.code_end("<% f(%>", 2, &[], &flat), Some((5, "%>")));
    }

    #[test]
    fn blocks_count_per_owner() {
        let mut b = Blocks::default();
        assert!(b.step(1, 0, Step::Open));
        assert!(!b.step(2, 5, Step::Close));
        assert_eq!(b.unclosed(), vec![0]);
        assert!(b.step(1, 9, Step::Close));
        assert!(b.unclosed().is_empty());
    }
}
