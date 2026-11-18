//! Backslash (or backtick) line continuations and heredoc bodies: the layout of one logical line
//! that the source splits over several physical lines (Dockerfile instructions, Makefile
//! statements, shell here-documents).

use crate::doc::{self, Doc};

/// One logical line: the parts up to the first continuation, then the indented rest.
#[derive(Default)]
pub struct Continued {
    head: Vec<Doc>,
    tail: Vec<Doc>,
    broken: bool,
}

impl Continued {
    /// Appends to the current physical line; right after [`Continued::end_line`] a new one starts.
    pub fn push(&mut self, d: Doc) {
        if self.broken {
            self.tail.push(doc::hardline());
            self.broken = false;
        }
        if self.tail.is_empty() {
            self.head.push(d);
        } else {
            self.tail.push(d);
        }
    }

    /// A blank between two parts, none at the start of a physical line.
    pub fn space(&mut self) {
        if !self.broken {
            self.push(doc::lit(" "));
        }
    }

    /// Ends the physical line with ` <marker>` (the escape character).
    pub fn end_line(&mut self, marker: &str) {
        self.push(doc::text(format!(" {marker}")));
        self.broken = true;
    }

    /// A line of its own in the indented part (a comment between continued lines).
    pub fn own_line(&mut self, d: Doc) {
        self.push(d);
        self.broken = true;
    }

    /// Text of several lines, a continuation ending each but the last: the first line goes on,
    /// every further line is indented and ends in ` \` (blanks before the escape character
    /// collapse), except inside a quote, where a line stays as written. A line that starts with
    /// `#` is a comment and never opens a quote.
    pub fn text(&mut self, text: &str) {
        let lines: Vec<&str> = text.split('\n').map(|l| l.trim_end_matches('\r')).collect();
        let esc = match lines[0].trim_end().chars().last() {
            Some('`') => '`',
            _ => '\\',
        };
        let mut quote = None::<char>;
        for (i, raw) in lines.iter().enumerate() {
            let verbatim = quote.is_some();
            let line = if verbatim { raw } else { raw.trim_start() };
            let comment = !verbatim && line.starts_with('#');
            if !comment {
                let mut chars = line.chars();
                while let Some(c) = chars.next() {
                    match quote {
                        _ if c == esc && quote != Some('\'') => {
                            chars.next();
                        }
                        None if c == '"' || c == '\'' => quote = Some(c),
                        Some(q) if c == q => quote = None,
                        _ => {}
                    }
                }
            }
            // A line that ends inside a quote keeps its blanks before the escape character.
            let body = if quote.is_some() {
                line.trim_end_matches([' ', '\t'])
            } else {
                line.trim_end()
            };
            let body = match body.strip_suffix(esc) {
                Some(before) if quote.is_none() && !comment => {
                    let before = before.trim_end();
                    if before.is_empty() {
                        esc.to_string()
                    } else {
                        format!("{before} {esc}")
                    }
                }
                _ => body.to_string(),
            };
            match (i, verbatim) {
                (0, _) => self.push(doc::text(body)),
                (_, true) => self.tail.push(doc::line_start(body)),
                _ => {
                    self.tail.push(doc::hardline());
                    self.tail.push(doc::text(body));
                }
            }
        }
    }

    pub fn finish(self) -> Doc {
        doc::concat(vec![
            doc::concat(self.head),
            doc::indent(doc::concat(self.tail)),
        ])
    }
}

/// A heredoc body and its delimiter line as written, at column 0 from the next line break on.
pub fn heredoc(raw: &str) -> Doc {
    doc::line_suffix(doc::line_start(raw))
}

/// Text of several lines as written, with no trailing blanks (but after a `\`, which they would
/// turn into a continuation) and no carriage returns; after the first line each starts at column 0
/// (`tab`: the first after a tab).
pub fn as_written(text: &str, tab: bool) -> Doc {
    let mut parts = Vec::new();
    for (i, raw) in text.split('\n').enumerate() {
        let raw = raw.trim_end_matches('\r');
        let trimmed = raw.trim_end_matches([' ', '\t']);
        let line = if trimmed.ends_with('\\') {
            raw
        } else {
            trimmed
        };
        parts.push(match (i, tab) {
            (0, false) => doc::text(line),
            (0, true) => doc::line_start(format!("\t{line}")),
            _ => doc::line_start(line),
        });
    }
    // A last line that ends in `\` continues onto the next line, which stays empty: without it the
    // next statement would be swallowed.
    if text.ends_with('\\') {
        parts.push(doc::line_start(""));
    }
    doc::concat(parts)
}
