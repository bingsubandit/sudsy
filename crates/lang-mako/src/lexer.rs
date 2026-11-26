//! The lossless lexer of Mako (the rules of Mako 1.4's `lexer.py`): every byte of the input ends
//! up in exactly one token.
//!
//! The host text is one `Text` token per run of non-blank characters. `${`, `<%` and a valid
//! `</%name>` end a run; `%` and `##` at the start of a line (after blanks) start a control line and
//! a comment; `%%` there is text. `<%name attributes>` is a tag when it has the shape of one, any other `<%` is
//! a Python block that runs to the first `%>` outside Python strings and comments. A `${`
//! expression ends at the first `}` that is no part of nested brackets, and at a `|` that
//! starts its filters. Where a construct has no end, the opener is a token of its own and an error,
//! and the lexer goes on after it.
//!
//! concept: none (the tokens of Mako on the lexer base of core_syntax::parse)

use crate::generated::kind as k;
use core_syntax::SyntaxKind;
use core_syntax::grammars;
use core_syntax::parse::{
    self, ParseError, Token, escaped_break_at, is_blank_form as is_blank,
    is_blank_form_char as is_blank_char, is_space_form as is_space,
};
use core_syntax::template_actions::{Actions, Code};

pub fn lex(src: &str) -> (Vec<Token>, Vec<ParseError>) {
    let mut lx = Lexer {
        lx: parse::Lexer::new(src, src.len() / 4),
        line_first: true,
        code: Code {
            strings: grammars::MAKO.get().string_row(k::PY_STRING),
            comment: Some(b'#'),
            nest: true,
        },
    };
    while lx.lx.pos < src.len() {
        lx.next();
    }
    lx.lx.finish(k::EOF)
}

struct Lexer<'a> {
    lx: parse::Lexer<'a>,
    /// Only blanks stand between the position and the start of its line.
    line_first: bool,
    /// How the Python of `${}` and `<% %>` is read to find its end.
    code: Code<'static>,
}

