//! Backslash (or backtick) line continuations and heredoc bodies: the layout of one logical line
//! that the source splits over several physical lines (Dockerfile instructions, Makefile
//! statements, shell here-documents).
//!
//! concept: line-continuation-layout
//! what: Line continuations kept at the end of lines.
//! tools: shfmt, hadolint, checkmake

use crate::doc::{self, Doc};
use std::borrow::Cow;

/// One logical line: the parts up to the first continuation, then the indented rest.
#[derive(Default)]
pub struct Continued {
    head: Vec<Doc>,
    tail: Vec<Doc>,
    broken: bool,
    /// The blanks the next physical line starts with, at column 0 (as written).
    next_blanks: Option<String>,
}

impl Continued {
    /// Appends to the current physical line; right after [`Continued::end_line`] a new one starts.
    pub fn push(&mut self, d: Doc) {
        if self.broken {
            self.tail.push(doc::hardline());
            if let Some(blanks) = self.next_blanks.take() {
                self.tail.push(doc::line_start(blanks));
            }
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

    /// [`Continued::end_line`]; the next physical line starts with `blanks` at column 0.
    pub fn end_line_at(&mut self, marker: &str, blanks: String) {
        self.end_line(marker);
        self.next_blanks = Some(blanks);
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
                quote = quote_after(line, quote, esc);
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

/// The quote (`'` or `"`) that is open after `line`, given the one open before it; `esc` escapes
/// the next character outside single quotes.
fn quote_after(line: &str, mut quote: Option<char>, esc: char) -> Option<char> {
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
    quote
}

/// A heredoc body and its delimiter line as written, at column 0 from the next line break on.
pub fn heredoc(raw: &str) -> Doc {
    doc::line_suffix(doc::line_start(raw))
}

/// What ends the last item of a container: the item's own terminator `existing` (written as is), or,
/// when it has none and the string option `option` is `add`, the fixed text of the `#optional`
/// token `kind` (`;;` ending the last `case` arm). It follows a blank, or a line break with
/// `own_line`. `None`: nothing is written.
pub fn item_terminator(
    cx: &crate::FormatCtx<'_>,
    existing: Option<Doc>,
    (option, kind): (core_config::OptId, core_syntax::SyntaxKind),
    own_line: bool,
) -> Option<Doc> {
    let end = match existing {
        Some(d) => d,
        None if cx.options.is(option, "add") => doc::text(cx.grammar.def(kind)?.text.clone()?),
        None => return None,
    };
    let gap = if own_line {
        doc::hardline()
    } else {
        doc::lit(" ")
    };
    Some(doc::concat(vec![gap, end]))
}

/// Does the heredoc operator of the node that holds `body` (a body or delimiter token) read
/// `dash_op` (`<<-`, whose body and delimiter ignore leading tabs)?
fn is_dash_heredoc(
    body: &core_syntax::SyntaxToken,
    op: core_syntax::SyntaxKind,
    dash_op: &str,
) -> bool {
    body.parent().is_some_and(|p| {
        p.children_with_tokens()
            .filter_map(|e| e.into_token())
            .any(|t| t.kind() == op && t.text() == dash_op)
    })
}

/// The value of a heredoc body or delimiter token for the meaning check: the text, but of a
/// `dash_op` heredoc without the leading tabs of its lines (a plain heredoc keeps them: they are
/// content).
pub fn heredoc_value<'a>(
    body: &'a core_syntax::SyntaxToken,
    op: core_syntax::SyntaxKind,
    dash_op: &str,
) -> std::borrow::Cow<'a, str> {
    if !is_dash_heredoc(body, op, dash_op) {
        return body.text().into();
    }
    body.text()
        .split('\n')
        .map(|l| l.trim_start_matches('\t'))
        .collect::<Vec<_>>()
        .join("\n")
        .into()
}

/// A heredoc body and delimiter line (`raw`): with the string option `option` set to `reindent`,
/// tab indentation and the operator `op_text` being `dash_op`, [`heredoc_reindented`]; else
/// [`heredoc`].
pub fn heredoc_by_option(
    cx: &crate::FormatCtx<'_>,
    raw: &str,
    (op_text, dash_op): (&str, &str),
    option: core_config::OptId,
) -> Doc {
    if op_text == dash_op
        && cx.options.is(option, "reindent")
        && cx.options.is(core_config::opt::INDENT_STYLE, "tab")
    {
        heredoc_reindented(raw)
    } else {
        heredoc(raw)
    }
}

/// A `<<-` heredoc whose body and delimiter line lose their leading tabs (which `<<-` ignores):
/// the body is indented one level below the line of the command, the delimiter line is as deep.
fn heredoc_reindented(raw: &str) -> Doc {
    let mut lines: Vec<&str> = raw
        .split('\n')
        .map(|l| l.trim_start_matches('\t'))
        .collect();
    let end = lines.pop().unwrap_or_default();
    let body: Vec<Doc> = lines
        .into_iter()
        .flat_map(|l| {
            let line = if l.is_empty() {
                Doc::Nil
            } else {
                Doc::Verbatim(l.to_string())
            };
            [doc::hardline(), line]
        })
        .collect();
    doc::line_suffix(doc::concat(vec![
        doc::indent(doc::concat(body)),
        doc::hardline(),
        doc::text(end),
    ]))
}

/// Text of several lines as written, with no trailing blanks (but after a `\`, which they would
/// turn into a continuation) and no carriage returns; after the first line each starts at column 0
/// (`tab`: the first after a tab).
pub fn as_written(text: &str, tab: bool, reindent: Option<Reindent<'_>>) -> Doc {
    as_written_with(text, tab, None, reindent)
}

/// [`as_written`] for a value that the blanks at the end of each line belong to (a Makefile
/// variable's); `condense`: the blanks before the backslash that continues a line after the first
/// collapse to one (make reads them as one).
pub fn as_written_value(text: &str, condense: bool, reindent: Option<Reindent<'_>>) -> Doc {
    as_written_with(text, false, Some(condense), reindent)
}

/// How the lines after the first of a continued text take the indentation of the second.
#[derive(Clone, Copy)]
pub struct Reindent<'a> {
    /// The text stays as written when a line starts with one of these words (after blanks and
    /// `@-+`): the indentation of a shell block shows its nesting.
    pub keep_words: &'a [&'a str],
    /// A second line that starts with a space gives one tab (the text may be read as a command).
    pub tab_for_spaces: bool,
    /// The text is a command of a recipe: a line that starts inside a quote keeps its blanks (the
    /// shell reads them).
    pub recipe: bool,
}