impl Lexer<'_> {
    fn bytes(&self) -> &[u8] {
        self.lx.src.as_bytes()
    }

    fn rest(&self) -> &str {
        &self.lx.src[self.lx.pos..]
    }

    /// A token of `kind` from `start` to `end`; the position moves to `end`.
    fn put(&mut self, kind: SyntaxKind, start: usize, end: usize) {
        self.lx.pos = end;
        self.lx.emit(kind, start);
        self.line_first = false;
    }

    /// The blanks and line breaks of `start..end` as trivia tokens.
    fn blanks(&mut self, start: usize, end: usize) {
        self.lx.pos = start;
        while self.lx.pos < end {
            if !self.lx.blank(k::WHITESPACE, k::NEWLINE, is_blank) {
                break;
            }
            if matches!(self.bytes()[self.lx.pos - 1], b'\n' | b'\r') {
                self.line_first = true;
            }
        }
        self.lx.pos = end;
    }

    fn next(&mut self) {
        let start = self.lx.pos;
        let first = self.line_first;
        let c = self.bytes()[start];
        if is_blank(c) || c == b'\n' || c == b'\r' {
            let was = self.line_first;
            let mut end = start;
            if is_blank(c) {
                while self.bytes().get(end).is_some_and(|&b| is_blank(b)) {
                    end += 1;
                }
            } else {
                end += if self.rest().starts_with("\r\n") {
                    2
                } else {
                    1
                };
            }
            self.blanks(start, end);
            // Blanks keep the line's start; a line break starts a new one.
            self.line_first = was || c == b'\n' || c == b'\r';
            return;
        }
        let rest = self.rest();
        if first && c == b'%' && !rest.starts_with("%%") {
            return self.control_line(start);
        }
        if first && rest.starts_with("##") {
            self.put(k::HASH_HASH, start, start + 2);
            let mut text = start + 2;
            while self.bytes().get(text).is_some_and(|&b| is_blank(b)) {
                text += 1;
            }
            self.blanks(start + 2, text);
            self.lx.pos = text;
            self.lx.eat_line_escaped(Some(b'\\'));
            if self.lx.pos > text {
                self.put(k::COMMENT_TEXT, text, self.lx.pos);
            }
            return;
        }
        if rest.starts_with("${") {
            return self.expression(start);
        }
        if rest.starts_with("<%doc>")
            && let Some(at) = rest.find("</%doc>")
        {
            return self.put(k::DOC_COMMENT, start, start + at + "</%doc>".len());
        }
        if rest.starts_with("<%") {
            if let Some(tag) = scan_tag(self.lx.src, start) {
                return self.tag(start, &tag);
            }
            return self.python_block(start);
        }
        if let Some(end) = close_tag(self.lx.src, start) {
            return self.put(k::CLOSE_TAG, start, end);
        }
        let n = escaped_break_at(self.bytes(), start, b'\\');
        if n > 0 {
            self.put(k::CONTINUATION, start, start + n);
            // The next line starts after a line break.
            self.line_first = true;
            return;
        }
        self.text(start);
    }

    /// A run of text: up to a blank, a line break or the start of something else.
    fn text(&mut self, start: usize) {
        let src = self.lx.src;
        let b = src.as_bytes();
        let mut i = start;
        while i < b.len() {
            let c = b[i];
            if is_blank(c) || c == b'\n' || c == b'\r' {
                break;
            }
            if i > start
                && (src[i..].starts_with("${")
                    || src[i..].starts_with("<%")
                    || escaped_break_at(self.bytes(), i, b'\\') > 0
                    || close_tag(src, i).is_some())
            {
                break;
            }
            // A character may be several bytes: step to the next boundary.
            i += 1;
            while i < b.len() && !src.is_char_boundary(i) {
                i += 1;
            }
        }
        self.put(k::TEXT, start, i);
    }

    /// `${ expression | filters }`.
    fn expression(&mut self, start: usize) {
        let src = self.lx.src;
        let body = start + 2;
        let Some((end, term)) = EXPRESSION.code_end(src, body, &["|"], &self.code) else {
            self.put(k::EXPR_OPEN, start, body);
            self.lx.error(
                start,
                "unterminated-expression",
                "expression `${` has no closing `}`",
            );
            return;
        };
        self.put(k::EXPR_OPEN, start, body);
        self.code(body, end, k::PY_EXPR);
        if term == "|" {
            let after = end + 1;
            let Some((close, _)) = EXPRESSION.code_end(src, after, &[], &self.code) else {
                self.put(k::PIPE, end, after);
                self.lx.error(
                    start,
                    "unterminated-expression",
                    "expression `${` has no closing `}`",
                );
                return;
            };
            self.put(k::PIPE, end, after);
            self.code(after, close, k::FILTERS);
            self.put(k::EXPR_CLOSE, close, close + 1);
        } else {
            self.put(k::EXPR_CLOSE, end, end + 1);
        }
    }

    /// The text `from..to` as blanks, a token of `kind` over its non-blank part, blanks.
    fn code(&mut self, from: usize, to: usize, kind: SyntaxKind) {
        let src = self.lx.src;
        let text = &src[from..to];
        let lead = text.len() - text.trim_start_matches(is_space).len();
        let trimmed = text.trim_matches(is_space);
        self.blanks(from, from + lead);
        if !trimmed.is_empty() {
            self.put(kind, from + lead, from + lead + trimmed.len());
        }
        self.blanks(from + lead + trimmed.len(), to);
        self.lx.pos = to;
    }

    /// `<% code %>` and `<%! code %>`.
    fn python_block(&mut self, start: usize) {
        let src = self.lx.src;
        let open = start
            + if src[start..].starts_with("<%!") {
                3
            } else {
                2
            };
        let Some((close, _)) = BLOCK.code_end(
            src,
            open,
            &[],
            &Code {
                nest: false,
                ..self.code
            },
        ) else {
            self.put(k::PY_OPEN, start, open);
            self.lx.error(
                start,
                "unterminated-block",
                "block `<%` has no closing `%>`",
            );
            return;
        };
        self.put(k::PY_OPEN, start, open);
        let text = &src[open..close];
        let lead = text.len() - text.trim_start_matches(is_space).len();
        let trimmed = text.trim_matches(is_space);
        let code_start = open + lead;
        // The margin of a first line that starts its own line is a token of its own.
        let line_start = src[..code_start].rfind(['\n', '\r']).map_or(0, |i| i + 1);
        let margin = line_start > open && src[line_start..code_start].bytes().all(is_blank);
        let (blank_end, indent) = if margin && code_start > line_start {
            (line_start, Some(line_start))
        } else {
            (code_start, None)
        };
        self.blanks(open, blank_end);
        if let Some(from) = indent {
            self.put(k::CODE_INDENT, from, code_start);
        }
        if !trimmed.is_empty() {
            self.put(k::PY_CODE, code_start, code_start + trimmed.len());
        }
        self.blanks(code_start + trimmed.len(), close);
        self.put(k::PY_CLOSE, close, close + 2);
    }

    /// `<%name attributes>` or `<%name attributes/>`; for `<%text>` the raw text up to its closing tag.
    fn tag(&mut self, start: usize, tag: &Tag) {
        let src = self.lx.src;
        self.put(k::TAG_OPEN, start, tag.name_end);
        let mut at = tag.name_end;
        for &(kind, a, b) in &tag.items {
            self.blanks(at, a);
            self.put(kind, a, b);
            at = b;
        }
        self.blanks(at, tag.end - if tag.self_close { 2 } else { 1 });
        let (kind, from) = if tag.self_close {
            (k::TAG_SELF_END, tag.end - 2)
        } else {
            (k::TAG_END, tag.end - 1)
        };
        self.put(kind, from, tag.end);
        if !tag.self_close
            && src[start + 2..tag.name_end].eq("text")
            && let Some(at) = src[tag.end..].find("</%text>")
            && at > 0
        {
            self.put(k::RAW_TEXT, tag.end, tag.end + at);
        }
    }

    /// `% keyword rest` at the start of a line.
    fn control_line(&mut self, start: usize) {
        let src = self.lx.src;
        self.put(k::PERCENT, start, start + 1);
        let mut at = start + 1;
        let blanks = src[at..].bytes().take_while(|&b| is_blank(b)).count();
        self.blanks(at, at + blanks);
        at += blanks;
        let word = src[at..]
            .bytes()
            .take_while(|&b| b.is_ascii_alphanumeric() || b == b'_' || b >= 0x80)
            .count();
        let line_end = {
            self.lx.pos = at;
            self.lx.eat_line_escaped(Some(b'\\'));
            self.lx.pos
        };
        if word == 0 {
            let rest = &src[at..line_end];
            if !rest.is_empty() {
                self.put(
                    k::CONTROL_REST,
                    at,
                    at + rest.trim_end_matches(is_blank_char).len(),
                );
            }
            self.lx.error(
                start,
                "invalid-control-line",
                "a control line needs a keyword after `%`",
            );
            self.lx.pos = line_end;
            return;
        }
        let text = &src[at..at + word];
        let (kind, name) = match text {
            _ if text.starts_with("end") && text.len() > 3 => (k::CONTROL_CLOSE, &text[3..]),
            "for" | "if" | "while" | "try" | "with" => (k::CONTROL_OPEN, text),
            "else" | "elif" | "except" | "finally" => (k::CONTROL_ARM, text),
            _ => (k::CONTROL_WORD, text),
        };
        self.put(kind, at, at + word);
        let rest_from = at + word;
        let blanks = src[rest_from..line_end]
            .bytes()
            .take_while(|&b| is_blank(b))
            .count();
        self.blanks(rest_from, rest_from + blanks);
        let rest = src[rest_from + blanks..line_end].trim_end_matches(is_blank_char);
        let rest_at = rest_from + blanks;
        let (code, colon) = match rest.strip_suffix(':') {
            Some(code) if kind != k::CONTROL_CLOSE => (code.trim_end_matches(is_blank_char), true),
            _ => (rest, false),
        };
        if !code.is_empty() {
            let kind = match name {
                "if" | "elif" | "while" | "for" if colon && kind != k::CONTROL_CLOSE => k::PY_EXPR,
                _ => k::CONTROL_REST,
            };
            self.put(kind, rest_at, rest_at + code.len());
        }
        if colon {
            let colon_at = rest_at + rest.len() - 1;
            self.blanks(rest_at + code.len(), colon_at);
            self.put(k::COLON, colon_at, colon_at + 1);
        }
        self.blanks(rest_at + rest.len(), line_end);
        self.lx.pos = line_end;
    }
}