/// The indentation that `blanks_of_second` gives the lines of a block.
pub fn block_indent(blanks_of_second: &str, tab_for_spaces: bool) -> String {
    if tab_for_spaces && blanks_of_second.starts_with(' ') {
        "\t".to_string()
    } else {
        blanks_of_second.to_string()
    }
}

/// The blanks a line starts with as its indentation: those of a line of only a backslash lose the
/// one blank that stands before the backslash.
fn blanks(line: &str) -> &str {
    let content = line.trim_start_matches([' ', '\t']);
    let blanks = &line[..line.len() - content.len()];
    if content == "\\" {
        blanks.strip_suffix(' ').unwrap_or(blanks)
    } else {
        blanks
    }
}

/// Does a line of `text` start with one of the words (after blanks and the `@-+` prefixes of a
/// command), as a whole word?
fn starts_with_word(text: &str, words: &[&str]) -> bool {
    text.split('\n').any(|l| {
        let l = l.trim_start_matches([' ', '\t', '@', '-', '+']);
        words.iter().any(|w| {
            l.strip_prefix(w)
                .is_some_and(|r| r.is_empty() || r.starts_with([' ', '\t', ';', '\\']))
        })
    })
}

/// The line with one blank before its continuation backslash, whatever it has now (make reads the
/// blanks around a backslash and line break as one); the line itself when it has no single
/// continuing backslash (a run of more keeps its text) or only the backslash.
fn condense_line(line: &str) -> Cow<'_, str> {
    let trimmed = line.trim_end_matches([' ', '\t']);
    let before =
        trimmed[..trimmed.len() - trailing_backslashes(trimmed)].trim_end_matches([' ', '\t']);
    if trailing_backslashes(trimmed) == 1 && !before.is_empty() {
        Cow::Owned(format!("{before} \\"))
    } else {
        Cow::Borrowed(line)
    }
}