/// A tag the lexer found: its name's end, the attribute tokens, the end of the tag.
struct Tag {
    name_end: usize,
    items: Vec<(SyntaxKind, usize, usize)>,
    end: usize,
    self_close: bool,
}

/// The tag that starts at `at` (`<%`): a name of word characters, `.` and `:`, then names,
/// `=`, quoted values and `,` (blanks between), then `>` or `/>`. `None` when the text has not
/// that shape (it is a Python block).
fn scan_tag(src: &str, at: usize) -> Option<Tag> {
    let b = src.as_bytes();
    let name = src[at + 2..]
        .bytes()
        .take_while(|&c| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'.' | b':') || c >= 0x80)
        .count();
    if name == 0 {
        return None;
    }
    let name_end = at + 2 + name;
    let mut items = Vec::new();
    let mut i = name_end;
    let space = |i: usize| {
        i + src[i..]
            .bytes()
            .take_while(|c| c.is_ascii_whitespace())
            .count()
    };
    loop {
        let j = space(i);
        match b.get(j) {
            Some(b'=') => {
                items.push((k::EQUALS, j, j + 1));
                i = j + 1;
            }
            Some(b',') => {
                items.push((k::COMMA, j, j + 1));
                i = j + 1;
            }
            Some(&q @ (b'"' | b'\'')) => {
                let close = src[j + 1..].find(q as char)?;
                items.push((k::ATTR_VALUE, j, j + close + 2));
                i = j + close + 2;
            }
            Some(&c) if j > i && (c.is_ascii_alphanumeric() || c == b'_') => {
                let n = src[j..]
                    .bytes()
                    .take_while(|&c| c.is_ascii_alphanumeric() || c == b'_')
                    .count();
                items.push((k::ATTR_NAME, j, j + n));
                i = j + n;
            }
            _ => break,
        }
    }
    // The name of a def or a block is a part of its own: its repeats are an error.
    if matches!(&src[at + 2..name_end], "def" | "block") {
        for n in 0..items.len().saturating_sub(2) {
            let (kind, a, z) = items[n];
            if kind == k::ATTR_NAME
                && &src[a..z] == "name"
                && items[n + 1].0 == k::EQUALS
                && items[n + 2].0 == k::ATTR_VALUE
            {
                items[n].0 = k::DEF_KEY;
                items[n + 2].0 = k::DEF_VALUE;
            }
        }
    }
    let j = space(i);
    let self_close = b.get(j) == Some(&b'/');
    let end = j + usize::from(self_close);
    (b.get(end) == Some(&b'>')).then_some(Tag {
        name_end,
        items,
        end: end + 1,
        self_close,
    })
}