/// `text` with the blanks before the backslash of each continued line collapsed to one.
pub fn condensed(text: &str) -> String {
    text.split('\n')
        .map(condense_line)
        .collect::<Vec<_>>()
        .join("\n")
}

/// Does the line end in a backslash right after a closing bracket?
fn after_reference(line: &str) -> bool {
    line.strip_suffix('\\')
        .is_some_and(|before| before.ends_with([')', '}']))
}

/// Does `text` hold several lines, each but the last ending in the backslash that continues it?
pub fn is_continued_text(text: &str) -> bool {
    let lines: Vec<&str> = text.split('\n').map(|l| l.trim_end_matches('\r')).collect();
    lines.len() > 1 && lines[..lines.len() - 1].iter().all(|l| continues(l))
}

/// `text` without the backslash that continues its last line, and the blanks before it (one
/// blank before it, or none, reads the same).
pub fn without_continuation(text: &str) -> &str {
    if continues(text) && trailing_backslashes(text) == 1 {
        text[..text.len() - 1].trim_end_matches([' ', '\t'])
    } else {
        text
    }
}

/// `text` with the lines after the first stripped of their leading blanks: what [`as_written_value`]
/// with `reindent` changes of a variable's value.
pub fn unindented(text: &str) -> String {
    let mut lines = text.split('\n');
    let first = lines.next().unwrap_or_default();
    std::iter::once(first)
        .chain(lines.map(|l| l.trim_start_matches([' ', '\t'])))
        .collect::<Vec<_>>()
        .join("\n")
}

/// The backslashes at the end of `line`.
fn trailing_backslashes(line: &str) -> usize {
    line.len() - line.trim_end_matches('\\').len()
}

/// A line continues when it ends in an odd number of backslashes (a pair is an escaped backslash).
fn continues(line: &str) -> bool {
    trailing_backslashes(line) % 2 == 1
}

/// `value`: `None` cuts the blanks at the end of each line; `Some(condense)` keeps them.
/// `reindent`: every line after the first starts with the blanks of the second, except a line that
/// starts inside a quote; the text stays as written when a line starts with one of the words.
fn as_written_with(
    text: &str,
    tab: bool,
    value: Option<bool>,
    reindent: Option<Reindent<'_>>,
) -> Doc {
    let mut parts = Vec::new();
    let first = text.split('\n').next().unwrap_or_default();
    let indent = reindent
        .filter(|r| !starts_with_word(text, r.keep_words))
        .filter(|r| !r.recipe || quote_after(first, None, '\\').is_none())
        .and_then(|r| {
            Some(block_indent(
                blanks(text.split('\n').nth(1)?),
                r.tab_for_spaces,
            ))
        });
    let quoted = reindent.is_some_and(|r| r.recipe);
    let mut quote = None::<char>;
    for (i, raw) in text.split('\n').enumerate() {
        let raw = raw.trim_end_matches('\r');
        let inside = quoted && quote.is_some();
        quote = quote_after(raw, quote, '\\');
        let raw = match &indent {
            Some(indent) if i > 0 && !inside && !raw.trim().is_empty() => {
                let content = raw.trim_start_matches([' ', '\t']);
                // (a line of only the backslash gets the blank before it too)
                let gap = if content == "\\" { " " } else { "" };
                format!("{indent}{gap}{content}")
            }
            _ => raw.to_string(),
        };
        let raw = raw.as_str();
        let trimmed = raw.trim_end_matches([' ', '\t']);
        let condense_recipe = quoted && i > 0 && !inside && quote.is_none();
        let line = match value {
            Some(true) if i > 0 && continues(trimmed) => condense_line(raw),
            Some(_) => Cow::Borrowed(raw),
            // (a backslash right after a `)` or `}` of a reference stays: the parser reads the
            // text after it as a new one, a blank in between as the same)
            None if condense_recipe && continues(trimmed) && !after_reference(trimmed) => {
                condense_line(raw)
            }
            None if continues(trimmed) => Cow::Borrowed(raw),
            None => Cow::Borrowed(trimmed),
        };
        let line = line.as_ref();
        // (blanks that are kept at the end of the text are written verbatim: the printer drops the others)
        let text_end = if value.is_some() {
            line.trim_end_matches([' ', '\t'])
        } else {
            line
        };
        parts.push(match (i, tab) {
            (0, false) => doc::text(text_end),
            (0, true) => doc::line_start(format!("\t{text_end}")),
            _ => doc::line_start(text_end),
        });
        if text_end.len() < line.len() {
            parts.push(doc::verbatim(&line[text_end.len()..]));
        }
    }
    // A last line that ends in `\` continues onto the next line, which stays empty: without it the
    // next statement would be swallowed.
    if continues(text) {
        parts.push(doc::line_start(""));
    }
    doc::concat(parts)
}