/// The end of `</%name>` at `at`, as Mako's `</%[\t ]*([^\t ]+?)[\t ]*>`.
fn close_tag(src: &str, at: usize) -> Option<usize> {
    let rest = src[at..].strip_prefix("</%")?;
    let b = rest.as_bytes();
    let mut i = b.iter().take_while(|&&c| parse::is_blank(c)).count();
    // The name: one or more characters that are no blank, up to blanks and `>`.
    if i >= b.len() {
        return None;
    }
    i += rest[i..].chars().next()?.len_utf8();
    loop {
        let mut j = i;
        while b.get(j).is_some_and(|&c| parse::is_blank(c)) {
            j += 1;
        }
        if b.get(j) == Some(&b'>') {
            return Some(at + 3 + j + 1);
        }
        if j > i || i >= b.len() {
            return None;
        }
        i += rest[i..].chars().next()?.len_utf8();
    }
}

/// `${ ... }` and `<% ... %>` as the template actions of Mako.
const EXPRESSION: Actions = Actions {
    open: "${",
    close: "}",
    trim: '-',
    openers: &[],
    continuers: &[],
    closers: &[],
};
const BLOCK: Actions = Actions {
    open: "<%",
    close: "%>",
    trim: '-',
    openers: &[],
    continuers: &[],
    closers: &[],
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_byte_is_in_a_token() {
        let src = "<%page args=\"a\"/>\n% if x:\n  ${ a | h }\n  <% y = \"%>\" %>\r\n% endif\n## c\n<%doc>d</%doc>\\\nt<%text> ${</%text> </%x> %%\n";
        let (tokens, _) = lex(src);
        let joined: String = tokens.iter().map(|t| &src[t.start..t.end]).collect();
        assert_eq!(joined, src);
    }
}