#[cfg(test)]
mod tests {
    use super::{
        Reindent, as_written, blanks, condensed, continues, is_continued_text,
        trailing_backslashes, unindented, without_continuation,
    };
    use crate::printer::{PrintOptions, print};

    #[test]
    fn backslashes_at_the_end_make_a_continuation_when_odd() {
        assert_eq!(trailing_backslashes("a \\\\\\"), 3);
        assert!(continues("a \\"));
        assert!(!continues("a \\\\"));
        assert!(continues("a\\\\\\"));
        assert!(!continues("a"));
    }

    #[test]
    fn condensed_collapses_the_blanks_before_a_continuation_backslash() {
        assert_eq!(condensed("a   \\\n b\t\\\nc"), "a \\\n b \\\nc");
        // no blank before it: one is added; a pair of backslashes does not continue; a run of three keeps its blanks; a line of only a backslash stays
        assert_eq!(condensed("a\\\n  \\\nb   \\\\"), "a \\\n  \\\nb   \\\\");
        assert_eq!(condensed("a  \\\\\\\nb"), "a  \\\\\\\nb");
        // the blanks at the end of the last line stay (they are part of a value)
        assert_eq!(condensed("a   \\\nb  "), "a \\\nb  ");
    }

    #[test]
    fn a_lone_backslash_line_loses_the_blank_before_it() {
        assert_eq!(blanks("\t \\"), "\t");
        assert_eq!(blanks("  b"), "  ");
    }

    #[test]
    fn the_end_of_a_continued_text_is_cut_and_the_indentation_dropped() {
        assert_eq!(without_continuation("a  \\"), "a");
        assert_eq!(without_continuation("a \\\\"), "a \\\\");
        assert_eq!(unindented("a\n  b\n\tc"), "a\nb\nc");
        assert!(is_continued_text("a \\\nb"));
        assert!(!is_continued_text("a\nb"));
        assert!(!is_continued_text("a \\"));
    }

    #[test]
    fn as_written_reindents_with_the_second_line_and_keeps_shell_blocks_and_quotes() {
        let r = |recipe| Reindent {
            keep_words: &["if"],
            tab_for_spaces: true,
            recipe,
        };
        let text = |t: &str, recipe| {
            print(
                &mut as_written(t, false, Some(r(recipe))),
                &PrintOptions::default(),
            )
        };
        assert_eq!(text("a \\\n  b \\\n\t\tc", false), "a \\\n\tb \\\n\tc");
        assert_eq!(text("if a \\\n  b \\\nfi", true), "if a \\\n  b \\\nfi");
        assert_eq!(
            text("echo 'a \\\n  b' \\\n c", true),
            "echo 'a \\\n  b' \\\n c"
        );
    }
}
